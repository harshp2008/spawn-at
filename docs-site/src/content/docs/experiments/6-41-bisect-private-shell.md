---
title: "6.41 Bisect in a private Shell: no crash reproduced"
---

The configurations that could have caused the crashes in [6.38](/spawn-at/experiments/6-38-shell-crash-live-xwayland-editor/) were run in the private Shell of [6.39](/spawn-at/experiments/6-39-private-headless-shell/). **The crash was not reproduced: 0 crashes in 69 runs, 27 of them the exact crashing configuration** (the editor forced onto X11, launched by spawn-at, window focused, graceful close). The private Shell is **headless** (software rendering, one virtual monitor, no real input), so this does not clear the live session.

| Field | Value |
| :-- | :-- |
| **ID** | X11-FLASH-02f step 3. Follows [6.40](/spawn-at/experiments/6-40-startup-id-audit/). |
| **Date** | 2026-10-29, 13:43 to 13:58 (+05:30). |
| **Build** | GNOME Shell 46.0 headless, Mutter 46.2, spawn-at 0.2.4. Tree: branch `exp/x11-flash-02`. |
| **Verdict** | Not decidable. Nothing crashed, so no hypothesis was confirmed or contradicted by a crash. |
| **Raw results** | In the repository (Evidence below). |

## At a glance

| Cell | Runs | Crashed | Timestamp-0 warning | Focus at close |
| :-- | --: | --: | --: | :-- |
| 1 xterm, spawn-at, observer | 9 | 0 | 0 | 3 no, 6 yes |
| 2 editor, native Wayland, plain launch | 6 | 0 | 0 | 3 no, 3 yes |
| 3 editor, native Wayland, spawn-at | 6 | 0 | 0 | 3 no, 3 yes |
| 4 editor, forced X11, plain launch | 6 | 0 | 0 | 3 no, 3 yes |
| 5 editor, forced X11, spawn-at | 9 | 0 | 0 | 3 no, 6 yes |
| 6 as 5 without the startup variables | 6 | 0 | 0 | 3 no, 3 yes |
| 7 as 5 with a 32-bit-safe `_TIME` | 6 | 0 | 0 | 3 no, 3 yes |
| 8 as 5 with the observer | 6 | 0 | 0 | 3 no, 3 yes |
| 9 eog, forced X11, spawn-at | 9 | 0 | 0 | 3 no, 6 yes |
| 10a as 5, close by WM_DELETE_WINDOW | 3 | 0 | 0 | yes (fell back, see below) |
| 10b as 5, close by a synthetic key | 3 | 0 | 0 | yes (fell back, see below) |

## What was tested

**Question**

Which of H1 to H6 (and the startup-id idea) makes a focused GTK app on XWayland crash the Shell on close?

**Setup and environment**

Each cell ran 3 times in a fresh private Shell. Rounds: A no focus request (27 runs); B focus requested with the extension (27); C focus requested and a native Wayland eog window kept open, so that closing the test window hands focus to another window as it did live (9); D close variants (6). A headless Shell gives new windows no focus, which round A shows; rounds B to D ask for it. Windows were spawned with `--no-focus` as in the live batches.

**Commands run**

`bisect-cells.sh OUTDIR CELL...` with `XFLAGS=--focus` or `--focus --bg-window`; each cell calls `xflash cell` through the wrapper. Exact lines are in the tools directory.

## Result

**Measured.** 69 runs, no Shell crash, no "timestamp of 0" line, no `_NET_ACTIVE_WINDOW` line. The only Shell warning in any run was "Tried to ping window ... with a bad serial", in every run, for X11 and for native Wayland windows alike, so it is background noise here. In cell 6 the app's environment lacked the two variables (checked in `/proc`); in cells 5, 7 and 8 they were set.

**Close variants.** The WM_DELETE_WINDOW message and the synthetic key did not close the window within 6 s; both fell back to the graceful `spawn-at close`, which is recorded. So those two methods were not really exercised.

**A focused close is not enough.** All 80 live xterm closes were focused (`focused_at_close` true in every record) and none crashed the Shell; the focused editor closes here did not either. The H6 verdict is **contradicted as sufficient**.

| Hypothesis | Verdict |
| :-- | :-- |
| H1 gnome-text-editor | not decidable here; it never crashed |
| H2 any GTK app forced onto X11 | not decidable |
| H3 the observer | no effect seen (cell 8 equals cell 5); not decidable |
| H4 the extension's legacy path | no effect seen; not decidable |
| H5 live-session ingredients or an aged Shell | the one left standing, by elimination; low to medium confidence |
| H6 closing a focused XWayland window | contradicted as sufficient; medium |
| Startup-id idea | not supported as the whole story; medium |

## Not verified

- The crash itself. The live session had a 9-day-old Shell with degraded warnings, portals, settings daemons, real input and other applications; the private one has none of these.
- Rates: 3 runs per cell is a smoke-level sample. Cells 10a and 10b were not a real test of other close methods.
- Next cells, not run: add live-session ingredients one at a time (portal, xsettings, input), age the Shell with many windows, use an Electron window as the background Wayland window, and repeat cell 5 with 20 runs.

## Sources

- [Bisect record](https://github.com/harshp2008/spawn-at/blob/main/docs/evidence/x11-flash-02/bisect-2026-10-29.md)
- [Raw per-run data](https://github.com/harshp2008/spawn-at/blob/main/docs/evidence/x11-flash-02/data/bisect/) (`cells.jsonl` and the log of each round)
- [The 80 live xterm run records](https://github.com/harshp2008/spawn-at/blob/main/docs/evidence/x11-flash-02/data/xterm-live/)
- [The tools](https://github.com/harshp2008/spawn-at/blob/main/docs/evidence/x11-flash-02/tools/) (`bisect-cells.sh`, `src/cell.rs`)
- The links point into the repository on GitHub and resolve once the branch is merged.
