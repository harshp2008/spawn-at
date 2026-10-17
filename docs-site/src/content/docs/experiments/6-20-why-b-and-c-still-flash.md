---
title: "6.20 Why arms B and C still flash under load"
---

## Title

Why arms B and C still flash under load: classification and a protocol-level re-run (no code change)

## ID

TEST_LOG 6.20. Follows [6.19](/spawn-at/experiments/6-19-ready-wait-abc/); the next step is
[6.21](/spawn-at/experiments/6-21-default-cap-250-wire-paint-time/).

## Date

2026-10-13.

## Build

The user's dataset was recorded with `binary_commit` `0f90bc1` (not dirty). The commit of the traced
re-run's binary is not recorded in this section. The version string is not recorded. Nothing in the
product, the extension, the fixture or the harness was changed; the only file written in the repository
was the log section. Tests were not run.

## Question

Why do B (cap 150) and C (cap 250) still flash in 21 loaded runs, and is that the mechanism of A's
shrink flashes? The observation to test, given by the user: the remaining flashes do not look like
"the app answered late".

## Setup and environment

GNOME on Wayland, 22 CPUs. Two data sources. (1) The user's dataset, 150 run JSONs (`br/`:
`bottom-right-smaller` and `-larger`, 20 per arm per spec; `tl/`: `top-left-smaller`, 10 per arm), under
22 `burn-cpu` threads, mean load at run start A 22.62, B 22.63, C 22.62. Flashed runs: A 31, B 13, C 8.
(2) Two live batches the log's author ran with `SPAWN_AT_FIXTURE_TRACE_DIR` set, so that the fixture
writes its Wayland protocol dump: loaded 80 runs (arms B and C; 72 `reached` and 8 `timed-out`) and idle
40 runs. The dump perturbs timing, but the flash rate of the `reached` runs was the same as in the
untraced data (12 of 72 = 16.7% against 16 of 94 = 17.0%). The clocks were aligned with 120 bracketed
`SetLogging` calls to an interval 6.02 ms wide (about +/-3 ms).

## Commands run

The harness `run` with the fixture trace variable set; a throwaway Rust `CLOCK_MONOTONIC` reader in
`scratch/`. Full command lines not recorded.

## Raw result location

Not in the repository: the throwaway scripts (`lf_rows.jq`, `lf_abs.jq`, `lf_trace.awk`,
`lf_client.awk`, `lf_join.sh`, `lf_timeline.sh`, `lf_calib.sh`, `lf_trace_loaded.sh`, `mono.rs`) are not
committed; the raw traced runs and the dumps stayed in the session scratchpad.

## Result

All 32 classified runs (21 B and C, 11 A shrink) were native Wayland, and the ready-wait engaged in all 100 B
and C runs.

| class | meaning | A | B | C |
| :--- | :--- | ---: | ---: | ---: |
| a | `reached`, frame at target, yet the harness saw a first paint after the reveal | 0 | 8 | 8 |
| b | ended at the cap | 0 | 5 | 0 |
| c | position only | 0 | 0 | 0 |
| d1 | legacy rule resolved on the old 400x300 frame | 10 | n/a | n/a |
| d2 | legacy rule reached the target | 1 | n/a | n/a |

- **Class a, from the extension log:** `READY_WAIT_RESOLVED outcome=reached` came from a `size-changed`
  sample at the target, the anchor, the frame at `UNCLOAK_4C` and the frame at the reveal were all the
  target size, there was no `ANCHOR_CORRECTION`, and the printed rectangle was the target. The only
  event after the reveal is the fixture's `FIRST_PAINT` at the target size, 10.7 to 27.7 ms after it.
- **Protocol-level re-run:** in the traced loaded batch the extension's `size-changed` follows the
  client's commit of the target-size buffer by about 3 ms, and in all 72 `reached` runs the commit came
  at least 3.3 ms before the reveal (median 12.2 ms before). Every flashed `reached` run is one where
  the fixture's `FIRST_PAINT` stamp was written 13.6 to 23.4 ms after the commit it describes; at idle
  the stamp is never late (0 in 40 of 40). The stamp is written after `buffer.present()` returns.
- **Class b (B timeouts, 5 of 21):** the app really was late for a cap of 150 ms: on this loaded debug
  fixture the configure arrives about 94 ms (median) after the request and drawing takes 49 ms at
  800x600, so the commit lands near 145 ms. 4 of the 8 traced timeouts flashed, and 3 of those 4
  committed 35 to 73 ms after the reveal.
- **A's shrink flashes (11):** 10 of 11 are the legacy quiet rule resolving on the old 400x300 frame,
  bottom-right anchored on the stale frame and corrected 6 to 178 ms later: "the app answered late" in
  the plain sense, which the ready-wait removes.
- One traced C run had its cap timer fire 450 ms late (`elapsed_ms` 700.8 against `waited_ms` 250).

## Verdict

The user's observation is confirmed for the 16 `reached` flashes of B and C: they are not "the app
answered late"; in the traced re-run the content was committed before the reveal and the flash is the
fixture's late `FIRST_PAINT` stamp. It does not hold for the 5 B timeouts. Proposals, not implemented:
for class a, take the `FIRST_PAINT` time before `present()` in the harness and fixture (no product
change); for class b, raise the default cap from 150 to 250 as 6.19 recommended.

## Not verified

That the committed buffer was on screen in the first frame after the reveal (no screen capture exists).
The cause of the stamp lag (hypothesis: the fixture thread waits for a CPU behind the burn threads after
the flush; not tested). The 16 untraced `reached` runs individually. Hung or refusing apps, the 1500 ms
hard limit, X11 and XWayland, a release-build fixture, and real toolkit apps under this analysis.

## Sources

- `docs/TEST_LOG.md` section 6.20 (lines 925-1059)
