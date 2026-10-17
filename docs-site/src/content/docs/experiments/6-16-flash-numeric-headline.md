---
title: "6.16 Flash: numeric headline and visibility label"
---

## Title

Flash: numeric headline, per-screen visibility label (harness only)

## ID

TEST_LOG 6.16. Follows [6.15](/spawn-at/experiments/6-15-flash-aware-verdict/).

## Date

2026-10-13.

## Build

Harness commit `1de817f`. Debug `spawn-at` and fixture. The version string is not recorded. No
extension change. The only things read from the system were `xrandr --query` and one read-only
`GetCurrentState` call to check its output format.

## Question

6.15 counted every late paint, including sub-frame ones (3 to 27 ms), as a flash. Observed by eye,
about 70% of "PASS (flash)" runs show a flash. Can the verdict be split into a numeric flash (lateness,
the same on every screen) and a per-screen visibility label?

## Setup and environment

A change to `spawn-at-verify`. A numeric flash is a reveal before the final paint (or an
`ANCHOR_CORRECTION`, or a different final position) by a lead strictly above `max(measured clock
uncertainty, --flash-threshold-ms)`. The visibility label per event: `frame_ms = 1000 / refresh_hz`;
`certain` when `lead_ms >= frame_ms`, `possible` when shorter; `visible_probability = min(1, lead_ms /
frame_ms)`, and for a run `1 - prod(1 - p)`. The probability treats the reveal as landing at a random
point of the frame period, which the log marks a hypothesis. The refresh rate is read from `xrandr
--query` on X11 and Mutter's `DisplayConfig.GetCurrentState` over `gdbus` on Wayland, otherwise 60 Hz
with a note.

Validation: an X11 session (`XDG_SESSION_TYPE=x11`), GNOME Shell 46.0, 22 CPUs, 60.00 Hz, 1-minute load
average 2.1 to 2.6; 20 isolated `run --glob placement/bottom-right-larger` invocations, 1.5 s apart.

## Commands run

`spawn-at-verify run --glob placement/bottom-right-larger` (20 times, 1.5 s apart), `xrandr --query`,
and `gdbus` for `GetCurrentState`. Full command lines not recorded.

## Raw result location

Not in the repository (the log's header says raw results and scripts are not).

## Result

| | runs |
| :--- | ---: |
| numeric flashes (headline) | 19 of 20 (run 18: lead -0.2 ms, inside the uncertainty, not counted) |
| certain (lead >= 16.7 ms) | 0 |
| possible (lead < 16.7 ms) | 19 |
| leads, ms (min / median / max) | 3.1 / 10.8 / 15.1 |
| old warnings (stale print, post-reveal correction) | 0 of 20 |
| mean `visible_probability`, flashing runs / all 20 runs | 0.575 / 0.546 |

Tests: 352 passed and 4 ignored with `tui,fixture` (`flash_test` 20, +4), covering the frame-period
boundaries, the clock uncertainty, the threshold floor, 60 Hz against 144 Hz, combining events, and
`--strict-flash`.

## Verdict

The headline stays numeric; visibility is a label. Against the observation of about 70%, the model
expects 55% to 57% of these runs to show the flash: the same range, not the same number. The log says
the 70% is "an impression by eye, not a measurement", that the model's own spread at 19 runs is about
10 points, and that the probability model is a hypothesis. Two readings are open and not separated:
the real exposure is longer than the lead, or the user counts a milder artefact as a flash.

## Not verified

Wayland and a refresh rate other than 60 Hz on a live screen (the 144 Hz case is a synthetic test);
real apps; any screen capture, which alone would show which reading is right.

## Sources

- `docs/TEST_LOG.md` section 6.16 (lines 602-630)
