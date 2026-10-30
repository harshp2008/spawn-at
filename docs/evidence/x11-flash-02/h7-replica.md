# H7 replica: the live extension set in the private Shell reproduces the crash (X11-FLASH-02h, step 2)

**Key result.** With the live session's extensions and settings loaded, the private headless Shell
aborts with the same assertion as the two live crashes. The bisect names one extension:
Desktop Icons NG (DING, `ding@rastersoft.com`, Ubuntu's stock desktop icons). Tags: MEASURED (a run,
data under `data/h7/`), INFERRED, HYPOTHESIS. The private Shell is headless (software rendering,
one virtual monitor, no real input). Live Shell untouched: pid 2718897, start time 81373971,
before and after.

## Setup (MEASURED)

`H7=1 tools/psh-start.sh`: copies of the 10 user extensions and the 9 system ones that the live
session has enabled, plus spawn-at's, enabled in the private keyfile (GSConnect and Win11Clip Bridge
left out: network and clipboard). The allowlisted live settings (Mutter keys, focus mode, text
scaling 1.25, animations, extension settings; `data/h7/keyfile.ini`) were applied to the private
keyfile only. All 20 extensions reported ACTIVE. Cell: `gnome-text-editor` forced to X11, launched
by `spawn-at spawn --no-focus`, graceful `spawn-at close`; 5 runs per fresh Shell. Dash to Dock's own
settings were not copied (its schema is not readable the same way). No "Dock Focus Tracker" exists;
the maintainer's own extension is Adaptive Dock.

## Result (MEASURED)

| Extensions loaded (besides spawn-at) | Fresh Shells | Result |
| :-- | --: | :-- |
| all 19 | 3 | **crash in each, at run 2** (run 1 unfocused and clean); timestamp-0 warning; same assertion `meta_window_unmanage ... focus_window != window`; fallback target `Failed to focus window W1` |
| the 10 user extensions | 1 | 5 runs clean |
| the 9 system extensions | 1 | **crash at run 1** |
| auto-move, launch-new-instance, light-style, native-window-placement | 1 | 5 clean |
| system-monitor, user-theme, DING, tiling-assistant, appindicators | 1 | **crash at run 1** |
| tiling-assistant alone | 1 | 5 clean |
| system-monitor, user-theme, appindicators | 1 | 5 clean |
| **DING alone** | 1 | **crash at run 1** |
| all 18 except DING | 1 | 5 clean, no timestamp-0 warning |

So DING is **necessary** (without it, no crash in 5 runs) and **sufficient with the spawn-at path**
(alone, it crashes at run 1). Also MEASURED: with only DING loaded, the desktop-icons window is the
private Shell's only other window: a maximized Wayland window, class `gjs`, app id
`com.rastersoft.ding`, holding focus.

## What else is needed, with DING loaded (MEASURED; 5 runs per Shell unless stated)

| Variant | Result |
| :-- | :-- |
| editor X11 by `spawn-at spawn` (cell 5) | crash at run 1 |
| same with a 32-bit-safe `_TIME` (cell 7), or with the observer (cell 8) | crash at run 1 each |
| same without `DESKTOP_STARTUP_ID` (cell 15), or without `XDG_ACTIVATION_TOKEN` (cell 16) | crash at run 1 each |
| same without both variables (cell 6) | **5 clean**, but the timestamp-0 warning appears 5 of 5 |
| editor X11, plain launch (no spawn-at command), focused, graceful `spawn-at close` (cell 4) | **6 clean**, timestamp-0 warning 6 of 6 |
| plain launch with both variables set (cell 17), only the startup id (18), only the token (19) | 5 clean each, warning 5 of 5 |
| editor native Wayland by spawn-at (cell 3); eog forced to X11 by spawn-at (cell 9) | 5 clean each |

Not valid and superseded: the first plain-launch runs with DING (cells 4, 14, 2) took DING's window
for the app's (the detection took the first new window); the tool now matches the window by the pid
it started. An xterm plain launch with DING was therefore not measured validly; the 80 live xterm
runs (DING enabled live) stand instead. One 3-run record of cell 19 and one empty `all_but_ding`
record in `data/h7/results.txt` are from a session where two private Shells overlapped by my
mistake; they are repeated cleanly in the table above.

## Reading (INFERRED / HYPOTHESIS)

- INFERRED: the timestamp-0 `_NET_ACTIVE_WINDOW` message from the forced-X11 editor depends on DING
  being loaded (zero in 69 earlier runs and in the all-but-DING run; present in every DING run),
  and it is not the crash: plain launches show it without crashing.
- INFERRED: the crash needs DING, a forced-X11 GTK4 app, the spawn-at launch path, and one of the
  two startup variables. Removing both variables removes the crash; the oversized `_TIME` is not the
  factor (a 32-bit-safe value still crashes). The startup-id idea of `startup-id-lead.md` is
  therefore contradicted in its original form.
- HYPOTHESIS: the failing focus hand-over goes to DING's desktop window ("Failed to focus window
  W1"): DING hooks `raised` to keep its window at the bottom and emulates X11 window types
  (`emulateX11WindowType.js:51`), and Mutter, closing the focused window, cannot focus it. Why the
  spawn-at path and the variables change the outcome is not understood.
- Not established whether spawn-at is "needed": the plain launches did not crash, but they differ
  from spawn-at launches in more than the variables (the extension claims, cloaks, resizes and
  moves the window). Not tested: DING plus an extension-free Shell with a hand-made equivalent of
  spawn-at's steps; whether other Wayland desktop windows trigger it; DING's separate desktop
  process versus its Shell part.

Steps 3 to 6 of the prompt (calibration, matrix, rule evaluation, pixel capture) were skipped as the
prompt says when a crash reproduces. Raw data: `data/h7/runs/`, `data/h7/results.txt`.
