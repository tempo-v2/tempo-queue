//! Content order used to choose the next new repertoire target.
use serde::{Deserialize, Serialize};
use tempo_types::TargetId;

/// Ordered repertoire occurrences used by Daily Study to find each Teach frontier.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Line {
    /// Content occurrence identity; it does not create a separate learning identity.
    pub id: String,
    /// Ordered occurrences; transpositions and repeated positions reuse target IDs.
    pub targets: Vec<TargetId>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DailyActivity, DailyCandidate, DailyContext, DailyKind, DailySession};
    use tempo_types::StudyDay;

    #[test]
    fn shared_repertoire_frontier_counts_once_across_lines_and_repetitions() {
        let candidates = ["shared-position", "later-position"]
            .into_iter()
            .map(|id| {
                (
                    id.into(),
                    DailyCandidate {
                        kind: DailyKind::Repertoire,
                        introduced_this_version: false,
                        first_introduced: None,
                        due: None,
                    },
                )
            })
            .collect();
        let main = Line {
            id: "main-line".into(),
            targets: vec!["shared-position".into(), "later-position".into()],
        };
        let repeat = Line {
            id: "transposition".into(),
            targets: vec!["shared-position".into(), "shared-position".into()],
        };
        let select = |routes: &[Line]| {
            DailySession::default().select(
                DailyContext {
                    seed: 0,
                    today: StudyDay(10),
                    repertoire_limit: 10,
                    tactics_limit: 0,
                },
                &candidates,
                routes,
                (0, 0),
                None,
            )
        };
        for selection in [
            select(std::slice::from_ref(&main)),
            select(&[main.clone(), repeat]),
        ] {
            assert_eq!(selection.outstanding, 1);
            let next = selection.next.unwrap();
            assert_eq!(next.activity, DailyActivity::Teach);
            assert_eq!(next.target, TargetId::from("shared-position"));
        }
    }
}
