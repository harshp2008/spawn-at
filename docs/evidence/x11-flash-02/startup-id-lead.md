# The startup-id lead: what spawn-at sets, and what X11 clients may do with it

X11-FLASH-02f, step 2. Read-only audit; no application was launched for this note. Tags: MEASURED
(read from the source or computed), SECONDHAND (web, snippets only), HYPOTHESIS (untested). The
bisect result is in `bisect-2026-10-29.md`.

## Where the value is made and set (MEASURED, source)

- Made in `src/commands/spawn.rs:276-283`: `format!("spawn-at-{}-{}_TIME{}", pid, ts_ms, ts_ms)`
  where `ts_ms` is `SystemTime::now()` in milliseconds since 1970. The comment above it says
  "Whether it follows the freedesktop startup-notification format is not verified."
- Set for the launched program by the GNOME driver (`src/platform/linux/gnome/driver.rs:317-318`:
  `XDG_ACTIVATION_TOKEN` and `DESKTOP_STARTUP_ID`, both to this key) and by the X11 driver
  (`src/platform/linux/x11/mod.rs:57-58`, same pair). The Sway driver sets neither
  (`src/platform/linux/sway/tests_write.rs:104`: `launch_env` empty). The CLI applies the pair to
  the child it starts (`src/commands/spawn.rs:352`). So it is set for every program started through
  the GNOME or X11 driver, native Wayland or X11, whatever the toolkit.
- Read back by spawn-at in two places: the CLI looks for the key in a window's title or class
  (`src/commands/mod.rs:208-253`, `startup_id_clean`), and the extension reads Mutter's
  `window.get_startup_id()` as one of a window's identifiers when it matches an arm
  (`assets/gnome/modern/extension.js:385`). Mutter fills that from `_NET_STARTUP_ID`, which a client
  sets from `DESKTOP_STARTUP_ID`. So the value has a job: window matching. A native Wayland
  program matches by app id and `XDG_ACTIVATION_TOKEN`, which Mutter ignores here because the token
  is not one it issued (HYPOTHESIS about the Mutter side; not tested).

## History (MEASURED, `git log -S`)

- `0ce3485` (2026-10-06) first set `DESKTOP_STARTUP_ID`; `27b4db6` (2026-10-08) "fixed X11 issues".
- `ccc84dd` (2026-10-08): "Fixed `DESKTOP_STARTUP_ID` formatting to prevent GLib-CRITICAL assertion
  failures in GTK3 apps". That is where the `_TIME<ms>` suffix comes from: it was added so GTK's
  parser accepts the string. The value of the number was never checked against what a timestamp
  is.
- No test or document states the intended `_TIME` value. `src/platform/linux/x11/tests.rs:42-46`
  checks only that both variables carry the key given. `docs/` has no section on it.

## Arithmetic (MEASURED)

- The suffix is the current time in milliseconds since 1970, for example `1793261378397` (13
  digits; real sample from the smoke test: `spawn-at-2738705-1793261378397_TIME1793261378397`).
- An X11 timestamp is a 32-bit unsigned count of milliseconds since the X server started, 0 meaning
  `CurrentTime`. The largest value is 4294967295. `1793261378397` is 417 times that
  (`1793261378397 > 2^32`). Truncated to 32 bits it is `2260015965`, which is not zero and not a time
  on this server.

## What GTK and the specification do (SECONDHAND)

- The startup-notification specification says the ID should be globally unique, for example
  hostname plus pid plus `_TIME` plus the time, where the time is "the X server timestamp of the
  user action that caused the launch".
- GDK's X11 documentation for `gdk_x11_display_set_startup_notification_id` says that if the ID
  contains `_TIME`, the part after it is taken as the X11 timestamp of the event that triggered the
  launch. The GTK3 commit that added it searches from the end for `_TIME`, skips five characters and
  parses the rest with `strtoul`, checking `errno` and that something was parsed (snippet from a
  mirror of the GTK3 history, not the current source).
- Not read: GTK4's current code. What it does with a value that does not fit 32 bits (truncate,
  reject, or use 0) is not known from these sources.

## The lead (HYPOTHESIS)

GTK, running on X11 and given a startup ID whose `_TIME` is not a valid 32-bit server time, sends
`_NET_ACTIVE_WINDOW` with a timestamp Mutter treats as 0 ("Buggy client sent a _NET_ACTIVE_WINDOW
message with a timestamp of 0"), or with a wrong time, and Mutter's focus handling is then confused
when that window is closed. Fits the evidence: both crashed runs were GTK4 apps forced onto X11; the
timestamp-0 warning never appeared in 80 xterm runs (xterm does not read the variable); a native
Wayland GTK app takes the Wayland activation path and does not parse `_TIME` at all. Does not fit:
the truncated value is not 0, so if GTK truncates, the warning needs another explanation. The
bisect cells 5, 6 and 7 test this directly.

## What a fix would have to keep

The id is also the window-matching key (above), so it must stay unique per spawn and still be
readable from `_NET_STARTUP_ID`. Options for the patch, to be chosen from the bisect: put a valid
32-bit value after `_TIME` (what value is a real server time is not knowable without asking the X
server); drop the `_TIME` part only for X11 clients and keep the unique prefix; or not set
`DESKTOP_STARTUP_ID` for X11 clients and match them another way. None is applied.
