//! A second fixture policy that exercises substitution without changing grouping.
use crate::{
    Block, Eligibility, Group, QueueError, QueueInput, QueueOrderContext, QueuePolicy,
    block::ordered_blocks, eligibility,
};

/// Use this in boundary tests to prove that callers accept another ordering
/// implementation through [`QueuePolicy`]. It reverses StableQueue's block order,
/// never the target order inside a block. Seed and history are ignored.
#[derive(Clone, Copy, Debug, Default)]
pub struct ReverseQueue;

impl QueuePolicy for ReverseQueue {
    fn eligible(&self, input: &QueueInput) -> Result<Eligibility, QueueError> {
        eligibility(input)
    }
    fn order(
        &self,
        eligible: &Eligibility,
        groups: &[Group],
        _context: &QueueOrderContext,
    ) -> Result<Vec<Block>, QueueError> {
        let mut blocks = ordered_blocks(eligible, groups)?;
        blocks.reverse();
        Ok(blocks)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::fixture;
    use tempo_types::TargetId;
    #[test]
    fn reversing_blocks_moves_a_tactic_first_without_reversing_the_repertoire_line() {
        let eligible = ReverseQueue.eligible(&fixture()).unwrap();
        let groups = [Group {
            id: "line".into(),
            targets: vec!["due".into(), "new1".into()],
        }];
        let blocks = ReverseQueue
            .order(&eligible, &groups, &QueueOrderContext::default())
            .unwrap();
        assert_eq!(blocks[0].targets, vec![TargetId::from("tactic")]);
        assert_eq!(
            blocks[1].targets,
            vec![TargetId::from("due"), TargetId::from("new1")]
        );
    }
}
