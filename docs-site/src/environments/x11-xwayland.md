# X11 and XWayland

**Status: fallback only, experimental, untested with real applications.** Two different things share
this name, and they behave differently.

## 1. XWayland and X11 windows on a GNOME Wayland session

The GNOME extension handles them like any window, but it keeps the older, shorter wait: a 30 ms
quiet period, a 120 ms minimum, a 1500 ms hard cap. The newer ready-wait is native-Wayland only.
These windows can still flash. They were tried with the project's test window, not with real
X11 applications.

## 2. The native X11 driver

A separate driver (`src/platform/linux/x11/`) that talks to the X server with EWMH atoms
(`_NET_CLIENT_LIST`, `_NET_WM_PID`, `_NET_WORKAREA`, `_NET_MOVERESIZE_WINDOW`, `_NET_WM_STATE`) and
XRandR. It needs no helper to install and needs a running window manager that supports EWMH. It has
no cloak: placement happens after the window is mapped, so the window can show at its default place
and then move.

It is used when:

- you are in a real X11 session, on any desktop, with nothing set;
- you set `SPAWN_AT_BACKEND=x11`, which forces it, including on a non-GNOME Wayland session, where it
  sees XWayland windows only and "may or may not work";
- a GNOME session's D-Bus connection fails: it falls back to the X11 driver.

On a non-GNOME Wayland session without `SPAWN_AT_BACKEND`, spawn-at exits with an error instead; see
[Unsupported desktops](unsupported-desktops.md).

### How well it is tested

- GNOME on X11: partly, with the test window only.
- The X11 driver has 8 unit tests (token generation, rectangle arithmetic, clamping). **No test
  talks to an X server.** The tests named "supported on X11" for `transform`, `focus`, `maximize`
  and `close` call the real X11 driver but check only that an error message lacks a phrase; without
  an X server they pass on a connection error.
- No tests at all for the X11 driver's `query layout`, `query windows`, `query pointer`, `defocus`,
  `minimize` and `restore`.

## What was tried to make X11 flicker-free

The question was how to tell that an X11 client has painted at its new size. Between 2026-10-13 and
2026-10-14 several signals were measured on the project's test window (160 runs idle and under
load for the first table). Nothing from this shipped in any release up to 0.2.0-beta.4.

| Candidate | Result | Why it fails |
| :--- | :--- | :--- |
| First `damaged` after `set_size` | Before the client's final paint in 59 of 80 runs | It comes about 5 ms after `set_size` when idle and tracks the shell, not the client (the exact cause is a hypothesis in the source) |
| `first-frame` | One per run, before the client's resize and paint in 80 of 80 runs | It is a map event |
| Actor size, buffer rect | Equal the frame size in 351 of 351 lines | They are the window manager's size, set at once |
| Texture size | Equals the target at the first `damaged` in 80 of 80 runs, up to 359 ms before the client painted under load | Same moment as the first `damaged` |
| Hold the cloak, wait for a second `damaged` | Not usable (160 runs) | The second one arrives about 30 ms after `set_size` whatever the client does |
| Resize before the map, reveal on `first-frame` | Not usable | `first-frame` arrived 205 to 300 ms before a loaded app painted |
| Window ping | No API exposes the reply | `Meta.Window.check_alive` has no reply signal |
| Pixel probe (read pixels from the window texture) | Read a blank buffer, then the fixture's colour, in 3 smoke runs | In the first batch `get_image` returned a NULL surface, GJS aborted on it and **GNOME Shell crashed** (signal 5). It was removed. No batch data exists |
| Screen-capture rig | Not built | No local consumer could attach to the Mutter ScreenCast stream, and the portal needs a user prompt |

Two later leads, both only on our own GTK4 test client on one machine:

- **X sync counter** (`_NET_WM_SYNC_REQUEST_COUNTER`): the extended counter stepped a few ms after the
  client's final paint in 100 of 100 runs. The basic counter, which Chromium and Electron use, never
  moved. In an earlier survey about 3 of 9 windows would have benefited. A sync request that spawn-at
  sent itself was negative at the timing tried.
- **`LD_PRELOAD` geometry shim**: windows were born at the target size in 80 of 80 runs, and loaded
  flashes were 0 of 40 against 36 of 40. It covers no xcb client, does not exempt override-redirect
  windows, left popups untested, reveals later, showed 10 of 40 idle stalls of unverified cause, and
  carries the security and compatibility cost of injecting a library into applications. It is not
  recommended to ship.

The code for the failed experiments is not in the current tree. It exists in the local branch
`archive/x11-experiments` (tip `673e6f2`, 2026-10-14), which is not on the remote as of this writing.

### Future work, not a plan

The design notes sketch choosing a method per application type and falling back to the old rule:
native Wayland uses the ready-wait; GTK4 on X11 reads the sync counter; applications with a size or
position launch option get it on the command line; the geometry shim only as an opt-in; everything
else keeps the old rule. Nothing of it is implemented, and none of it has been tested on real
applications. The proposed order starts with testing real apps, then opt-in native-Wayland launch
variables for apps that can run on Wayland.

## Limits of the flicker measurement

- Nothing captures pixels. "Flash" is computed from the client's own paint time against the
  extension's reveal time, with a stated clock uncertainty of 1.5 to 3.0 ms.
- On X11 the reveal comes before the client paints by construction: 19 of 20 recorded X11 runs were
  flagged, all as "possible" (leads 3.1 to 15.1 ms). On XWayland 1 of 10. So "possible" is not a
  noise floor there.
- The "possible" probability assumes the reveal lands at a random point of the frame; the code
  itself calls that a hypothesis.
- None of it is measured for Sway or the native X11 driver: it needs the GNOME extension's clock.

## Sources

- `README.md` ("Support", "X11/XWayland: investigated, no reliable signal")
- `docs/DESIGN_READY_WAIT.md`, sections 12 to 15 (the candidate table, the status after 6.31 to 6.35, the future direction)
- `docs/TEST_LOG.md`, sections 6.23 to 6.35 (via the pointers in `docs/TEST_PLAN.md`)
- `docs/DESIGN_PLATFORMS.md`, sections 3 (row 11) and 5 ("X11")
- `docs/TEST_COVERAGE_AUDIT.md`, section 3 and gap 3, section 4.2 and gap 10
- `git log archive/x11-experiments` and `git ls-tree` on that branch (read only)
