---
title: "query"
---

```text
spawn-at query layout
spawn-at query pointer
spawn-at query windows
```

Read-only. Nothing is moved.

| Subcommand | Prints |
| :--- | :--- |
| `layout` | Monitors, work areas and insets. `--json` prints JSON instead of a table |
| `pointer` | The pointer coordinates |
| `windows` | The open windows with metadata. `--json` prints JSON; `--area` chooses the space of the `REL` column |

## query windows

`GEOMETRY` is always the window's position in global screen coordinates. A maximized window sits at
the work area's origin.

`REL` is the position relative to a reference area:

| `--area` | `REL` |
| :--- | :--- |
| `workarea` (default) | Relative to the work area of the window's monitor |
| `screen` | Relative to the full monitor rectangle |
| `unbounded` | Repeats `GEOMETRY` |

The JSON form has `x`, `y`, `rel` (with `x` and `y`) and `area`. Reading `REL` and giving the values
to `transform -p X Y` (with the same `--area`) puts the window back at the same screen position, in
all three modes.

The `id` it prints is what `--id` takes on the other commands.

## Backends

- **GNOME Wayland:** all three.
- **X11:** all three, untested beyond the project's test window (no test touches an X server for the
  query commands).
- **Sway:** `layout` and `windows` work, and the repo's live script compares them with `swaymsg`.
  `pointer` is unsupported because Sway has no pointer query. `--monitor` counts outputs in the
  order Sway lists them. In `windows`, `state` is `normal` or `fullscreen`, and a window in the
  scratchpad is listed as minimized.

## Sources

- `README.md` (CLI reference table)
- `docs/behavior-notes.md`, section 5
- `docs/TEST_COVERAGE_AUDIT.md`, sections 2 and 3.3
- `docs/DESIGN_SWAY.md`, sections 3 and 12
- `docs/TEST_PLAN.md`, section 7.2
- the output of `spawn-at query windows --help` and `spawn-at query layout --help`
