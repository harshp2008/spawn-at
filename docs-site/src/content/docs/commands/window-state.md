---
title: "Focus and window state"
---

These commands act on a window that is already open. Each takes a window selector (`--id`,
`--class`, `--title`, `--pid`, `--focused`); see [Commands](/spawn-at/commands/).

| Command | What it does |
| :--- | :--- |
| `focus` | Raises and activates a window; unminimizes it if needed |
| `defocus` | Takes keyboard focus away from a window |
| `maximize` | Maximizes |
| `minimize` | Minimizes |
| `restore` | Unmaximizes and unminimizes (aliases: `unmaximize`, `unmaximise`, `unminimize`, `unminimise`) |
| `close` | Asks the window to close gracefully |

```bash
spawn-at focus --class org.gnome.Calculator
spawn-at maximize --focused
spawn-at close --id 4
```

:::caution
These act on real windows. `close --id 4` asks whichever window has id 4 to close. It is a request, not a
kill, but what the application does about unsaved work was not tested. The id `4` is only an example: take the id
from `spawn-at query windows`. `maximize --focused` changes whichever window has focus when you
press enter, which may be your terminal.
:::

## defocus

By default it hands focus to the most recently used other window. Two options choose the
destination: `--to-desktop` (alias `--desktop`) gives focus to the desktop or shell, and
`--to-window <TO_WINDOW>` (alias `--to`) gives it to a window named by title, class or id. The two
options cannot be combined. The aliases are not shown in `--help` (`src/cli/args.rs`, `DefocusArgs`). If the target window is not focused already, `defocus`
sends nothing.

## Focus options

`--focus`, `--no-focus` and `--defocus` control focus after the action on `transform`, `maximize`
and `restore`. On `spawn` they are accepted and ignored; see [spawn](/spawn-at/commands/spawn/).

## Animations on GNOME

- `minimize` and `restore` play the normal animation (shrink into the dock, zoom back).
- Resizing plays the normal resize transitions.
- A `transform` of a minimized window suppresses the glide; see [transform](/spawn-at/commands/transform/).

## Things to know

- **A window on another workspace is not found.** On GNOME, `focus`, `defocus`, the state commands
  and `close` look at the windows of the active workspace only. The extension answers "false" and the
  CLI reports that the compositor failed to do it. This is pinned by a node test, not fixed.
- **Success does not prove the state changed.** After the action, these commands wait up to 2
  seconds, polling every 50 ms, for the window to reach the new state. If it never does, they still
  exit 0. This is pinned by tests and is not fixed, because reporting it would change an exit code.
- **`close` asks, it does not force.** It needs extension protocol 3 on GNOME. The `close` tests that
  exist use a mock backend and a D-Bus stub; the live use is cleanup of the project's test windows.
- **Sway** supports `focus` and `close`. It does not support `defocus`, `maximize`, `minimize` or
  `restore`: they exit 1 with a "does not support this feature" error and send nothing.
- **Nothing here has been run live on GNOME.** The commands have unit, D-Bus stub and node tests.
  The test-catalog specs that exercise them on a real session exist but have not been run there.
  See [Known shortcomings](/spawn-at/shortcomings/).

## Sources

- `README.md` (CLI reference table)
- `docs/behavior-notes.md`, section 6 (animations)
- `docs/TEST_COVERAGE_AUDIT.md`, sections 2, 3.4 to 3.7 and gaps 1, 2, 6, 14 with their updates
- `docs/DESIGN_SWAY.md`, section 12
- the output of `spawn-at focus --help`, `spawn-at defocus --help` and `spawn-at restore --help`
