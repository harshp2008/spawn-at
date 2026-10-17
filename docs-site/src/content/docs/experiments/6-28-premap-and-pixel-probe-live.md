---
title: "6.28 Pre-map resize and pixel probe: live"
---

## Title

`x11-pixel-probe` and `x11-premap`, live on the XWayland fixture

## ID

TEST_LOG 6.28. Follows [6.27](/spawn-at/experiments/6-27-premap-and-pixel-probe-built/); the probe is
tried again in [6.29](/spawn-at/experiments/6-29-pixel-probe-retry-shell-crash/).

## Date

2026-10-14.

## Build

Tag: none recorded. Tree: HEAD `cd7967f`, tree clean. The debug fixture binary was built with
`--features tui,fixture` and supports `--state-colors`. The output of `spawn-at --version` is not
recorded for this section. The session was GNOME on Wayland, extension ACTIVE, protocol 6, installed
files identical to `assets/gnome/modern` including `x11_premap.js` and `x11_probe.js`, after a new
login.

## Question

(1) Does the pixel probe read the window texture, and can "blank new buffer" be told from "painted"?
(2) Does resizing the window before it is mapped make the app's first paint land at the target size,
and does revealing on `first-frame` then avoid the flash?

## Setup and environment

Fixture only: `--protocol x11`, plus `--state-colors` for the probe. No real X11 or XWayland
application. Defaults unchanged. Probe logging is enabled by the batch scripts sending `SetLogging(true)`
over D-Bus at the start (an in-memory flag) and `SetLogging(false)` at the end and on exit. For
pre-map: 5 runs per spec per arm in the same session, arms rotated, idle and then under 22 burn
threads after a 70 s warm-up. Arm A is `legacy`; arm P is `x11-premap`. "Hidden" is `window-created`
to `UNCLOAK`.

## Commands run

Exact command lines not recorded. By the log: `spawn-at spawn` of the fixture with
`--wait-strategy x11-pixel-probe --state-colors` (3 idle `larger` spawns), and with
`--wait-strategy legacy` or `x11-premap` (40 smoke runs).

## Raw result location

`scratch/p29r/` (gitignored). Not in the repository.

## Result

**Pixel probe: it did not work as built.** The probe started (`X11_PROBE_START readback=available
max=16 gap_ms=8 clip=2x2`) and ran 2 to 3 probes per run, but every sample of all 8 probes was an
error: `Object 0x... is not a subclass of GObject_Boxed, it's a Object`. GJS refused the plain
`{x, y, width, height}` object as the clip argument of `WindowActor.get_image`, before any readback.
The logged `cost_ms` of 0.03 to 0.16 is the failed call. No pixel was read and no PNG was written. The
log's proposed fix was a real `Mtk.Rectangle` imported lazily; it needed an install and a log-out and
log-in, so the batch was not run.

**Pre-map: 40 smoke runs, all exit 0, gnome-shell unchanged, no assertion, critical or JS error in the
journal.** One idle run showed `PREMAP_RESIZE` at `window-created`, `SET_SIZE_PREMAPPED` 17.0 ms
later, `first-frame` 19.4 ms after the resize and `READY_WAIT_RESOLVED ... reason=x11-premap-first-frame`.

| Cell (5 runs) | Arm | First paint at the target size | Flashes (clearly early) | Reveal minus final paint, median, ms |
| :-- | :-: | :-- | :-: | :-- |
| idle smaller | A | 0 of 5 | 0 | +26.9 |
| idle smaller | P | 3 of 5 | 0 | +38.3 |
| idle larger | A | 0 of 5 | 1 (0) | +11.9 |
| idle larger | P | 3 of 5 | 1 (0) | +7.6 |
| loaded smaller | A | 2 of 5 | 4 (2) | -13.4 |
| loaded smaller | P | 2 of 5 | 3 (1) | -4.4 |
| loaded larger | A | 2 of 5 | 5 (5) | -171.8 |
| loaded larger | P | 5 of 5 | 5 (5) | -253.5 |

- Pre-map moved the first paint to the target size in 6 of 10 idle runs (0 of 10 for A), but not
  reliably. In loaded `larger` all five P first paints were at 800x600, yet they came 147 to 249 ms
  after the resize and the final paint at 255 to 358 ms, while `first-frame` came about 2 ms after the
  map, so the window was revealed 205 to 300 ms before the app had painted anything it would keep.
- The gate for the full 20-per-arm A/B failed on both conditions: first paint at the target size in
  loaded P runs was 7 of 10 (70%, below the 80% bar), and loaded flashes were 8 of 10 for P against 9
  of 10 for A. The full A/B was not run.
- Cleanup: no `xtask`, fixture or `gst-launch` process left; no PNG left; gnome-shell pid unchanged.

## Verdict

`x11-pixel-probe`: inconclusive, nothing measured. `x11-premap`: not usable as a ready signal on this
evidence, because `first-frame` is a map event, not an app event. The log's note: the early resize
itself works and is cheap, and a signal that follows the app's commit would be needed.

## Not verified

`get_image` on a cloaked actor and its cost; whether the early resize is safe for every X11 client;
real X11 applications; more than 5 runs per pre-map cell (the smoke is small). A solid-colour fixture
cannot prove a blank test is general, the log adds, because a real app's first frame may itself be
white or black.

## Sources

- `docs/TEST_LOG.md` section 6.28 (lines 1501-1532)
- [X11 and XWayland](/spawn-at/environments/x11-xwayland/)
