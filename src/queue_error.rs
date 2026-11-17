//! Malformed queue inputs and broken policy output contracts.
use std::fmt;
use tempo_types::TargetId;

/// Match these when an application adapter or injected ordering policy violates
/// Queue's boundary. Empty availability is not an error: use [`crate::Selection::Empty`].
/// Fix the source facts or policy before accepting a plan; retrying the same facts
/// does not repair an invalid contract.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum QueueError {
    /// More than one candidate supplied the same learning identity.
    DuplicateCandidate(TargetId),
    /// A line references a target absent from the candidate facts.
    UnknownLineTarget(TargetId),
    /// An eligibility result repeats one learning identity.
    DuplicateEligibleTarget(TargetId),
    /// Two requested groups contain the same eligible target.
    OverlappingGroups(TargetId),
    /// A named group was reversed, split, or interleaved in the returned blocks.
    InvalidGroupOrder(String),
    /// Empty blocks or added, dropped, or duplicated target identities.
    InvalidOrder,
}
impl fmt::Display for QueueError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "queue contract: {self:?}")
    }
}
impl std::error::Error for QueueError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::fixture;
    use crate::{Line, eligibility};
    #[test]
    fn an_adapter_can_report_the_missing_line_target_without_treating_it_as_empty() {
        let mut facts = fixture();
        facts.lines.push(Line {
            id: "broken-import".into(),
            targets: vec!["absent".into()],
        });
        let missing = match eligibility(&facts) {
            Err(QueueError::UnknownLineTarget(target)) => target,
            result => panic!("expected an invalid line reference, got {result:?}"),
        };
        assert_eq!(missing, TargetId::from("absent"));
    }

    #[test]
    fn duplicate_candidate_facts_are_rejected_instead_of_overwriting_a_schedule() {
        let mut facts = fixture();
        let mut conflicting = facts.candidates[0].clone();
        conflicting.active = false;
        facts.candidates.push(conflicting);
        assert_eq!(
            eligibility(&facts),
            Err(QueueError::DuplicateCandidate("due".into()))
        );
    }
}
