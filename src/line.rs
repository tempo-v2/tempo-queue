//! Content order used to choose the next new repertoire target.
use serde::{Deserialize, Serialize};
use tempo_types::TargetId;

/// Supply a line when new targets should be introduced in content order.
/// [`crate::eligibility`] admits the first active, unintroduced target on each line,
/// deduplicating a shared target across transpositions and repeated occurrences.
/// Every occurrence must refer to a candidate in [`crate::QueueInput`].
///
/// A line controls *introduction*. Use [`crate::Group`] separately to request that
/// already eligible exercises stay together during presentation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Line {
    /// Content occurrence identity; it does not create a separate learning identity.
    pub id: String,
    /// Ordered occurrences; transpositions and repeated positions reuse target IDs.
    pub targets: Vec<TargetId>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Candidate, QueueInput, eligibility};
    use tempo_types::StudyDay;
    #[test]
    fn two_lines_that_transpose_introduce_the_shared_position_only_once() {
        let candidates = ["shared-position", "later-position"]
            .into_iter()
            .map(|id| Candidate {
                target: id.into(),
                active: true,
                introduced: false,
                due: None,
                repeat_pending: false,
                independent_new: false,
            })
            .collect();
        let input = QueueInput {
            today: StudyDay(10),
            candidates,
            lines: vec![
                Line {
                    id: "main-line".into(),
                    targets: vec!["shared-position".into(), "later-position".into()],
                },
                Line {
                    id: "transposition".into(),
                    targets: vec!["shared-position".into(), "shared-position".into()],
                },
            ],
        };
        let eligible = eligibility(&input).unwrap();
        assert_eq!(eligible.targets.len(), 1);
        assert_eq!(
            eligible.targets[0].target,
            TargetId::from("shared-position")
        );
    }
}
