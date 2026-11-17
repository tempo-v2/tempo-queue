# tempo-queue

Pure eligibility, grouping and ordering for Tempo 2. Its only Tempo dependency is
`tempo-types`; it never imports Study, Learning, Chess, Store or the application.

## Public contract

| Input / operation | Output / guarantee |
| --- | --- |
| `QueueInput { today, candidates, lines }` | Compact content and learning facts supplied by the application's adapter. Repeated line occurrences retain the same target identity. |
| `QueuePolicy::eligible(&QueueInput)` | `Eligibility`: active due/repeat targets, the first active unintroduced target per line, and independent new tactics. Each target occurs once. |
| `QueuePolicy::order(&Eligibility, &[Group], &QueueOrderContext)` | Ordered `Block`s containing exactly the eligible targets. Each block preserves its target order. |
| `validate_order(&Eligibility, &[Block])` | Basic set check: rejects dropped, added or duplicated targets and empty blocks. |
| `validate_grouped_order(&Eligibility, &[Group], &[Block])` | Full injected-policy check: also rejects reversed, interleaved or split groups and overlapping group membership. Each filtered group remains consecutive inside one block. |
| `next(&[Block])` | `Selection::Ready { target, remaining_block }` or normal `Selection::Empty`. |

A `Candidate` contains `target`, `active`, `introduced`, `due`,
`repeat_pending`, and `independent_new`. The adapter translates Learning state
into these facts; Queue never receives or mutates a Learning record.

`QueueOrderContext` contains a seed and ordered `Missed { target }` /
`Completed { target }` events. The application appends these only after a
successful commit. A miss can precede presentation completion, and automatic
opponent moves never count as completed exercises. This port gives a future
repeat-spacing policy enough information to place a missed target after other
completed exercises. Ordering history is rebuildable session data; restart may
begin with empty history while durable learning obligations remain intact.

Groups may include ineligible context targets; those targets are filtered out.
Repeated occurrences within one group are deduplicated. The same eligible target
in two groups is an explicit `OverlappingGroups` error. Catalog lines commonly
share prefixes, so the application's grouping adapter must select nonoverlapping
presentation occurrences before asking for an order. It must retain and recheck
the selected block's tail between completed exercises.

## Real behavior and replaceable policy

Eligibility, uniqueness, group filtering and order validation are implemented.
`StableQueue` and `ReverseQueue` are interchangeable deterministic fixture
policies. They intentionally ignore seed and history; Reverse changes only block
order, never order inside a block. They prove the interface without pretending to
implement the planned random order or 5–10-exercise repeat spacing.

There is no storage, provider access, wall-clock read, global RNG, queue worker,
or background refresh in this crate. Methods receive owned facts or read-only
borrows and return proposed selections.

## Checks and development

From the parent development workspace, run `cargo test -p tempo-queue`.
The focused tests cover eligibility from learning facts, transposition deduplication,
policy substitution with contiguous groups, ordering corruption, empty selection,
overlapping groups, and policies that preserve the target set while corrupting
group order or contiguity. They need no database or browser.

The manifest requests the exact Tempo dependency version. The parent workspace's
local patches resolve the unreleased interface scaffold; package metadata does
not inherit from that parent. See the shared `.planning` interface specification
for release adoption and application integration acceptance.
