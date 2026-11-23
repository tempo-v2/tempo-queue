# tempo-queue

Queue proposes Daily Study activity from an explicit study day, projected
Learning facts, and ordered repertoire occurrences. It receives data values
from the application and has one Tempo dependency, `tempo-types`.

[Daily selection](src/daily.rs) exposes `DailySession::select_daily` and
the facts it accepts. [Line](src/line.rs) identifies each ordered repertoire
occurrence. Multiple lines can share a target. Shared frontier entries count
once toward the outstanding work.

The application validates current content and persists attempt issuance with the
ordering effect Queue proposes. Queue's session remembers skips and recent
presentation choices. Learning owns availability and grading effects.

## Immediate recall from a real missed tactic

`DailyEvidence::immediate_recalls` carries durable request identities and
Learning's chosen Test or Practice activity. These requests take priority over
ordinary work. If several are ready, the newest accepted request wins. More
than one request can refer to the same target.

Tempo resumes an active attempt before asking Queue for another one. Store
binds the request to the issued attempt, so completing an older attempt cannot
erase a later request. When Store accepts a new request, Tempo clears the
affected target's skip for the current day. Receipt replay leaves that skip
alone.

## Tests

From the assembled workspace, run:

```sh
cargo test -p tempo-queue
```
