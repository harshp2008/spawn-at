---
title: "Findings"
---

What was measured, how, and what each result cannot tell you. All numbers come from the
repository's test logs. They were taken on **one machine** (GNOME Shell 46, Wayland, one monitor)
with the **project's own test window**, not with real applications, unless a line says otherwise.
None of them was captured from the screen.

## How a "flash" is decided

The test window writes its own events to a file with the system's monotonic clock: when it first
paints, and when it is resized or moved. With the protocol dump turned on (the default), the paint
time is the client's `wl_surface.commit` on the Wayland wire. The GNOME extension logs when it
cloaks and reveals the window, using its own clock. The harness ties the two clocks together by
bracketing the log switch with timestamps; the uncertainty measured for that was 1.5 to 3.0 ms.

A run counts as a flash when the reveal came before the client's first paint at the final size, or
the size or position changed by more than 2 pixels within 1500 ms after the reveal. A reveal that
precedes the paint by at least one frame period (16.7 ms at 60 Hz) is labelled `certain`; a smaller
lead is `possible`.

Limits: this is "wire time", the optimistic bound. It says nothing about what the compositor drew in
a given frame. No one captured pixels. A run with no flash means no late paint was found, not that
none can happen.

## GNOME Wayland: the ready-wait against the old rule

Two measurements, same method: the test window anchored bottom-right and asked for a smaller size
(400x300 to 200x150, spec `bottom-right-smaller`) and a larger one (`bottom-right-larger`); the old
30 ms rule (arm A) against the default ready-wait (arm D), alternating each run, in one login. "Loaded" means 22 busy threads
(`xtask burn-cpu`) after a 70 s warm-up.

| | Old rule | Ready-wait (default) |
| :--- | :--- | :--- |
| Idle, first measurement | 0 of 20 flashed | 0 of 20 |
| Loaded, first measurement | **26 of 40** flashed | **0 of 40** |
| Loaded, repeat after other changes | 27 of 40 | 0 of 40 |
| Idle, repeat | 0 of 40 | 1 of 40 (hit the 250 ms cap, cause unknown) |
| Loaded cloak median (ms), first | 45.3 | 66.8 |
| Loaded cloak median (ms), repeat | 48.9 | 56.2 |

Reading it:

- Under load the ready-wait stopped the flashes in these runs. Idle there was nothing to separate.
- The price is that the window is hidden longer under load. The two measurements put the increase at
  21.5 ms and 7.3 ms at the median; the medians varied between batches, and no one repeated an
  identical batch to measure that spread.
- Idle there is no difference in cloak time (about 19.5 ms median in both arms, first measurement).
- The old rule's loaded flash rate is not a constant of nature: other logins gave 15 of 20
  (the baseline login, `TEST_LOG.md` 6.18 table) and 40 of 40 (`TEST_LOG.md` 6.19, line 862). The log suggests a difference between logins; the cause was not examined.
- In the repeat, 1 of 40 idle ready-wait runs hit the 250 ms cap: the app did not answer in time, the
  window was revealed at its old size and corrected 49 ms later. The cause of the stall was not found.

## Apps that cannot answer

A test window that is told to answer late, or never, shows what the cap does:

- An app **already at its minimum size** cannot answer a resize. The ready-wait then runs to
  `wait.cap_ms`: about +230 ms in one test.
- An app that **never answers** (hung) behaves the same.
- A `--resize-delay-ms` test window that answers after the cap is revealed at its old size and
  corrected afterwards, so it flashes; with the old rule it flashed too.

## Real applications

A few runs each, Wayland only: `gnome-text-editor`, `nautilus`, `gnome-terminal`, `zenity`, `firefox`.
Firefox flashed in 6 of 6 runs under the old rule. The README states that the ready-wait
measurements above used the test window, not real apps; the documented minimum-size evidence
covers the test window, `gnome-text-editor`, `zenity` and `firefox`. `zenity --text-info` has a real minimum of about 300 by
201, found while checking an earlier wrong claim of 300 by 400.

## X11 and XWayland

All of the signals tried to detect an X11 client's paint failed on the test window; the X11 flash
numbers are "possible" almost always because the reveal precedes the paint by construction. The
table and the numbers are on [X11 and XWayland](/spawn-at/environments/x11-xwayland/).

## Sway

62 live checks passed on a private headless Sway 1.9, and 15 catalog specs passed. These check
positions and commands, not timing: **the one-frame flash that Sway can have has not been measured**,
and the harness's flash analysis cannot run there. See [Sway](/spawn-at/environments/sway/).

## Shell crashes with a GTK app on XWayland

Two live GNOME Shell aborts (same assertion, 34 minutes apart) followed the graceful close of a `gnome-text-editor` window forced onto X11. A focused close alone is not the cause: 80 focused xterm closes were clean. In a private headless Shell the same configuration did not crash in 69 runs, 27 of them the exact one, so the cause is **unexplained**. That Shell has no real input and no desktop services, so this says nothing yet about the live session. See [6.38](/spawn-at/experiments/6-38-shell-crash-live-xwayland-editor/), [6.39](/spawn-at/experiments/6-39-private-headless-shell/), [6.40](/spawn-at/experiments/6-40-startup-id-audit/) and [6.41](/spawn-at/experiments/6-41-bisect-private-shell/).

## What no result covers

- GNOME versions other than 46; multi-monitor; fractional scaling.
- Qt and Electron apps; Chrome (never matched).
- Anything under a different Shell build or a different login than the logs describe.
- Pixels on screen.

## Sources

- `README.md` ("How well it works"). Its pointer says `docs/TEST_PLAN.md` sections 6.22 and 6.23; the two measurement pairs above are in `docs/TEST_LOG.md` sections 6.23 and 6.30 (the plan only links there)
- `docs/TEST_LOG.md`, 6.19 (login-to-login variation), 6.22, 6.23 and 6.30
- `docs/TEST_COVERAGE_AUDIT.md`, section 4.2 (how flash is computed; X11 and XWayland run counts)
- `docs/TEST_PLAN.md`, section 6.0 (the zenity minimum) and 7
- `docs/DESIGN_READY_WAIT.md`, section 12
- `docs/DESIGN_SWAY.md`, section 5
- `docs/evidence/x11-flash-02/` (the Shell crash round, 6.38 to 6.41)
