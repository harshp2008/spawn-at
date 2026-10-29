# Upstream report draft (not sent)

Two reports: mutter (primary, the abort is its bug) and Desktop Icons NG (secondary, its desktop
window is the focus target). A third, short note for GTK/GDK is optional. Written 2026-10-29 from
the measurements in `ding-crash.md`; every claim is tagged in that file. Do not send anything
without the maintainer's decision.

## 1. Mutter

Title: `meta_window_unmanage()` aborts when a client sets a future `_NET_WM_USER_TIME` (startup
notification `_TIME` truncated from 64-bit epoch milliseconds)

Product: mutter 46.2 (Ubuntu `46.2-1ubuntu0.24.04.18`), GNOME Shell 46.0, Wayland session with
XWayland; the assertion and the timestamp check are unchanged on `main` (checked against the
sources of 2026-10-29).

### Steps to reproduce (private/test session, not a production one)

1. Have Desktop Icons NG loaded (its maximized Wayland "Desktop Icons 1" window, class `gjs`, app
   id `com.rastersoft.ding`, is the MRU focus candidate).
2. Launch an X11 (XWayland) GTK4 client whose startup id contains a `_TIME` value larger than the
   compositor's current time, e.g. `DESKTOP_STARTUP_ID=spawn-at-1234-1793266852988_TIME1793266852988
   XDG_ACTIVATION_TOKEN=same GDK_BACKEND=x11 gnome-text-editor --new-window`.
3. Wait until the window has focus (`_NET_ACTIVE_WINDOW` was sent with 2265490556 by the client).
4. Close it (`Meta.Window.delete`/WM_DELETE_WINDOW).

### Actual

```
Window manager warning: Failed to focus window W0 while avoiding 0x400004
libmutter:ERROR:../src/core/window.c:1518:meta_window_unmanage: assertion failed: (window->display->focus_window != window)
Bail out!
```

The Shell aborts (SIGTRAP), the session loses its windows. Reproduced 11 times in 11 attempts in a
private headless Shell; with the same value treated as a past time, or with no `_TIME`, 30 of 30
runs are clean.

### Expected

The window closes; focus moves to the next window; no abort.

### Analysis

- `meta_display_get_current_time_roundtrip()` on Wayland is `g_get_monotonic_time()/1000` (ms since
  boot; 820,688,193 at the time). The client's `_TIME`, truncated to 32 bits by GDK, was
  2,265,490,556: 16.7 days in the future in that clock, below 2^31, so the 32-bit comparison reads
  it as *after* now.
- `meta_window_activate_full()` accepts it (it only guards against a time older than
  `last_user_time`). `meta_display_set_input_focus()` stores it as `last_focus_time`;
  `meta_window_set_user_time()` stores it as `last_user_time`. `meta_display_timestamp_too_old()`
  has the FIXME "we would want to rename to timestamp_too_old_or_in_future" and no future check.
- On the close, `meta_workspace_focus_default_window(workspace, not_this_one, now)` picks the DING
  desktop window W0; `meta_window_focus()` → `meta_display_set_input_focus()` →
  `meta_display_timestamp_too_old()` returns TRUE (now < last_focus_time and now < last_user_time),
  so focus does not move. `try_to_set_focus_and_check()` (from `e4d8e4b9f7d`, issue #862) warns,
  then the fallback `meta_display_unset_input_focus()` is refused by the same check.
  `meta_window_unmanage()` then hits `g_assert (display->focus_window != window)`.

### Suggested fixes (any one stops the abort)

1. Clamp or reject a `_NET_WM_USER_TIME` that is in the future relative to the current time (or
   treat it as `META_CURRENT_TIME`), as the FIXME in `meta_display_timestamp_too_old()` suggests.
2. Never let `meta_display_timestamp_too_old()` block `meta_display_unset_input_focus()` (unsetting
   focus cannot be "too old").
3. In `meta_window_unmanage()`, if the focus window is still the window being unmanaged, force
   `meta_display_unset_input_focus()` with `META_CURRENT_TIME` before asserting (and do not assert
   in a release build).

References: mutter issue #862 and commit `e4d8e4b9f7d`; issue #45 (the same assertion, 2017);
Ubuntu #2016308 (same assertion, Fix Released for a different trigger, 2023).

## 2. Desktop Icons NG

Title: DING's desktop window is the focus target when a focused XWayland window closes, and Mutter's
refused focus hand-over aborts the Shell

Environment: DING 47.0.9 (`46+really47.0.9-1ubuntu5`), GNOME Shell 46, Ubuntu 24.04, Wayland, XWayland.

What happened: with DING enabled, closing a focused XWayland GTK4 window (started with a startup id
whose `_TIME` is a 64-bit epoch) makes the Shell log `Failed to focus window W0 while avoiding
0x400004` and abort. W0 is DING's "Desktop Icons 1" window. Without DING the same app and startup id
do not crash, because the new window is not focused and there is no MRU focus target.

DING code involved (read only): `emulateX11WindowType.js:285-292` (the `destroy` handler →
`_refreshWindows(true)`) and `:372-404` (activate the top-most window, else a keep-at-bottom desktop
window), and `extension.js` for the desktop window's title/`hide_from_window_list`. The desktop
window is `keepAtBottom`, `hide_from_window_list`, maximized, and on all desktops.

Suggested improvement (mutter's bug is the abort; this only removes DING from the trigger): in the
`destroy` handler, do not call `Main.activateWindow()` while a window is unmanaging, or skip the
activation entirely and let Mutter hand focus over. The "make the desktop receive focus when no
other windows are open" behaviour is what puts DING's window in MRU at that moment.

Attach: the two log excerpts from `ding-crash.md` §3 and the DING-only private-Shell recipe.

## 3. GTK/GDK (optional, one paragraph)

`gdk_x11_display_set_startup_notification_id()` takes the part after `_TIME` as an X11 server
timestamp and truncates it to 32 bits. A value that does not fit (here a 64-bit epoch in
milliseconds) becomes a plausible but wrong time, which ends up in `_NET_WM_USER_TIME` and
`_NET_ACTIVE_WINDOW`. Suggest rejecting or ignoring a `_TIME` value that does not fit 32 bits
instead of truncating it. GDK behaved this way in GTK 4.14 (Ubuntu 24.04); the truncation is
measured, the code path is inferred from the GDK documentation.
