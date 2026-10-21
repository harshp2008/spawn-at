---
title: "Quick start"
---

Five steps: check the install, read the screen, open an app in a place, move a window that is
already open, and change a setting. They assume spawn-at is installed on GNOME with Wayland, the one
tested setup. On any other desktop, see [Environments](/spawn-at/environments/) first. If a step
does not do what it says, the last section points to the fix.

## 1. Check that it is installed

```bash
spawn-at --version
```

A release build prints `spawn-at <version>`, the release tag without its leading `v`. A build that
is not on a release tag prints `DEV BUILD <crate version>`, then the last release and a commit id
(the first line never starts with `spawn-at `, so a script that matches `spawn-at <version>` still
sees "not a release"). If the shell
says `command not found`, see
[the `PATH` entry](/spawn-at/troubleshooting/#spawn-at-command-not-found-after-install).

Only the `DEV BUILD` form was run, on a build that is not on a tag. The release form is read from
the code (see Sources) and pinned by a unit test, not run.

## 2. See the screen

```bash
spawn-at query layout
```

It only reads; nothing is moved. This is the output from a GNOME 46 Wayland session with one
monitor:

```text
$ spawn-at query layout
MON  NAME          PRIMARY  SCALE  SCREEN             WORKAREA           INSETS (T/R/B/L)
0    -             yes      1      1920x1080@0,0      1920x1040@0,40     40/0/0/0
```

Run by the maintainer on 2026-10-20 on GNOME 46 Wayland with a single monitor; other setups print
one row per monitor, so they print more rows.

The columns, one line each:

- `MON`: the monitor's index.
- `NAME`: the monitor's name; `-` when none is known, as here.
- `PRIMARY`: `yes` for the primary monitor, `no` otherwise.
- `SCALE`: the monitor's scale factor; `-` when unknown.
- `SCREEN`: the whole monitor as `width x height @ x,y`.
- `WORKAREA`: the usable part of the monitor, in the same form: the screen minus the top bar and
  docks. Here it starts 40 pixels down and is 40 pixels shorter than the screen.
- `INSETS (T/R/B/L)`: the pixels taken at the top, right, bottom and left edge, in that order.

A `-` in any of the last four columns means the value is not available. If the backend has a note
about the layout, it is printed after the table as `Note: ...`. How these numbers are used to place a
window is on [Concepts](/spawn-at/concepts/#the-coordinate-model).

## 3. Open an app in a place

```bash
spawn-at spawn --anchor top-right --size 700 450 -m 24 gnome-terminal
```

`--anchor top-right` puts the window against the top-right corner of the work area. `-m 24` keeps it
24 pixels in from the edges. `--size 700 450` is the window size. Everything after the options is
the command to run.

What you should see: the window opens in that corner at that size, and `spawn` prints one line on
stderr, `Window mapped at final size (700x450), anchor top-right in workarea (origin <x>, <y>) ->
screen position (<x>, <y>).`, with the window's real numbers. It names what you asked for and the
area it is counted from, then the absolute position on the screen. If the app ends up larger than asked, the line says so instead. This text is read from
the code (see Sources), not run.

To check it, list the windows and find the row of the new one:

```bash
spawn-at query windows
```

The table has the columns `ID`, `PID`, `CLASS`, `MON`, `STATE`, `GEOMETRY`, `REL`, `FOCUSED` (`*` for
the focused window) and `TITLE`. `GEOMETRY` is the window's size and position as `width x height @
x,y`, in the global coordinates described in [the coordinate model](/spawn-at/concepts/#the-coordinate-model).

If no window was placed, or it landed in the wrong place, see
[the warning entry](/spawn-at/troubleshooting/#spawn-printed-a-warning-and-exited-0-but-no-window-was-placed)
and [the wrong-size entry](/spawn-at/troubleshooting/#windows-appear-at-the-wrong-size-or-jump-just-after-they-appear).

More examples, from the README:

```bash
# Calculator popup under the mouse
spawn-at spawn --anchor cursor --pivot center --size 400 500 gnome-calculator

# Scratchpad terminal pinned to the top-right
spawn-at spawn --anchor top-right --pivot top-right --size 800 500 -m 20 alacritty

# Dashboard in the corner
spawn-at spawn --anchor bottom-right --size 600 350 -m 16 btop
```

`spawn` accepts `--focus`, `--no-focus` and `--defocus` and they have no effect; see
[spawn](/spawn-at/commands/spawn/). The README suggests binding any of these to a GNOME custom
shortcut. That was not tried for this guide.

## 4. Move a window that is already open

```bash
spawn-at transform --class org.gnome.Calculator --anchor center --size 400 500
```

`transform` picks the window with `--class`, `--title`, `--pid`, `--focused`, or `--id`. To find an
id or a class, run `spawn-at query windows` (or add `--json`).

What you should see: the window moves and resizes, and `transform` prints `Window transformed at
final size (<w>x<h>), anchor center in workarea (origin <x>, <y>) -> screen position (<x>, <y>).`
(read from the code, not run). To check it, run
`spawn-at query windows` again and compare the `GEOMETRY` of that row. If nothing changed but the
command exited 0, see
[the exit-0 entry](/spawn-at/troubleshooting/#focus-maximize-minimize-restore-transform-or-defocus-exited-0-but-nothing-changed).

## 5. Change how long it waits

```bash
spawn-at config                    # every setting, its value, and where the value comes from
spawn-at config set wait.cap_ms 400
```

What you should see, and the check, are on [Configuration](/spawn-at/configuration/#the-config-command):
the first command prints a table of the settings, and `config set` prints `wait.cap_ms = 400` and
the path of the file. With no config file and no flags, the defaults apply.

## If a step fails

- An error about the desktop: on a Wayland desktop that is not GNOME, spawn-at stops with an error
  and says so. See [Unsupported desktops](/spawn-at/environments/unsupported-desktops/).
- Anything else: [Troubleshooting](/spawn-at/troubleshooting/).

## Not verified

- No `spawn`, `transform` or `config` command was run for this page. Their result lines are read
  from the code. Only the `query layout` output was run, and by the maintainer, not while this page
  was written.
- Anything on a desktop other than GNOME 46 on Wayland with one monitor. See
  [Known shortcomings](/spawn-at/shortcomings/#not-verified-anywhere).

## Sources

- `README.md` ("What it looks like", "Things I use it for", "CLI reference")
- `src/commands/query.rs`: `run_query`, the `Layout` arm (lines 33-77) prints the table and the
  `Note:` line; the `Windows` arm (lines 82-207) prints the `query windows` table
- `src/update.rs`: `get_version_display` (lines 60-72) and `build.rs` (the `SPAWN_AT_VERSION` value)
- `src/diagnostics.rs`: `Diagnostic::PlacedSuccess`, `evaluate_placement_diagnostics_for_action`,
  `verify_and_report_placement` and `verify_and_report_transform`
- the output of `spawn-at --version`, `spawn-at query windows --help`, `spawn-at query layout --help`
  and `spawn-at transform --help`
- `docs/behavior-notes.md`, section 5 (`query windows` coordinates)
