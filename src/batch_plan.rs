//! Build a stable order from one coherent capture of current Study facts.

use crate::{PlanEntry, PlanEntryState, PlanPriority};
use std::{collections::BTreeSet, fmt};
use tempo_types::{
    ContentVersion, DeferredWork, MissRevision, PlanBasis, StudyDay, TargetId, WorkKind,
    WorkObligation, WorkToken,
};

/// Content admission to Teach, supplied independently of Learning availability.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TeachAdmission {
    Repertoire { at_frontier: bool },
    Tactic { theme_admitted: bool },
}

impl TeachAdmission {
    fn can_teach(self) -> bool {
        match self {
            Self::Repertoire { at_frontier } => at_frontier,
            Self::Tactic { theme_admitted } => theme_admitted,
        }
    }
}

/// At most one currently relevant Learning operation per target.
/// The capture owner represents future corrections as waiting work.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BatchWork {
    Correction {
        revision: MissRevision,
    },
    ScheduledTest {
        due: StudyDay,
        revision: MissRevision,
    },
    Teach {
        revision: MissRevision,
    },
}

impl BatchWork {
    fn for_day(
        self,
        day: StudyDay,
        teaching: TeachAdmission,
    ) -> Option<(PlanPriority, MissRevision)> {
        match self {
            Self::Correction { revision } => Some((PlanPriority::Correction, revision)),
            Self::ScheduledTest { due, revision } if due <= day => {
                Some((PlanPriority::DueTest(due), revision))
            }
            Self::Teach { revision } if teaching.can_teach() => {
                Some((PlanPriority::Teach, revision))
            }
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BatchTarget {
    pub target: TargetId,
    pub version: ContentVersion,
    /// Authoritative content availability, including suspension and qualification.
    pub available: bool,
    pub teaching: TeachAdmission,
    pub work: Option<BatchWork>,
}

/// Exact current issued work carried by the active attempt.
/// Use the captured correction token once one is bound to the attempt.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CapturedIssuedWork {
    pub work: WorkToken,
    pub priority: PlanPriority,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TeachingRemaining {
    pub repertoire: usize,
    pub tactics: usize,
}

impl TeachingRemaining {
    fn admit(&mut self, family: TeachAdmission) -> bool {
        let remaining = match family {
            TeachAdmission::Repertoire { .. } => &mut self.repertoire,
            TeachAdmission::Tactic { .. } => &mut self.tactics,
        };
        if *remaining == 0 {
            return false;
        }
        *remaining -= 1;
        true
    }
}

/// Facts and expected coverage from a single coherent Store capture.
pub struct BatchPlanInput<'a> {
    pub basis: &'a PlanBasis,
    pub expected_targets: usize,
    pub targets: &'a [BatchTarget],
    pub remaining: TeachingRemaining,
    /// Effective deferrals after the captured resume cutoff.
    pub deferrals: &'a [DeferredWork],
    pub issued: Option<&'a CapturedIssuedWork>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreparedBatch {
    basis: PlanBasis,
    entries: Vec<PlanEntry>,
}

impl PreparedBatch {
    pub fn into_parts(self) -> (PlanBasis, Vec<PlanEntry>) {
        (self.basis, self.entries)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BatchPlanError {
    IncompleteCoverage { expected: usize, captured: usize },
    DuplicateTarget(TargetId),
    InvalidTarget(TargetId),
    InvalidLearningWork(TargetId),
    InvalidIssuedWork,
}

impl fmt::Display for BatchPlanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for BatchPlanError {}

/// Prepare a complete order for the captured day and basis. Publication still
/// requires Store to verify the original basis after staging the entries.
pub fn build_batch_plan(input: BatchPlanInput<'_>) -> Result<PreparedBatch, BatchPlanError> {
    if input.targets.len() != input.expected_targets {
        return Err(BatchPlanError::IncompleteCoverage {
            expected: input.expected_targets,
            captured: input.targets.len(),
        });
    }
    if input.issued.is_some_and(|issued| !valid_issued(issued)) {
        return Err(BatchPlanError::InvalidIssuedWork);
    }
    let day = input.basis.day;
    let deferrals: BTreeSet<_> = input
        .deferrals
        .iter()
        .filter(|deferral| deferral.day == day)
        .map(|deferral| &deferral.obligation)
        .collect();
    let mut seen = BTreeSet::new();
    let mut rows = Vec::new();
    for target in input.targets {
        if !valid_target(&target.target, target.version) {
            return Err(BatchPlanError::InvalidTarget(target.target.clone()));
        }
        if !seen.insert(&target.target) {
            return Err(BatchPlanError::DuplicateTarget(target.target.clone()));
        }
        if !target.available {
            continue;
        }
        if let Some(BatchWork::Correction {
            revision: MissRevision(0),
        }) = target.work
        {
            return Err(BatchPlanError::InvalidLearningWork(target.target.clone()));
        }
        let issued = input.issued.filter(|issued| {
            issued.work.obligation.target == target.target
                && issued.work.obligation.version == target.version
        });
        if let Some(issued) = issued.filter(|issued| issued.work.day == day) {
            rows.push((
                PlanEntry {
                    work: issued.work.clone(),
                    priority: issued.priority,
                    ordinal: 0,
                    state: PlanEntryState::Issued,
                },
                target.teaching,
            ));
        }
        let Some((priority, revision)) = target
            .work
            .and_then(|work| work.for_day(day, target.teaching))
        else {
            continue;
        };
        let obligation = WorkObligation {
            target: target.target.clone(),
            version: target.version,
            kind: priority_kind(priority),
            revision,
        };
        if issued.is_some_and(|issued| issued.work.obligation == obligation) {
            continue;
        }
        let state = if deferrals.contains(&obligation) {
            PlanEntryState::Deferred
        } else {
            PlanEntryState::Ready
        };
        rows.push((
            PlanEntry {
                work: WorkToken { day, obligation },
                priority,
                ordinal: 0,
                state,
            },
            target.teaching,
        ));
    }
    rows.sort_unstable_by(|(left, _), (right, _)| {
        left.priority
            .cmp(&right.priority)
            .then_with(|| {
                left.work
                    .obligation
                    .target
                    .cmp(&right.work.obligation.target)
            })
            .then_with(|| {
                left.work
                    .obligation
                    .revision
                    .cmp(&right.work.obligation.revision)
            })
    });
    let mut remaining = input.remaining;
    let entries = rows
        .into_iter()
        .filter(|(entry, family)| {
            entry.priority != PlanPriority::Teach
                || entry.state != PlanEntryState::Ready
                || remaining.admit(*family)
        })
        .enumerate()
        .map(|(ordinal, (mut entry, _))| {
            entry.ordinal = ordinal as u64;
            entry
        })
        .collect();
    Ok(PreparedBatch {
        basis: input.basis.clone(),
        entries,
    })
}

fn priority_kind(priority: PlanPriority) -> WorkKind {
    match priority {
        PlanPriority::Correction => WorkKind::Correction,
        PlanPriority::DueTest(_) => WorkKind::Test,
        PlanPriority::Teach => WorkKind::Teach,
    }
}

fn valid_target(target: &TargetId, version: ContentVersion) -> bool {
    !target.0.is_empty() && target.0.is_ascii() && version.0 != 0
}

fn valid_issued(issued: &CapturedIssuedWork) -> bool {
    let work = &issued.work;
    let obligation = &work.obligation;
    valid_target(&obligation.target, obligation.version)
        && obligation.kind == priority_kind(issued.priority)
        && match issued.priority {
            PlanPriority::Correction => obligation.revision.0 != 0,
            PlanPriority::DueTest(due) => due <= work.day,
            PlanPriority::Teach => true,
        }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempo_types::{MembershipEpoch, RankRevision};

    const TACTIC: TeachAdmission = TeachAdmission::Tactic {
        theme_admitted: true,
    };
    const OPENING: TeachAdmission = TeachAdmission::Repertoire { at_frontier: true };
    const TEACH: BatchWork = BatchWork::Teach {
        revision: MissRevision(0),
    };

    fn basis() -> PlanBasis {
        PlanBasis {
            dataset: "batch".into(),
            day: StudyDay(10),
            membership_epoch: MembershipEpoch(2),
            rank_revision: RankRevision(3),
        }
    }

    fn target(id: &str, teaching: TeachAdmission, work: BatchWork) -> BatchTarget {
        BatchTarget {
            target: id.into(),
            version: ContentVersion(1),
            available: true,
            teaching,
            work: Some(work),
        }
    }

    fn scheduled(day: i32) -> BatchWork {
        BatchWork::ScheduledTest {
            due: StudyDay(day),
            revision: MissRevision(0),
        }
    }

    fn correction(revision: u64) -> BatchWork {
        BatchWork::Correction {
            revision: MissRevision(revision),
        }
    }

    fn capture(work: &BatchTarget, day: i32, priority: PlanPriority) -> CapturedIssuedWork {
        let (kind, revision) = match work.work.unwrap() {
            BatchWork::Correction { revision } => (WorkKind::Correction, revision),
            BatchWork::ScheduledTest { revision, .. } => (WorkKind::Test, revision),
            BatchWork::Teach { revision } => (WorkKind::Teach, revision),
        };
        CapturedIssuedWork {
            work: WorkToken {
                day: StudyDay(day),
                obligation: WorkObligation {
                    target: work.target.clone(),
                    version: work.version,
                    kind,
                    revision,
                },
            },
            priority,
        }
    }

    fn input<'a>(basis: &'a PlanBasis, targets: &'a [BatchTarget]) -> BatchPlanInput<'a> {
        BatchPlanInput {
            basis,
            expected_targets: targets.len(),
            targets,
            remaining: TeachingRemaining {
                repertoire: 4,
                tactics: 4,
            },
            deferrals: &[],
            issued: None,
        }
    }

    fn identities(entries: &[PlanEntry]) -> Vec<(&str, PlanEntryState)> {
        entries
            .iter()
            .map(|entry| (entry.work.obligation.target.0.as_str(), entry.state))
            .collect()
    }

    #[test]
    fn batch_plan_orders_correction_due_and_teach_independently_of_capture_order() {
        let unavailable = BatchTarget {
            available: false,
            ..target("blocked-due", TACTIC, scheduled(1))
        };
        let mut targets = [
            target("z-due-oldest", TACTIC, scheduled(3)),
            target("b-due", OPENING, scheduled(7)),
            target("c-teach-tactic", TACTIC, TEACH),
            target("a-due", TACTIC, scheduled(7)),
            target("b-teach-opening", OPENING, TEACH),
            target("z-correction", TACTIC, correction(3)),
            target("a-correction", OPENING, correction(1)),
            target("future-due", TACTIC, scheduled(11)),
            target(
                "not-frontier",
                TeachAdmission::Repertoire { at_frontier: false },
                TEACH,
            ),
            target(
                "other-theme",
                TeachAdmission::Tactic {
                    theme_admitted: false,
                },
                TEACH,
            ),
            unavailable,
        ];
        let basis = basis();
        let first = build_batch_plan(input(&basis, &targets)).unwrap();
        targets.reverse();
        assert_eq!(build_batch_plan(input(&basis, &targets)).unwrap(), first);
        let (original, entries) = first.into_parts();
        assert_eq!(original, basis);
        assert_eq!(
            identities(&entries),
            [
                ("a-correction", PlanEntryState::Ready),
                ("z-correction", PlanEntryState::Ready),
                ("z-due-oldest", PlanEntryState::Ready),
                ("a-due", PlanEntryState::Ready),
                ("b-due", PlanEntryState::Ready),
                ("b-teach-opening", PlanEntryState::Ready),
                ("c-teach-tactic", PlanEntryState::Ready),
            ],
        );
        assert!(
            entries
                .iter()
                .enumerate()
                .all(|(ordinal, row)| row.ordinal == ordinal as u64)
        );
        assert_eq!(
            entries
                .iter()
                .map(|entry| entry.work.obligation.kind)
                .collect::<Vec<_>>(),
            [
                WorkKind::Correction,
                WorkKind::Correction,
                WorkKind::Test,
                WorkKind::Test,
                WorkKind::Test,
                WorkKind::Teach,
                WorkKind::Teach,
            ],
        );
    }

    #[test]
    fn batch_plan_preserves_effective_deferrals_without_spending_independent_quotas() {
        let basis = basis();
        let targets = [
            target("a-opening-deferred", OPENING, TEACH),
            target("b-opening-ready", OPENING, TEACH),
            target("c-opening-over-cap", OPENING, TEACH),
            target("a-tactic-deferred", TACTIC, TEACH),
            target("b-tactic-ready", TACTIC, TEACH),
            target("c-tactic-over-cap", TACTIC, TEACH),
        ];
        let deferrals = [
            capture(&targets[0], 9, PlanPriority::Teach)
                .work
                .deferred_on(basis.day),
            capture(&targets[3], 10, PlanPriority::Teach)
                .work
                .deferred_on(basis.day),
            capture(&targets[4], 9, PlanPriority::Teach)
                .work
                .deferred_on(StudyDay(9)),
        ];
        let (_, entries) = build_batch_plan(BatchPlanInput {
            remaining: TeachingRemaining {
                repertoire: 1,
                tactics: 1,
            },
            deferrals: &deferrals,
            ..input(&basis, &targets)
        })
        .unwrap()
        .into_parts();
        assert_eq!(
            identities(&entries),
            [
                ("a-opening-deferred", PlanEntryState::Deferred),
                ("a-tactic-deferred", PlanEntryState::Deferred),
                ("b-opening-ready", PlanEntryState::Ready),
                ("b-tactic-ready", PlanEntryState::Ready),
            ],
        );
        let (_, tactics_only) = build_batch_plan(BatchPlanInput {
            remaining: TeachingRemaining {
                repertoire: 0,
                tactics: 2,
            },
            deferrals: &deferrals,
            ..input(&basis, &targets)
        })
        .unwrap()
        .into_parts();
        assert_eq!(
            identities(&tactics_only),
            [
                ("a-opening-deferred", PlanEntryState::Deferred),
                ("a-tactic-deferred", PlanEntryState::Deferred),
                ("b-tactic-ready", PlanEntryState::Ready),
                ("c-tactic-over-cap", PlanEntryState::Ready),
            ],
        );
        let (_, exhausted) = build_batch_plan(BatchPlanInput {
            remaining: TeachingRemaining::default(),
            deferrals: &deferrals,
            ..input(&basis, &targets)
        })
        .unwrap()
        .into_parts();
        assert_eq!(
            identities(&exhausted),
            [
                ("a-opening-deferred", PlanEntryState::Deferred),
                ("a-tactic-deferred", PlanEntryState::Deferred),
            ],
        );
    }

    #[test]
    fn batch_plan_keeps_new_correction_independent_of_the_same_targets_deferred_revision() {
        let basis = basis();
        let old = target("position", OPENING, correction(3));
        let current = BatchTarget {
            work: Some(correction(4)),
            ..old.clone()
        };
        let deferrals = [capture(&old, 10, PlanPriority::Correction)
            .work
            .deferred_on(basis.day)];
        let old_input = [old];
        let new_input = [current];
        let build = |targets| {
            build_batch_plan(BatchPlanInput {
                deferrals: &deferrals,
                ..input(&basis, targets)
            })
            .unwrap()
            .into_parts()
            .1
        };
        assert_eq!(build(&old_input)[0].state, PlanEntryState::Deferred);
        let new = build(&new_input);
        assert_eq!(new[0].state, PlanEntryState::Ready);
        assert_eq!(new[0].work.obligation.revision, MissRevision(4));
    }

    #[test]
    fn batch_plan_projects_only_current_owned_issued_work_without_aliasing_new_work() {
        let basis = basis();
        let old = target("position", OPENING, correction(3));
        let current = BatchTarget {
            work: Some(correction(4)),
            ..old.clone()
        };
        let issued = capture(&old, 10, PlanPriority::Correction);
        let build = |target: BatchTarget, issued: &CapturedIssuedWork| {
            build_batch_plan(BatchPlanInput {
                issued: Some(issued),
                ..input(&basis, &[target])
            })
            .unwrap()
            .into_parts()
            .1
        };
        let rows = build(old.clone(), &issued);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].state, PlanEntryState::Issued);
        assert_eq!(rows[0].work, issued.work);

        let rows = build(current.clone(), &issued);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].work, issued.work);
        assert_eq!(rows[0].state, PlanEntryState::Issued);
        assert_eq!(rows[1].work.obligation.revision, MissRevision(4));
        assert_eq!(rows[1].state, PlanEntryState::Ready);

        let prior_day = capture(&old, 9, PlanPriority::Correction);
        assert!(build(old.clone(), &prior_day).is_empty());

        let replaced = BatchTarget {
            version: ContentVersion(2),
            ..current
        };
        let rows = build(replaced.clone(), &issued);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].work.obligation.version, ContentVersion(2));
        assert_eq!(rows[0].state, PlanEntryState::Ready);
        let suspended = BatchTarget {
            available: false,
            ..replaced
        };
        assert!(build(suspended, &issued).is_empty());

        let (_, empty) = build_batch_plan(BatchPlanInput {
            issued: Some(&issued),
            ..input(&basis, &[])
        })
        .unwrap()
        .into_parts();
        assert!(empty.is_empty());
    }

    #[test]
    fn batch_plan_rejects_incomplete_duplicate_or_malformed_captures() {
        let basis = basis();
        let target = target("position", OPENING, correction(3));
        let targets = [target.clone()];
        assert_eq!(
            build_batch_plan(BatchPlanInput {
                expected_targets: 2,
                ..input(&basis, &targets)
            }),
            Err(BatchPlanError::IncompleteCoverage {
                expected: 2,
                captured: 1
            }),
        );
        assert_eq!(
            build_batch_plan(input(&basis, &[target.clone(), target.clone()])),
            Err(BatchPlanError::DuplicateTarget(target.target.clone())),
        );
        let malformed_id = BatchTarget {
            version: ContentVersion(0),
            ..target.clone()
        };
        assert_eq!(
            build_batch_plan(input(&basis, &[malformed_id])),
            Err(BatchPlanError::InvalidTarget(target.target.clone())),
        );
        let malformed_work = BatchTarget {
            work: Some(correction(0)),
            ..target.clone()
        };
        assert_eq!(
            build_batch_plan(input(&basis, &[malformed_work])),
            Err(BatchPlanError::InvalidLearningWork(target.target.clone())),
        );
        let issued = CapturedIssuedWork {
            priority: PlanPriority::Teach,
            ..capture(&target, 10, PlanPriority::Correction)
        };
        assert_eq!(
            build_batch_plan(BatchPlanInput {
                issued: Some(&issued),
                ..input(&basis, &targets)
            }),
            Err(BatchPlanError::InvalidIssuedWork),
        );
    }
}
