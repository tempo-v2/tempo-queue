//! Daily selection over projected target and route facts.
use crate::Line;
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
    Test,
    Practice,
}

pub struct DailyChoice {
    pub target: TargetId,
    pub activity: DailyActivity,
    pub route: Option<String>,
    pub new_kind: Option<DailyKind>,
}

pub struct DailySelection {
    pub next: Option<DailyChoice>,
    pub outstanding: usize,
    pub due_count: usize,
    pub reteach_count: usize,
    pub repertoire_admitted: usize,
    pub tactics_admitted: usize,
    pub repertoire_remaining: usize,
    pub tactics_remaining: usize,
    pub practice: Option<DailyChoice>,
    pub unseen_tactics: bool,
}

pub struct DailyContext {
    pub seed: u64,
    pub today: StudyDay,
    pub repertoire_limit: usize,
    pub tactics_limit: usize,
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
        let repertoire_remaining = context.repertoire_limit.saturating_sub(admitted.0);
        let tactics_remaining = context.tactics_limit.saturating_sub(admitted.1);
        let available =
            |id: &TargetId| self.day != Some(context.today) || !self.deferred.contains(id);
        let rank = |id: &TargetId| daily_rank(context.seed, context.today, id);
        let mut due = Vec::new();
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
            if candidate.due.is_some_and(|day| day <= context.today) {
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
        let mut frontiers = Vec::new();
        if repertoire_remaining > 0 {
            for route in routes {
                if let Some(id) = route
                    .targets
                    .iter()
                    .find(|id| {
                        candidates
                            .get(*id)
                            .is_some_and(|candidate| !candidate.introduced_this_version)
                    })
                    .filter(|id| candidates[*id].first_introduced.is_none())
                {
                    frontiers.push((id, &route.id));
                }
            }
        }
        frontiers.sort_by_key(|(_, route)| *route);

        let frontier_count = frontiers
            .iter()
            .map(|(id, _)| *id)
            .collect::<BTreeSet<_>>()
            .len();
        let active_only = required_activity.is_some_and(|active| {
            !due.contains(&active)
                && !reteach.contains(&active)
                && !tactics.contains(&active)
                && !frontiers.iter().any(|(target, _)| *target == active)
        });
        let due_count = due.len();
        let reteach_count = reteach.len();
        let outstanding = usize::from(active_only)
            + due.len()
            + reteach.len()
            + frontier_count.min(repertoire_remaining)
            + tactics.len().min(tactics_remaining);
        let first = |ids: Vec<&TargetId>| -> Option<TargetId> {
            ids.into_iter()
                .filter(|id| available(id))
                .min_by_key(|id| (rank(id), *id))
                .cloned()
        };
        let next_due = due
            .into_iter()
            .filter(|id| available(id))
            .min_by_key(|id| {
                (
                    capture_priority
                        .get(*id)
                        .map(|time| (0, std::cmp::Reverse(*time)))
                        .unwrap_or((1, std::cmp::Reverse(0))),
                    rank(id),
                    *id,
                )
            })
            .cloned();
        let next = if let Some(target) = next_due {
            Some(DailyChoice {
                target,
                activity: DailyActivity::Test,
                route: None,
                new_kind: None,
            })
        } else if let Some(target) = first(reteach) {
            Some(DailyChoice {
                target,
                activity: DailyActivity::Teach,
                route: None,
                new_kind: None,
            })
        } else {
            let rotated = self
                .last_route
                .as_ref()
                .filter(|_| self.day == Some(context.today))
                .map_or(0, |route| frontiers.partition_point(|(_, id)| *id <= route));
            let repertoire = frontiers
                .iter()
                .cycle()
                .skip(rotated)
                .take(frontiers.len())
                .find(|(id, _)| available(id))
                .map(|(id, route)| DailyChoice {
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
                target,
                activity: DailyActivity::Test,
                route: None,
                new_kind: Some(DailyKind::Tactic),
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
            target: target.clone(),
            activity: DailyActivity::Practice,
            route: None,
            new_kind: None,
        });
        DailySelection {
            next,
            outstanding,
            due_count,
            reteach_count,
            repertoire_admitted: admitted.0,
            tactics_admitted: admitted.1,
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
