//! Order eligible first introductions from checked membership and resolved preferences.
use std::collections::BTreeMap;
use tempo_types::{StudyDay, TargetId};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackDifficulty {
    Easy,
    Medium,
    Hard,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PackMembership<'a> {
    pub theme: &'a str,
    pub difficulty: PackDifficulty,
    pub ordinal: usize,
    pub position: usize,
}

pub struct TacticPackCandidate<'a> {
    pub target: &'a TargetId,
    pub memberships: Vec<PackMembership<'a>>,
    pub first_introduced: Option<StudyDay>,
    pub guided_introduction: bool,
}

/// An absent preference is unrestricted. A present preference limits new
/// admissions, with themes in the caller's canonical rotation order.
pub fn rank_new_tactics<'a>(
    candidates: impl IntoIterator<Item = TacticPackCandidate<'a>>,
    today: StudyDay,
    selected_themes: Option<&[&str]>,
) -> Option<BTreeMap<TargetId, usize>> {
    let themes = selected_themes?
        .iter()
        .copied()
        .enumerate()
        .map(|(index, theme)| (theme, index))
        .collect::<BTreeMap<_, _>>();
    if themes.is_empty() {
        return Some(BTreeMap::new());
    }

    let mut admitted = vec![0; themes.len()];
    let mut eligible = Vec::new();
    for candidate in candidates {
        if candidate.guided_introduction {
            continue;
        }
        let first = candidate.first_introduced;
        if first.is_some() && first != Some(today) {
            continue;
        }
        // Attribute overlapping source memberships to their earliest eligible pack.
        if let Some((order, theme)) = candidate
            .memberships
            .iter()
            .filter_map(|pack| {
                Some((
                    (pack.difficulty, pack.ordinal, pack.position),
                    *themes.get(pack.theme)?,
                ))
            })
            .min()
        {
            if first.is_some() {
                admitted[theme] += 1;
            } else {
                eligible.push((theme, order, candidate.target));
            }
        }
    }
    let start = today.0.rem_euclid(themes.len() as i32) as usize;
    eligible.sort_unstable_by_key(|&(theme, order, target)| {
        (
            admitted[theme],
            (theme + themes.len() - start) % themes.len(),
            order,
            target,
        )
    });
    Some(
        eligible
            .into_iter()
            .enumerate()
            .map(|(rank, (_, _, target))| (target.clone(), rank))
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filtering_preserves_the_difference_between_unrestricted_and_empty() {
        assert_eq!(rank_new_tactics([], StudyDay(0), None), None);
        assert_eq!(
            rank_new_tactics([], StudyDay(0), Some(&[])),
            Some(BTreeMap::new())
        );
    }

    #[test]
    fn overlapping_membership_attribution_and_canonical_day_rotation_are_stable() {
        let first = TargetId::from("first");
        let second = TargetId::from("second");
        let pack = |theme, ordinal| PackMembership {
            theme,
            ordinal,
            difficulty: PackDifficulty::Easy,
            position: 1,
        };
        let candidates = || {
            [
                TacticPackCandidate {
                    target: &first,
                    memberships: vec![pack("fork", 2), pack("pin", 1)],
                    first_introduced: None,
                    guided_introduction: false,
                },
                TacticPackCandidate {
                    target: &second,
                    memberships: vec![pack("fork", 1)],
                    first_introduced: None,
                    guided_introduction: false,
                },
            ]
        };
        let selected = Some(["pin", "fork"].as_slice());
        let day_zero = rank_new_tactics(candidates(), StudyDay(0), selected).unwrap();
        assert_eq!(day_zero[&first], 0);
        assert_eq!(
            day_zero,
            rank_new_tactics(candidates().into_iter().rev(), StudyDay(0), selected).unwrap()
        );
        assert_eq!(
            rank_new_tactics(candidates(), StudyDay(1), selected).unwrap()[&second],
            0
        );
    }
}
