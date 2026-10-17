---
title: "6.24 Texture reader and texture size as a signal"
---

Texture reader live check and texture size as a signal. The corrected reader works, but the texture size is not a usable signal.

| Field | Value |
| :-- | :-- |
| **ID** | TEST_LOG 6.24. Re-runs candidate (iv) of [6.23](/spawn-at/experiments/6-23-post-reinstall-live-check/). |
| **Date** | 2026-10-13. |
| **Build** | `target/debug/spawn-at` built at commit `64faab9`, the commit of the tree. The version string is not recorded. After a reinstall and a new login at 22:05:22 (gnome-shell pid 245567), protocol 6, installed files identical to `assets/gnome/modern`, and the installed `x11_observe.js` containing `readTextureSize`. No code changed. |
| **Verdict** | Not usable. The texture size reaches the target at the first `damaged` in 80 of 80 runs, before the client has painted in 59 of them, so it does not say "the client painted at the target". It is the same signal as the frame size and the first `damaged` of 6.23. The log names as still open whether a cloaked window gets `damaged` for its repaint (answered in [6.26](/spawn-at/experiments/6-26-x11-hold-damage-live/)) and a client-side signal (the X sync counter, later tried in [6.31](/spawn-at/experiments/6-31-sync-counter-feasibility/) onward). |
| **Raw results** | `scratch/p24r/` (gitignored). Not in the repository. |

## What was tested

**Question**

Does the corrected texture reader work, and is "the window texture has the target size" a usable signal
that an XWayland client has painted at the target?

**Setup and environment**

GNOME on Wayland. The fixture only (`--protocol x11` for XWayland); no real X11 or XWayland application.
Phase 2: 10 runs with `--protocol x11` and 10 with `--protocol wayland`, direct `spawn-at spawn --json`
as in 6.23. Phase 3: the 6.23 method, 20 runs per spec idle (40 runs) and the same under 22
`xtask burn-cpu` threads after a 70 s warm-up (load average 19.5, 24.8 at the end); 80 runs in total.
"Texture match" is the first `X11_OBS_SIGNAL` whose `texture=` equals the target size; "final paint" is
the last fixture `FIRST_PAINT` at the target size.

**Commands run**

Direct `spawn-at spawn --json` of the fixture with `SPAWN_AT_FIXTURE_EVENT_DIR` set and
`SetLogging(true)` for the batch; `xtask burn-cpu`. Full command lines not recorded.

## Result

- The reader works: on XWayland, 35 `X11_OBS_SIGNAL` lines in 10 runs all had `texture_via=shaped>multi`
  and no `texture_err`; `texture=(na)` appeared in 0 lines (351 of 351 in 6.23). Native Wayland: 10
  windows found and 0 `X11_OBS` lines.
- A first attempt failed for a reason in the author's shell: all 10 XWayland spawns timed out because
  the shell's `XAUTHORITY` was stale after the new login; the 10 failed rows are not data.
- In all 267 `damaged` lines the texture equals the target size; in all 80 `first-frame` lines it is
  400x300 while the frame is already at the target. The texture reaches the target at the first
  `damaged`, which is the shell-side event of 6.23, and does not depend on when the client paints (idle
  larger: match at 5.8 ms and final paint at 21.6 ms; loaded larger: 25.9 ms and 188.1 ms).

| Cell (20 runs) | Match before / at or after / never the final paint | Gap match minus paint, median |
| :-- | :-: | :-- |
| idle smaller | 2 / 18 / 0 | +1.2 ms |
| idle larger | 20 / 0 / 0 | -15.6 ms |
| loaded smaller | 17 / 3 / 0 | -48.6 ms |
| loaded larger | 20 / 0 / 0 | -171.0 ms |

- The rule "reveal when the texture size equals the target" would have revealed before the client
  painted in 59 of 80 runs (57 clearly early; the 2 idle smaller cases are within stamp lag), by a median
  15.6 ms (idle larger) and 171.0 ms (loaded larger).

## Not verified

Any real X11 or XWayland application, native X11, Qt, GTK and Electron clients (the fixture is a
cooperative winit client that repaints at once); screen capture; the texture size of a client that
resizes its buffer late or never (the `hostile/` flags exist but were not run on `--protocol x11`);
multi-monitor and fractional scaling. The mechanism (the surface the shell reads is resized when the X
window is configured, not when the client commits) is the log's hypothesis, not tested.

## Sources

- `docs/TEST_LOG.md` section 6.24 (lines 1322-1356)
- `docs/TEST_LOG.md` section 6.23, candidate (iv) and its 2026-10-16 note (lines 1300-1303)
