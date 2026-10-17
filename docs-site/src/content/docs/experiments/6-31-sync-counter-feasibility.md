---
title: "6.31 X sync counter: feasibility"
---

The X sync counter (`_NET_WM_SYNC_REQUEST_COUNTER`) as a client-side ready signal: feasibility. The idea could not be tested with the fixture, which does not take part in X sync.

| Field | Value |
| :-- | :-- |
| **ID** | TEST_LOG 6.31. Superseded by [6.32](/spawn-at/experiments/6-32-gtk4-client-legacy-rule/) and [6.33](/spawn-at/experiments/6-33-sync-counter-moves-on-repaint/), which tested the idea with a GTK4 client. |
| **Date** | 2026-10-14. |
| **Build** | Tag: none recorded. Tree: HEAD `a950022`, tree clean. The previous section (6.30, a regression check after the removal of the X11 experiments, not an experiment) recorded `spawn-at --version` as `DEV BUILD`, previous commit `acccca8`; this section does not record the version output. Session: GNOME on Wayland, no gnome-shell assertion, crash, signal, "uncatchable" or critical line in the journal at the start. |
| **Verdict** | Inconclusive, and not testable with the fixture. The log lists what would change it: a purpose-built scratch client (raw Xlib or x11rb) that advertises `_NET_WM_SYNC_REQUEST`, sets the counter, repaints at a new size when asked and bumps the counter after the repaint; then the 6.23 method on that client. The log also sketches a design (watch the actor's `thawed` signal first; otherwise a helper process reading the counter through XSync) and marks it a hypothesis; nothing of it was built. |
| **Raw results** | `scratch/p32r/` (gitignored). Not in the repository. |

## What was tested

**Question**

Does an X11 client's sync counter move when it has repainted at a new size, so that it could serve
as a ready signal? In this section: is there an open X11 client on the machine, or the fixture, that
takes part in sync, to test it with?

**Setup and environment**

Nothing in the Shell; no code or extension change; no install. Read-only property queries and one
fixture spawn (`--protocol x11`, class `verify.p32`, kept open for 7 s). The live Xwayland authority
file was exported for the queries. No real X11 or XWayland application was open: the user's windows
were native Wayland.

**Commands run**

`xwininfo` and `xprop` (read-only), and `xdpyinfo` to list the X server's extensions. Exact command
lines not recorded.

## Result

Survey of the X clients that were open (all desktop helpers; none advertises sync):

| Window | `WM_CLASS` | `_NET_WM_SYNC_REQUEST` in `WM_PROTOCOLS` | `_NET_WM_SYNC_REQUEST_COUNTER` |
| :-- | :-- | :-: | :-: |
| `gsd-xsettings` | `gsd-xsettings` | no | not set |
| `ibus-x11` | `ibus-x11` | no | not set |
| `mutter-x11-frames` | `mutter-x11-frames` | no | not set |
| `GNOME Shell`, `ibus-xim`, `mutter guard window` | none | no | not set |

The fixture window had `WM_PROTOCOLS = WM_DELETE_WINDOW, _NET_WM_PING`, no `_NET_WM_SYNC_REQUEST`,
and no counter; the X server does have the SYNC extension. This agrees with the earlier measurement in
`docs/DESIGN_READY_WAIT.md` section 3.2 (the fixture does not advertise sync; winit 0.29.15 has no
sync-request code, found by grepping the vendored crate). Phase 3 (a counter log with 20 idle and
20 loaded runs per spec) was not run.

The same earlier measurement, on an X11 session, found sync advertised by GTK4 apps (Nautilus,
gnome-text-editor, gnome-calculator), Chromium and two Electron apps, with one counter for
Chromium/Electron and two (extended sync) for GTK4; and not advertised by the fixture, xterm, xclock
and xcalc. The log states this as 6 of 9 measured windows; the site's X11 page notes that the table
it comes from has 10 rows.

## Not verified

When a sync counter moves relative to a client's paint (no counter log exists). Whether Mutter
freezes and thaws XWayland windows at all (hypothesis in the log). Whether the same toolkits
advertise sync under XWayland.

## Sources

- `docs/TEST_LOG.md` section 6.31 (lines 1598-1629)
- `docs/DESIGN_READY_WAIT.md` section 3.2
- [X11 and XWayland](/spawn-at/environments/x11-xwayland/)
