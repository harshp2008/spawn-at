---
title: "6.22 Hostile apps"
---

## Title

Hostile apps: fixture flags, nine catalog cells and a live A/C run

## ID

TEST_LOG 6.22. Follows [6.21](/spawn-at/experiments/6-21-default-cap-250-wire-paint-time/).

## Date

2026-10-13.

## Build

Commit `70dc120` (code, tests and cells), plus a follow-up fixture fix and the log section (hashes of
the fix not recorded). The version string is not recorded. Fixture, harness and catalog only; the
product (CLI, core, extension) was not changed. The installed extension was the previous build
(ready-wait present, default cap 150, hard limit 1500) and was not reinstalled; the ready-wait arm
passes `--wait-cap-ms 250` explicitly, and the extension log shows `cap=250` in every C run.

## Question

How do the legacy rule and the ready-wait behave with an app that answers slowly, refuses or clamps the
size, snaps to a grid, is not resizable, stalls, or keeps changing its size? The expectations of the
nine cells were written from the design and the extension source before the first live run.

## Setup and environment

New fixture flags (harness only, never in the product binary): `--resize-delay-ms N`, `--min-size WxH`
and `--max-size WxH`, `--fixed-size`, `--resize-increment WxH` (emulated as a terminal does: the app
rounds its size down to the grid), `--stall-after-map-ms N` (under 4000, so GNOME's 5 s not-responding
dialog is not reached) and `--flap-ms T`. A first version of grid-snap and flap painted a smaller buffer
only and did not work, because `winit` sets `xdg_surface.set_window_geometry` to the configured size on
every configure; the fixture now also calls `request_inner_size`.

Nine specs in `verify/hostile/` (the catalog went from 36 to 45), with a new `[expect.resize]` section.
Live run: one login, idle, bottom-right with a 16 px margin, 10 runs per cell per arm, arm order rotated,
190 runs. A is `--wait-strategy legacy`; C is `--wait-strategy ready-wait --wait-cap-ms 250`; D is
ready-wait with `--wait-cap-ms 1500` on `hung` only, the log author's addition, so that the 1500 ms hard
limit could run live for the first time.

## Commands run

`spawn-at-verify run` over the nine cells, with `ab_batch.sh` rotating the arms; four direct
`spawn-at spawn` probes for the event check. Full command lines not recorded.

## Raw result location

Not in the repository: throwaway scripts (`h22_batch.sh`, `h22_rows.jq`, `h22_rows.sh`) in `scratch/`;
raw JSON stayed in the session scratchpad.

## Result

All 190 runs were valid. Arm C against the declared outcome: slow-below-cap, slow-above-cap,
refuse-grow, grid-snap, non-resizable and `hung` matched in 60 of 60 runs. Mismatches:

| Cell | Declared | Seen | Cause as recorded |
| :--- | :--- | :--- | :--- |
| clamp-to-min C (10 of 10) | Reached, 300x200 | Adjusted/quiet, 300x200, at 39 ms | The extension logs `reportedMinSize=(0x0)` in all runs and the `Meta-14.typelib` of GNOME 46 has no min-size accessor, so the clamp event cannot fire and the machine waits for the unraised 200x150. Benign here. Product finding, not fixed. |
| default-equals-min C (10 of 10) | Reached at once | TimedOut/cap at 250 ms | Same cause. An app at its minimum never answers, so the cloak was 267.8 ms against 35.4 ms for legacy: +232 ms. The design's first-spawn cost risk happening live. Not fixed. |
| restless C (1 of 10) | TimedOut/hard | Adjusted/quiet at 322 ms | A fixture jitter: one flap tick gap exceeded the 30 ms quiet window; the legacy arm had the same once. |

- slow-below-cap: legacy revealed at 400x300 after a 32 ms quiet window and the app painted 71 ms
  later (flash 10 of 10); the ready-wait waited 119 ms and flashed 0 of 10, at +86 ms of cloak.
- slow-above-cap: both flash 10 of 10; the ready-wait reveals 151 ms before the app paints instead of
  375 ms.
- **The hard limit, live for the first time:** `hung` with cap 1500 (D) and `restless` with cap 250 (C)
  ended in `TimedOut` with `waited_ms` exactly 1500 and `reason=hard`; the window was revealed 1503 to
  1534 ms after the request, at the old or flapping size, never at 800x600. An `UNCLOAK` is in 190 of
  190 runs, the longest cloak was 1534.6 ms against the 6000 ms watchdog, and every run exited 0.
- `hung` in A and C: the harness did not see the late resize, because the app wakes about 2.5 s after it
  maps, outside the 1500 ms post-reveal watch, so "flash 0 of 10" there is a limit of the watch, not a
  clean result.
- `non-resizable`: the ready-wait is faster (18.9 ms against 49.7 ms cloak median).
- Tests: fixture binary 20 (was 10); `spawn-at-verify` lib +6; `category_library_test` +1.

## Verdict

No single verdict. The cells behave as declared except the two findings above and one fixture jitter.
The two product findings are listed by the log as open: the missing min-size accessor on GNOME 46
(which the knowledge base and [6.14](/spawn-at/experiments/6-14-mutter-api-and-arrival-wait/) also
note) and the +232 ms cost for an app at its minimum.

## Not verified

Load (everything here is idle); a release-build fixture; real hostile apps and any app that ignores a
configure without telling the compositor; whether GNOME shows a not-responding dialog for the 2500 ms
stall (no screen capture); on-screen visibility of any flash; X11 and XWayland (every cell requires
Wayland); cap sweeps beyond 250 (and 1500 for `hung`); multi-monitor and fractional scaling; the cause
of the one restless jitter.

## Sources

- `docs/TEST_LOG.md` section 6.22 (lines 1149-1230)
