//! A deterministic policy for wiring the application and learning the interface.
use crate::{
    Block, Eligibility, Group, QueueError, QueueInput, QueueOrderContext, QueuePolicy,
    block::ordered_blocks, eligibility,
};

/// Use this fixture when a predictable plan is more useful than randomized study.
/// It emits filtered groups in their supplied order, then ungrouped targets in
/// eligibility order. It ignores the seed and history; repeat spacing is not
/// implemented by this fixture.
#[derive(Clone, Copy, Debug, Default)]
pub struct StableQueue;

impl QueuePolicy for StableQueue {
    fn eligible(&self, input: &QueueInput) -> Result<Eligibility, QueueError> {
        eligibility(input)
    }
    fn order(
        &self,
        eligible: &Eligibility,
        groups: &[Group],
        _context: &QueueOrderContext,
    ) -> Result<Vec<Block>, QueueError> {
        ordered_blocks(eligible, groups)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::fixture;
    use tempo_types::TargetId;
    #[test]
    fn groups_are_emitted_before_independent_tactics_in_a_reproducible_order() {
        let eligible = StableQueue.eligible(&fixture()).unwrap();
        let groups = [Group {
            id: "line".into(),
            targets: vec!["due".into(), "new1".into()],
        }];
        let context = QueueOrderContext::default();
        let plan = StableQueue.order(&eligible, &groups, &context).unwrap();
        assert_eq!(
            plan[0].targets,
            vec![TargetId::from("due"), TargetId::from("new1")]
        );
        assert_eq!(plan[1].targets, vec![TargetId::from("tactic")]);
        assert_eq!(
            StableQueue.order(&eligible, &groups, &context).unwrap(),
            plan
        );
    }
}
