---
title: "P8 Sway flicker experiment (not run)"
---

## Title

Sway flicker experiment, proposed as P8 in the test coverage audit. It was not run.

## ID

TEST_COVERAGE_AUDIT P8. It has no TEST_LOG section.

## Date

Proposed in the audit dated 2026-10-15 (`main` at `ff1eb6a`). No run date: it was never run.

## Build

Not applicable: nothing was built or run for it.

## Question

Whether, and for how long, a one-frame flash exists on Sway: whether a window is ever in the tiled
layout before it floats. `docs/DESIGN_SWAY.md` section 5 says the "one-frame flash" is unmeasured and
that the option the Sway driver uses (react to `window::new` and send one chained command) has an
unverified race, because the event is emitted when the view is mapped, with a node of `rect` 0x0.

## Setup and environment

As proposed, not performed: a private headless Sway (started by `scripts/sway-headless.sh`), record the
`window::new` IPC events and `get_tree` at tight intervals around `spawn`, and, if a screencopy tool
works in the headless session, capture frames. Windows would be closed by id and the harness would stop
Sway. The audit marks the feasibility of screencopy on the headless backend as unverified, and the size
as medium to large.

## Commands run

None.

## Raw result location

None. No result exists.

## Result

No result. The audit notes there is no flash row for non-GNOME backends today: it would need a new flash
basis in the verify crate (`FlashBasis`), because the existing flash analysis reads the GNOME
extension's log, and on Sway there is no extension, hence no `UNCLOAK` and no clock anchor.

## Verdict

None. The audit orders it after the headless Sway test proposal P4: "P8 is research and should come
after P4".

## Not verified

Everything about it. The knowledge base lists, among open questions, whether a one-frame flash on Sway
is visible on a real session ("Not measured").

## Sources

- `docs/TEST_COVERAGE_AUDIT.md` (section 4, the Sway spawn row; section 6, proposal P8 and the priority
  line)
- `docs/DESIGN_SWAY.md` section 5 ("Flicker-free strategy")
- `docs/KNOWLEDGE.md` ("Open questions")
