//! An application-supplied constraint that keeps related exercises together.
use serde::{Deserialize, Serialize};
use tempo_types::TargetId;

/// Use a group when separate eligible targets should be presented consecutively
/// in a particular order, such as positions from one repertoire line. It does not
/// introduce targets or merge their learning schedules.
///
/// Ordering filters out ineligible context and deduplicates occurrences within
/// this group. If two groups claim the same eligible target, Queue rejects the
/// ambiguity; the application's occurrence adapter must choose nonoverlapping
/// groups. [`crate::Line`] separately controls new-target introduction.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Group {
    /// Presentation occurrence label, carried into the resulting block.
    pub id: String,
    /// Required relative order, possibly including ineligible/repeated context.
    pub targets: Vec<TargetId>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::fixture;
    use crate::{QueueError, QueueOrderContext, QueuePolicy, StableQueue, eligibility};
    #[test]
    fn ambiguous_groups_fail_instead_of_duplicating_transpositions() {
        let eligible = eligibility(&fixture()).unwrap();
        let groups = vec![
            Group {
                id: "one".into(),
                targets: vec!["due".into()],
            },
            Group {
                id: "two".into(),
                targets: vec!["due".into(), "new1".into()],
            },
        ];
        assert_eq!(
            StableQueue.order(&eligible, &groups, &QueueOrderContext::default()),
            Err(QueueError::OverlappingGroups("due".into()))
        );
    }

    #[test]
    fn context_and_repeated_occurrences_do_not_create_extra_exercises() {
        let eligible = eligibility(&fixture()).unwrap();
        let group = Group {
            id: "line-context".into(),
            targets: vec!["due".into(), "future".into(), "new1".into(), "new1".into()],
        };
        let blocks = StableQueue
            .order(&eligible, &[group], &QueueOrderContext::default())
            .unwrap();
        assert_eq!(blocks[0].group.as_deref(), Some("line-context"));
        assert_eq!(
            blocks[0].targets,
            vec![TargetId::from("due"), TargetId::from("new1")]
        );
        assert_eq!(blocks[1].targets, vec![TargetId::from("tactic")]);
    }
}
