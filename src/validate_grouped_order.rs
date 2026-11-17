//! Validate ordering constraints as well as target-set preservation.
use crate::{Block, Eligibility, Group, QueueError, eligibility::eligible_set, validate_order};
use std::collections::{BTreeMap, BTreeSet};

/// Run this at the application boundary before accepting an injected policy's
/// output. Each group's eligible targets must appear consecutively, in order,
/// inside one block. Ineligible context is removed and repeated occurrences
/// within a group collapse to one target.
///
/// Block group labels are descriptive, not proof of the contract: validation
/// checks actual target locations. Groups cannot claim the same eligible target.
///
/// # Errors
///
/// Includes [`validate_order`] failures, overlapping groups, and reversed,
/// interleaved, or split group targets.
pub fn validate_grouped_order(
    eligible: &Eligibility,
    groups: &[Group],
    blocks: &[Block],
) -> Result<(), QueueError> {
    validate_order(eligible, blocks)?;
    let expected = eligible_set(eligible)?;
    let locations: BTreeMap<_, _> = blocks
        .iter()
        .enumerate()
        .flat_map(|(block_index, block)| {
            block
                .targets
                .iter()
                .enumerate()
                .map(move |(index, target)| (target, (block_index, index)))
        })
        .collect();
    let mut assigned = BTreeSet::new();
    for group in groups {
        let mut within_group = BTreeSet::new();
        let targets: Vec<_> = group
            .targets
            .iter()
            .filter(|target| expected.contains(*target) && within_group.insert((*target).clone()))
            .collect();
        for target in &targets {
            if !assigned.insert((*target).clone()) {
                return Err(QueueError::OverlappingGroups((*target).clone()));
            }
        }
        let Some(first) = targets.first() else {
            continue;
        };
        let &(block_index, start) = locations.get(*first).ok_or(QueueError::InvalidOrder)?;
        for (offset, target) in targets.iter().enumerate() {
            if locations.get(*target) != Some(&(block_index, start + offset)) {
                return Err(QueueError::InvalidGroupOrder(group.id.clone()));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::fixture;
    use crate::{QueueOrderContext, QueuePolicy, StableQueue, eligibility};
    use tempo_types::TargetId;
    #[test]
    fn injected_policy_cannot_reverse_split_or_interleave_a_group() {
        let eligible = eligibility(&fixture()).unwrap();
        let groups = vec![Group {
            id: "context".into(),
            // Ineligible context and repeated positions do not change the constraint.
            targets: vec!["due".into(), "future".into(), "new1".into(), "new1".into()],
        }];
        for target_blocks in [
            vec![vec!["new1", "due", "tactic"]],
            vec![vec!["due"], vec!["new1", "tactic"]],
            vec![vec!["due", "tactic", "new1"]],
        ] {
            let blocks: Vec<_> = target_blocks
                .into_iter()
                .map(|targets| Block {
                    group: None,
                    targets: targets.into_iter().map(TargetId::from).collect(),
                })
                .collect();
            // These corruptions all pass the old set-only check.
            validate_order(&eligible, &blocks).unwrap();
            assert_eq!(
                validate_grouped_order(&eligible, &groups, &blocks),
                Err(QueueError::InvalidGroupOrder("context".into())),
            );
        }
        let valid = StableQueue
            .order(&eligible, &groups, &QueueOrderContext::default())
            .unwrap();
        validate_grouped_order(&eligible, &groups, &valid).unwrap();
        let overlap = vec![
            groups[0].clone(),
            Group {
                id: "overlap".into(),
                targets: vec!["new1".into()],
            },
        ];
        assert_eq!(
            validate_grouped_order(&eligible, &overlap, &valid),
            Err(QueueError::OverlappingGroups("new1".into())),
        );
    }
}
