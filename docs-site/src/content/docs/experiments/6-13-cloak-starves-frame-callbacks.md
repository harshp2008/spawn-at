---
title: "6.13 Does the cloak starve frame callbacks"
---

## Title

H4: does the cloak starve the app of frame callbacks?

## ID

TEST_LOG 6.13. Follows [6.12](/spawn-at/experiments/6-12-why-revealed-at-pre-resize-size/).

## Date

2026-10-12.

## Build

Not recorded as one hash for the whole section. The log names the instrumentation commit `fb4e082`
(the fixture trace directory). It says no extension change was made, and that after new commits
`spawn-at` must be rebuilt or the verify preflight reports a stale binary. The version string is not
recorded.

## Question

Hypothesis H4: the app applies our resize only when it gets a `wl_surface.frame` callback; the
compositor does not paint a cloaked (opacity 0) actor, so callbacks stop, and the app resizes only
after the reveal.

## Setup and environment

GNOME 46 on Wayland, mutter 46.2. The environment variable `SPAWN_AT_FIXTURE_TRACE_DIR=<dir>` makes the
fixture write `fixture-<pid>.log`: its own `Resized`, `RedrawRequested` and commit lines in
`CLOCK_MONOTONIC` nanoseconds, plus the libwayland client dump (`WAYLAND_DEBUG=client`). The dump's
stamp is `CLOCK_REALTIME` microseconds modulo 2^32, not the monotonic clock, so it was mapped using
the fixture's own `COMMIT_DONE` lines. The extension's clock was tied to `CLOCK_MONOTONIC` with 16
`SetLogging` echoes per batch.

Data: the stored runs of 6.11.1 and the sampler runs of 6.12 (post-reveal corrected runs only); the
fixture through the normal `run` path (`bottom-right-smaller` and `bottom-smaller`, 20 each,
interleaved, back-to-back, idle with 30 s gaps, and under 22 `burn-cpu` threads after a 60 s warm-up;
120 runs); toolkit apps with `WAYLAND_DEBUG` (`gnome-text-editor` GTK4, idle, 12 runs alternating 500x300
and 580x380; Firefox (snap), 6 runs at 300x200; 14 editor runs under 22 threads); and a reading of
mutter 46.2 source through a fetch tool, whose summaries the log says are indirect.

## Commands run

Exact command lines not recorded. By the log: runs through `spawn-at-verify run`, `WAYLAND_DEBUG=client`
on the real apps, and `cargo build -p spawn-at` after commits.

## Raw result location

Not in the repository: the log's header states raw results and scripts are not in the repository.

## Result

- The fixture never asks for a frame callback (winit 0.29 with softbuffer): 0 `wl_surface.frame` lines in
  the 120 dumps, so H4 cannot apply to it. It applies the configure within 0.3 ms (median) of
  receiving it.
- Under 22 threads, the configure reached the client at a median of 23.1 ms after the size request
  (p95 59.1, max 120.4), against 7.9 to 8.5 ms back-to-back and idle. The 14 stale runs had the
  configure at a median 41.7 ms, against 19.3 ms for the 26 that settled correctly. Stage (c), the
  commit, adds to it under load.
- `gnome-text-editor` (GTK4): the resized commit followed the configure by 3 to 6 ms and preceded the
  next frame callback `done`, so GTK4 did not wait for a callback to commit the resize.
- Callbacks while cloaked: the first `done` after the request was emitted 5.4 to 7.5 ms after it, with
  the window cloaked. A callback requested at the resized commit was not answered until 46 to 50 ms,
  0.6 to 1.0 ms before the reveal, in the stage update that the extension's one-frame wait forces. The
  log notes this is the part of H4's premise that holds, and it did not delay the resize.
- Firefox (snap): Mutter stamped its frame-callback `done` events 6.6 to 17.7 ms after the request, but
  the client printed them at 108 to 133 ms together with the configure; its socket was not read for
  about 190 ms. The 135 to 312 ms to correction is the browser's startup, not the compositor
  withholding callbacks.
- Source reading (mutter 46.2, indirect): frame callbacks are sent from the stage's `after-update`
  signal with no opacity test; nothing forces an update for a cloaked window between its first frame
  and the extension's pre-reveal wait.
- Expected residual under a 22-thread load: about 25% of runs corrected after the reveal in the `run`
  path (10 of 40).

## Verdict

"H4 is not supported." Two apps whose resize path is known show no gating by frame callbacks; Mutter
delivers callbacks to a cloaked surface; the delay sits in the configure's delivery and the app's own
CPU time. The log's recommendation: no cloak change; keep the late-resize check and the post-reveal
anchor correction; if the flash must shrink, a bounded grace after `SIZE_SETTLED` with a learned
minimum per app.

## Not verified

No extension or product change was made. Chrome, Nautilus and terminals were not traced at the
protocol level. The idle difference between the sampler harness of 6.12 and the `run` path is open.
Whether Mutter defers the configure send to the frame clock was not established (configure arrival at
idle is 6 to 14 ms after the call, never earlier than 6 ms). Parts of the mutter source were not read
(the body of `meta_surface_actor_wayland_is_view_primary`, Clutter's paint path for opacity 0).

## Sources

- `docs/TEST_LOG.md` section 6.13 (lines 369-441)
