//! Daily selection over projected target and route facts.
use crate::Line;
use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet};
use tempo_types::{StudyDay, TargetId};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DailyKind {
    Tactic,
    Repertoire,
}

pub struct DailyCandidate {
    pub kind: DailyKind,
    pub introduced_this_version: bool,
    pub first_introduced: Option<StudyDay>,
    pub due: Option<StudyDay>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DailyActivity {
    Teach,
    Repair,
    Test,
    Practice,
}

/// The durable turn update proposed by Queue when the chosen attempt is issued.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DailyOrderingEffect {
    Preserve,
    OweRegularReview,
    SatisfyRegularReview,
}

#[derive(Clone)]
pub struct DailyChoice {
    pub ordering: DailyOrderingEffect,
    /// The exact durable immediate-recall request bound when this attempt is issued.
    pub immediate_request: Option<u64>,
    pub target: TargetId,
    pub activity: DailyActivity,
    pub route: Option<String>,
    pub new_kind: Option<DailyKind>,
    pub relevance: Option<RelevanceReason>,
}

pub struct DailySelection {
    pub next: Option<DailyChoice>,
    pub outstanding: usize,
    pub due_count: usize,
    /// Pending guided repairs, including the active target and session skips.
    pub repairs_remaining: usize,
    pub corrective_recalls_remaining: usize,
    pub reteach_count: usize,
    pub repertoire_admitted: usize,
    pub tactics_admitted: usize,
    pub repertoire_remaining: usize,
    pub tactics_remaining: usize,
    pub practice: Option<DailyChoice>,
    pub unseen_tactics: bool,
    ordinary_reviews: BTreeMap<TargetId, Option<RelevanceReason>>,
}

impl DailySelection {
    /// Keep priority work first. A linked review may take the place of an
    /// ordinary due review, using the same eligibility and ordering effect.
    pub fn linked_review(&self, target: &TargetId) -> Option<DailyChoice> {
        self.next.as_ref().filter(|choice| {
            choice.activity == DailyActivity::Test
                && choice.immediate_request.is_none()
                && self.ordinary_reviews.contains_key(&choice.target)
        })?;
        self.ordinary_reviews
            .get(target)
            .map(|relevance| due_review(target.clone(), *relevance))
    }
}

fn due_review(target: TargetId, relevance: Option<RelevanceReason>) -> DailyChoice {
    DailyChoice {
        ordering: DailyOrderingEffect::SatisfyRegularReview,
        immediate_request: None,
        target,
        activity: DailyActivity::Test,
        route: None,
        new_kind: None,
        relevance,
    }
}

pub struct DailyContext {
    pub seed: u64,
    pub today: StudyDay,
    pub repertoire_limit: usize,
    pub tactics_limit: usize,
}

#[derive(Default)]
pub struct DailyAdmissions {
    pub repertoire: usize,
    pub tactics: usize,
}

pub struct DailySelectionInput<'a> {
    pub context: DailyContext,
    pub candidates: &'a BTreeMap<TargetId, DailyCandidate>,
    pub routes: &'a [Line],
    pub admissions: DailyAdmissions,
    pub required_activity: Option<&'a TargetId>,
    pub new_tactic_order: Option<&'a BTreeMap<TargetId, usize>>,
    pub capture_priority: &'a BTreeMap<TargetId, u64>,
    pub evidence: &'a DailyEvidence,
}

/// Relevance is an ordering input for work that is already eligible.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TargetRelevance {
    pub exposure: u32,
    pub misses: u32,
    pub modeled_millionths: Option<u32>,
    pub window_games: u32,
}

impl TargetRelevance {
    fn rank(self) -> (u8, u32, u32) {
        if self.exposure > 0 {
            (2, self.exposure, self.misses)
        } else if let Some(reach) = self.modeled_millionths {
            (1, reach, 0)
        } else {
            (0, 0, 0)
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RelevanceReason {
    pub evidence: TargetRelevance,
    pub downstream: bool,
}

/// One explicitly requested recall. The supplied activity is decided by Learning;
/// only Test and Practice are accepted for an immediate blind recall.
pub struct ImmediateRecall {
    pub request_id: u64,
    pub target: TargetId,
    pub activity: DailyActivity,
}

/// Evidence facts projected by the daily application from Learning and Games.
#[derive(Default)]
pub struct DailyEvidence {
    pub regular_review_owed: bool,
    pub immediate_recalls: Vec<ImmediateRecall>,
    pub requested_tests: BTreeSet<TargetId>,
    pub teaching_targets: BTreeSet<TargetId>,
    pub repair_targets: BTreeSet<TargetId>,
    pub corrective_recalls_remaining: usize,
    pub relevance: BTreeMap<TargetId, TargetRelevance>,
}

#[derive(Default)]
pub struct DailySession {
    day: Option<StudyDay>,
    deferred: BTreeSet<TargetId>,
    last_route: Option<String>,
    last_new: Option<DailyKind>,
    last_practice: Option<TargetId>,
}

impl DailySession {
    fn for_day(&mut self, day: StudyDay) {
        if self.day != Some(day) {
            *self = Self {
                day: Some(day),
                ..Self::default()
            };
        }
    }

    pub fn defer(&mut self, target: TargetId, day: StudyDay) {
        self.for_day(day);
        self.deferred.insert(target);
    }

    pub fn is_deferred(&self, target: &TargetId, day: StudyDay) -> bool {
        self.day == Some(day) && self.deferred.contains(target)
    }

    /// A reset removes this target's skip without resuming other skipped work.
    pub fn clear_deferral(&mut self, target: &TargetId) {
        self.deferred.remove(target);
    }

    pub fn resume_skipped(&mut self) {
        self.deferred.clear();
    }

    /// Call after durable issuance succeeds.
    pub fn issued(&mut self, choice: &DailyChoice, day: StudyDay) {
        self.for_day(day);
        if choice.activity == DailyActivity::Practice {
            self.last_practice = Some(choice.target.clone());
        }
        if let Some(route) = &choice.route {
            self.last_route = Some(route.clone());
        }
        if let Some(kind) = choice.new_kind {
            self.last_new = Some(kind);
        }
    }

    pub fn select(
        &self,
        context: DailyContext,
        candidates: &BTreeMap<TargetId, DailyCandidate>,
        routes: &[Line],
        admitted: (usize, usize),
        required_activity: Option<&TargetId>,
    ) -> DailySelection {
        self.select_with_tactics(
            context,
            candidates,
            routes,
            admitted,
            required_activity,
            None,
        )
    }

    pub fn select_with_tactics(
        &self,
        context: DailyContext,
        candidates: &BTreeMap<TargetId, DailyCandidate>,
        routes: &[Line],
        admitted: (usize, usize),
        required_activity: Option<&TargetId>,
        new_order: Option<&BTreeMap<TargetId, usize>>,
    ) -> DailySelection {
        self.select_with_capture_priority(
            context,
            candidates,
            routes,
            admitted,
            required_activity,
            new_order,
            &BTreeMap::new(),
        )
    }

    /// A supplied order both restricts and ranks new tactics. Reviews and practice
    /// still use the full candidate set.
    #[allow(clippy::too_many_arguments)]
    pub fn select_with_capture_priority(
        &self,
        context: DailyContext,
        candidates: &BTreeMap<TargetId, DailyCandidate>,
        routes: &[Line],
        admitted: (usize, usize),
        required_activity: Option<&TargetId>,
        new_order: Option<&BTreeMap<TargetId, usize>>,
        capture_priority: &BTreeMap<TargetId, u64>,
    ) -> DailySelection {
        self.select_with_evidence(
            context,
            candidates,
            routes,
            admitted,
            required_activity,
            new_order,
            capture_priority,
            &DailyEvidence::default(),
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn select_with_evidence(
        &self,
        context: DailyContext,
        candidates: &BTreeMap<TargetId, DailyCandidate>,
        routes: &[Line],
        admitted: (usize, usize),
        required_activity: Option<&TargetId>,
        new_order: Option<&BTreeMap<TargetId, usize>>,
        capture_priority: &BTreeMap<TargetId, u64>,
        evidence: &DailyEvidence,
    ) -> DailySelection {
        self.select_daily(DailySelectionInput {
            context,
            candidates,
            routes,
            admissions: DailyAdmissions {
                repertoire: admitted.0,
                tactics: admitted.1,
            },
            required_activity,
            new_tactic_order: new_order,
            capture_priority,
            evidence,
        })
    }

    pub fn select_daily(&self, input: DailySelectionInput<'_>) -> DailySelection {
        let DailySelectionInput {
            context,
            candidates,
            routes,
            admissions,
            required_activity,
            new_tactic_order: new_order,
            capture_priority,
            evidence,
        } = input;
        let repertoire_remaining = context
            .repertoire_limit
            .saturating_sub(admissions.repertoire);
        let tactics_remaining = context.tactics_limit.saturating_sub(admissions.tactics);
        let available = |id: &TargetId| !self.is_deferred(id, context.today);
        let rank = |id: &TargetId| daily_rank(context.seed, context.today, id);
        let mut due = Vec::new();
        let mut repairs = Vec::new();
        let mut reteach = Vec::new();
        let mut tactics = Vec::new();
        let mut practiced = Vec::new();
        let mut unseen_tactics = false;
        for (id, candidate) in candidates {
            let eligible_new = new_order.is_none_or(|order| order.contains_key(id));
            if candidate.kind == DailyKind::Tactic {
                if candidate.introduced_this_version {
                    practiced.push(id);
                } else {
                    unseen_tactics |= eligible_new && candidate.first_introduced.is_none();
                }
            }
            if evidence.repair_targets.contains(id) {
                repairs.push(id);
            } else if candidate.due.is_some_and(|day| day <= context.today)
                || evidence.requested_tests.contains(id)
            {
                due.push(id);
            } else if !candidate.introduced_this_version {
                match (
                    candidate.kind,
                    candidate.first_introduced,
                    tactics_remaining,
                ) {
                    (DailyKind::Repertoire, Some(_), _) => reteach.push(id),
                    (DailyKind::Tactic, None, n) if n > 0 && eligible_new => tactics.push(id),
                    _ => {}
                }
            }
        }
        let relevance = |id: &TargetId| {
            evidence
                .relevance
                .get(id)
                .copied()
                .filter(|fact| fact.rank().0 > 0)
        };
        let ordinary_relevance = |id: &TargetId| {
            (!evidence.requested_tests.contains(id) && !capture_priority.contains_key(id))
                .then(|| relevance(id))
                .flatten()
        };
        let mut frontiers = Vec::new();
        if repertoire_remaining > 0 {
            for route in routes {
                if let Some(index) = route
                    .targets
                    .iter()
                    .position(|id| {
                        candidates
                            .get(id)
                            .is_some_and(|candidate| !candidate.introduced_this_version)
                    })
                    .filter(|index| {
                        candidates[&route.targets[*index]]
                            .first_introduced
                            .is_none()
                    })
                {
                    let relevant = !evidence.teaching_targets.is_empty()
                        && route
                            .targets
                            .iter()
                            .any(|target| evidence.teaching_targets.contains(target));
                    let id = &route.targets[index];
                    let reason = route.targets[index..]
                        .iter()
                        .filter(|target| {
                            candidates
                                .get(*target)
                                .is_some_and(|candidate| !candidate.introduced_this_version)
                        })
                        .filter_map(|target| {
                            relevance(target).map(|evidence| RelevanceReason {
                                evidence,
                                downstream: target != id,
                            })
                        })
                        .min_by_key(|reason| (Reverse(reason.evidence.rank()), reason.downstream));
                    frontiers.push((id, &route.id, relevant, reason));
                }
            }
        }
        frontiers.sort_by_key(|(_, route, _, _)| *route);

        let frontier_count = frontiers
            .iter()
            .map(|(id, _, _, _)| *id)
            .collect::<BTreeSet<_>>()
            .len();
        let active_only = required_activity.is_some_and(|active| {
            !repairs.contains(&active)
                && !due.contains(&active)
                && !reteach.contains(&active)
                && !tactics.contains(&active)
                && !frontiers.iter().any(|(target, _, _, _)| *target == active)
        });
        let due_count = due.len();
        let repairs_remaining = repairs.len();
        let reteach_count = reteach.len();
        let immediate = evidence
            .immediate_recalls
            .iter()
            .filter(|request| candidates.contains_key(&request.target))
            .filter(|request| {
                matches!(
                    request.activity,
                    DailyActivity::Test | DailyActivity::Practice
                )
            })
            .filter(|request| available(&request.target))
            .max_by_key(|request| request.request_id);
        let immediate_targets = evidence
            .immediate_recalls
            .iter()
            .filter(|request| candidates.contains_key(&request.target))
            .filter(|request| {
                matches!(
                    request.activity,
                    DailyActivity::Test | DailyActivity::Practice
                )
            })
            .collect::<Vec<_>>();
        let covered_ordinary = immediate_targets
            .iter()
            .filter(|request| request.activity == DailyActivity::Test)
            .map(|request| &request.target)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .filter(|target| required_activity != Some(*target) && due.contains(target))
            .count();
        let outstanding = immediate_targets.len()
            + usize::from(active_only)
            + repairs_remaining
            + due.len()
            + reteach.len()
            + frontier_count.min(repertoire_remaining)
            + tactics.len().min(tactics_remaining)
            - covered_ordinary;
        let first = |ids: Vec<&TargetId>| -> Option<TargetId> {
            ids.into_iter()
                .filter(|id| available(id))
                .min_by_key(|id| (rank(id), *id))
                .cloned()
        };
        let next_repair = first(repairs);
        let available_due = |id: &&TargetId| {
            available(id)
                && !immediate_targets
                    .iter()
                    .any(|request| &request.target == *id)
        };
        let due_relevance = |id| {
            ordinary_relevance(id).map(|evidence| RelevanceReason {
                evidence,
                downstream: false,
            })
        };
        let ordinary_reviews = due
            .iter()
            .copied()
            .filter(available_due)
            .filter(|id| {
                !evidence.requested_tests.contains(*id) && !capture_priority.contains_key(*id)
            })
            .map(|id| (id.clone(), due_relevance(id)))
            .collect();
        let next_due = due
            .into_iter()
            .filter(available_due)
            .min_by_key(|id| {
                (
                    !evidence.requested_tests.contains(*id),
                    capture_priority
                        .get(*id)
                        .map(|time| (0, std::cmp::Reverse(*time)))
                        .unwrap_or((1, std::cmp::Reverse(0))),
                    Reverse(ordinary_relevance(id).map(TargetRelevance::rank)),
                    rank(id),
                    *id,
                )
            })
            .cloned();
        let regular_available = next_repair.is_some() || next_due.is_some();
        let next = if let Some(request) =
            immediate.filter(|_| !evidence.regular_review_owed || !regular_available)
        {
            Some(DailyChoice {
                immediate_request: Some(request.request_id),
                ordering: DailyOrderingEffect::OweRegularReview,
                target: request.target.clone(),
                activity: request.activity,
                relevance: None,
                route: None,
                new_kind: None,
            })
        } else if let Some(target) = next_repair {
            Some(DailyChoice {
                immediate_request: None,
                ordering: DailyOrderingEffect::SatisfyRegularReview,
                target,
                activity: DailyActivity::Repair,
                relevance: None,
                route: None,
                new_kind: None,
            })
        } else if let Some(target) = next_due {
            let relevance = due_relevance(&target);
            Some(due_review(target, relevance))
        } else if let Some(target) = first(reteach) {
            Some(DailyChoice {
                immediate_request: None,
                ordering: DailyOrderingEffect::Preserve,
                target,
                activity: DailyActivity::Teach,
                relevance: None,
                route: None,
                new_kind: None,
            })
        } else {
            let rotated = self
                .last_route
                .as_ref()
                .filter(|_| self.day == Some(context.today))
                .map_or(0, |route| {
                    frontiers.partition_point(|(_, id, _, _)| *id <= route)
                });
            let rotated_frontiers = frontiers
                .iter()
                .cycle()
                .skip(rotated)
                .take(frontiers.len())
                .filter(|(id, _, _, _)| available(id));
            let repertoire = rotated_frontiers
                .min_by_key(|(_, _, teaching, reason)| {
                    (
                        !*teaching,
                        Reverse(reason.map(|reason| reason.evidence.rank())),
                    )
                })
                .map(|(id, route, _, reason)| DailyChoice {
                    immediate_request: None,
                    ordering: DailyOrderingEffect::Preserve,
                    relevance: *reason,
                    target: (*id).clone(),
                    activity: DailyActivity::Teach,
                    route: Some((*route).clone()),
                    new_kind: Some(DailyKind::Repertoire),
                });
            let target = match new_order {
                Some(order) => tactics
                    .into_iter()
                    .filter(|id| available(id))
                    .min_by_key(|id| order[*id])
                    .cloned(),
                None => first(tactics),
            };
            let tactic = target.map(|target| DailyChoice {
                immediate_request: None,
                ordering: DailyOrderingEffect::Preserve,
                target,
                activity: DailyActivity::Test,
                route: None,
                new_kind: Some(DailyKind::Tactic),
                relevance: None,
            });
            if self.day == Some(context.today) && self.last_new == Some(DailyKind::Repertoire) {
                tactic.or(repertoire)
            } else {
                repertoire.or(tactic)
            }
        };
        let last = self
            .last_practice
            .as_ref()
            .filter(|_| self.day == Some(context.today));
        let next_practice = last
            .and_then(|id| {
                let key = (rank(id), id);
                practiced
                    .iter()
                    .copied()
                    .filter(|target| (rank(target), *target) > key)
                    .min_by_key(|target| (rank(target), *target))
            })
            .or_else(|| {
                practiced
                    .into_iter()
                    .min_by_key(|target| (rank(target), *target))
            });
        let practice = next_practice.map(|target| DailyChoice {
            immediate_request: None,
            ordering: DailyOrderingEffect::Preserve,
            target: target.clone(),
            activity: DailyActivity::Practice,
            relevance: None,
            route: None,
            new_kind: None,
        });
        DailySelection {
            ordinary_reviews,
            corrective_recalls_remaining: evidence.corrective_recalls_remaining,
            next,
            outstanding,
            due_count,
            repairs_remaining,
            reteach_count,
            repertoire_admitted: admissions.repertoire,
            tactics_admitted: admissions.tactics,
            repertoire_remaining,
            tactics_remaining,
            practice,
            unseen_tactics,
        }
    }
}

/// Stable per-target rank. Candidate removal leaves other ranks unchanged.
pub fn daily_rank(seed: u64, day: StudyDay, target: &TargetId) -> u64 {
    let mut hash = seed ^ 0xcbf29ce484222325;
    for byte in day.0.to_le_bytes().into_iter().chain(target.0.bytes()) {
        hash = (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
    }
    hash ^= hash >> 30;
    hash = hash.wrapping_mul(0xbf58476d1ce4e5b9);
    hash ^= hash >> 27;
    hash = hash.wrapping_mul(0x94d049bb133111eb);
    hash ^ (hash >> 31)
}

#[cfg(test)]
mod immediate_tests {
    use super::*;

    fn select(
        candidates: &BTreeMap<TargetId, DailyCandidate>,
        evidence: &DailyEvidence,
    ) -> DailySelection {
        DailySession::default().select_daily(DailySelectionInput {
            context: DailyContext {
                seed: 0,
                today: StudyDay(10),
                repertoire_limit: 0,
                tactics_limit: 0,
            },
            candidates,
            routes: &[],
            admissions: DailyAdmissions::default(),
            required_activity: None,
            new_tactic_order: Some(&BTreeMap::new()),
            capture_priority: &BTreeMap::new(),
            evidence,
        })
    }

    #[test]
    fn newest_immediate_recall_precedes_repairs_due_and_capture_priority_without_new_allowance() {
        let candidates = ["repair", "due", "older", "newest"]
            .into_iter()
            .map(|id| {
                (
                    TargetId::from(id),
                    DailyCandidate {
                        kind: DailyKind::Tactic,
                        introduced_this_version: true,
                        first_introduced: Some(StudyDay(1)),
                        due: (id == "due").then_some(StudyDay(10)),
                    },
                )
            })
            .collect();
        let mut evidence = DailyEvidence::default();
        evidence.repair_targets.insert("repair".into());
        evidence.requested_tests.insert("due".into());
        evidence.immediate_recalls = vec![
            ImmediateRecall {
                request_id: 7,
                target: "older".into(),
                activity: DailyActivity::Test,
            },
            ImmediateRecall {
                request_id: 8,
                target: "newest".into(),
                activity: DailyActivity::Practice,
            },
        ];
        let selected = select(&candidates, &evidence);
        let next = selected.next.unwrap();
        assert_eq!(next.target, TargetId::from("newest"));
        assert_eq!(next.immediate_request, Some(8));
        assert_eq!(next.activity, DailyActivity::Practice);
        assert_eq!(next.new_kind, None);
        assert_eq!(selected.outstanding, 4);
        // Selection never consumes durable work; only accepted completion can do that.
        assert_eq!(
            select(&candidates, &evidence)
                .next
                .unwrap()
                .immediate_request,
            Some(8)
        );
    }

    #[test]
    fn separate_occurrences_of_one_puzzle_remain_separate_requests() {
        let candidates = BTreeMap::from([(
            "same".into(),
            DailyCandidate {
                kind: DailyKind::Tactic,
                introduced_this_version: true,
                first_introduced: Some(StudyDay(1)),
                due: Some(StudyDay(10)),
            },
        )]);
        let mut evidence = DailyEvidence::default();
        evidence.immediate_recalls = vec![
            ImmediateRecall {
                request_id: 10,
                target: "same".into(),
                activity: DailyActivity::Test,
            },
            ImmediateRecall {
                request_id: 11,
                target: "same".into(),
                activity: DailyActivity::Test,
            },
        ];
        let selected = select(&candidates, &evidence);
        assert_eq!(selected.outstanding, 2);
        assert_eq!(selected.next.unwrap().immediate_request, Some(11));
        evidence.immediate_recalls.pop();
        assert_eq!(
            select(&candidates, &evidence)
                .next
                .unwrap()
                .immediate_request,
            Some(10)
        );
    }
}
