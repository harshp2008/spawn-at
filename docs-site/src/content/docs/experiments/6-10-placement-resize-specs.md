---
title: "6.10 Placement resize specs"
---

## Title

Placement resize specs: the spec set, the loader rule and live results

## ID

TEST_LOG 6.10.

## Date

2026-10-12.

## Build

Tag: none recorded. Commit: not recorded in this section. The section says the first batch "started
minutes after `cargo build`"; no hash and no `spawn-at --version` output is given.

## Question

Does `spawn-at` place a window correctly when the requested size is smaller than, larger than, or
below the application's own minimum size, for several anchors? The log compared two regimes:
isolated runs and a `--repeat 20` batch.

## Setup and environment

17 specs in the `placement` category, written for `spawn-at-verify`. Every spec states `margin = 16` and
its fixture minimum size (`--min-width`, `--min-height`); the expected rectangle comes from an
independent oracle (`max(requested, minimum)` per axis, the anchor for that final size, far-edge
containment), never from pixel values in the spec. The `[matrix]` mechanism could not express paired
sizes, so each case is one file.

| Anchor | Cases (requested vs fixture minimum) |
| :--- | :--- |
| bottom-right | smaller 200x150 (min 100x100), larger 800x600, minsize-at-min 300x200, below-min-width 200x300, below-min-height 400x150, below-min-both 200x150, far-below-min 50x50 (these four with min 300x200) |
| bottom-left, top-right, bottom, right | smaller 200x150; below-min-both 200x150 (min 300x200) |
| top-left | below-min-both (a control: its position does not depend on size) |
| none | `invalid-size-zero`: an expected failure |

Regime (a), isolated: 20 separate `spawn-at-verify run --glob placement/<id>` invocations per spec,
1.5 s apart, nothing else alive; before each of 820 invocations the driver counted windows whose class
starts with `verify.` and running `fixture` processes. Regime (b): `--repeat 20`, earlier windows
alive until the end.

## Commands run

`spawn-at-verify run --glob placement/<id>` (isolated) and with `--repeat 20`; `spawn-at query
windows --json` and `ps -eo comm=` for the isolation check. Full command lines not recorded.

## Raw result location

Report files of the runs were read from "the 16 stored `report.json` files". The log's own header says
raw results and scripts are not in the repository. Not in the repository.

## Result

| Batch | What | Auto check passes (16 resize specs x 20) | Runs with no `size-changed` before the 30 ms `SIZE_SETTLED` |
| :--- | :--- | :---: | :---: |
| (a1) isolated, first batch | started minutes after `cargo build`, 11 min long | 95 / 320 (29.7%) | 249 |
| (a2) isolated, clean rerun | nothing else running, same specs | 318 / 320 (99.4%) | 2 |
| (b) `--repeat 20` | earlier windows alive | 315 / 320 (98.4%) | 6 |

- Regime (b) had 5 failures, one each on bottom-right smaller, minsize-at-min, below-min-width,
  below-min-height and far-below-min. The log corrects an earlier summary that had said six failing
  bottom-right specs.
- Failure signature, all regimes: the printed rectangle is the fixture's default 400x300 at the
  position for 400x300, `COMMIT_RESOLVED SIZE_SETTLED`, and no `size-changed` sample inside the 30 ms
  window. The log identifies this as the late-resize bug, so those runs fail on both position and
  size.
- **Loader rule added:** a spec whose `[expect.rect]` states neither `margin` nor all four per-side
  margins is a load error naming the spec and file (the oracle assumes margin 0, `spawn-at` defaults to
  16). No existing spec was affected.
- **Harness fix:** the expected-failure spec (`exit_code = 2`) first passed 0 of 20 because attribution
  of a window failed when none was created; the harness now skips attribution when a non-zero exit
  code was expected. After the fix, 20 of 20 in both regimes.
- Isolation check: 0 windows and 0 processes before 819 of the 820 isolated invocations; the one
  exception started right after a direct fixture drive whose last window was still closing, and it
  passed.
- Harness against direct drive on `placement/center-200x150`: `run` was 1.4 to 2.0 ms slower on
  average (10.3 and 9.7 ms against 8.3 ms). `center-1200x800` via `run` passed 20 of 20, mean 28.1 ms,
  the closest approach to the 30 ms limit of any measured group.
- Recorded only, not asserted: the raised flags (`size_raised`, `raised_w`, `raised_h`).

## Verdict

The log gives no single verdict. It reports that there is no single "isolated" rate, because 29.7%
and 99.4% were both measured on different minutes, and that failures are reported as they came, with
no expectation changed.

## Not verified

Why batch (a1) was slow (hypothesis: the Shell or machine was busy after the preceding builds; no load
was recorded). Whether the harness adds 1 to 2 ms. GTK3, Qt and Electron apps. Whether other toolkits
enforce the fixture minimum as Mutter does through size hints. The log lists cases left UNDEFINED
because no spec asserts them (the exit code when the window ends mis-positioned, an oversized request,
the INFO threshold for toolkit minimums, the default margin 16 as documented behaviour, the size the
anchor uses while the app has not answered).

## Sources

- `docs/TEST_LOG.md` section 6.10 (lines 5-74)
