//! Small shared fixture for invariant tests; examples for individual input types
//! construct their own facts so their usage can be read without this helper.
use crate::{Candidate, Line, QueueInput};
use tempo_types::StudyDay;

pub(crate) fn candidate(id: &str, introduced: bool, due: Option<i32>) -> Candidate {
    Candidate {
        target: id.into(),
        active: true,
        introduced,
        due: due.map(StudyDay),
        repeat_pending: false,
        independent_new: false,
    }
}

pub(crate) fn fixture() -> QueueInput {
    let mut tactic = candidate("tactic", false, None);
    tactic.independent_new = true;
    let mut quarantined = candidate("invalid", true, Some(1));
    quarantined.active = false;
    QueueInput {
        today: StudyDay(10),
        candidates: vec![
            candidate("due", true, Some(10)),
            candidate("future", true, Some(11)),
            candidate("new1", false, None),
            candidate("new2", false, None),
            tactic,
            quarantined,
        ],
        lines: vec![
            Line {
                id: "line1".into(),
                targets: vec!["due".into(), "new1".into(), "new2".into()],
            },
            Line {
                id: "transposition".into(),
                targets: vec!["new1".into(), "new1".into(), "new2".into()],
            },
        ],
    }
}
