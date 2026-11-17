//! Rebuildable inputs for deterministic or session-aware ordering.
use crate::OrderEvent;
use serde::{Deserialize, Serialize};

/// Pass this to [`crate::QueuePolicy::order`] when a policy needs randomness or
/// committed session history. Providing a seed explicitly makes a randomized
/// implementation reproducible without reading a global RNG.
///
/// [`crate::StableQueue`] and [`crate::ReverseQueue`] are fixtures and ignore both
/// fields. A future spacing policy can consume history through the same interface.
/// Default context is valid, including after a restart: eligibility still comes
/// from durable facts, independent of this rebuildable session state.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueueOrderContext {
    /// Explicit seed for a policy that chooses to randomize its order.
    pub seed: u64,
    /// Events in commit order; the application supplies and extends this list.
    pub history: Vec<OrderEvent>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::fixture;
    use crate::{QueuePolicy, StableQueue, eligibility};
    #[test]
    fn restarting_session_history_does_not_remove_a_durable_repeat() {
        let mut facts = fixture();
        facts.candidates[1].repeat_pending = true;
        let eligible = eligibility(&facts).unwrap();
        let old_context = QueueOrderContext {
            seed: 99,
            history: vec![OrderEvent::Missed {
                target: "future".into(),
            }],
        };
        let before = StableQueue.order(&eligible, &[], &old_context).unwrap();
        let restarted = StableQueue
            .order(&eligible, &[], &QueueOrderContext::default())
            .unwrap();
        assert_eq!(before, restarted);
        assert!(
            restarted
                .iter()
                .any(|block| block.targets.contains(&"future".into()))
        );
    }
}
