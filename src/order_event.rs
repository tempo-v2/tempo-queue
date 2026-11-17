//! Committed session events available to an ordering policy.
use serde::{Deserialize, Serialize};
use tempo_types::TargetId;

/// Append these to [`crate::QueueOrderContext::history`] in the application only
/// after the corresponding transaction commits. A miss can happen before its
/// correction/explanation finishes, so Missed and Completed are separate events.
/// Automatic opponent moves produce neither completed exercises nor spacing credit.
///
/// This is rebuildable ordering history. Durable repeat and due facts still come
/// from [`crate::Candidate`]; losing history must not erase those obligations.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OrderEvent {
    /// An accepted miss; a repeat-spacing policy can use this as its starting point.
    Missed {
        /// Learning target whose assessment was Again.
        target: TargetId,
    },
    /// Presentation of one exercise finished, including completion of correction.
    Completed {
        /// Learning target whose exercise finished.
        target: TargetId,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::QueueOrderContext;
    #[test]
    fn spacing_consumers_can_distinguish_a_miss_from_completed_exercises() {
        let context = QueueOrderContext {
            seed: 42,
            history: vec![
                OrderEvent::Missed {
                    target: "opening-position".into(),
                },
                OrderEvent::Completed {
                    target: "opening-position".into(),
                },
                OrderEvent::Completed {
                    target: "fork-puzzle".into(),
                },
            ],
        };
        // Example of reading the port, not a production repeat-spacing algorithm.
        let completed_after_miss = context
            .history
            .iter()
            .skip(1)
            .filter(|event| matches!(event, OrderEvent::Completed { .. }))
            .count();
        assert_eq!(completed_after_miss, 2);
        assert!(matches!(&context.history[0], OrderEvent::Missed { target }
            if target == &TargetId::from("opening-position")));
    }
}
