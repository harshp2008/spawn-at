---
title: "Known limits"
---

Behaviors a user can trip over, stated as the code does them. Everything here was read from the
source, not run on a live session, unless a line says otherwise. For what is broken or unmeasured in
the project as a whole, see [Known shortcomings](/spawn-at/shortcomings/).

## Exit status and results

- **`spawn` exits 0 when no window appears.** If the program starts but no window shows up within
  `spawn.window_timeout_ms`, `spawn` prints a warning and exits 0. This is the same on GNOME, Sway
  and X11. On Sway the placement step reports that the application was started but not placed, and
  on X11 that step returns success without a message unless `SPAWN_AT_DEBUG=1`; in both cases `spawn`
  still waits for the window and then exits 0. It exits non-zero only for an empty command, invalid
  options, `--area screen` without the helper it needs, or a program that cannot be started.
- **`spawn --json` can print a result with no window.** `window_id` is `null` and the position and
  size are `0` when the window never appeared, with `--no-wait`, and when the placement could not be
  armed (the program still starts, unplaced). With `--no-wait` and a placement that could not be
  armed, nothing is printed.
- **`focus`, `defocus`, `maximize`, `minimize`, `restore` and `transform` exit 0 when the state
  does not change.** Each waits up to 2 seconds for the new state, drops the outcome of that wait and
  exits 0. `--no-wait` skips the wait.
- **`defocus` prints `Defocused window ...` in every case where it sent the request**, including
  after a timed-out wait and with `--no-wait`. If the window is not focused, it prints a note and
  sends nothing.
- **`close` is the exception.** It polls for up to 1500 ms and fails with `did not close within ...`
  if the window is still listed. A window list that cannot be read counts as closed. `--no-wait`
  skips the check.
- **A failed `gnome-extensions` call is a note or warning, not an error status.** After a non-zero
  `gnome-extensions enable`, `install` prints a note (files deployed, extension not confirmed
  enabled) instead of `successfully installed and enabled`, and still exits 0. If
  `gnome-extensions disable` cannot be started or exits non-zero, `uninstall` prints a warning (the
  extension may still be enabled; the files were removed or not) instead of `successfully
  uninstalled`, carries on, and still exits 0. `install` stops with an error when a file cannot be
  written or `gnome-extensions` cannot be started; `uninstall` when the extension directory cannot
  be deleted. Read from source, not run.

## Matching a window by name

There is no single matching rule, and none of the rules is strict.

- **Selectors on existing windows** (`-c`, `-t`, `--id`, `--pid`, `--focused`) must all hold
  together. `-c` is the whole class or its last dot-separated part, compared case-insensitively.
  `-t` is a case-insensitive substring of the title. `--id` is the numeric id.
- **`spawn --class`, or the name `spawn` derives from the command, recognises the new window.** The
  match is case-insensitive and accepts: the whole class; the last dot-separated part on either
  side; reverse-DNS tails joined with `-` or nothing (`org.gnome.Terminal` and `gnome-terminal`); a
  `-server`, `.server`, `-daemon`, `.daemon` or `-service` suffix on either side; and the name
  appearing anywhere in the window title. A short name matches any new window whose title contains
  it. An empty name or `*` matches any new window.
- **State waits** after `focus`, `maximize` and the like find the window again from the resolved
  target with a looser rule: `pid:class`, then a window id, then a process id (or an ancestor of the
  window's process), then the name rule that `spawn` uses. A number is tried as a window id and as a
  process id, so `1234` can match window 1234 and process 1234.
- **On GNOME the extension matches by itself.** It compares the registered name with several
  identifiers of each new window, case-insensitively. It is a match when the two are equal or when
  either one contains the other.
- **An existing window can be taken for the new one.** When no new window qualifies, `spawn`
  accepts a window that was already open if its process is the launched one or a descendant of it.

## Geometry: what "reached" means

- **Position** counts as reached within 1 px, in x and in y, of the position computed for the size
  the window actually has.
- **Size** counts as reached when width and height are each at least the requested values. A larger
  window passes the wait. The final report then treats a size that differs from the request as
  raised or different and says so.
- **The wait for geometry returns the last rectangle it read.** `spawn` reads the geometry again for
  up to 300 ms (every 25 ms). If the target is not reached by then, the last rectangle read is used
  and reported, with no error. The wait for a new geometry after `transform` accepts 4 px on each of
  x, y, width and height, and a timeout there is ignored.
- **A window that maximized itself** after `spawn` placed it gets one `Restore` plus the same
  placement again, once, when a size, position or anchor was requested.

## After a failed `spawn`

- **On GNOME, a registration is not withdrawn.** `spawn` registers the placement with the extension
  before the program starts. On Linux the extension's disarm call is never sent, not even when the
  program fails to start. The registration ends on its own: after 15
  seconds when it has a name, after 1.2 seconds when it has none (`--class '*'` or an empty name).
- **In that time an unrelated window can be placed.** A window that the extension matches to the name
  (see above) and that opens within those 15 seconds gets the placement meant for the program that failed.
  With the 1.2 second wildcard, the next window that opens is taken.
- **Registrations under one name queue up to 16**; adding a 17th drops the oldest.

## Sway

- **`--anchor cursor` and `--monitor cursor` in `spawn` use the pointer position (0, 0)** and print
  no message, because Sway has no pointer query. `transform` with either option, and
  `query pointer`, return an error instead.
- **Only the first entry of a request is placed.** The Sway placement step handles the first entry
  of the request, and `spawn` sends one entry, so the first window that appears is the one placed.
  Later windows of the same program are left alone.
- **`--no-wait` still waits** up to 5 seconds for the window, because the placement step runs in every
  mode.

## X11

- **`arm` does nothing on the X server.** It only returns the launch environment
  (`XDG_ACTIVATION_TOKEN` and `DESKTOP_STARTUP_ID`, both set to the entry key). The window is found
  and moved afterwards by the placement step, which waits up to 5 seconds to find it and up to
  3 seconds for it to map, also with `--no-wait`. Errors from the X11 requests there are ignored.

## Not verified

None of the above was run on a live GNOME, Sway or X11 session for this page. The statements come
from reading the CLI source, the GNOME extension source and the Sway and X11 backends. Anything about
other GNOME versions, real Sway sessions and real X11 applications is listed in
[Known shortcomings](/spawn-at/shortcomings/).

## Sources

- `src/commands/spawn.rs`, `src/commands/focus.rs`, `src/commands/lifecycle.rs`,
  `src/commands/transform.rs` and `src/commands/mod.rs` (exit status, waits, the matching rules,
  the geometry loop)
- `src/target.rs` (selectors for existing windows)
- `src/diagnostics.rs` (the position and size report)
- `src/platform/linux/gnome/deploy.rs` (`install` and `uninstall` output)
- `src/platform/linux/mod.rs` (`disarm` is not forwarded) and `assets/gnome/modern/extension.js`
  (registration expiry, queue limit and name matching)
- `src/platform/linux/sway/mod.rs` and `src/platform/linux/x11/interception.rs` (the placement step)
