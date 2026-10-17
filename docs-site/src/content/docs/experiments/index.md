---
title: "Experiments"
---

One record for each experiment the project ran, written from `docs/TEST_LOG.md`. The log itself is a
long dated notebook that is not published here: it names the owner and local paths, and its raw
results live in directories under `scratch/` that git ignores. These pages are written, not copied.

Each record opens with a short paragraph and a table (ID, date, build, verdict, raw results), then
has the sections What was tested, Result, Not verified and Sources. A field the log does not record
is written "not recorded". Where the log gives the commands only in prose the page says "exact
command line not recorded" and keeps the prose. Nothing was re-run to write these pages: every
number comes from the log.

**Build.** The log names the tree or binary it ran on as a commit hash (and sometimes a branch), not as
a version string. The output of `spawn-at --version` is recorded once, in section 6.30 (a regression
check, not an experiment), as `DEV BUILD` with the previous commit `acccca8`. Each record gives the
hash the log gives, or says it is not recorded, and does not infer a version from a hash. The
releases of that time (`v0.2.0-beta.1` and `v0.2.0-beta.2` were tagged on 2026-10-14) are not matched
to any experiment, because the log does not say which build was a release.

**Raw results.** The header of the log says its raw results and scripts are not in the repository:
the result directories (`scratch/p23r` to `scratch/p36r`) are archived outside it, and names such as
`verify-results/...` are gitignored working files. The throwaway clients, watchers and the shim used
in the X11 experiments are not in the repository either. The numbers in the log are what is kept.

## GNOME placement and the late resize (2026-10-12)

| ID | Experiment | Verdict |
| :--- | :--- | :--- |
| [6.10](/spawn-at/experiments/6-10-placement-resize-specs/) | Placement resize specs | 29.7% and 99.4% in two isolated batches; late-resize bug |
| [6.11](/spawn-at/experiments/6-11-late-resize-fix-live/) | Live results after the late-resize fix | Idle 100%; loaded 66% on the printed rectangle |
| [6.11.1](/spawn-at/experiments/6-11-1-final-state-oracle/) | Final-state oracle and re-measure | 0 of 640 fail on the final rectangle; stale printed rectangle remains |
| [6.12](/spawn-at/experiments/6-12-why-revealed-at-pre-resize-size/) | Why the window is revealed at its old size | Mechanism found; "accept" recommended; no fix built |

## Flash detection and the Mutter investigation (2026-10-12 to 2026-10-13)

| ID | Experiment | Verdict |
| :--- | :--- | :--- |
| [6.13](/spawn-at/experiments/6-13-cloak-starves-frame-callbacks/) | Does the cloak starve frame callbacks (H4) | H4 not supported |
| [6.14](/spawn-at/experiments/6-14-mutter-api-and-arrival-wait/) | Mutter API leads and an arrival-wait simulation | Do not build the arrival wait with the cap as the only stop |
| [6.15](/spawn-at/experiments/6-15-flash-aware-verdict/) | Flash-aware verdict (harness) | The extension cannot see a slow client's late paint |
| [6.16](/spawn-at/experiments/6-16-flash-numeric-headline/) | Flash: numeric headline and visibility label | 19 of 20 numeric flashes; model 55 to 57% against about 70% by eye |

## The ready-wait (2026-10-13)

| ID | Experiment | Verdict |
| :--- | :--- | :--- |
| [6.17](/spawn-at/experiments/6-17-flash-baseline/) | Flash baseline before the ready-wait change | Baseline: idle 0 of 80; loaded 15 of 20; XWayland 1 of 10 |
| [6.18](/spawn-at/experiments/6-18-ready-wait-live-validation/) | Ready-wait: live validation | Criteria 2 and 3 not met, 4 not shown |
| [6.19](/spawn-at/experiments/6-19-ready-wait-abc/) | Same-session A/B/C with the override | Cap 250 recommended; B and C not separable |
| [6.20](/spawn-at/experiments/6-20-why-b-and-c-still-flash/) | Why B and C still flash under load | `reached` flashes are a late stamp; B timeouts are real |
| [6.21](/spawn-at/experiments/6-21-default-cap-250-wire-paint-time/) | Default cap 250 and the wire paint time | Wire time 0 of 40 flashes for B and C against 24 for A; no timeouts |
| [6.22](/spawn-at/experiments/6-22-hostile-apps/) | Hostile apps | Cells match except two product findings; hard limit run live |
| [6.23](/spawn-at/experiments/6-23-post-reinstall-live-check/) | Post-reinstall check and XWayland signals | Loaded D 0 of 40 against A 26 of 40; no XWayland signal reliable |
| [6.24](/spawn-at/experiments/6-24-texture-reader/) | Texture reader and texture size as a signal | Reader works; texture size not usable |

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

## Proposed and not run

| ID | Experiment | State |
| :--- | :--- | :--- |
| [P8](/spawn-at/experiments/p8-sway-flicker-not-run/) | Sway flicker experiment | Proposed in the test coverage audit; never run |

## Where the code is

The code of the failed X11 experiments (the hold-the-cloak, pre-map and pixel-probe modules) is not in
the current tree. It was removed by the commit `80ced02` and is still in the history of `main`; see
[X11 and XWayland](/spawn-at/environments/x11-xwayland/).

## Sources

- `docs/TEST_LOG.md` (sections 6.25 to 6.35)
- `docs/KNOWLEDGE.md` ("Dead ends")
