---
title: "Quick start"
---

These examples assume spawn-at is installed on GNOME with Wayland, the one tested setup. On any
other desktop, see [Environments](/spawn-at/environments/) first.

## Open an app in a place

```bash
spawn-at spawn --anchor top-right --size 700 450 -m 24 gnome-terminal
```

`--anchor top-right` puts the window against the top-right corner of the work area (the screen
minus the top bar and docks). `-m 24` keeps it 24 pixels in from the edges. `--size 700 450` is the
window size. Everything after the options is the command to run.

More examples, from the README:

```bash
# Calculator popup under the mouse
spawn-at spawn --anchor cursor --pivot center --size 400 500 gnome-calculator

# Scratchpad terminal pinned to the top-right
spawn-at spawn --anchor top-right --pivot top-right --size 800 500 -m 20 alacritty

# Dashboard in the corner (`--no-focus` is accepted on `spawn` but currently has no effect)
spawn-at spawn --anchor bottom-right --size 600 350 -m 16 --no-focus btop
```

The README suggests binding any of these to a GNOME custom shortcut. That was not tried for this
book.

## Move a window that is already open

```bash
spawn-at transform --class org.gnome.Calculator --anchor center --size 400 500
```

`transform` picks the window with `--class`, `--title`, `--pid`, `--focused`, or `--id`. To find an
id or a class, list the windows:

```bash
spawn-at query windows
spawn-at query windows --json
```

`GEOMETRY` in that list is in global screen coordinates.

## See the screens

```bash
spawn-at query layout
```

prints the monitors, their work areas and insets.

## Change how long it waits

```bash
spawn-at config                    # every setting, its value, and where the value comes from
spawn-at config set wait.cap_ms 400
```

See [Configuration](/spawn-at/configuration/). With no config file and no flags, the defaults apply.

## If it prints an error about the desktop

On a Wayland desktop that is not GNOME, spawn-at stops with an error and says so. See
[Unsupported desktops](/spawn-at/environments/unsupported-desktops/).

## Sources

- `README.md` ("What it looks like", "Things I use it for", "CLI reference")
- the output of `spawn-at query windows --help`, `spawn-at query layout --help` and
  `spawn-at transform --help`
- `docs/behavior-notes.md`, section 5 (`query windows` coordinates)
