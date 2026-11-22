//! Pure decisions over published prepared-order metadata and accepted Study work.

use tempo_types::{
    DeferredWork, PlanBasis, PlanGeneration, StudyDay, SummaryBasis, WorkKind, WorkToken,
};

/// Maximum indexed candidates the application can validate in one Next.
pub const MAX_PLAN_CANDIDATES: usize = 32;

/// Only Store's complete, published generation can be a serving head.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlanHead {
    pub generation: PlanGeneration,
    pub basis: PlanBasis,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlanFreshness {
    Missing,
    MembershipDirty,
    RankDirty,
    Current,
}

impl PlanFreshness {
    pub fn can_serve(self) -> bool {
        matches!(self, Self::RankDirty | Self::Current)
    }
}

pub fn plan_freshness(current: &PlanBasis, head: Option<&PlanHead>) -> PlanFreshness {
    let Some(head) = head else {
        return PlanFreshness::Missing;
    };
    let published = &head.basis;
    if (published.dataset != current.dataset)
        || (published.day != current.day)
        || (published.membership_epoch != current.membership_epoch)
    {
        return PlanFreshness::MembershipDirty;
    }
    if published.rank_revision != current.rank_revision {
        PlanFreshness::RankDirty
    } else {
        PlanFreshness::Current
    }
}

/// Count freshness is independent of rank revision and generation publication.
pub fn summary_is_current(current: &SummaryBasis, published: Option<&SummaryBasis>) -> bool {
    published == Some(current)
}

/// Ordering class for ready work. Equal priorities use stable target identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum PlanPriority {
    Correction,
    DueTest(StudyDay),
    Teach,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlanEntryState {
    Ready,
    Issued,
    Deferred,
    Consumed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlanEntry {
    pub work: WorkToken,
    pub priority: PlanPriority,
    pub ordinal: u64,
    pub state: PlanEntryState,
}

/// Proof supplied by the addressed lookup of one published generation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlanScanResult {
    /// The first authoritative eligible ready item in index order.
    Validated(WorkToken),
    /// The index proved no ready candidate remains under current membership.
    Exhausted,
    /// The candidate limit or an unavailable validation prevented a conclusion.
    Unfinished,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlanScan {
    pub generation: PlanGeneration,
    pub result: PlanScanResult,
}

/// Result for fresh Next after the application checked the active pointer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlanReadiness {
    Ready {
        generation: PlanGeneration,
        work: WorkToken,
    },
    Preparing,
    Empty,
}

pub fn plan_readiness(
    current: &PlanBasis,
    head: Option<&PlanHead>,
    scan: Option<PlanScan>,
) -> PlanReadiness {
    if !plan_freshness(current, head).can_serve() {
        return PlanReadiness::Preparing;
    }
    let Some(scan) =
        scan.filter(|scan| head.is_some_and(|head| head.generation == scan.generation))
    else {
        return PlanReadiness::Preparing;
    };
    match scan.result {
        PlanScanResult::Validated(work) if work.day == current.day => PlanReadiness::Ready {
            generation: scan.generation,
            work,
        },
        PlanScanResult::Exhausted => PlanReadiness::Empty,
        _ => PlanReadiness::Preparing,
    }
}

/// The semantic effect of authority already accepted by the command.
///
/// Learning and Study establish which effect occurred. Receipt replays bypass
/// projection work, and intermediate moves carry Unchanged.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AcceptedPlanChange<'a> {
    Unchanged,
    Consumed(&'a WorkToken),
    FirstMiss {
        issued_test: &'a WorkToken,
        bound_correction: &'a WorkToken,
    },
    Deferred(&'a DeferredWork),
    MembershipChanged,
    LegacyWorkChanged,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlanMutation {
    Consume(WorkToken),
    ProjectCorrection {
        consumed_test: WorkToken,
        issued_correction: PlanEntry,
    },
    Defer(WorkToken),
}

/// A bounded write proposal for the projected order or a durable dirty marker.
///
/// The caller commits this alongside accepted authority. Failure to find a
/// compatible projected row must not reject a valid Study command.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlanUpdate {
    Preserve,
    Apply {
        generation: PlanGeneration,
        mutation: PlanMutation,
    },
    MarkMembershipDirty,
}

/// Resolve the projection effect using the pre-transition basis. Store must
/// commit a proposed mutation only while the row and generation still match.
pub fn plan_update(
    current: &PlanBasis,
    head: Option<&PlanHead>,
    row: Option<(PlanGeneration, &PlanEntry)>,
    change: AcceptedPlanChange<'_>,
) -> PlanUpdate {
    match change {
        AcceptedPlanChange::Unchanged => return PlanUpdate::Preserve,
        AcceptedPlanChange::MembershipChanged | AcceptedPlanChange::LegacyWorkChanged => {
            return PlanUpdate::MarkMembershipDirty;
        }
        _ => {}
    }
    if !plan_freshness(current, head).can_serve() {
        return PlanUpdate::MarkMembershipDirty;
    }
    let Some((generation, entry)) = row.filter(|(generation, entry)| {
        head.is_some_and(|head| head.generation == *generation) && entry.work.day == current.day
    }) else {
        return PlanUpdate::MarkMembershipDirty;
    };
    let mutation = match change {
        AcceptedPlanChange::Consumed(work) if issued(entry, work) => {
            PlanMutation::Consume(work.clone())
        }
        AcceptedPlanChange::FirstMiss {
            issued_test,
            bound_correction,
        } if issued(entry, issued_test) && is_first_correction(issued_test, bound_correction) => {
            PlanMutation::ProjectCorrection {
                consumed_test: issued_test.clone(),
                issued_correction: PlanEntry {
                    work: bound_correction.clone(),
                    priority: PlanPriority::Correction,
                    ordinal: entry.ordinal,
                    state: PlanEntryState::Issued,
                },
            }
        }
        AcceptedPlanChange::Deferred(deferral)
            if deferral.matches(&entry.work)
                && matches!(entry.state, PlanEntryState::Ready | PlanEntryState::Issued) =>
        {
            PlanMutation::Defer(entry.work.clone())
        }
        _ => return PlanUpdate::MarkMembershipDirty,
    };
    PlanUpdate::Apply {
        generation,
        mutation,
    }
}

fn issued(entry: &PlanEntry, work: &WorkToken) -> bool {
    entry.state == PlanEntryState::Issued && &entry.work == work
}

fn is_first_correction(test: &WorkToken, correction: &WorkToken) -> bool {
    let previous = &test.obligation;
    let next = &correction.obligation;
    test.day == correction.day
        && previous.kind == WorkKind::Test
        && next.kind == WorkKind::Correction
        && previous.target == next.target
        && previous.version == next.version
        && previous.revision.0.checked_add(1) == Some(next.revision.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempo_types::{
        ContentVersion, MembershipEpoch, MissRevision, RankRevision, SummaryEpoch, WorkObligation,
    };

    fn basis() -> PlanBasis {
        PlanBasis {
            dataset: "dataset".into(),
            day: StudyDay(10),
            membership_epoch: MembershipEpoch(2),
            rank_revision: RankRevision(1),
        }
    }

    fn head(basis: &PlanBasis) -> PlanHead {
        PlanHead {
            generation: PlanGeneration(7),
            basis: basis.clone(),
        }
    }

    fn work(day: StudyDay, kind: WorkKind, revision: u64) -> WorkToken {
        WorkToken {
            day,
            obligation: WorkObligation {
                target: "target".into(),
                version: ContentVersion(3),
                kind,
                revision: MissRevision(revision),
            },
        }
    }

    fn test_row(day: StudyDay, revision: u64) -> PlanEntry {
        PlanEntry {
            work: work(day, WorkKind::Test, revision),
            priority: PlanPriority::DueTest(StudyDay(9)),
            ordinal: 4,
            state: PlanEntryState::Issued,
        }
    }

    #[test]
    fn plan_contract_needs_complete_current_membership_before_proving_empty() {
        let basis = basis();
        let head = head(&basis);
        let exhausted = PlanScan {
            generation: head.generation,
            result: PlanScanResult::Exhausted,
        };
        assert_eq!(
            plan_readiness(&basis, None, Some(exhausted.clone())),
            PlanReadiness::Preparing,
        );
        assert_eq!(
            plan_readiness(&basis, Some(&head), None),
            PlanReadiness::Preparing,
        );
        let unfinished = PlanScan {
            generation: head.generation,
            result: PlanScanResult::Unfinished,
        };
        assert_eq!(
            plan_readiness(&basis, Some(&head), Some(unfinished)),
            PlanReadiness::Preparing,
        );
        let other_generation = PlanScan {
            generation: PlanGeneration(8),
            ..exhausted.clone()
        };
        assert_eq!(
            plan_readiness(&basis, Some(&head), Some(other_generation)),
            PlanReadiness::Preparing,
        );
        for changed in [
            PlanBasis {
                day: StudyDay(11),
                ..basis.clone()
            },
            PlanBasis {
                dataset: "other dataset".into(),
                ..basis.clone()
            },
            PlanBasis {
                membership_epoch: MembershipEpoch(3),
                ..basis.clone()
            },
        ] {
            assert_eq!(
                plan_freshness(&changed, Some(&head)),
                PlanFreshness::MembershipDirty,
            );
            assert_eq!(
                plan_readiness(&changed, Some(&head), Some(exhausted.clone())),
                PlanReadiness::Preparing,
            );
        }
        assert_eq!(
            plan_readiness(&basis, Some(&head), Some(exhausted)),
            PlanReadiness::Empty,
        );
    }

    #[test]
    fn plan_contract_rank_refresh_preserves_serving_order_and_summary() {
        let mut basis = basis();
        let head = head(&basis);
        let summary = SummaryBasis {
            dataset: basis.dataset.clone(),
            day: basis.day,
            epoch: SummaryEpoch(4),
        };
        basis.rank_revision = RankRevision(2);
        assert_eq!(
            plan_freshness(&basis, Some(&head)),
            PlanFreshness::RankDirty
        );
        let token = work(basis.day, WorkKind::Teach, 3);
        assert_eq!(
            plan_readiness(
                &basis,
                Some(&head),
                Some(PlanScan {
                    generation: head.generation,
                    result: PlanScanResult::Validated(token.clone()),
                }),
            ),
            PlanReadiness::Ready {
                generation: head.generation,
                work: token,
            },
        );
        assert!(summary_is_current(&summary, Some(&summary)));
        assert!(!summary_is_current(&summary, None));
        assert!(!summary_is_current(
            &SummaryBasis {
                epoch: SummaryEpoch(5),
                ..summary.clone()
            },
            Some(&summary),
        ));
    }

    #[test]
    fn plan_contract_first_miss_projects_a_distinct_nonready_correction() {
        let basis = basis();
        let head = head(&basis);
        let row = test_row(basis.day, 3);
        let correction = work(basis.day, WorkKind::Correction, 4);
        let change = AcceptedPlanChange::FirstMiss {
            issued_test: &row.work,
            bound_correction: &correction,
        };
        let update = plan_update(&basis, Some(&head), Some((head.generation, &row)), change);
        let PlanUpdate::Apply {
            generation,
            mutation:
                PlanMutation::ProjectCorrection {
                    consumed_test,
                    issued_correction,
                },
        } = update
        else {
            panic!("expected both exact projection addresses");
        };
        assert_eq!(generation, head.generation);
        assert_eq!(consumed_test, row.work);
        assert_eq!(issued_correction.work, correction);
        assert_eq!(issued_correction.state, PlanEntryState::Issued);
        assert_eq!(issued_correction.priority, PlanPriority::Correction);
        assert_eq!(
            plan_update(
                &basis,
                Some(&head),
                Some((generation, &issued_correction)),
                AcceptedPlanChange::Consumed(&row.work),
            ),
            PlanUpdate::MarkMembershipDirty,
        );
        assert_eq!(
            plan_update(
                &basis,
                Some(&head),
                Some((generation, &issued_correction)),
                AcceptedPlanChange::Consumed(&correction),
            ),
            PlanUpdate::Apply {
                generation,
                mutation: PlanMutation::Consume(correction.clone()),
            },
        );

        let newer = PlanEntry {
            work: work(basis.day, WorkKind::Correction, 5),
            ..issued_correction
        };
        assert_eq!(
            plan_update(
                &basis,
                Some(&head),
                Some((generation, &newer)),
                AcceptedPlanChange::Consumed(&correction),
            ),
            PlanUpdate::MarkMembershipDirty,
        );

        let stale = work(basis.day, WorkKind::Correction, 3);
        assert_eq!(
            plan_update(
                &basis,
                Some(&head),
                Some((generation, &row)),
                AcceptedPlanChange::FirstMiss {
                    issued_test: &row.work,
                    bound_correction: &stale,
                },
            ),
            PlanUpdate::MarkMembershipDirty,
        );
    }

    #[test]
    fn plan_contract_absent_legacy_foreign_or_stale_projection_never_guesses_consumption() {
        let basis = basis();
        let head = head(&basis);
        let row = test_row(basis.day, 3);
        let consumption = AcceptedPlanChange::Consumed(&row.work);
        let stale_basis = PlanBasis {
            membership_epoch: MembershipEpoch(3),
            ..basis.clone()
        };
        for (current, published, projection) in [
            (&basis, None, None),
            (&basis, Some(&head), None),
            (&basis, Some(&head), Some((PlanGeneration(8), &row))),
            (&stale_basis, Some(&head), Some((head.generation, &row))),
        ] {
            assert_eq!(
                plan_update(current, published, projection, consumption.clone()),
                PlanUpdate::MarkMembershipDirty,
            );
        }
        assert_eq!(
            plan_update(
                &basis,
                Some(&head),
                Some((head.generation, &row)),
                AcceptedPlanChange::LegacyWorkChanged,
            ),
            PlanUpdate::MarkMembershipDirty,
        );
        assert_eq!(
            plan_update(&basis, None, None, AcceptedPlanChange::Unchanged),
            PlanUpdate::Preserve,
        );
    }

    #[test]
    fn plan_contract_skip_rebases_only_the_same_obligation_to_acceptance_day() {
        let mut basis = basis();
        let issued = work(basis.day, WorkKind::Test, 3);
        basis.day = StudyDay(11);
        let head = head(&basis);
        let current = test_row(basis.day, 3);
        let deferral = issued.deferred_on(basis.day);
        assert_eq!(
            plan_update(
                &basis,
                Some(&head),
                Some((head.generation, &current)),
                AcceptedPlanChange::Deferred(&deferral),
            ),
            PlanUpdate::Apply {
                generation: head.generation,
                mutation: PlanMutation::Defer(current.work),
            },
        );
        let newer = test_row(basis.day, 4);
        assert_eq!(
            plan_update(
                &basis,
                Some(&head),
                Some((head.generation, &newer)),
                AcceptedPlanChange::Deferred(&deferral),
            ),
            PlanUpdate::MarkMembershipDirty,
        );
        assert_eq!(issued.day, StudyDay(10));
    }
}
