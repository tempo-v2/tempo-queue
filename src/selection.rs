//! The next exercise identity and the part of its block that remains.
use crate::Block;
use serde::{Deserialize, Serialize};
use tempo_types::TargetId;

/// Match this after calling [`next`]. Ready tells the application which target to
/// resolve and which block tail to retain. Empty means no work is currently in
/// the plan; it is a normal outcome, distinct from [`crate::QueueError`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Selection {
    /// A target is available; resolving its content/grader belongs to the application.
    Ready {
        /// The next learning target to turn into an exercise.
        target: TargetId,
        /// Remaining targets from this block only, in their required order.
        remaining_block: Vec<TargetId>,
    },
    /// No nonempty block remains; a later refresh may make more targets available.
    Empty,
}

/// Read the first nonempty block without consuming or mutating it.
///
/// The application retains the returned tail, rechecks its targets against fresh
/// facts, and asks for a new order after the block is exhausted. This helper does
/// not retain a cursor: calling it again with the same blocks selects the same ID.
pub fn next(blocks: &[Block]) -> Selection {
    match blocks.iter().find(|block| !block.targets.is_empty()) {
        Some(block) => Selection::Ready {
            target: block.targets[0].clone(),
            remaining_block: block.targets[1..].to_vec(),
        },
        None => Selection::Empty,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{QueueInput, QueueOrderContext, QueuePolicy, StableQueue};
    use tempo_types::StudyDay;

    #[test]
    fn an_empty_daily_plan_is_a_normal_selection() {
        let input = QueueInput {
            today: StudyDay(10),
            candidates: vec![],
            lines: vec![],
        };
        let eligible = StableQueue.eligible(&input).unwrap();
        let plan = StableQueue
            .order(&eligible, &[], &QueueOrderContext::default())
            .unwrap();
        assert_eq!(next(&plan), Selection::Empty);
    }

    #[test]
    fn the_caller_carries_the_remaining_block_to_the_next_selection() {
        let plan = [Block {
            group: Some("opening".into()),
            targets: vec!["first".into(), "second".into()],
        }];
        let Selection::Ready {
            target,
            remaining_block,
        } = next(&plan)
        else {
            panic!("the first position should be ready");
        };
        assert_eq!(target, TargetId::from("first"));
        let retained = [Block {
            group: plan[0].group.clone(),
            targets: remaining_block,
        }];
        assert_eq!(
            next(&retained),
            Selection::Ready {
                target: "second".into(),
                remaining_block: vec![],
            }
        );
        assert_eq!(next(&[]), Selection::Empty);
    }
}
