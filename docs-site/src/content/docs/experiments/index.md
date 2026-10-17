---
title: "Experiments"
---

One record for each experiment the project ran, written from `docs/TEST_LOG.md`. The log itself is a
long dated notebook that is not published here: it names the owner and local paths, and its raw
results live in directories under `scratch/` that git ignores. These pages are written, not copied.

Each record has the same headings: Title, ID, Date, Build, Question, Setup and environment, Commands
run, Raw result location, Result, Verdict, Not verified, Sources. A field the log does not record is
written "not recorded". Where the log gives the commands only in prose the page says "exact command
line not recorded" and keeps the prose. Nothing was re-run to write these pages: every number comes
from the log.

**Build.** The log names the tree it ran on as a branch and a `HEAD` hash, not as a version string.
The output of `spawn-at --version` is recorded once, in section 6.30 (a regression check, not an
experiment), as `DEV BUILD` with the previous commit `acccca8`. The records say which hash the log
gives and do not infer a version from it.

**Raw results.** Every raw result directory is under `scratch/`, which is gitignored: none is in the
repository. The throwaway clients, watchers and shim used in the later X11 experiments are not in the
repository either.

## X11 and XWayland investigation (2026-10-14)

The question was how to tell that an X11 client has painted at its new size. The summary table is on
[X11 and XWayland](/spawn-at/environments/x11-xwayland/#what-was-tried-to-make-x11-flicker-free).

| ID | Experiment | Verdict |
| :--- | :--- | :--- |
| [6.25a](/spawn-at/experiments/6-25a-window-ping/) | Window ping | No usable ping/pong API |
| [6.25b](/spawn-at/experiments/6-25b-capture-rig/) | Screen-capture rig | No prompt-free method reached a frame; rig not built |
| [6.25c](/spawn-at/experiments/6-25c-x11-hold-damage-built/) | Hold the cloak, built | Built and tested offline only |
| [6.26](/spawn-at/experiments/6-26-x11-hold-damage-live/) | Hold the cloak, live A/B | Not usable as a ready signal |
| [6.27](/spawn-at/experiments/6-27-premap-and-pixel-probe-built/) | Pre-map resize and pixel probe: timeline, built | No live run |
| [6.28](/spawn-at/experiments/6-28-premap-and-pixel-probe-live/) | Pre-map resize and pixel probe: live | Probe failed at the first call; pre-map not usable |
| [6.29](/spawn-at/experiments/6-29-pixel-probe-retry-shell-crash/) | Pixel probe retry | **GNOME Shell crashed (signal 5)**; probe must not be used |
| [6.31](/spawn-at/experiments/6-31-sync-counter-feasibility/) | X sync counter: feasibility | Inconclusive; fixture cannot be tested |
| [6.32](/spawn-at/experiments/6-32-gtk4-client-legacy-rule/) | Sync-capable GTK4 client, legacy rule | Flashes as much as the fixture when loaded |
| [6.33](/spawn-at/experiments/6-33-sync-counter-moves-on-repaint/) | Does the sync counter move on repaint | Usable as an experimental signal for GTK4 only |
| [6.34](/spawn-at/experiments/6-34-self-sent-sync-request/) | Self-sent sync request | Counter A never moved; stopped |
| [6.35](/spawn-at/experiments/6-35-ld-preload-geometry-shim/) | `LD_PRELOAD` geometry shim | Good on one toolkit; not recommended to ship |

Section 6.30 of the log, "Regression after the removal of the X11 experiments", is a regression
check and has no record here.

## Sources

- `docs/TEST_LOG.md` (sections 6.25 to 6.35)
- `docs/KNOWLEDGE.md` ("Dead ends")
