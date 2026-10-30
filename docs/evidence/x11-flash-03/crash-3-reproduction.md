# Private Shell abort 3: the permitted reproduction of crash 2 (X11-FLASH-03b, Task A)

Date 2026-10-29 21:59 (local). The only cell in which the focus assertion was permitted.
Private headless Shell, DING not loaded (`ding@rastersoft.com` state 6, disabled), extensions
`spawn-at@harsh.local` and `xflash-probe@local`, Overview closed. Live Shell pid 2718897 / start
81373971 unchanged before and after.

| Cell | CLI | Result |
| :-- | :-- | :-- |
| `data/A-old-editor` | built from this tree (`bf62909-dirty`, `src/` == main 7c053b9), startup id `spawn-at-3625858-1793291347721_TIME1793291347721` | run 0 spawned, revealed, closed gracefully with `focused_at_close: true`; the Shell aborted before run 1 |
| `data/A-fix-editor` | PR #94 head `703dfb3`, startup id `..._TIME0` | 10 of 10 runs closed (`focused_at_close` true in 10), 0 aborts |

Abort lines (`data/A-old-editor/crash-lines.txt`):

```
Window manager warning: Buggy client sent a _NET_ACTIVE_WINDOW message with a timestamp of 0 for 0xa00004
libmutter:ERROR:../src/core/window.c:1518:meta_window_unmanage: assertion failed: (window->display->focus_window != window)
Bail out! libmutter:ERROR:../src/core/window.c:1518:meta_window_unmanage: assertion failed: (window->display->focus_window != window)
```

Same signature as `crash-2-focus-assertion.md` and `docs/evidence/x11-flash-02/ding-crash.md`. The old-id
CLI aborts on the first close, so the "up to 5 runs" were one per Shell; I stopped at one
reproduction (two in total with crash 2) and moved on to the fixed binary, as the task says.

Conclusion for the question asked: DING is not required. Old id + focused close + no DING aborts the
Shell; the fixed id + focused close + no DING did not abort in 50 of 50 closes (this cell, `val-fix-editor`
and the passive `pas-gnome-text-editor`). What remains untested: the focused close with the old id
after a `spawn-at focus` (cell 1d of FIX-01b, whose record is not in this repository).
