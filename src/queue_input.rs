//! A single, explicit snapshot of facts used for eligibility.
use crate::{Candidate, Line};
use serde::{Deserialize, Serialize};
use tempo_types::StudyDay;

/// Build this in the application adapter from one consistent content/learning
/// snapshot. It contains everything needed by [`crate::QueuePolicy::eligible`].
/// Queue does not fetch missing facts or read the current clock.
///
/// `candidates` must have unique target IDs. Every target mentioned in `lines`
/// must be present, including inactive or not-yet-due context positions.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueueInput {
    /// Application-selected study day used for all due comparisons in this call.
    pub today: StudyDay,
    /// One record per learning target, independent of how many lines contain it.
    pub candidates: Vec<Candidate>,
    /// Ordered content occurrences that govern new repertoire introductions.
    pub lines: Vec<Line>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{EligibilityReason, eligibility};
    #[test]
    fn advancing_the_supplied_day_makes_a_future_review_eligible() {
        let mut facts = QueueInput {
            today: StudyDay(10),
            candidates: vec![Candidate {
                target: "opening-position".into(),
                active: true,
                introduced: true,
                due: Some(StudyDay(11)),
                repeat_pending: false,
                independent_new: false,
            }],
            lines: vec![],
        };
        assert!(eligibility(&facts).unwrap().targets.is_empty());
        facts.today = StudyDay(11);
        assert_eq!(
            eligibility(&facts).unwrap().targets[0].reason,
            EligibilityReason::Due
        );
        // Selection itself never changes the durable due fact.
        assert_eq!(facts.candidates[0].due, Some(StudyDay(11)));
    }
}
