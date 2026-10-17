---
title: "6.11.1 Final-state oracle and re-measure"
---

## Title

Final-state oracle, post-reveal warning and re-measure

## ID

TEST_LOG 6.11.1. A correction to [6.11](/spawn-at/experiments/6-11-late-resize-fix-live/).

## Date

2026-10-12.

## Build

Binary at commit `e47c41f` (HEAD, clean build, preflight pass). The fix `9e6b65d` was live in the same
Shell as in 6.11. Two harness commits are part of the section: `d01687b` (the final-state oracle) and
`e47c41f` (the trace warning). The version string is not recorded. No product code changed.

## Question

6.11 judged placement on the rectangle `spawn-at` printed at exit. Is the window's final rectangle
correct, and how often is the window shown at the wrong geometry first?

## Setup and environment

GNOME 46 on Wayland. 16 resize specs x 20 runs, each a separate `spawn-at-verify run --glob
placement/<id>`, isolated (no `verify.*` window and no fixture process before any of the 640 runs),
once idle and once under `cargo xtask burn-cpu --threads 22` after a 90 s warm-up.

Harness change (commit `d01687b`): when the printed rectangle fails the geometry check, the runner keeps
the window alive for `SETTLE_MS = 1300` ms (1 s for the extension's late-resize listener plus 300 ms),
re-reads it with `spawn-at query windows --json`, and judges the final rectangle. If printed differs
from final the result carries the warning `spawn-at printed a stale rectangle (printed A, final B)`; it
is not a failure. A printed rectangle that already matches waits for nothing. Commit `e47c41f`: the
trace analyser warns `placement corrected after reveal (+N ms)` for the first `ANCHOR_CORRECTION` after
`UNCLOAK`. Both are tested in both directions.

## Commands run

`spawn-at-verify run --glob placement/<id>` per run; `cargo xtask burn-cpu --threads 22`; `spawn-at
query windows --json` for the re-read; `journalctl --user -b _COMM=gnome-shell` for the arm and release
check. Full command lines not recorded.

## Raw result location

`verify-results/gate-20261012/g3/` and `scratch/gap.jq` (throwaway), both gitignored. Not in the
repository.

## Result

| Condition | Final-pass | Fails | Stale-print warnings | Post-reveal-correction warnings | `LATE_RESIZE_CORRECTED` |
| :--- | :---: | :---: | :---: | :---: | :---: |
| idle, isolated | 320/320 | 0 | 3 | 3 | 0 |
| `burn-cpu` 22 threads, isolated | 320/320 | 0 | 99 | 134 | 62 |

- Target "idle: 0 failures and 0 post-reveal corrections": 0 failures met; 0 post-reveal corrections
  **not** met (3 of 320, at +42.8, +53.3 and +7.8 ms after `UNCLOAK`). Nothing was relaxed.
- On the final rectangle there are no loaded failures: 0 of 320, where 6.11 had 108 and 107 failures on
  the printed rectangle. The windows were revealed at the size the app had before its resize and moved
  to the expected place by the reactive anchor 2.5 to 330.7 ms after the reveal (loaded n=134, median
  33.8 ms, p90 140.2 ms); `spawn-at` had printed the earlier rectangle. Every one of the 99 settle reads
  found the window at the expected rectangle.
- Load differed from 6.11 (37.97 to 48.53 now against 21.13 to 28.83 then, same 22 threads), so the
  stale-print rate (99 of 320 against 108 of 320) is not a controlled comparison.
- **Cloak gap check** (`UNCLOAK_4C` to `UNCLOAK`, stored pre-fix reports against today, no log-out):
  the gap is larger today by about 7.7 ms (delay-0 median 5.44 to 13.15 ms) and 9.0 ms (delay-500
  median 2.55 to 11.55 ms) against same-era sessions. The pre-fix gaps are bimodal (a fast mode of
  about 1.2 to 5 ms and a slow mode of about 9 to 15 ms); 40 of 42 post-fix delay-500 runs and 42 of 42
  delay-0 runs are in the slow mode. By code the fix adds no wait when no late resize is found. The
  cloak duration itself stayed far under the 100 ms budget (maximum 55.95 ms in 120 runs).
- Read-only checks: the verify runner never passes `--no-wait` to `spawn`; in the isolated regime no
  run's `ARM_SPAWN` came before the previous run's `ANCHOR_RELEASED` (0 of 357), while in `--repeat`
  304 of 319 did, which is that regime's definition.

## Verdict

Placement is correct on the final rectangle (0 of 640 failures across idle and loaded). What remains
wrong, and now shows as warnings, is that the window is shown at the wrong geometry first and
`spawn-at`'s printed rectangle is not where the window ends up. The log calls the "known flaky under
CPU load" marks of 6.11 a statement about the printed rectangle, not about final placement.

## Not verified

Whether a stale-print jump is visible on screen (hypothesis: it is, for one to a few frames). The TUI
drawer's busy warning. The cause of the larger cloak gap: the log's hypothesis, not tested, is the
compositor's frame timing at that time and not the fix; a before and after with the old extension
needs a log-out and was not done. A named `--class` `zenity` run under load. The 37
`verify.cloak.delayedpaint` spawns inside the load-phase journal range were not traced to a run.

## Sources

- `docs/TEST_LOG.md` section 6.11.1 (lines 197-282)
