---
title: "6.26 Hold the cloak: live A/B"
---

:::note[Reader's note]
This page was written on 2026-10-14. The verdict below says the X sync counter was still untried.
6.31 to 6.35 are the experiments that tried it and the routes around it. The result was not usable as
a ready signal:

- [6.31](/spawn-at/experiments/6-31-sync-counter-feasibility/): not testable with the fixture, which
  does not take part in X sync.
- [6.32](/spawn-at/experiments/6-32-gtk4-client-legacy-rule/): a sync-capable GTK4 client flashed as
  much as the fixture under load, and Mutter did not visibly wait for it.
- [6.33](/spawn-at/experiments/6-33-sync-counter-moves-on-repaint/): the extended counter moved on
  repaint for that GTK4 client only; the basic counter never moved, and the extension cannot read
  XSync counters.
- [6.34](/spawn-at/experiments/6-34-self-sent-sync-request/): counter A never moved, so the batches
  were not run.
- [6.35](/spawn-at/experiments/6-35-ld-preload-geometry-shim/): an `LD_PRELOAD` shim worked on one
  toolkit, and the log does not recommend shipping it.
:::

`x11-hold-damage`, live A/B against the legacy rule on the XWayland fixture. The second damaged event is a shell-side event, so the method is not usable as a ready signal.

| Field | Value |
| :-- | :-- |
| **ID** | TEST_LOG 6.26. Follows [6.25c](/spawn-at/experiments/6-25c-x11-hold-damage-built/). |
| **Date** | 2026-10-14. |
| **Build** | Tag: none recorded. Commit: not recorded as a hash. The log says `target/debug/spawn-at` was "built at the HEAD of the previous step". The harness fix made during this section is commit `21cf0d4`. The output of `spawn-at --version` is not recorded. The extension in the session was the modern set with the hold module installed (the log checked that the installed files were identical to the tree's `assets/gnome/modern`, with `x11_hold.js` present). Extension protocol 6; a new login before the runs. |
| **Verdict** | Not usable as a ready signal for XWayland and X11. The log names what would change that: a second `damaged`, or any shell signal, that tracks the client's commit; or a client-side signal (the X sync counter, then still untried; see [6.31](/spawn-at/experiments/6-31-sync-counter-feasibility/)). |
| **Raw results** | `scratch/p27r/` (gitignored). Not in the repository. |

## What was tested

**Question**

Does ending the wait on the second `damaged` event of a cloaked XWayland window give a usable
ready signal, compared with the legacy rule? Is the second `damaged` the client's paint, or a
shell-side event?

**Setup and environment**

GNOME on Wayland, one login for all runs. Fixture only, started with `--protocol x11`; no real X11
or XWayland application. Defaults unchanged. Arm A is `--wait-strategy legacy`; arm H is
`--wait-strategy x11-hold-damage`. Arms were rotated per iteration, spec order alternated, 3.2 s
apart. 20 runs per spec per arm idle (80 runs), then the same under 22 `xtask burn-cpu` threads after
a 70 s warm-up (80 runs; load average 18.0 after the warm-up, 23.7 at the end). Time is measured in
ms after the `set_size` mark. "Final paint" is the last fixture `FIRST_PAINT` at the target size; the
stamp can be late by up to about 19 ms under load. "Clearly early" means a reveal more than 19 ms
before it.

**Commands run**

Exact command lines not recorded. By the log: direct `spawn-at spawn --json` of the fixture with
`--wait-strategy legacy` or `--wait-strategy x11-hold-damage`, `--protocol x11`, and the environment
variable `SPAWN_AT_FIXTURE_EVENT_DIR` set; `SetLogging(true)` for the smoke only; `pw-dump`,
`gst-launch-1.0 pipewiresrc` (phase 4); `PIPEWIRE_DEBUG=4` (attempt 3). A throwaway Python Gst
script was used for attempt 2.

## Result

Smoke (idle, `larger` 400x300 to 800x600): the strategy ended on `x11-second-damaged` at 45.2 ms
(`damaged_seen=2`; first `damaged` at 11.4 ms, second at 45.1 ms), and the reveal followed 13.9 ms
later. Both `damaged` lines showed the frame still at the old position. A native Wayland fixture
given the same flag was unaffected (`unused=strategy(x11-hold-damage:not-x11)`).

A/B, 160 runs, all exit 0 and all windows found:

| Cell (20 runs) | Arm | Reveal minus final paint, median (min to max), ms | Flashes | Clearly early |
| :-- | :-: | :-- | :-: | :-: |
| idle smaller | A | +26.1 (+9.9 to +35.3) | 0 | 0 |
| idle smaller | H | +40.3 (+28.7 to +49.5) | 0 | 0 |
| idle larger | A | +6.9 (-5.3 to +13.6) | 3 | 0 |
| idle larger | H | +26.0 (+12.1 to +42.0) | 0 | 0 |
| loaded smaller | A | -41.7 (-676.9 to +5.5) | 17 | 16 |
| loaded smaller | H | +8.0 (-42.0 to +36.3) | 6 | 1 |
| loaded larger | A | -147.9 (-322.2 to -60.1) | 20 | 20 |
| loaded larger | H | -123.9 (-223.2 to +10.1) | 18 | 18 |

- Arm H ended on the second `damaged` in 79 of 80 runs; one run (loaded larger, run 1) ended on the
  cap, with no `damaged` for 262 ms while the shell itself was stalled. No H run ended on
  `x11-hard`, `x11-unmanaged` or `x11-no-damaged-signal`.
- The second `damaged` arrived about 30 to 34 ms after `set_size` when idle, in both specs, although
  the client painted at 4.3 ms (smaller) and 20.8 ms (larger). Under load it came before the
  client's final paint in 30 of 39 runs, by up to 195 ms. The log concludes it is a shell-side event.
- A cloaked window does get `damaged` (at least 2 during the hold in 79 of 80 H runs). The log notes
  this contradicts its reading in [6.25](/spawn-at/experiments/6-25c-x11-hold-damage-built/) that a
  cloaked window may get none.
- What H bought was a longer, fixed-looking cloak: +13 to +18 ms idle and +27 to +38 ms median under
  load, which cuts loaded `smaller` flashes from 17 to 6 of 20, loaded `larger` from 20 to 18 of 20,
  and idle `larger` from 3 to 0 of 20. The log calls that a delay, not a readiness signal.
- Harness: two small gaps were fixed in `spawn-at-verify` (commit `21cf0d4`): the cap of a hold run
  was not reported, and a missing override line was not noted.
- Phase 4, capture diagnostic: 3 more attempts to read the Mutter ScreenCast node with
  `pipewiresrc` all ended with `stream error: target not found`. Together with
  [6.25b](/spawn-at/experiments/6-25b-capture-rig/), every spelling of the target was refused.
- Cleanup: no `xtask`, fixture or `gst-launch` process left; nothing was closed by hand.

## Not verified

Real X11 and XWayland applications, and clients that resize late or never; screen capture of what is
visible; the cause of the shell-side second `damaged` (an extension step, the frame clock or
XWayland itself); whether a fixed 40 to 50 ms hold would do as well as H (not run); H on more than one
monitor or with fractional scaling. The log's open item: a fixed-hold control arm.

## Sources

- `docs/TEST_LOG.md` section 6.26 (lines 1400-1458)
- `git log` for `21cf0d4`
- [X11 and XWayland](/spawn-at/environments/x11-xwayland/)
