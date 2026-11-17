//! Ordered output: one run of targets that travels together.
use crate::{Eligibility, Group, QueueError, eligibility::eligible_set, validate_grouped_order};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use tempo_types::TargetId;

/// Read blocks returned by [`crate::QueuePolicy::order`] to choose the next
/// exercise. A policy can reorder blocks while preserving required group order
/// inside them. A block is presentation state; each target keeps its own schedule.
///
/// [`crate::next`] returns the first target and the remaining targets of that
/// block. The application retains and rechecks that tail between attempts.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Block {
    /// Optional originating group label; validators check target order, not this label.
    pub group: Option<String>,
    /// Exercises in presentation order; a validated block is never empty.
    pub targets: Vec<TargetId>,
}

pub(crate) fn ordered_blocks(
    eligible: &Eligibility,
    groups: &[Group],
) -> Result<Vec<Block>, QueueError> {
    let expected = eligible_set(eligible)?;
    let mut assigned = BTreeSet::new();
    let mut blocks = Vec::new();
    for group in groups {
        let mut within_group = BTreeSet::new();
        let mut targets = Vec::new();
        for target in &group.targets {
            // An occurrence/context specification cannot introduce eligibility.
            if !expected.contains(target) || !within_group.insert(target.clone()) {
                continue;
            }
            if !assigned.insert(target.clone()) {
                return Err(QueueError::OverlappingGroups(target.clone()));
            }
            targets.push(target.clone());
        }
        if !targets.is_empty() {
            blocks.push(Block {
                group: Some(group.id.clone()),
                targets,
            });
        }
    }
    for item in &eligible.targets {
        if assigned.insert(item.target.clone()) {
            blocks.push(Block {
                group: None,
                targets: vec![item.target.clone()],
            });
        }
    }
    validate_grouped_order(eligible, groups, &blocks)?;
    Ok(blocks)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Selection, next};
    #[test]
    fn selecting_a_block_returns_its_tail_without_merging_an_unrelated_tactic() {
        let blocks = vec![
            Block {
                group: Some("opening-line".into()),
                targets: vec!["first-position".into(), "second-position".into()],
            },
            Block {
                group: None,
                targets: vec!["fork-puzzle".into()],
            },
        ];
        assert_eq!(
            next(&blocks),
            Selection::Ready {
                target: "first-position".into(),
                remaining_block: vec!["second-position".into()],
            }
        );
        assert_eq!(blocks[0].targets.len(), 2); // A selection does not consume its input.
    }
}
