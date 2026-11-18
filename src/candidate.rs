//! Compact facts for one target, prepared by the application adapter.
use serde::{Deserialize, Serialize};
use tempo_types::{StudyDay, TargetId};

/// Use this to tell Queue what it needs to select a target, without giving it a
/// Learning record or content implementation. The application copies scheduling
/// facts and marks unusable content inactive. Queue reads these facts; it cannot
/// introduce a target or clear its repeat obligation in persistent storage.
///
/// Repertoire targets normally set `independent_new` to false and enter through a
/// [`crate::Line`]. A tactic may set it to true and enter without any line.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Candidate {
    /// Stable learning identity; repeated line occurrences reuse this ID.
    pub target: TargetId,
    /// Whether content can currently be studied (false for retired/quarantined content).
    pub active: bool,
    /// Whether a learning transition has already introduced this target.
    pub introduced: bool,
    /// Next scheduled day; compared against the explicit day in [`crate::QueueInput`].
    pub due: Option<StudyDay>,
    /// Durable repeat obligation; for introduced targets this takes precedence over due.
    pub repeat_pending: bool,
    /// Tactics can be introduced independently; repertoire positions use lines.
    pub independent_new: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{EligibilityReason, QueueInput, eligibility};
    #[test]
    fn an_inactive_target_stays_out_even_when_it_needs_a_repeat() {
        let mut target = Candidate {
            target: "quarantined-tactic".into(),
            active: false,
            introduced: true,
            due: Some(StudyDay(20)),
            repeat_pending: true,
            independent_new: true,
        };
        let facts = QueueInput {
            today: StudyDay(10),
            candidates: vec![target.clone()],
            lines: vec![],
        };
        assert!(eligibility(&facts).unwrap().targets.is_empty());

        // The adapter can reactivate validated content without changing Learning.
        target.active = true;
        let restored = QueueInput {
            candidates: vec![target],
            ..facts
        };
        assert_eq!(
            eligibility(&restored).unwrap().targets[0].reason,
            EligibilityReason::Repeat
        );
    }
}
