---
title: "spawn"
---

```text
spawn-at spawn [OPTIONS] <COMMAND>...
```

Tells the compositor where the next window goes, starts the program, and places the window that
program opens. Everything after the options is the command to run, with its own arguments.

## Examples

From the README:

```bash
# Top-right of the work area, 24 px in, 700x450
spawn-at spawn --anchor top-right --size 700 450 -m 24 gnome-terminal

# Calculator popup under the mouse
spawn-at spawn --anchor cursor --pivot center --size 400 500 gnome-calculator

# Scratchpad terminal pinned to the top-right
spawn-at spawn --anchor top-right --pivot top-right --size 800 500 -m 20 alacritty
```

From the test catalog (`verify/placement/bottom-right.toml`), which runs the project's test window:

```bash
spawn-at spawn --class verify.placement.bottomright --anchor bottom-right --size 400 300 --margin 16 <test window>
```

On Sway, set the backend and put `--` before the program:

```bash
SPAWN_AT_BACKEND=sway spawn-at spawn --anchor top-right -m 24 --size 600 400 -- foot
```

## Notes

- **`--class`** is an explicit Wayland application id or `WM_CLASS` override (for example
  `org.gnome.TextEditor`, or `*`), used to recognise the new window.
- **`--json`** prints one structured result object on stdout. Diagnostics stay on stderr. The
  result includes how the wait for the app ended (for example `resize.outcome`).
- **`--no-wait`** exits without waiting for the compositor. On Sway it still waits for the
  placement, because that step runs in every mode.
- **Slow start.** If the window does not appear in time, spawn warns
  `WARNING: ... did not appear within 2s`. The limit is the setting `spawn.window_timeout_ms`; raise
  it for heavy programs that start cold. See [Configuration](/spawn-at/configuration/).
- **`--focus`, `--no-focus` and `--defocus` are accepted on `spawn` and do nothing.** The code
  never reads them for `spawn`: it always asks for exclusive focus. `--help` still describes them.
  A test pins this behaviour as it is today; it is not a promise.
- **The size is a request.** A program with a larger minimum size ends up larger, and is positioned
  with the size it ended up with. See [Concepts](/spawn-at/concepts/).
- **Only the first window of a launch** is placed on Sway.
- Hidden flags for the wait rule exist (`--wait-strategy`, a cap in milliseconds). They are not
  listed in `--help`; use the settings in [Configuration](/spawn-at/configuration/).

## Sources

- `README.md` ("Things I use it for", "Sway (experimental)", CLI reference table)
- `verify/placement/bottom-right.toml`
- `docs/TEST_COVERAGE_AUDIT.md`, section 7 gaps 5 and its updates (focus modifiers have no effect on `spawn`)
- `docs/DESIGN_SWAY.md`, section 12 (`--no-wait`, first entry of a batch)
- `docs/CONFIG.md` (`spawn.window_timeout_ms`, hidden flags)
- `docs/TEST_PLAN.md` section 7.2 (`spawn --json` `window_id` on Sway)
- the output of `spawn-at spawn --help`
