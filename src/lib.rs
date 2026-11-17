//! Pure queue eligibility and ordering ports. No storage, chess, or clock access.
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use tempo_types::{StudyDay, TargetId};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Candidate {
    pub target: TargetId,
    pub active: bool,
    pub introduced: bool,
    pub due: Option<StudyDay>,
    pub repeat_pending: bool,
    /// Tactics can be introduced independently; repertoire positions use lines.
    pub independent_new: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Line {
    pub id: String,
    /// Ordered occurrences; transpositions and repeated positions reuse target IDs.
    pub targets: Vec<TargetId>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueueInput {
    pub today: StudyDay,
    pub candidates: Vec<Candidate>,
    pub lines: Vec<Line>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EligibilityReason {
    Repeat,
    Due,
    New,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EligibleTarget {
    pub target: TargetId,
    pub reason: EligibilityReason,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Eligibility {
    pub targets: Vec<EligibleTarget>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Group {
    pub id: String,
    pub targets: Vec<TargetId>,
}

/// Rebuildable session facts, appended by the app only after a successful commit.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OrderEvent {
    Missed { target: TargetId },
    Completed { target: TargetId },
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueueOrderContext {
    pub seed: u64,
    pub history: Vec<OrderEvent>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Block {
    pub group: Option<String>,
    pub targets: Vec<TargetId>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Selection {
    Ready {
        target: TargetId,
        remaining_block: Vec<TargetId>,
    },
    Empty,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum QueueError {
    DuplicateCandidate(TargetId),
    UnknownLineTarget(TargetId),
    DuplicateEligibleTarget(TargetId),
    OverlappingGroups(TargetId),
    InvalidGroupOrder(String),
    InvalidOrder,
}
impl fmt::Display for QueueError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "queue contract: {self:?}")
    }
}
impl std::error::Error for QueueError {}

/// The app passes facts, not a repository or database handle.
pub trait QueuePolicy {
    fn eligible(&self, input: &QueueInput) -> Result<Eligibility, QueueError>;
    fn order(
        &self,
        eligible: &Eligibility,
        groups: &[Group],
        context: &QueueOrderContext,
    ) -> Result<Vec<Block>, QueueError>;
}

/// Deterministic fixture ordering. Replace it with a randomized policy later.
#[derive(Clone, Copy, Debug, Default)]
pub struct StableQueue;

/// A second implementation that reverses blocks, never their inner order.
#[derive(Clone, Copy, Debug, Default)]
pub struct ReverseQueue;

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

/// First unintroduced active target per line, deduplicated across occurrences.
pub fn eligibility(input: &QueueInput) -> Result<Eligibility, QueueError> {
    let mut candidates = BTreeMap::new();
    for candidate in &input.candidates {
        if candidates.insert(&candidate.target, candidate).is_some() {
            return Err(QueueError::DuplicateCandidate(candidate.target.clone()));
        }
    }
    let mut next_new = BTreeSet::new();
    for line in &input.lines {
        let mut selected = false;
        for target in &line.targets {
            let candidate = candidates
                .get(target)
                .ok_or_else(|| QueueError::UnknownLineTarget(target.clone()))?;
            if !selected && candidate.active && !candidate.introduced {
                next_new.insert(target.clone());
                selected = true;
            }
        }
    }
    let targets = input
        .candidates
        .iter()
        .filter_map(|candidate| {
            if !candidate.active {
                return None;
            }
            let reason = if candidate.introduced && candidate.repeat_pending {
                EligibilityReason::Repeat
            } else if candidate.introduced && candidate.due.is_some_and(|due| due <= input.today) {
                EligibilityReason::Due
            } else if !candidate.introduced
                && (candidate.independent_new || next_new.contains(&candidate.target))
            {
                EligibilityReason::New
            } else {
                return None;
            };
            Some(EligibleTarget {
                target: candidate.target.clone(),
                reason,
            })
        })
        .collect();
    Ok(Eligibility { targets })
}

fn eligible_set(eligible: &Eligibility) -> Result<BTreeSet<TargetId>, QueueError> {
    let mut expected = BTreeSet::new();
    for item in &eligible.targets {
        if !expected.insert(item.target.clone()) {
            return Err(QueueError::DuplicateEligibleTarget(item.target.clone()));
        }
    }
    Ok(expected)
}

fn ordered_blocks(eligible: &Eligibility, groups: &[Group]) -> Result<Vec<Block>, QueueError> {
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

/// Recheck this boundary when accepting another implementation's ordered plan.
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

/// Validate an injected policy's complete grouping contract, not just its target set.
/// Each supplied group's eligible occurrences must remain consecutive in one block.
/// Repeated occurrences within a group collapse to one learning target.
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

/// The application retains the current block's tail between completed attempts.
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

    fn candidate(id: &str, introduced: bool, due: Option<i32>) -> Candidate {
        Candidate {
            target: id.into(),
            active: true,
            introduced,
            due: due.map(StudyDay),
            repeat_pending: false,
            independent_new: false,
        }
    }
    fn fixture() -> QueueInput {
        let mut tactic = candidate("tactic", false, None);
        tactic.independent_new = true;
        let mut quarantined = candidate("invalid", true, Some(1));
        quarantined.active = false;
        QueueInput {
            today: StudyDay(10),
            candidates: vec![
                candidate("due", true, Some(10)),
                candidate("future", true, Some(11)),
                candidate("new1", false, None),
                candidate("new2", false, None),
                tactic,
                quarantined,
            ],
            lines: vec![
                Line {
                    id: "line1".into(),
                    targets: vec!["due".into(), "new1".into(), "new2".into()],
                },
                Line {
                    id: "transposition".into(),
                    targets: vec!["new1".into(), "new1".into(), "new2".into()],
                },
            ],
        }
    }

    #[test]
    fn eligibility_uses_learning_and_deduplicates_new_line_occurrences() {
        let result = eligibility(&fixture()).unwrap();
        assert_eq!(
            result
                .targets
                .iter()
                .map(|e| e.target.0.as_str())
                .collect::<Vec<_>>(),
            vec!["due", "new1", "tactic"]
        );
        assert_eq!(result.targets[0].reason, EligibilityReason::Due);
        assert_eq!(result.targets[1].reason, EligibilityReason::New);
    }

    #[test]
    fn swapping_policies_changes_only_block_order() {
        let input = fixture();
        let original = input.clone();
        let eligible = StableQueue.eligible(&input).unwrap();
        let groups = vec![Group {
            id: "context".into(),
            targets: vec!["due".into(), "new1".into(), "future".into()],
        }];
        let stable = StableQueue
            .order(&eligible, &groups, &QueueOrderContext::default())
            .unwrap();
        let reversed = ReverseQueue
            .order(&eligible, &groups, &QueueOrderContext::default())
            .unwrap();
        assert_eq!(
            stable[0].targets,
            vec![TargetId::from("due"), TargetId::from("new1")]
        );
        assert_eq!(stable[0], reversed[1]);
        assert_eq!(stable[1], reversed[0]);
        assert_eq!(input, original);
        validate_order(&eligible, &stable).unwrap();
        validate_order(&eligible, &reversed).unwrap();
    }

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
    fn empty_is_a_normal_selection_and_repeat_overrides_future_due() {
        assert_eq!(next(&[]), Selection::Empty);
        let mut input = fixture();
        input.candidates[1].repeat_pending = true;
        let eligible = eligibility(&input).unwrap();
        assert!(
            eligible
                .targets
                .iter()
                .any(|e| e.target == TargetId::from("future")
                    && e.reason == EligibilityReason::Repeat)
        );
    }

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
