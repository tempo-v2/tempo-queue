//! Why an active target entered today's eligible set.
use serde::{Deserialize, Serialize};

/// Read this alongside [`crate::EligibleTarget`] to explain a selection or apply
/// an ordering policy. It is a reason for admission, not a grade or a mutation of
/// Learning state. When several conditions hold, the built-in eligibility rule
/// checks Repeat before Due; New applies only to unintroduced targets.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EligibilityReason {
    /// An introduced target still has a repeat obligation, even if due is in the future.
    Repeat,
    /// An introduced target is scheduled for today or an earlier day.
    Due,
    /// First active unintroduced target of a line, or an independent new target.
    New,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Candidate, QueueInput, eligibility};
    use tempo_types::StudyDay;
    #[test]
    fn repeat_is_the_reason_when_a_target_is_also_due() {
        let facts = QueueInput {
            today: StudyDay(10),
            candidates: vec![Candidate {
                target: "missed-position".into(),
                active: true,
                introduced: true,
                due: Some(StudyDay(9)),
                repeat_pending: true,
                independent_new: false,
            }],
            lines: vec![],
        };
        assert_eq!(
            eligibility(&facts).unwrap().targets[0].reason,
            EligibilityReason::Repeat
        );
    }
}
