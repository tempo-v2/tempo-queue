//! Validate that an ordering policy preserved the eligible target set.
use crate::{Block, Eligibility, QueueError, eligibility::eligible_set};
use std::collections::BTreeSet;

/// Check an injected policy's blocks for added, dropped, or duplicated targets,
/// and for empty blocks. An empty eligible set with no blocks is valid.
///
/// Use [`crate::validate_grouped_order`] when groups were requested: a set check
/// alone cannot detect a reversed, split, or interleaved group.
///
/// # Errors
///
/// Returns DuplicateEligibleTarget for malformed input or InvalidOrder for a
/// plan that does not contain exactly the eligible targets once each.
pub fn validate_order(eligible: &Eligibility, blocks: &[Block]) -> Result<(), QueueError> {
    let expected = eligible_set(eligible)?;
    let mut actual = BTreeSet::new();
    for block in blocks {
        if block.targets.is_empty() {
            return Err(QueueError::InvalidOrder);
        }
        for target in &block.targets {
            if !actual.insert(target.clone()) {
                return Err(QueueError::InvalidOrder);
            }
        }
    }
    if expected != actual {
        return Err(QueueError::InvalidOrder);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{eligibility, test_support::fixture};
    use tempo_types::TargetId;
    #[test]
    fn malicious_order_cannot_add_drop_or_duplicate_a_target() {
        let eligible = eligibility(&fixture()).unwrap();
        for targets in [
            vec!["due", "new1"],
            vec!["due", "new1", "tactic", "future"],
            vec!["due", "new1", "tactic", "due"],
        ] {
            let blocks = vec![Block {
                group: None,
                targets: targets.into_iter().map(TargetId::from).collect(),
            }];
            assert_eq!(
                validate_order(&eligible, &blocks),
                Err(QueueError::InvalidOrder)
            );
        }
    }

    #[test]
    fn an_empty_plan_is_valid_but_an_empty_block_is_not() {
        let eligible = Eligibility::default();
        assert_eq!(validate_order(&eligible, &[]), Ok(()));
        let meaningless_block = Block {
            group: None,
            targets: vec![],
        };
        assert_eq!(
            validate_order(&eligible, &[meaningless_block]),
            Err(QueueError::InvalidOrder)
        );
    }
}
