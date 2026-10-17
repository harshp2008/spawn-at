---
title: "6.11 Live results after the late-resize fix"
---

## Title

Live results after the late-resize fix

## ID

TEST_LOG 6.11. Follows [6.10](/spawn-at/experiments/6-10-placement-resize-specs/). The log adds a
correction to this section in [6.11.1](/spawn-at/experiments/6-11-1-final-state-oracle/).

## Date

2026-10-12.

## Build

Commit `c7c4084` (HEAD; `is_dirty_build: false` in every report). The fix under test is commit
`9e6b65d` (modern extension tree only): a second size check in the reveal's own tick that logs
`LATE_RESIZE_CORRECTED` and re-anchors. The version string is not recorded. No product code, spec,
oracle, tolerance or fixture was changed for the measurements; the only new code is
`cargo xtask burn-cpu`.

## Question

After the late-resize fix, do the placement specs of [6.10](/spawn-at/experiments/6-10-placement-resize-specs/)
pass when idle and when the CPU is loaded? The log's target: 0 failures in the idle gate (a) and the
loaded gate (b).

## Setup and environment

GNOME 46 on Wayland, one machine (22 CPUs, `--virtual-monitor 1920x1080`, workarea `0,40 1920x1040`),
4 other desktop windows open and ordinary desktop load. Load was made with `cargo xtask burn-cpu
--threads 22` after a 90 s warm-up (one process spinning N threads; checked: 22 threads for 4 s gave
2140% CPU and no process left afterwards). "Busy" is a 1-minute load average above `cpu_count / 2 =
11`. Gates: (a) 16 specs x 20 isolated and with `--repeat 20`, idle; (b) the same under `burn-cpu`;
(c) `zenity` 500x450 centred, 20 runs under load; (d) `spawn-at-verify calibrate --runs 20`; (e) a real
app, `gnome-text-editor`, 10 runs per size. The step-0 check confirmed that the running Shell had the
fix: the installed `extension.js` had the same sha256 as the tree's, the Shell started after the
log-out and log-in, and the journal held a `LATE_RESIZE_CORRECTED` line.

## Commands run

- `cargo xtask burn-cpu --threads N --seconds S`.
- `spawn-at-verify run` over the 16 specs, isolated and with `--repeat 20`; `spawn-at-verify calibrate
  --runs 20`; the full library `spawn-at-verify run`.
- `spawn-at spawn --class '*' --size 500 450 --anchor center --json -- env GDK_BACKEND=wayland zenity
  --text-info`, with the window re-read by `spawn-at query windows` 1.3 s after exit.
- `spawn-at spawn --size W H --anchor bottom-right --json gnome-text-editor`, re-read 1.5 s after exit.

## Raw result location

`verify-results/gate-20261012/` and throwaway drivers in `scratch/`, both gitignored. Not in the
repository.

## Result

Judged on the rectangle `spawn-at` printed at exit (the numbers the log kept as measured):

| Gate | Before (6.10) | After |
| :--- | :---: | :---: |
| (a) 16 specs x 20, isolated, idle | 95/320 and 318/320 | 320/320 (100%) |
| (a) `--repeat 20`, idle | 315/320 | 320/320 (100%) |
| (b) isolated, `burn-cpu` | not measured | 212/320 (66.3%) |
| (b) `--repeat 20`, `burn-cpu` | not measured | 213/320 (66.6%) |
| (c) `zenity`, centred, `burn-cpu` | 16/20 and 19/20 centred (load not recorded) | 17/20 |

- Target 0 failures: met for (a), not met for (b). Nothing was relaxed to make a gate pass.
- `LATE_RESIZE_CORRECTED` fired in 60 isolated and 85 `--repeat` runs under load, all passing, and in 5
  `zenity` runs; by code reading each such window is one the old code would have left anchored for its
  old size. That the old code would have failed those runs was not tested.
- All 215 failing fixture runs under load show `COMMIT_RESOLVED reason=SIZE_SETTLED finalFrame=(400x300)`
  and a reveal at 400x300: the app's resize arrived after the reveal, which the reactive anchor
  handles. In 195 of the 215 the journal holds an `ANCHOR_CORRECTION` to exactly the expected place;
  the other 20 are the top-left control, where only the printed size was stale. So `spawn-at` printed
  the old rectangle and exited; the window was corrected afterwards.
- `zenity`: 17 of 20 ended centred. The 3 failures ended at 300x400 and the extension never
  dispatched: `ARM_SPAWN` to `WINDOW_CREATED` took 1338, 1242 and 1356 ms in them, against 469 to
  1083 ms in the 17 dispatched runs. A wildcard target (`--class '*'`) expires after 1200 ms, which
  fits; the log marks it a hypothesis.
- Cloak duration (`calibrate`): the largest of 120 runs was 55.95 ms, below the 100 ms budget. The log
  makes no claim of "no change beyond noise" because there is no "before" for delay-0 and delay-50.
- Full library `spawn-at-verify run`: `Total: 35, Passed: 27, Failed: 0, Skipped: 0, Blocked: 0`; the
  8 non-passed are `kind = both` specs awaiting a human verdict, all with `auto_status: auto-pass`.
- Real app, `gnome-text-editor`: for 30x20 the editor's own minimum is 391x100 and the placement was
  correct for it (10 of 10 identical); for 3000x2000 the app was limited to 1920x1040 and the
  window overflowed the work area by the 16 px margin on the right and bottom (10 of 10 identical).
  The CLI printed a warning that the window is oversized.

## Verdict

Mixed: the fix removes the idle failures (0 of 640) and re-anchors 150 windows under load, but a
resize that lands after the reveal under heavy load is corrected afterwards while `spawn-at` prints
the old rectangle. The log states that rows 2, 3 and 3b of its section 6.4 are fixed for a resize that
lands before the reveal and not for one after it, and that the correction of this section means the
"failures" counted here were judged on a stale printed rectangle ([6.11.1](/spawn-at/experiments/6-11-1-final-state-oracle/)).

## Not verified

Whether the 215 + 3 failures are visible on screen; the TUI drawer's busy warning; a "before" for the
cloak numbers of delay-0 and delay-50 and for the 16 specs under load (the old extension is not
loadable without a log-out); a fixture-side re-read of the final geometry after a failing run; a named
`--class` (instead of `'*'`) `zenity` run under load; why `bottom-right-larger` never failed under
load; GTK3, Qt and Electron apps.

## Sources

- `docs/TEST_LOG.md` section 6.11 (lines 77-194)
