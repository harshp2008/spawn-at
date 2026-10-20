# Behavior Notes: Coordinate Model, Reference Areas, and Monitor Defaults

Status: reference. Checked on 2026-10-20 against `src/`, `crates/spawn-at-core/`, the GNOME extension
and `--help`. The tests it cites exist. Nothing here was run on a live session unless a line says so.

How spawn-at turns the placement options into a window rectangle, which monitor it uses when you do
not name one, and when the extension animates a window.

## 1. Reference areas

The `--area` option picks the space that positions, margins and clamping refer to.

| `--area` | Coordinate space | `-p` / `-o` is relative to | Clamped | Diagnostic wording |
| :-- | :-- | :-- | :-- | :-- |
| `workarea` (default) | The target monitor's usable area, without panels, docks and top bars | The top-left corner of that area | Yes | "in the work area" |
| `screen` | The whole target monitor | The top-left corner of the monitor | Yes | "on the screen" |
| `unbounded` | The whole multi-monitor desktop, as in `GEOMETRY` of `query windows` | The desktop origin (0, 0) | Never; no margins | "on the desktop" |

With `--clamp true` (the default), the window's top-left corner is kept inside the area. The lowest
allowed value is the area's top-left corner moved in by the left and top margins. The highest is the
bottom-right corner moved in by the right and bottom margins, less the window's own size. The window
is never resized.

A clamped request is reported like this:

```text
Requested (0, 0) in the work area; clamped to (16, 16) [screen (16, 56)]
```

## 2. Position flags and anchors

### Without an anchor

- The origin is the area's raw top-left corner.
- **Margins are not added to the requested position.** They only set the clamp bounds.
- The default pivot is `top-left`.

Example: a work area of 1920x1040 at (0, 40) and a margin of 16.

| Flags | Screen position |
| :-- | :-- |
| `-p 700 300` | (700, 340) |
| `-p 0 0` | (16, 56): the request (0, 0) is clamped to the margin |

### With an anchor

- The anchor point already includes the directional margins. `top-left` is the area's corner moved in
  by the left and top margins.
- `-p` / `-o` is an offset from that point.
- The pivot defaults to the anchor's own alignment: `center` gives `center`, `bottom-right` gives
  `bottom-right`, and `cursor` gives `top-left`.

## 3. Unbounded mode

Unbounded mode gives raw coordinates across the whole desktop.

Allowed options: `-p` / `-o`, `--pivot`, `-s` / `--size`, and `--anchor cursor`.

Every other option that bounds the window is refused before anything runs:

| Option | Error |
| :-- | :-- |
| Any other `--anchor` | `Cannot specify --anchor (except --anchor cursor) with --area unbounded.` |
| `--clamp` | `Cannot specify --clamp with --area unbounded: unbounded mode is never clamped.` |
| Margins | `Cannot specify margins (-m, --mt, --mb, --ml, --mr) with --area unbounded: unbounded mode has no margins.` |
| `--monitor` | `Cannot specify --monitor with --area unbounded: unbounded mode uses global desktop coordinates.` |

### On the wire

The instruction sent to the extension has no `area`, zero margins and `clamp: false`. The extension's
anchor code then skips its bounds check and uses the raw coordinates.

### Straddling monitors

If the final rectangle overlaps two or more monitors, `spawn` and `transform` print the INFO line
`Window straddles monitors.` They never move or snap the window.

### Mutter keeps part of the window visible

If an unbounded coordinate puts a window almost entirely off-screen, Mutter moves it back so a small
part stays visible. The CLI reports the real final position and says the compositor adjusted it:

```text
INFO: Window at (-292, 100) [367x514]; compositor adjusted position from requested (-320, 100) to maintain visibility.
```

This was reported as observed on GNOME Shell 46, Wayland, one monitor, but **the repository holds no
log of that run**. In the example above 75 px stay visible (the window is 367 px wide and ends at
x = 75). Whether 75 px is Mutter's fixed limit is not verified. X11 and other GNOME versions are
unknown.

## 4. Monitor choice without `--monitor`

| Command | Monitor used |
| :-- | :-- |
| `spawn` | The monitor flagged primary. If none is flagged, monitor index 0, and a `DEBUG` line on stderr: `No monitor flagged primary; falling back to monitor index 0.` |
| `transform` | The monitor with the largest pixel overlap with the window. If the window overlaps none, the monitor at the window's own monitor index, else the first one. |

`--monitor cursor` picks the monitor under the pointer, and falls back to the primary monitor when
the pointer is outside all monitors. **`--anchor cursor` alone does not change the monitor**: with a
readable layout, `spawn` still clamps to the primary monitor's area. This is read from the code, not
run.

## 5. `query windows` and the round trip

`GEOMETRY` is always the global screen position. `REL` depends on `--area`:

| `--area` | `REL` |
| :-- | :-- |
| `workarea` (default) | Position relative to the work area of the window's monitor |
| `screen` | Position relative to the whole monitor |
| `unbounded` | The same as `GEOMETRY` |

The JSON has `x`, `y`, `rel` (with `x` and `y`) and `area` for each window. A window with no monitor
gets `null` for `rel` and `area`.

Passing the `REL` values of a window that lies inside its area back to `transform` reproduces its
position, in all three modes:

| Mode | Steps |
| :-- | :-- |
| Work area | `query windows`, then `transform -p X Y` |
| Screen | `query windows --area screen`, then `transform --area screen -p X Y` |
| Unbounded | `query windows --area unbounded`, then `transform --area unbounded -p X Y` |

The test uses one window at (700, 300) on one monitor. A window partly outside its area would be
clamped by `transform`.

## 6. Window animation per command

The GNOME extension keeps the desktop's own animations where it can and suppresses them where a
glide would look like a flicker. This is read from the extension code; the animations were **not
observed on screen**.

| Command | Animation | Reason |
| :-- | :-- | :-- |
| `minimize` | Normal (the window shrinks to the dock) | A plain minimize, with no effects skipped |
| `restore`, `unminimize` | Normal (the window grows back) | A plain unminimize |
| `transform` on a minimized window | Suppressed: the window appears at its target | Effects are skipped around the unminimize |
| `transform` on a covered window | Normal focus transition | The window is raised and focused while cloaked |
| `transform -s` | Normal resize transition | Effects are not skipped |

To suppress the animation, the extension adds the window's actor to the window manager's
skipped-effects set and resets its transitions, scale and pivot. The entry is removed right after the
unminimize. It is also cleared when a batch fails and when the extension is disabled, so the set never
keeps an actor for a later command.

### Logging does not change the animation

The actor operations are the same whether telemetry logging is on or off. The trace calls only read
properties. They never change transitions, scale, opacity or the skipped-effects set.

## 7. Where each rule is tested

| Rule | Test |
| :-- | :-- |
| Options refused with `--area unbounded` | `src/cli/tests.rs`, `test_unbounded_allowed_and_rejected_options` |
| Unbounded instruction on the wire | `src/platform/linux/gnome/mechanics.rs`, `test_golden_transform_batch_unbounded_instructions`; `tests/dispatcher_test.js` (the golden unbounded pipeline test) |
| Straddling check | `crates/spawn-at-core/src/geometry.rs`, `test_window_straddles_monitors_helper` |
| Compositor adjustment message | `src/diagnostics.rs`, `test_diagnostic_unbounded_compositor_adjustment` |
| `spawn` monitor default | `crates/spawn-at-core/src/geometry.rs`, `test_resolve_monitor_from_layout_two_monitors_primary_not_index_zero` and `test_resolve_monitor_from_layout_nothing_flagged_primary_falls_back` |
| `transform` monitor default | `crates/spawn-at-core/src/geometry.rs`, `test_find_monitor_with_largest_overlap` |
| `query windows` JSON fields | `src/commands/query.rs`, `test_characterization_query_windows_json_schema` |
| Round trip in all three modes | `src/commands/query.rs`, `test_query_windows_round_trip_all_area_modes`; `crates/spawn-at-verify/tests/oracle_cross_check.rs`, `test_oracle_vs_core_explicit_pos_all_three_area_modes_cross_check` |
| Logging does not change actor operations | `tests/dispatcher_test.js` and `tests/legacy_dispatcher_test.js` (the tests that compare actor operations with logging on and off) |
