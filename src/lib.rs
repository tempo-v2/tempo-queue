//! Daily Study ordering over projected target, repertoire line and schedule facts.
//!
//! [`DailySession::select_daily`] proposes the next activity from the supplied day
//! and Learning facts. The application commits issuance and its ordering effect.

mod batch_plan;
mod daily;
mod line;
mod plan;
mod summary;

pub use batch_plan::{
    BatchPlanError, BatchPlanInput, BatchTarget, BatchWork, CapturedIssuedWork, PreparedBatch,
    TeachAdmission, TeachKind, TeachingRemaining, build_batch_plan,
};
pub use daily::{
    DailyActivity, DailyAdmissions, DailyCandidate, DailyChoice, DailyContext, DailyEvidence,
    DailyKind, DailyOrderingEffect, DailySelection, DailySelectionInput, DailySession,
    ImmediateRecall, RelevanceReason, TargetRelevance, daily_rank,
};
pub use summary::{ReadyWorkCounts, StudySummaryCounts, classify_study_summary};
pub use line::Line;
pub use plan::{
    AcceptedPlanChange, MAX_PLAN_CANDIDATES, PlanEntry, PlanEntryState, PlanFreshness, PlanHead,
    PlanMutation, PlanPriority, PlanReadiness, PlanScan, PlanScanResult, PlanUpdate,
    plan_freshness, plan_readiness, plan_update, summary_is_current,
};
