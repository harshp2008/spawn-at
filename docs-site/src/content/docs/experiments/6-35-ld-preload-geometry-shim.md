---
title: "6.35 LD_PRELOAD geometry shim"
---

Born at the target geometry: an `LD_PRELOAD` shim on the test client. The shim worked on one toolkit, and the log does not recommend shipping it.

| Field | Value |
| :-- | :-- |
| **ID** | TEST_LOG 6.35. The last of the X11 experiments in the log. |
| **Date** | 2026-10-14. |
| **Build** | Tag: none recorded. Tree: HEAD `7954a84`. The extension and the binary were the installed, unchanged ones; no code in the tree changed. The output of `spawn-at --version` is not recorded for this section. The journal was checked before and after every batch: the same gnome-shell process throughout, no stop-rule line, no new critical. |
| **Verdict** | "GOOD" for this toolkit and this machine, under the rules fixed up front: at least 95% of windows were mapped at the target size (80 of 80 in the S arm (with the shim) in the main cells, 5 of 5 in (a) and (c)), and the S arm had zero clear flashes loaded where L (without the shim) had 36 of 40. The log states the exceptions plainly (case (b), the extra delay) and ends: "I do not recommend shipping it." The supported route stays the extension's own resize. |
| **Raw results** | `scratch/p36r/`, `scratch/shim/` and `scratch/p36_*` (gitignored). Not in the repository. |

## What was tested

**Question**

If a window is created at its target size and position (by rewriting the client's Xlib calls), is it
mapped at the target, and does that remove the flash that the legacy rule shows under load?

**Setup and environment**

GNOME 46 on Wayland with XWayland, one 1920x1080 virtual monitor, the legacy rule. Research only;
nothing ships. The client is the GTK4 test program (`scratch/p33_syncclient.py`, X11 backend); no
real X11 or XWayland application.

The shim (`scratch/shim/geomshim.c`, built with `gcc -shared -fPIC`, using `dlsym(RTLD_NEXT)`) reads
`SPAWN_AT_GEOM="WxH+X+Y"`. For windows whose parent is the root it rewrites size and position in
`XCreateWindow`, `XCreateSimpleWindow`, `XResizeWindow`, `XMoveResizeWindow`, `XMoveWindow` and
`XConfigureWindow` (mask aware), and sets the position and size hints in `XSetWMNormalHints` and
`XSetWMProperties`. GTK4 imports `XCreateWindow`, `XResizeWindow`, `XMoveResizeWindow`, `XMoveWindow`,
`XSetWMNormalHints`, `XSetWMProperties`, `XChangeProperty` and `XMapWindow` (`nm -D` on
`libgtk-4.so.1`), and no `xcb_*` window call.

`LD_PRELOAD` was set only through `env LD_PRELOAD=... SPAWN_AT_GEOM=... SPAWN_AT_SHIM_LOG=... python3
p33_syncclient.py` inside the command after `--` of `spawn-at spawn`, so in the client's own
environment: not in the shell, not in `systemctl --user`, not in any file. Arms were interleaved in one
batch: S with the shim, L without it, same specs (`larger` 800x600 and `smaller` 200x150, bottom-right,
margin 16), one run every 3.2 s. 20 runs per spec per arm idle (80 runs) and loaded (80 runs; 22
`xtask burn-cpu` threads, 70 s warm-up, load average 19.5 at the start and 24.7 at the end). Awkward
cases: 5 runs per arm each. "Clear flash" is the client's final first paint at the target size more
than 19 ms after the reveal; this is event time, not pixels.

**Commands run**

Exact full command lines not recorded. By the log: `gcc -shared -fPIC` for the shim; `nm -D` on
`libgtk-4.so.1`; `spawn-at spawn ... -- env LD_PRELOAD=<shim> SPAWN_AT_GEOM=<geom>
SPAWN_AT_SHIM_LOG=<file> python3 p33_syncclient.py ...`. `ltrace` was not installed; the shim's own log
was used. **Gotcha recorded:** the checkout path contained a space, and the dynamic loader splits
`LD_PRELOAD` on spaces, so the first smoke loaded nothing; the library was copied to a path without a
space and the smoke repeated.

## Result

| Cell | Arm | Flashes | Mapped at target | Cloak to reveal, median / worst | X map to reveal, median / worst |
| :-- | :-: | :-: | :-: | :-: | :-: |
| idle larger | S | 0/20 | 20/20 | 10.6 / 43.5 ms | 32.7 / 73.6 ms |
| idle larger | L | 0/20 | 0/20 | 12.8 / 21.3 ms | 22.1 / 28.7 ms |
| idle smaller | S | 0/20 | 20/20 | 16.3 / 44.3 ms | 36.0 / 70.6 ms |
| idle smaller | L | 0/20 | 0/20 | 13.2 / 17.9 ms | 20.6 / 26.0 ms |
| loaded larger | S | 0/20 | 20/20 | 14.4 / 35.7 ms | 127.2 / 234.5 ms |
| loaded larger | L | 18/20 | 0/20 | 28.0 / 41.3 ms | 41.8 / 56.0 ms |
| loaded smaller | S | 0/20 | 20/20 | 14.7 / 27.0 ms | 108.0 / 176.1 ms |
| loaded smaller | L | 18/20 | 0/20 | 23.7 / 35.0 ms | 40.2 / 59.9 ms |

- Mapped at the target size: 80 of 80 in the S arm, 0 of 80 in L. Clear flashes loaded: S 0 of 40, L 36
  of 40. Idle: 0 of 40 in both arms, so the idle cells cannot show a difference.
- Mutter did not change the initial position or size in any of the 80 S runs. The extension's SetSize
  step was a no-op in all of them.
- Costs: 10 of 40 idle S runs took more than 30 ms from cloak to reveal (worst 44.3 ms), none in L. In
  the one run read in detail the delay sat in the one-frame wait before the reveal; the log's
  explanation is a hypothesis, not tested. X map to reveal was longer in S (idle 33 to 36 ms against 21
  to 22 ms; loaded 108 to 127 ms against 40 to 42 ms).
- Awkward cases (5 runs each, idle): (a) a client default of 900x700 with a target of 200x150: S 5 of 5
  mapped at the target with no flash. (b) A client minimum above the target: 4 of 5 mapped at the
  target and then grown to the minimum within 3.7 to 6.8 ms; the extension's later step is still
  needed. (c) A client that resizes itself at +400 ms: S 5 of 5 had no post-map change because the shim
  pinned the app's own resize back to the target, which the log calls a behaviour change of the app.

## Not verified

Any toolkit other than GTK4 through Xlib: xcb-based clients (Qt, the Chromium and Electron X11 path,
winit and x11rb apps such as the fixture), static binaries, Flatpak and Snap sandboxes, setuid or
AppArmor-restricted apps, GTK3 and Wayland-native clients. Popups and menus (the shim rewrote every
parent-is-root `XCreateWindow`, including override-redirect windows, which a real shim would have to
skip). Second top-level windows and dialogs; apps that restore their own geometry; multi-monitor and
fractional scaling; a screen capture (no pixels were measured). The product concerns the log states
for the record, not tested: a wider security surface (code of ours inside every launched app, and
anything that can set `LD_PRELOAD` in the spawn environment can run code), possible breakage of apps
(symbol interposition, the unconditional pinning seen in (c)), and distribution or enterprise policies
that forbid or audit `LD_PRELOAD`.

## Sources

- `docs/TEST_LOG.md` section 6.35 (lines 1746-1796)
- [X11 and XWayland](/spawn-at/environments/x11-xwayland/)
