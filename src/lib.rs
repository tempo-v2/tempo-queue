//! Decide which learning targets can be studied, then arrange them for presentation.
//!
//! Start with [`QueueInput`] when adapting application facts. [`eligibility`] selects
//! targets; [`Group`] constrains their presentation; a [`QueuePolicy`] returns ordered
//! [`Block`]s. [`next`] chooses a target and gives the application the block's tail.
//! This crate receives facts, never a database, learning record, chess board, or clock.
//!
//! # Where to read
//!
//! - Input: [`Candidate`], [`Line`], [`QueueInput`].
//! - Eligibility: [`EligibleTarget`], [`EligibilityReason`], [`Eligibility`].
//! - Presentation: [`Group`], [`Block`], [`Selection`].
//! - Replacement boundary: [`QueuePolicy`], [`QueueOrderContext`], [`OrderEvent`].
//! - Fixture policies: [`StableQueue`] and [`ReverseQueue`].
//! - Boundary checks: [`validate_order`], [`validate_grouped_order`], [`QueueError`].
//!
//! Each concept lives in its named source file, with usage tests beside it. The
//! README maps common questions to those files. Existing root imports are unchanged.
//!
//! # Select an independently introduced tactic
//!
//! ```
//! use tempo_queue::{Candidate, QueueInput, QueuePolicy, StableQueue,
//!     QueueOrderContext, Selection, next};
//! use tempo_types::StudyDay;
//!
//! let facts = QueueInput {
//!     today: StudyDay(10),
//!     candidates: vec![Candidate {
//!         target: "fork-puzzle".into(),
//!         active: true,
//!         introduced: false,
//!         due: None,
//!         repeat_pending: false,
//!         independent_new: true,
//!     }],
//!     lines: vec![],
//! };
//! let eligible = StableQueue.eligible(&facts)?;
//! let blocks = StableQueue.order(&eligible, &[], &QueueOrderContext::default())?;
//! assert_eq!(next(&blocks), Selection::Ready {
//!     target: "fork-puzzle".into(),
//!     remaining_block: vec![],
//! });
//! # Ok::<(), tempo_queue::QueueError>(())
//! ```

mod block;
mod candidate;
mod daily;
mod eligibility;
mod eligibility_reason;
mod eligible_target;

mod group;
mod line;
mod order_event;
mod queue_error;
mod queue_input;
mod queue_order_context;
mod queue_policy;
mod reverse_queue;
mod selection;
mod stable_queue;
mod validate_grouped_order;
mod validate_order;

pub use block::Block;
pub use candidate::Candidate;
pub use eligibility::{Eligibility, eligibility};
pub use eligibility_reason::EligibilityReason;
pub use eligible_target::EligibleTarget;
pub use group::Group;
pub use line::Line;
pub use order_event::OrderEvent;
pub use queue_error::QueueError;
pub use queue_input::QueueInput;
pub use queue_order_context::QueueOrderContext;
pub use queue_policy::QueuePolicy;
pub use reverse_queue::ReverseQueue;
pub use selection::{Selection, next};
pub use stable_queue::StableQueue;
pub use validate_grouped_order::validate_grouped_order;
pub use validate_order::validate_order;

#[cfg(test)]
mod test_support;

pub use daily::{
    DailyActivity, DailyAdmissions, DailyCandidate, DailyChoice, DailyContext, DailyEvidence,
    DailyKind, DailySelection, DailySelectionInput, DailySession, ImmediateRecall, RelevanceReason,
    TargetRelevance, daily_rank,
};

mod tactic_order;
pub use tactic_order::{PackDifficulty, PackMembership, TacticPackCandidate, rank_new_tactics};
