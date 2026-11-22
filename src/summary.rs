//! Count eligible Study work independently of the order chosen for issuance.

use crate::{
    BatchPlanError, BatchPlanInput, PlanPriority, TeachAdmission,
    batch_plan::{ClassifiedWork, classify_work},
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ReadyWorkCounts {
    pub corrections: usize,
    pub tests: usize,
    pub teaches: usize,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StudySummaryCounts {
    pub repertoire: ReadyWorkCounts,
    pub tactics: ReadyWorkCounts,
    /// Matching current-day deferrals, independently of the remaining Teach slots.
    pub deferred: usize,
}

/// Count required work from the captured eligibility facts. Exact issued work
/// is excluded. A family's allowance bounds only its ready first introductions.
pub fn classify_study_summary(
    input: BatchPlanInput<'_>,
) -> Result<StudySummaryCounts, BatchPlanError> {
    let mut result = StudySummaryCounts::default();
    let mut repertoire_introductions = 0;
    let mut tactics_introductions = 0;
    classify_work(&input, |work| {
        let ClassifiedWork::Candidate {
            target,
            priority,
            deferred,
            first_introduction,
            ..
        } = work
        else {
            return;
        };
        if deferred {
            result.deferred += 1;
            return;
        }
        let (family, introductions) = match target.teaching {
            TeachAdmission::Repertoire { .. } => {
                (&mut result.repertoire, &mut repertoire_introductions)
            }
            TeachAdmission::Tactic { .. } => (&mut result.tactics, &mut tactics_introductions),
        };
        match priority {
            PlanPriority::Correction => family.corrections += 1,
            PlanPriority::DueTest(_) => family.tests += 1,
            PlanPriority::Teach if first_introduction => *introductions += 1,
            PlanPriority::Teach => family.teaches += 1,
        }
    })?;
    result.repertoire.teaches += repertoire_introductions.min(input.remaining.repertoire);
    result.tactics.teaches += tactics_introductions.min(input.remaining.tactics);
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BatchTarget, BatchWork, CapturedIssuedWork, TeachKind, TeachingRemaining};
    use tempo_types::{
        ContentVersion, DeferredWork, MembershipEpoch, MissRevision, PlanBasis, RankRevision,
        StudyDay, WorkKind, WorkObligation, WorkToken,
    };

    const DAY: StudyDay = StudyDay(10);
    const OPENING: TeachAdmission = TeachAdmission::Repertoire { at_frontier: true };
    const TACTIC: TeachAdmission = TeachAdmission::Tactic {
        theme_admitted: true,
    };
    const INTRODUCTION: BatchWork = BatchWork::Teach {
        revision: MissRevision(0),
        kind: TeachKind::Introduction,
    };
    const RELEARN: BatchWork = BatchWork::Teach {
        revision: MissRevision(0),
        kind: TeachKind::Relearn,
    };
    const DUE: BatchWork = BatchWork::ScheduledTest {
        due: StudyDay(7),
        revision: MissRevision(2),
    };

    fn target(id: &str, teaching: TeachAdmission, work: BatchWork) -> BatchTarget {
        BatchTarget {
            target: id.into(),
            version: ContentVersion(4),
            available: true,
            teaching,
            work: Some(work),
        }
    }

    fn basis() -> PlanBasis {
        PlanBasis {
            dataset: "summary-dataset".into(),
            day: DAY,
            membership_epoch: MembershipEpoch(7),
            rank_revision: RankRevision(1),
        }
    }

    #[test]
    fn study_summary_counts_exact_eligible_ready_and_deferred_work_without_order() {
        let mut targets = vec![
            target("z-opening-new", OPENING, INTRODUCTION),
            target("a-opening-new", OPENING, INTRODUCTION),
            target("opening-relearn", OPENING, RELEARN),
            target("tactic-new", TACTIC, INTRODUCTION),
            target("tactic-relearn", TACTIC, RELEARN),
            target("opening-test", OPENING, DUE),
            target("tactic-active", TACTIC, DUE),
            target("tactic-deferred", TACTIC, DUE),
            target("tactic-new-revision", TACTIC, DUE),
            target(
                "opening-correction",
                OPENING,
                BatchWork::Correction {
                    revision: MissRevision(3),
                },
            ),
            target(
                "tactic-future",
                TACTIC,
                BatchWork::ScheduledTest {
                    due: StudyDay(11),
                    revision: MissRevision(2),
                },
            ),
            target(
                "tactic-gate",
                TeachAdmission::Tactic {
                    theme_admitted: false,
                },
                INTRODUCTION,
            ),
            BatchTarget {
                available: false,
                ..target("opening-suspended", OPENING, DUE)
            },
        ];
        let obligation = |target, version, revision| WorkObligation {
            target,
            version: ContentVersion(version),
            kind: WorkKind::Test,
            revision: MissRevision(revision),
        };
        let deferrals = [
            DeferredWork {
                day: DAY,
                obligation: obligation("tactic-deferred".into(), 4, 2),
            },
            DeferredWork {
                day: StudyDay(9),
                obligation: obligation("opening-test".into(), 4, 2),
            },
            DeferredWork {
                day: DAY,
                obligation: obligation("tactic-new-revision".into(), 4, 1),
            },
            DeferredWork {
                day: DAY,
                obligation: obligation("tactic-new-revision".into(), 3, 2),
            },
        ];
        let mut issued = CapturedIssuedWork {
            work: WorkToken {
                day: StudyDay(9),
                obligation: obligation("tactic-active".into(), 4, 2),
            },
            stage_priority: Some(PlanPriority::DueTest(StudyDay(7))),
        };
        let mut basis = basis();
        let counts = |basis: &PlanBasis, targets: &[BatchTarget], issued: &CapturedIssuedWork| {
            classify_study_summary(BatchPlanInput {
                basis,
                expected_targets: targets.len(),
                targets,
                remaining: TeachingRemaining {
                    repertoire: 1,
                    tactics: 0,
                },
                deferrals: &deferrals,
                issued: Some(issued),
            })
            .unwrap()
        };
        let expected = StudySummaryCounts {
            repertoire: ReadyWorkCounts {
                corrections: 1,
                tests: 1,
                teaches: 2,
            },
            tactics: ReadyWorkCounts {
                corrections: 0,
                tests: 1,
                teaches: 1,
            },
            deferred: 1,
        };
        assert_eq!(counts(&basis, &targets, &issued), expected);
        targets.reverse();
        basis.rank_revision = RankRevision(32);
        issued.stage_priority = None;
        assert_eq!(counts(&basis, &targets, &issued), expected);
    }

    #[test]
    fn study_summary_does_not_report_zero_for_incomplete_capture() {
        let basis = basis();
        let targets = [target("one", TACTIC, DUE)];
        assert_eq!(
            classify_study_summary(BatchPlanInput {
                basis: &basis,
                expected_targets: 2,
                targets: &targets,
                remaining: TeachingRemaining::default(),
                deferrals: &[],
                issued: None,
            }),
            Err(BatchPlanError::IncompleteCoverage {
                expected: 2,
                captured: 1
            })
        );
    }
}
