# tempo-queue

Queue answers two separate questions: **which learning targets are available**, and
**how their exercises should be arranged**. It receives a snapshot of facts and
returns a proposed plan. Its only Tempo dependency is `tempo-types`; it never
imports Study, Learning, Chess, Store, or the application.

## Start here

The source tree is an outline of the concepts. Each public concept has its own
named file, purpose/usage documentation, and a same-file `#[cfg(test)] mod tests`
scenario. `lib.rs` is a navigation page and the root reexport list. Existing imports
such as `use tempo_queue::{Candidate, QueuePolicy};` continue to work.

Read a file's documentation, then its named test to see it used. Input examples
construct concrete facts in place. A small private `test_support.rs` supplies the
repeated multi-target fixture for larger invariant tests.

| When you reach for it | Concept and source | Executable usage scenario in that file |
| --- | --- | --- |
| Adapt one target's content and scheduling facts | [`Candidate`](src/candidate.rs) | `an_inactive_target_stays_out_even_when_it_needs_a_repeat` |
| Introduce repertoire positions in content order | [`Line`](src/line.rs) | `two_lines_that_transpose_introduce_the_shared_position_only_once` |
| Supply one explicit day and consistent facts | [`QueueInput`](src/queue_input.rs) | `advancing_the_supplied_day_makes_a_future_review_eligible` |
| Compute or inspect the available target set | [`Eligibility` and `eligibility`](src/eligibility.rs) | `eligibility_uses_learning_and_deduplicates_new_line_occurrences` |
| Understand precedence of admission reasons | [`EligibilityReason`](src/eligibility_reason.rs) | `repeat_is_the_reason_when_a_target_is_also_due` |
| Explain an admitted target, then resolve its identity | [`EligibleTarget`](src/eligible_target.rs) | `a_caller_can_explain_a_new_tactic_then_order_its_identity` |
| Keep eligible repertoire exercises together | [`Group`](src/group.rs) | `context_and_repeated_occurrences_do_not_create_extra_exercises` |
| Consume an ordered run of exercises | [`Block`](src/block.rs) | `selecting_a_block_returns_its_tail_without_merging_an_unrelated_tactic` |
| Inject a replacement selection/order implementation | [`QueuePolicy`](src/queue_policy.rs) | `application_composition_can_swap_a_policy_through_one_trait_object` |
| Supply rebuildable ordering history and a seed | [`QueueOrderContext`](src/queue_order_context.rs) | `restarting_session_history_does_not_remove_a_durable_repeat` |
| Distinguish a miss from presentation completion | [`OrderEvent`](src/order_event.rs) | `spacing_consumers_can_distinguish_a_miss_from_completed_exercises` |
| Get a predictable plan while wiring the application | [`StableQueue`](src/stable_queue.rs) | `groups_are_emitted_before_independent_tactics_in_a_reproducible_order` |
| Exercise policy substitution | [`ReverseQueue`](src/reverse_queue.rs) | `reversing_blocks_moves_a_tactic_first_without_reversing_the_repertoire_line` |
| Reject added, dropped, or duplicated targets | [`validate_order`](src/validate_order.rs) | `malicious_order_cannot_add_drop_or_duplicate_a_target` |
| Check group order and contiguity at the application boundary | [`validate_grouped_order`](src/validate_grouped_order.rs) | `injected_policy_cannot_reverse_split_or_interleave_a_group` |
| Select an exercise and retain its block tail | [`Selection` and `next`](src/selection.rs) | `the_caller_carries_the_remaining_block_to_the_next_selection` |
| Diagnose adapter/policy contract errors | [`QueueError`](src/queue_error.rs) | `an_adapter_can_report_the_missing_line_target_without_treating_it_as_empty` |

## Follow the data

1. The application constructs `Candidate` facts and finite `Line` occurrences from
   a consistent content/learning snapshot, then supplies them in `QueueInput`.
2. `QueuePolicy::eligible` returns `Eligibility`: active due/repeat targets,
   the first active unintroduced target on each line, and independent new targets.
   Repeated line occurrences retain one target identity.
3. The application chooses presentation `Group`s. `QueuePolicy::order` produces
   `Block`s containing exactly the eligible target set.
4. The application calls `validate_grouped_order` before accepting an injected
   policy's output, then calls `next` to get a target and `remaining_block`.
5. The application resolves that target's exercise/content, retains the tail, and
   rechecks it against current facts between exercises. Queue has no hidden cursor:
   calling `next` twice on the same plan gives the same target.

### Line, group, and block are different jobs

A **line** says which new repertoire position can be introduced next. A **group**
asks for already eligible positions to stay together in a specified order. A
**block** is the ordered output used for presentation. Grouping exercises does not
merge their learning schedules or make ineligible context earn credit.

For example, a line can contain an already due position, a new position, and a
later new position. Eligibility admits the due position and the first new one.
A group may mention the entire line, but ordering filters out the later position.
A separate tactic can be another block. Reversing block order may put that tactic
first while keeping the two opening exercises in order.

Within one group, repeated occurrences collapse to one target. The same eligible
target in two groups is an explicit `OverlappingGroups` error. Catalog lines can
share prefixes, so the application's grouping adapter must choose nonoverlapping
presentation occurrences. Block labels alone do not prove the constraint:
validation checks the actual locations of targets.

## Durable facts and session facts

`Candidate` copies introduction, due, and repeat facts from Learning. Queue never
receives a mutable Learning record. Eligibility can read a repeat obligation but
cannot clear it; it can admit a new target but cannot mark it introduced.

`QueueOrderContext` contains an explicit seed and committed `Missed`/`Completed`
events. The application appends events only after successful commits. A miss can
precede presentation completion; automatic opponent moves are not completed
exercises. A future repeat-spacing policy can consume this history without a new
port. It is rebuildable session data, so a restart may use an empty history while
durable repeat obligations remain intact.

## Implemented behavior and fixture policy

Eligibility, uniqueness, group filtering, and order validation are implemented.
`StableQueue` and `ReverseQueue` are deterministic fixture policies. They ignore
seed/history and differ only in block order. Random ordering and planned
5–10-exercise repeat spacing remain replacement work.

The crate has no storage, provider access, wall-clock read, global RNG, queue
worker, or background refresh. Empty availability is `Selection::Empty`, a normal
outcome. Invalid facts and policy output are `QueueError`s.

## Run the examples

Run these from the parent development workspace. The filtered test command opens
one concept at a time; the full command also runs the compiling crate-doc example.

```sh
cargo test -p tempo-queue line::tests
cargo test -p tempo-queue
cargo test -p tempo-queue --doc
cargo clippy -p tempo-queue --all-targets -- -D warnings
cargo doc -p tempo-queue --no-deps --open
```

The focused tests require no database or browser. Application acceptance separately
checks the real source adapter and retained-block journey. The exact Tempo
manifest dependency is unchanged; the parent workspace's local patches resolve
this unreleased scaffold. See `.planning/ARCHITECTURE.md` and
`.planning/INTERFACES.md` for cross-repository ownership and adoption checks.
