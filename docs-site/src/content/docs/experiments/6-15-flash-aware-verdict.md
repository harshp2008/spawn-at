---
title: "6.15 Flash-aware verdict"
---

Flash-aware verdict: audit, ground truth from the client, validation on X11 (harness only). The harness now judges flashes from the client's own paint events; the extension cannot see a slow client's late paint.

| Field | Value |
| :-- | :-- |
| **ID** | TEST_LOG 6.15. Follows [6.14](/spawn-at/experiments/6-14-mutter-api-and-arrival-wait/); refined by [6.16](/spawn-at/experiments/6-16-flash-numeric-headline/). |
| **Date** | 2026-10-13. |
| **Build** | Harness commits `199a6d5` (fixture events), `2abd62e` (detector), `c00ddd6` (runner), `1dc2752` (UI, report, `--strict-flash`), `27a7303` (a fix found by the first live run) and `1da549d` (frame note). Debug `spawn-at` and fixture unless noted (one batch used a release fixture). The version string is not recorded. No extension change; no setting, `/sys`, `/proc` write or nested shell was used. |
| **Verdict** | PASS/FAIL of placement is unchanged; a pass with a certain flash is shown as "PASS (flash)", and `run --strict-flash` (default off) turns a flashing pass into a failure. The log's finding is that the extension cannot see a slow client's late paint at all. |
| **Raw results** | Not in the repository (the log's header says raw results and scripts are not). |

## What was tested

**Question**

What counts as a flash today, and what cannot the existing checks see? Can the test fixture's own
paint events give a ground truth that the extension's log cannot?

**Setup and environment**

An audit of the signals in the trace analyser and runner. Then a change to `spawn-at-verify`: the
fixture writes `fixture-<pid>.events` when `SPAWN_AT_FIXTURE_EVENT_DIR` is set (`FIRST_PAINT WxH`,
`RESIZED WxH`, `MOVED X,Y`, in `CLOCK_MONOTONIC` ns). A flash is defined as the reveal (`UNCLOAK`)
coming before the fixture's first paint at its final size, or the painted size changing by more than
2 px per axis, or the position differing by more than 2 px, within 1500 ms after the reveal. The
extension's `+N ms` clock is aligned with the fixture's by 4 `SetLogging(true)` echoes per run
(measured half-width 1.5 to 3.0 ms over 90 runs).

Validation on an X11 session (`XDG_SESSION_TYPE=x11`, GNOME Shell 46.0, gjs 1.80.2, 22 CPUs): isolated
`run` invocations 1.5 s apart, 10 per spec per batch, three batches. Load average at batch start about
2 to 3; 14.4 at the first run of the second batch, right after a 17 CPU-minute release build. The
top-left shrink spec and the control (the fixture with `--paint-delay-ms 400`) were throwaway specs in
`scratch/`.

**Commands run**

`spawn-at-verify run` per spec (the debug build), and `cargo test -p spawn-at-verify --features
tui,fixture`. Full command lines not recorded.

## Result

- The audit found four things the existing checks do not see: a size change after the reveal with no
  position correction (a top-left anchor does not move, so the log is silent); a window visible before
  the cloak; on X11 the frame rectangle lies (`move_resize_frame` changes it at once, so every
  trace-based check passes by construction, confirmed in 60 stored traces); and a flash that is only
  paint cost.
- A first live run found that using the fixture's `Resized` and `Moved` events as times was wrong
  (those events are delivery times that lag the real change); commit `27a7303` stopped using them.
- Validation:

| batch | spec | runs / pass | flashes (new check) | runs with an old warning |
| :--- | :--- | :---: | :---: | :---: |
| val1 | `placement/bottom-right-larger` | 10 / 10 | 9 | 0 |
| val1 | control: `--paint-delay-ms 400` | 10 / 10 | 10 | 0 |
| val2 | `bottom-right-larger` | 10 / 10 | 10 | 0 |
| val3, release fixture | `bottom-right-larger` | 10 / 10 | 0 | 0 |
| val3, release fixture | control | 10 / 10 | 10 | 0 |

  The old warnings caught 0 of 90 runs. The control proves the check fires on a known late paint; the
  release-fixture batch proves it stays silent when the client is fast. With the debug fixture, the 800x600
  buffer paints 3 to 27 ms after the reveal; 17 of the 19 measured leads of this spec are shorter than
  one 60 Hz frame (16.7 ms), so how many were actually visible is not shown.
- Tests: `cargo test -p spawn-at-verify --features tui,fixture` gave 341 passed and 4 ignored (baseline
  305).
- The gap between `COMMIT_RESOLVED` and `BATCH_STEP SetPositionAnchored`, read from `commit.js` and
  `extension.js` and measured in 60 stored traces: 8.9 / 11.7 / 13.6 / 15.2 / 17.1 / 24.2 ms (min / p25 /
  median / p75 / p90 / max), all `AT_TARGET`, with no timer, wait or log line in between. The log's
  reading (a hypothesis, not profiled) is that native code that emitted the signal keeps running for 9
  to 24 ms before control reaches the main loop.

## Not verified

Which of the measured leads were visible (no screen capture was taken). A profile of the native work in
the gap. Wayland (where the fixture gets no `Moved` events, and whether the frame rect lies there was
not tested), real toolkit apps, the full suite, and the "window visible before the cloak" check. The
top-left shrink spec was a reliable flasher in neither batch (1 of 20 debug runs).

## Sources

- `docs/TEST_LOG.md` section 6.15 (lines 531-599)
