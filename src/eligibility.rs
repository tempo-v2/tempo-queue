//! Select the daily target set without choosing its presentation order.
use crate::{EligibilityReason, EligibleTarget, QueueError, QueueInput};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use tempo_types::TargetId;

/// Pass this result to [`crate::QueuePolicy::order`] after deciding which targets
/// are available. The built-in selector preserves candidate input order; an
/// ordering policy may rearrange targets only within the grouping contract.
/// An empty result is valid and produces [`crate::Selection::Empty`].
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Eligibility {
    /// Unique target IDs with diagnostic admission reasons.
    pub targets: Vec<EligibleTarget>,
}

/// Select active due/repeat targets and new introductions from explicit facts.
///
/// Repertoire introductions take the first active unintroduced target per line;
/// independent new targets need no line. Shared occurrences collapse to one ID.
/// This computes availability only: it neither advances Learning nor groups cards.
///
/// # Errors
///
/// Duplicate candidate IDs and line references to absent candidates are rejected.
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

pub(crate) fn eligible_set(eligible: &Eligibility) -> Result<BTreeSet<TargetId>, QueueError> {
    let mut expected = BTreeSet::new();
    for item in &eligible.targets {
        if !expected.insert(item.target.clone()) {
            return Err(QueueError::DuplicateEligibleTarget(item.target.clone()));
        }
    }
    Ok(expected)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::fixture;
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
    fn a_pending_repeat_is_eligible_before_its_future_due_day() {
        let mut input = fixture();
        input.candidates[1].repeat_pending = true;
        let result = eligibility(&input).unwrap();
        assert!(
            result
                .targets
                .iter()
                .any(|item| item.target == TargetId::from("future")
                    && item.reason == EligibilityReason::Repeat)
        );
    }

    #[test]
    fn duplicate_eligible_ids_are_rejected_at_the_ordering_boundary() {
        let mut eligible = eligibility(&fixture()).unwrap();
        eligible.targets.push(eligible.targets[0].clone());
        assert_eq!(
            eligible_set(&eligible),
            Err(QueueError::DuplicateEligibleTarget("due".into()))
        );
    }
}
