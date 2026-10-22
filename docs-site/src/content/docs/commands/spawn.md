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
  `org.gnome.TextEditor`, or `*`), used to recognise the new window. The match is loose, not
  exact; see [Known limits](/spawn-at/known-limits/).
- **`--json`** prints one structured result object on stdout. Diagnostics stay on stderr. The
  result includes how the wait for the app ended (for example `resize.outcome`). When no window was
  found, `window_id` is `null` and the position and size are `0`. That is the case when the window
  never appears, when `--no-wait` is given, and when the placement could not be armed (the program
  still starts, unplaced). With `--no-wait` and a placement that could not be armed, nothing is
  printed.
- **`--no-wait`** skips the wait for the new window to appear and settle. The backend's own
  placement step still runs first: on Sway it waits up to 5 seconds for the window, and on X11 up to
  5 seconds to find it plus up to 3 seconds for it to map. On GNOME that step does nothing.
- **Slow start.** If the window does not appear in time, spawn warns
  `WARNING: ... did not appear within 2s`. The limit is the setting `spawn.window_timeout_ms`; raise
  it for heavy programs that start cold. See [Configuration](/spawn-at/configuration/).
- **Exit status.** `spawn` exits 0 when the program started, whether or not a window appeared or
  was placed, on GNOME, Sway and X11. It exits non-zero only for an empty command, invalid options,
  `--area screen` without the helper it needs, or a program that cannot be started. This is read
  from the code, not run live.
- **`--focus`, `--no-focus` and `--defocus` are accepted on `spawn` and do nothing.** The code
  never reads them for `spawn`: it always asks for exclusive focus (`src/commands/spawn.rs:315`,
  `FocusIntent::Exclusive`; the test `test_spawn_focus_modifiers_have_no_effect_today` at
  `spawn.rs:951` pins it). A test pins this behaviour as it is today; it is not a promise.
  `--help` still describes the three flags as if they worked, and the generated
  [reference page for `spawn`](/spawn-at/reference/spawn-at-spawn/) is the unchanged `--help`
  output, so it shows that wording (for example "Passive: do not alter focus state; leave active
  input where it currently is" for `--no-focus`). It differs from the verified behavior above.
- **The size is a request.** A program with a larger minimum size ends up larger, and is positioned
  with the size it ended up with. See [Concepts](/spawn-at/concepts/).
- **Only the first window that appears** for a launch is placed on Sway; the Sway placement step
  handles the first entry of the request, and `spawn` sends one entry.
- Two hidden diagnostic flags exist, `--wait-strategy` and `--wait-cap-ms`. They are not listed in
  `--help`; use the settings in [Configuration](/spawn-at/configuration/), which also describes
  the two flags ([hidden flags](/spawn-at/configuration/#hidden-flags-for-one-run)).

## Sources

- `README.md` ("Things I use it for", "Sway (experimental)", CLI reference table)
- `verify/placement/bottom-right.toml`
- `docs/TEST_COVERAGE_AUDIT.md`, section 7 gaps 5 and its updates (focus modifiers have no effect on `spawn`)
- `docs/DESIGN_SWAY.md`, section 12 (`--no-wait`, first entry of a batch)
- `docs/CONFIG.md` (`spawn.window_timeout_ms`, hidden flags)
- `docs/TEST_PLAN.md` section 7.2 (`spawn --json` `window_id` on Sway)
- `src/commands/spawn.rs` (`run_spawn_with`: exit status and `--json` output), `src/platform/linux/x11/interception.rs`
  and `src/platform/linux/sway/mod.rs` (`post_spawn`), read, not run
- the output of `spawn-at spawn --help`
