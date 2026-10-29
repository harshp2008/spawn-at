# Private Shell crash 2: Mutter focus assertion on the first forced-X11 gnome-text-editor close (X11-FLASH-03a)

Date 2026-10-29 21:34 (local). Private headless Shell only; extensions enabled in it:
`spawn-at@harsh.local` (unpatched copy of `assets/gnome/modern`) and `xflash-probe@local`. DING not
loaded. Live Shell pid 2718897 / start 81373971 unchanged before and after. `spawn-at` CLI: built from
this tree (`src/` identical to main 7c053b9), not the installed 0.2.4.

## What happened

Cell `val-gnome-text-editor` (`pcell.sh val-gnome-text-editor gnome-text-editor --reps 10 --seed 6...`):
run 0 (small, forced X11 via `GDK_BACKEND=x11`) spawned, revealed and was closed gracefully by
`spawn-at close --id`; the Shell then aborted before run 1 (driver: `CRASH: gnome-shell or boot id
changed before run 1`). Last lines of the Shell log (`scratch`-only copy; these are the lines that matter):

```
Window manager warning: Buggy client sent a _NET_ACTIVE_WINDOW message with a timestamp of 0 for 0xa00004
... ANCHOR_RELEASED win=[... proto=XWayland] reason=EXTERNAL_CLOSE
Window manager warning: Tried to ping window 0xa00004 with a bad serial! Not allowed.
**
libmutter:ERROR:../src/core/window.c:1518:meta_window_unmanage: assertion failed: (window->display->focus_window != window)
Bail out! libmutter:ERROR:../src/core/window.c:1518:meta_window_unmanage: assertion failed: (window->display->focus_window != window)
(EE) failed to read Wayland events: Connection reset by peer
```

No journal line mentions spawn-at beyond its own timing log lines in the private Shell's file log.

## Why this is the known assertion, and what is new

Same assertion, same app, same toolkit path as `docs/evidence/x11-flash-02/ding-crash.md` and the
two live crashes. What differs from 02i-b: there the same app ran 50 forced-X11 runs in this
private Shell with 0 crashes and `focused_at_close` false in 50 of 50 (`data/matrix/gnome-text-editor/`
in x11-flash-02). Here run 0 has `focused_at_close: true` and no DING is loaded.

What changed between the two sessions, from the code: the probe extension closes the Overview
(`Main.overview.hide()` in `tools/probe/xflash-probe@local/extension.js`). A fresh GNOME session
starts in the Overview, which hides `Meta.WindowGroup`; in that state a new window is not focused.
With the Overview closed, as on a real desktop, the new window takes focus, and the close of a
focused XWayland window reaches `meta_window_unmanage` with `focus_window == window`.

Hypothesis, NOT tested: the focus hand-over at close is refused because the spawn-at launch sets a
startup time (`_TIME<epoch ms>`, truncated to 32 bits, far in the Shell's future); DING's desktop
window was only one of the targets that refusal can hit. FIX-01b (PR #94, branch
`fix/startup-id-time`) changes that value. It is not in main, so the main binary used here still has
it. Not run: the same cell with the FIX-01b CLI; the same cell with `focus-new-windows=strict`.

## Stop rule and state

Maintainer rule (chat, 2026-10-29): stop on any Mutter window or focus assertion. Work stopped here.
The sandbox was stopped with `psh-stop.sh` (logs kept under the git-ignored `scratch/`); no process
was signalled by name.
