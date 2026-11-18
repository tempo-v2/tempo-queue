//! Replacement interface between application facts and an ordered presentation plan.
use crate::{Block, Eligibility, Group, QueueError, QueueInput, QueueOrderContext};

/// Inject an implementation here when replacing selection/order policy. The
/// application passes read-only facts and receives a proposed plan; a policy has
/// no authority to mutate learning, access storage, or grade an answer.
///
/// Selection and ordering are separate so an ordering algorithm cannot add new
/// eligible targets. The application should run [`crate::validate_grouped_order`]
/// before accepting output from an injected implementation. The trait is object-safe;
/// application composition can hold `Box<dyn QueuePolicy>`.
pub trait QueuePolicy {
    /// Decide which targets are available from one explicit input snapshot.
    fn eligible(&self, input: &QueueInput) -> Result<Eligibility, QueueError>;
    /// Arrange exactly the eligible set while preserving filtered group constraints.
    /// Context may affect ordering, never target identity or eligibility.
    fn order(
        &self,
        eligible: &Eligibility,
        groups: &[Group],
        context: &QueueOrderContext,
    ) -> Result<Vec<Block>, QueueError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::fixture;
    use crate::{ReverseQueue, StableQueue, validate_order};
    use tempo_types::TargetId;
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
    fn application_composition_can_swap_a_policy_through_one_trait_object() {
        let input = fixture();
        let policies: [&dyn QueuePolicy; 2] = [&StableQueue, &ReverseQueue];
        let plans: Vec<_> = policies
            .into_iter()
            .map(|policy| {
                let eligible = policy.eligible(&input).unwrap();
                let plan = policy
                    .order(&eligible, &[], &QueueOrderContext::default())
                    .unwrap();
                validate_order(&eligible, &plan).unwrap();
                plan
            })
            .collect();
        assert_eq!(plans[0].first(), plans[1].last());
        assert_eq!(plans[0].last(), plans[1].first());
    }
}
