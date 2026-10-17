---
title: "6.32 Sync-capable GTK4 client, legacy rule"
---

A sync-capable X11 client against the winit fixture, under the legacy rule. A sync-capable client flashes as much as the fixture under load, and Mutter does not visibly wait for it.

| Field | Value |
| :-- | :-- |
| **ID** | TEST_LOG 6.32. Between [6.31](/spawn-at/experiments/6-31-sync-counter-feasibility/) and [6.33](/spawn-at/experiments/6-33-sync-counter-moves-on-repaint/). |
| **Date** | 2026-10-14. |
| **Build** | Tag: none recorded. Tree: HEAD `a2f5e78` at the start. Stage A changed no extension and installed nothing; the extension in the session was the one installed after the 6.30 regression check. The output of `spawn-at --version` is not recorded for this section. Session: GNOME on Wayland, no gnome-shell or mutter assertion, crash signal, "uncatchable exception" or critical line in the journal at the start, after the smoke, after the idle batch and after the loaded batch. |
| **Verdict** | A sync-capable GTK4 client under the legacy rule flashes as much as the fixture when loaded, and Mutter does not visibly wait for it. The fixture does not overstate the legacy rule's flashing relative to this client. The log limits this to one toolkit, one machine and a software-drawn solid colour. |
| **Raw results** | `scratch/p33r/` (gitignored). Not in the repository. |

## What was tested

**Question**

Under the legacy rule, does a client that takes part in X sync flash less than the winit fixture,
which does not? Does Mutter appear to wait for such a client?

**Setup and environment**

The client is a throwaway program, not a real X11 application: a PyGObject GTK4 app (Gtk 4.14,
`GDK_BACKEND=x11`) with the fixture's interface (`--app-id`, `--title`, `--width/--height` 400x300,
`--exit-after-ms`, `--paint-delay-ms`, `--state-colors`), undecorated, one `DrawingArea` filled with
one solid colour. It writes the fixture's event format (`FIRST_PAINT WxH` and `RESIZED WxH`) to
`fixture-<pid>.events` in `SPAWN_AT_FIXTURE_EVENT_DIR`. Its stamp is taken in the draw callback,
before GTK hands the frame to the display, while the fixture's is taken after its buffer present, so
the two "final paint" stamps are not the same moment. `xprop` on its window: `WM_PROTOCOLS` includes
`_NET_WM_SYNC_REQUEST`, and `_NET_WM_SYNC_REQUEST_COUNTER = 16777222, 16777223` (two counters, GTK4's
extended sync). The script `scratch/p33_syncclient.py` is not in the repository.

20 runs per spec per cell: 80 client runs, all exit 0, all windows found. The fixture numbers are the
legacy arm A of [6.26](/spawn-at/experiments/6-26-x11-hold-damage-live/), measured in an earlier
login. Specs `bottom-right-smaller` (to 200x150) and `bottom-right-larger` (to 800x600), idle and
under load. Flash is a reveal before the client's last `FIRST_PAINT` at the target size; "clearly
early" is more than 19 ms before it.

Stage B (observe-only `thawed` logging) was built and tested with node tests only; nothing was
installed. Stage C was a plan.

**Commands run**

Exact command line not recorded in full. The log gives the spawn method: `spawn-at spawn --json
--class <app-id> --anchor bottom-right --size W H --margin 16 -- env GDK_BACKEND=x11 python3
p33_syncclient.py ...` with no strategy flag, `SetLogging(true)` for the batch only. The XWayland
window took the legacy rule (`READY_WAIT_START strategy=legacy kind=XWayland
why=not-native-wayland`).

## Result

| Cell | Client | Flashes (clearly early) | Reveal minus final paint, median, ms | First paint at target size |
| :-- | :-- | :-: | :-- | :-: |
| idle smaller | GTK4 | 20 (0) | -5.0 | 0 of 20 |
| idle smaller | fixture | 0 (0) | +26.1 | 0 of 20 |
| idle larger | GTK4 | 19 (0) | -1.3 | 0 of 20 |
| idle larger | fixture | 3 (0) | +6.9 | 0 of 20 |
| loaded smaller | GTK4 | 19 (16) | -47.0 | 5 of 20 |
| loaded smaller | fixture | 17 (16) | -41.7 | 6 of 20 |
| loaded larger | GTK4 | 20 (17) | -56.0 | 6 of 20 |
| loaded larger | fixture | 20 (20) | -147.9 | 3 of 20 |

- Loaded, the GTK4 client flashed in 39 of 40 runs against 37 of 40 for the fixture. Idle it was
  flagged in 39 of 40 runs against 3 of 40, but the leads are small (median -5.0 and -1.3 ms, worst
  -16.8 ms, none beyond 19 ms), which the log reads as "reveal within about one frame of the draw",
  not large early reveals.
- Mutter showed no sign of waiting for the client: the reveal came 11.5 to 13.8 ms after the resize
  idle (the fixture: about 30 ms).
- First paint at the target size was no better: 11 of 40 loaded (fixture 9 of 40).
- Stage B code (not installed): in `x11_observe.js` only, the window actor's `thawed` signal logged as
  an ordinary `X11_OBS_SIGNAL` line. Node tests 176 to 183; `spawn-at` 205, `spawn-at-core` 64,
  `xtask` 10, `spawn-at-verify` (`tui,fixture`) 404 passed and 4 ignored.

## Not verified

Whether `thawed` fires for XWayland windows at all; any client other than this GTK4 one (Chromium and
Electron were not tried); whether the idle "flashes" of the GTK4 client are visible (no screen
capture); the difference between the two stamp definitions. The fixture numbers come from a different
login than the GTK4 numbers.

## Sources

- `docs/TEST_LOG.md` section 6.32 (lines 1632-1666)
- [X11 and XWayland](/spawn-at/environments/x11-xwayland/)
