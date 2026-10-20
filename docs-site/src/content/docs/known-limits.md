---
title: "Known limits"
---

How spawn-at behaves in cases a user can trip over, stated as the code does them. Everything here was
read from the source and not run on a live session. What is broken, missing or unmeasured in the
project as a whole is on [Known shortcomings](/spawn-at/shortcomings/).

## Exit status and results

### spawn exits 0 when no window appears

If the program starts but no window shows up within `spawn.window_timeout_ms`, `spawn` prints a
warning and exits 0. The default is 2000 ms. This is the same on GNOME, Sway and X11. On GNOME the
wait can last twice as long: first for the extension's claim, then by polling.

`spawn` exits non-zero only for an empty command, invalid options, `--area screen` without the
helper it needs, or a program that cannot be started.

### spawn --json can print a result with no window

`window_id` is `null` and the position and size are 0 in three cases: the window never appeared,
`--no-wait` was given, or the placement could not be armed (the program still starts, unplaced).
With `--no-wait` and a placement that could not be armed, nothing is printed.

### State commands exit 0 when the state does not change

`focus`, `defocus`, `maximize`, `minimize`, `restore` and `transform` wait up to 2 seconds for the new
state, ignore the outcome and exit 0. `--no-wait` skips the wait. A test pins this behaviour.

`defocus` prints `Defocused window ...` whenever it sent the request, even after a timed-out wait and
with `--no-wait`. If the window is not focused, it prints a note and sends nothing.

### close is the exception

`close` polls for up to 1500 ms and fails with `did not close within ...` if the window is still
listed. A window list that cannot be read counts as closed. `--no-wait` skips the check.

### A failed gnome-extensions call is a note or warning

| Command | If `gnome-extensions` fails | Exit |
| :-- | :-- | :-- |
| `install` | After a non-zero `enable`, prints a note instead of "successfully installed and enabled". The files are deployed, the extension is not confirmed enabled. | 0 |
| `uninstall` | If `disable` cannot start or exits non-zero, prints a warning instead of "successfully uninstalled", carries on, and says whether the files were removed. | 0 |

`install` stops with an error when a file cannot be written or `gnome-extensions` cannot be started.
`uninstall` stops with an error when the extension directory cannot be deleted.

## Matching a window by name

There is no single matching rule, and none of the rules is strict. Each of the four places below has
its own.

### Selectors for existing windows

`-c`, `-t`, `--pid` and `--focused` must all hold together. `--id` is strict: when it is given, only
the id counts, and a missing id is an error.

- `-c` matches the whole class or its last dot-separated part, ignoring case.
- `-t` matches a substring of the title, ignoring case.

If more than one window matches, the command fails and lists the candidates.

### The name spawn waits for

`spawn --class`, or the name `spawn` derives from the command, recognises the new window. The match
ignores case and holds in any of these cases:

| Rule | Example |
| :-- | :-- |
| The name equals the whole class. | |
| The name equals the last dot-separated part of the class, or the reverse. | `org.gnome.Calculator` and `calculator` |
| Reverse-DNS tails joined with `-` or nothing. | `org.gnome.Terminal` and `gnome-terminal` |
| Equal once a `-server`, `.server`, `-daemon`, `.daemon` or `-service` suffix is removed from either side. | `gnome-terminal-server` and `gnome-terminal` |
| The name appears anywhere in the window title. | |

So a short name matches any new window whose title contains it. An empty name or `*` matches any new
window.

When no new window qualifies, `spawn` accepts a window that was already open if its process is the
launched one or a descendant of it.

### State waits

After `focus`, `maximize` and the like, the command finds the window again from the resolved target
with a looser rule. It tries, in order: `pid:class`, a window id, a process id (or an ancestor of the
window's process), then the name rule that `spawn` uses. A number is tried as a window id and as a
process id, so `1234` can match window 1234 and process 1234.

### The GNOME extension

The extension matches by itself. It compares the registered name with several identifiers of each
new window, ignoring case. It is a match when the two are equal or when either contains the other.

## Geometry: what "reached" means

| Check | Counts as reached when |
| :-- | :-- |
| Position | x and y are each within 1 px of the position computed for the size the window actually has. |
| Size | Width and height are each at least the requested values. A larger window passes. |
| `spawn` wait | The geometry is read for up to 300 ms, every 25 ms. |
| `transform` wait | x, y, width and height are each within 4 px. A timeout is ignored. |

If the target is not reached in time, `spawn` uses and reports the last rectangle it read, without an
error.

A window that maximized itself after `spawn` placed it gets one restore and the same placement again,
once, when a size, position or anchor was requested.

## After a failed spawn

On GNOME, `spawn` registers the placement with the extension before the program starts. On Linux the
disarm call is never sent, not even when the program fails to start. The registration ends on its
own:

| Registered under | Expires after |
| :-- | :-- |
| A name | 15 seconds |
| No name (`--class '*'` or an empty name) | 1.2 seconds |

Until then, an unrelated window can be placed. A window that the extension matches to the name (see
above) and that opens in that time gets the placement meant for the program that failed. With the
1.2 second wildcard, the next window that opens is taken.

Registrations under one name queue up to 16. A 17th drops the oldest.

## Sway

- `--anchor cursor` and `--monitor cursor` in `spawn` use the pointer position (0, 0) and print no
  message, because Sway has no pointer query. `transform` with either option, and `query pointer`,
  return an error instead.
- Only the first entry of a request is placed. `spawn` sends one entry, so the first window that
  appears is the one placed. Later windows of the same program are left alone.
- `--no-wait` still waits up to 5 seconds for the window, because the placement step runs in every
  mode.
- If no window of the program appears in that time, `spawn` warns that the program was started but
  not placed, and still exits 0.

## X11

The X11 backend does nothing when `spawn` registers the placement. It only returns the launch
environment: `XDG_ACTIVATION_TOKEN` and `DESKTOP_STARTUP_ID`, both set to the entry key.

The window is found and moved afterwards. That step waits up to 5 seconds to find the window and up
to 3 seconds for it to map, also with `--no-wait`. It ignores errors from the X11 requests. If no
window is found it prints a message only when `SPAWN_AT_DEBUG=1`, and `spawn` still exits 0.

## Not verified

None of the above was run on a live GNOME, Sway or X11 session. The statements come from reading the
CLI source, the GNOME extension source and the Sway and X11 backends. Other GNOME versions, real Sway
sessions and real X11 applications are listed on [Known shortcomings](/spawn-at/shortcomings/).

## Sources

- `src/commands/spawn.rs`, `src/commands/focus.rs`, `src/commands/lifecycle.rs`,
  `src/commands/transform.rs` and `src/commands/mod.rs` (exit status, waits, the matching rules,
  the geometry loop)
- `src/target.rs` (selectors for existing windows)
- `src/main.rs` (exit codes for invalid options and errors)
- `src/diagnostics.rs` (the position and size report)
- `src/platform/linux/gnome/deploy.rs` (`install` and `uninstall` output)
- `src/platform/linux/mod.rs` (`disarm` is not forwarded) and `assets/gnome/modern/extension.js`
  (registration expiry, queue limit and name matching)
- `src/platform/linux/sway/mod.rs`, `src/platform/linux/sway/place.rs` and
  `src/platform/linux/x11/interception.rs` (the placement step)
- `src/platform/linux/x11/mod.rs` (the launch environment)
