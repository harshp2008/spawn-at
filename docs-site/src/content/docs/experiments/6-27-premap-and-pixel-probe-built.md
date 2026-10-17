---
title: "6.27 Pre-map resize and pixel probe: timeline, built"
---

X11 pre-map resize and pixel probe, stage 1: a timeline from existing data, and two experiments built but not run. A timeline was built from existing data and two experiments were built; nothing new was run live.

| Field | Value |
| :-- | :-- |
| **ID** | TEST_LOG 6.27. The live runs are [6.28](/spawn-at/experiments/6-28-premap-and-pixel-probe-live/) and [6.29](/spawn-at/experiments/6-29-pixel-probe-retry-shell-crash/). |
| **Date** | 2026-10-14. |
| **Build** | Tag: none recorded. Tree: branch `refactor/v0.2.0-beta.1`, HEAD `2916599`, tree clean, extension ACTIVE, installed files identical to `assets/gnome/modern`. The output of `spawn-at --version` is not recorded for this section. |
| **Verdict** | No verdict on the two experiments: nothing new was run live. The timeline shows that a resize at `window-created` would come before the fixture's first paint by only about 7 ms when idle, and that a resize shortly before the first paint is not enough (the app must have the configure before it first draws). The log's live-stage plan, which [6.28](/spawn-at/experiments/6-28-premap-and-pixel-probe-live/) carried out in part, was a smoke of each value, an A/B for pre-map, and a pixel-probe batch. |
| **Raw results** | `scratch/p28r/` (gitignored). The data of phase 1 are in the earlier scratch directories `p23r`, `p24r` and `p27r` (gitignored). None is in the repository. |

## What was tested

**Question**

Phase 1: is the resize issued before or after the app's first paint and its `first-frame` event, what
size is the app's first paint, and what is the earliest hook that has the window's identity? Phase 2:
can a resize before the window is mapped (`x11-premap`), or a read of pixels from the window texture
(`x11-pixel-probe`), tell that an X11 client has painted at its new size?

**Setup and environment**

Phase 1 used no live run. It joined the extension's log (`WINDOW_CREATED`, `ACTOR_MAP`,
`X11_OBS_MARK set_size`, first-frame, reveal) to the fixture's own events on `CLOCK_MONOTONIC` for 240
XWayland fixture runs taken from [6.23](/spawn-at/experiments/6-23-post-reinstall-live-check/),
[6.24](/spawn-at/experiments/6-24-texture-reader/) and the legacy arm of [6.26](/spawn-at/experiments/6-26-x11-hold-damage-live/) (40 runs per dataset,
idle and loaded). S is the resize call.

Phase 2 was build-only: node and Rust tests, nothing installed, nothing run in a Shell. Fixture only
(`--protocol x11`).

**Commands run**

Exact command lines not recorded. The log gives the test counts after the build and says
`cargo fmt --check` and `clippy --workspace --all-targets -D warnings` were clean. The probe's
accessors were introspected with `gjs` outside the Shell.

## Result

Phase 1 (240 runs, no live runs):

- The resize comes after the app's first paint in all 120 idle runs and in 49 of 120 loaded runs, and
  before `first-frame` in 240 of 240. The resize is issued from the batch, dispatched at the actor's
  `map`, about 21 ms (idle) and 37 to 41 ms (loaded) after `window-created`.
- `first-frame` is emitted after the map, so always after the resize call (minimum +1.1 ms). Idle it
  came after the first paint in 120 of 120; under load it came before the first paint in 65 of 120
  runs, so it is not a paint signal.
- The app's first paint was 400x300 in all 120 idle runs; under load 94 of 120 were 400x300 and 26 of
  120 were already at the target size. When the resize came before the first paint (71 loaded runs) the
  paint was at the target size in 26 and still 400x300 in 45, the stale paint arriving 0.9 to 221.6 ms
  (median 19.2 ms) after the resize call.
- The earliest hook is `window-created`, where the pid and class are available for the fixture's X11
  windows; only the dispatch waits for `map`. A resize there would precede the fixture's first paint
  by about 7 ms idle.

Phase 2, built but not run:

- Fixture flag `--state-colors` (off by default): every buffer is one solid opaque colour chosen from
  its size, magenta `0xFFFF00FF` at the initial size and green `0xFF00FF00` at any other size.
- `x11-premap` (hidden `--wait-strategy` value, refused by the config file): resizes the still-cloaked
  window at `window-created` (`PREMAP_RESIZE`), skips the batch's own resize (`SET_SIZE_PREMAPPED`) and
  waits for the first `first-frame` after the resize, bounded by `cap_ms` (default 250) and the 1500 ms
  hard limit.
- `x11-pixel-probe` (hidden value, observe-only): at each `damaged` and `first-frame` it queues a timer
  and samples four 2x2 clips of the window texture with `WindowActor.get_image(clip)`, writing a PNG
  and decoding it with `GdkPixbuf`; at most 16 probes per window, at least 8 ms apart.
- Tests after: node 213 (was 188), `spawn-at` 207, `spawn-at-core` 67, `xtask` 10, `spawn-at-verify`
  `--features tui,fixture` 407 passed and 4 ignored.

## Not verified

Whether a plain `{x, y, width, height}` object is accepted as the `Mtk.Rectangle` clip, whether
`get_image` on a cloaked actor returns its texture, and whether the PNG round trip is cheap enough
(all hypotheses in the log). A native crash in Mutter from an early `move_resize_frame` or from
`get_image` (the log: "NOT covered", hypothesis that neither crashes). X11 apps that set `WM_CLASS`
late. The log notes the timestamps of the X window's creation inside the app and of the app's receipt
of the configure were not logged.

## Sources

- `docs/TEST_LOG.md` section 6.27 (lines 1461-1498)
- [X11 and XWayland](/spawn-at/environments/x11-xwayland/)
