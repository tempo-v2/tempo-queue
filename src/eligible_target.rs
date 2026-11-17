//! One admitted learning target and the reason it was admitted.
use crate::EligibilityReason;
use serde::{Deserialize, Serialize};
use tempo_types::TargetId;

/// Consume this after eligibility has run. The `target` is used to order and
/// resolve an exercise; `reason` lets a caller explain why it is available.
/// Repeated occurrences of a position do not create extra eligible targets.
///
/// Usually obtain these from [`crate::eligibility`]. A replacement policy may
/// construct them, but its [`crate::Eligibility`] must still contain unique IDs.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EligibleTarget {
    /// Learning identity to resolve after selecting an ordered block.
    pub target: TargetId,
    /// Admission reason; changing this label does not change persistent learning.
    pub reason: EligibilityReason,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Eligibility, QueueOrderContext, QueuePolicy, StableQueue};
    #[test]
    fn a_caller_can_explain_a_new_tactic_then_order_its_identity() {
        let admitted = EligibleTarget {
            target: "fork-puzzle".into(),
            reason: EligibilityReason::New,
        };
        let label = match admitted.reason {
            EligibilityReason::New => "New exercise",
            EligibilityReason::Due => "Scheduled review",
            EligibilityReason::Repeat => "Repeat after a miss",
        };
        assert_eq!(label, "New exercise");
        let expected_id = admitted.target.clone();
        let eligible = Eligibility {
            targets: vec![admitted],
        };
        let blocks = StableQueue
            .order(&eligible, &[], &QueueOrderContext::default())
            .unwrap();
        assert_eq!(blocks[0].targets, vec![expected_id]);
    }
}
