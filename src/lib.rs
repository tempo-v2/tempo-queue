//! Daily Study ordering over projected target, repertoire line and schedule facts.
//!
//! [`DailySession::select_daily`] proposes the next activity from the supplied day
//! and Learning facts. The application commits issuance and its ordering effect.

mod daily;
mod line;
mod tactic_order;

pub use daily::{
    DailyActivity, DailyAdmissions, DailyCandidate, DailyChoice, DailyContext, DailyEvidence,
    DailyKind, DailyOrderingEffect, DailySelection, DailySelectionInput, DailySession,
    ImmediateRecall, RelevanceReason, TargetRelevance, daily_rank,
};
pub use line::Line;

pub use tactic_order::{PackDifficulty, PackMembership, TacticPackCandidate, rank_new_tactics};
