# Behavior Notes: Coordinate Model, Reference Areas, and Monitor Defaults

This document describes the placement coordinate model, reference areas, unbounded mode rules, and monitor selection defaults implemented in `spawn-at` v0.2.0.

---

## 1. Coordinate Model and Reference Areas

In `spawn-at`, coordinates are evaluated relative to a chosen reference space:

$$\text{Final Window Rect} = f(\text{Area}, \text{Anchor}, \text{Pivot}, \text{Offset/Pos}, \text{Size}, \text{Margins}, \text{Clamp})$$

The `--area` option selects the coordinate space:

### `--area workarea` (Default)
- **Coordinate Space**: The usable work area of the target monitor, excluding panels, docks, and top bars (e.g. Mutter `get_work_area_for_monitor`).
- **Position (`-p` / `-o`)**: Relative to the raw top-left origin `(wa.x, wa.y)` of the target monitor's work area. No margins are added to the requested position.
- **Clamping**: By default (`--clamp true`), the resulting window is clamped inside `[wa.x + ml, wa.y + mt]` to `[wa.x + wa.w - mr - win_w, wa.y + wa.h - mb - win_h]`.
- **Diagnostics**: Reported as `"in the work area"` with corresponding screen coordinates in brackets (e.g., `Requested (0, 0) in the work area; clamped to (16, 16) [screen (16, 56)]`).

### `--area screen`
- **Coordinate Space**: The full physical boundary of the target monitor `(screen.x, screen.y, screen.w, screen.h)`.
- **Position (`-p` / `-o`)**: Relative to the target monitor's raw top-left corner `(screen.x, screen.y)`.
- **Clamping**: By default (`--clamp true`), the resulting window is clamped inside the physical screen minus margins `[screen.x + ml, screen.y + mt]` to `[screen.x + screen.w - mr - win_w, screen.y + screen.h - mb - win_h]`.
- **Diagnostics**: Reported as `"on the screen"` with screen coordinates.

### `--area unbounded`
- **Coordinate Space**: The global multi-monitor desktop layout (the exact same global coordinates as reported by `GEOMETRY` in `spawn-at query windows`).
- **Position (`-p` / `-o`)**: Absolute desktop coordinates `(X, Y)`.
- **Clamping & Margins**: Disabled completely. No margin, no border, and no clamp are applied by `spawn-at`.
- **Diagnostics**: Reported as `"on the desktop"`.

---

## 2. `-p` / `-o` vs. Anchors

### Without `--anchor`
- The reference origin is the chosen area's **raw top-left corner** (`wa.x, wa.y` for workarea, `screen.x, screen.y` for screen, `(0, 0)` for unbounded).
- Margins (`-m`, `--mt`, etc.) are **not** added to the requested position. They only define the bounds if clamping occurs.
- The default pivot is `top-left`.
- Example: With work area `1920x1040@(0,40)` and margin 16:
  - `-p 700 300` &rarr; screen position `(700, 340)`.
  - `-p 0 0` &rarr; requested `(0, 0)` in work area; clamped to margin 16 &rarr; screen position `(16, 56)`.

### With `--anchor`
- The anchor point itself accounts for directional margins (e.g., `top-left` is `(wa.x + ml, wa.y + mt)`).
- `-p` / `-o` acts as an offset delta relative to that anchored point.
- The pivot defaults to the anchor's natural alignment (e.g., `center` &rarr; `center`, `bottom-right` &rarr; `bottom-right`, `cursor` &rarr; `top-left`).
- Existing anchor test vectors remain completely preserved and backward-compatible.

---

## 3. Unbounded Mode Rules and Validation

Unbounded mode provides raw coordinate access across the multi-monitor desktop.

### Allowed Arguments
- `-p, --pos <X> <Y>` / `-o, --offset <X> <Y>`
- `--pivot <PIVOT>`
- `-s, --size <W> <H>`
- `--anchor cursor` (places relative to cursor coordinates)

### Disallowed Arguments (Immediate Validation Errors)
- Any other `--anchor` (e.g. `--anchor center`, `--anchor top-left`):
  `"Cannot specify --anchor (except --anchor cursor) with --area unbounded."`
- `--clamp`:
  `"Cannot specify --clamp with --area unbounded: unbounded mode is never clamped."`
- Margins (`-m`, `--margin-top`, `--margin-bottom`, `--margin-left`, `--margin-right`):
  `"Cannot specify margins (-m, --mt, --mb, --ml, --mr) with --area unbounded: unbounded mode has no margins."`
- `--monitor`:
  `"Cannot specify --monitor with --area unbounded: unbounded mode uses global desktop coordinates."`
*(Pinned by: `src/cli/tests.rs::test_unbounded_allowed_and_rejected_options`)*

### Batch Dispatch and Extension Parity
- Unbounded mode is expressed on the wire (D-Bus instruction payload) as `area` omitted (`None`) plus `clamp: false`. This avoids unknown-area validation rejections in existing extensions while ensuring `anchor.js` completely bypasses bounds checking and margin clamping (`x = rawX`, `y = rawY`).
*(Pinned by: Rust golden test `src/platform/linux/gnome/mechanics.rs::test_golden_transform_batch_unbounded_instructions` and Node dispatcher test `tests/dispatcher_test.js::"dispatcher: executes Rust golden unbounded transform instruction pipeline without clamp"`)*

### Straddling Monitors
When a window's target geometry spans multiple monitors in unbounded mode:
- `spawn-at` prints an `INFO` diagnostic indicating that the window straddles monitors (naming the intersecting monitors).
- Coordinates are never altered or snapped.
*(Pinned by: `crates/spawn-at-core/src/geometry.rs::test_window_straddles_monitors_helper`)*

### Mutter Compositor Boundaries
- Mutter enforces an internal minimum visibility constraint (approximately 75px of the window surface must remain on an active monitor). If an unbounded coordinate places a window almost entirely off-screen, Mutter snaps the window back just enough to maintain this minimum visible handle.
- **Observed on**: GNOME Shell 46, Wayland, single monitor.
- **Other environments**: X11 and other GNOME/Mutter versions are unknown.
- The CLI reports the real final position and explains that the compositor adjusted it (`INFO: Window at (-292, 100) [367x514]; compositor adjusted position from requested (-320, 100) to maintain visibility.`).
*(Pinned by: `src/diagnostics.rs::test_diagnostic_unbounded_compositor_adjustment`)*

---

## 4. Monitor Selection Defaults

When `--monitor` is omitted:

1. **`spawn` Command**:
   - Defaults to the primary monitor (the monitor flagged `primary: true` in `MonitorLayout`).
   - If no monitor is flagged primary, falls back to index `0` and emits an `INFO` diagnostic message: `"No monitor flagged as primary; falling back to index 0."`.
   - If `--anchor cursor` is specified, uses the monitor containing the current cursor position.
*(Pinned by: `crates/spawn-at-core/src/geometry.rs::test_resolve_monitor_from_layout_two_monitors_primary_not_index_zero` and `crates/spawn-at-core/src/geometry.rs::test_resolve_monitor_from_layout_nothing_flagged_primary_falls_back`)*

2. **`transform` Command**:
   - Defaults to the window's **current monitor**—specifically the monitor with the largest pixel overlap area with the window's current rectangle.
   - If the window does not overlap any monitor (e.g. positioned off-screen), falls back to the primary monitor.
*(Pinned by: `crates/spawn-at-core/src/geometry.rs::test_find_monitor_with_largest_overlap`)*

---

## 5. Query Windows and Round-Trip Equivalence

In `spawn-at query windows`:
- `GEOMETRY` always displays global screen coordinates `(X, Y)`.
- `REL` displays the position relative to the window's current monitor's work area by default.
- `query windows --area screen`: `REL` displays the position relative to the window's current monitor's full screen rectangle.
- `query windows --area unbounded`: `REL` repeats the `GEOMETRY` desktop coordinates.
- JSON output includes `x`, `y`, `rel: { x, y }`, and `"area"`.
*(Pinned by: `src/commands/query.rs::test_query_windows_json_rel_and_area_fields` and `src/commands/query.rs::test_query_windows_unbounded_repeats_geometry`)*

### Round-Trip Guarantee
Reading the `REL` values from `query windows` and passing them to `transform -p X Y` reproduces the exact same screen position:
1. **Workarea Mode**: `query windows` (REL `(X, Y)`) &rarr; `transform -p X Y` &rarr; same position.
2. **Screen Mode**: `query windows --area screen` (REL `(X, Y)`) &rarr; `transform --area screen -p X Y` &rarr; same position.
3. **Unbounded Mode**: `query windows --area unbounded` (REL / GEOMETRY `(X, Y)`) &rarr; `transform --area unbounded -p X Y` &rarr; same position.
*(Pinned by: `src/commands/query.rs::test_query_windows_round_trip_all_area_modes` and `crates/spawn-at-verify/tests/oracle_cross_check.rs::test_oracle_vs_core_explicit_pos_all_three_area_modes_cross_check`)*

---

## 6. Window Animation Behavior per Command

The GNOME Shell extension manages window transitions to balance native desktop feel with flicker-free programmatic placement:

| Command / Operation | Animation Behavior | Code Mechanism & Rationale |
| :--- | :--- | :--- |
| `spawn-at minimize` | **Plays normal animation** (shrinks into dock / panel) | Invokes `MetaWindow.minimize()` without adding the actor to the window manager's skipped-effects set (`Main.wm.skipNextEffect` / `_skippedActors`), allowing Clutter/Mutter animation transitions to run smoothly. |
| `spawn-at restore` / `unminimize` | **Plays normal animation** (zooms back from dock) | Invokes `MetaWindow.unminimize()` without animation suppression. |
| `spawn-at transform` on minimized window | **Animation deliberately suppressed** (no jump / dock glide) | Deliberately calls `_skipWindowEffects(ctx.actor)` prior to `unminimize()`. This adds the actor to `Main.wm._skippedActors` and resets transitions/scale/pivot, ensuring the window appears directly at the newly computed target geometry without visual gliding. The skip entry is consumed on unminimize, ensuring `_skippedActors` is not poisoned for subsequent commands. |
| `spawn-at transform` on covered window | **Plays normal focus transitions** | Covered windows are raised and focused while cloaked (`_focusWindowObject`) without suppressing animations. |
| Window resizes (`SetSize` / `transform -s`) | **Plays normal resize transitions** | Normal resizing animation behavior is preserved; `_skipWindowEffects` is not invoked. |

### Logging Parity Invariant
- Actor operations and window animations are **strictly identical** whether telemetry logging is enabled (`SetLogging(true)`) or disabled (`SetLogging(false)`).
- Trace hooks (`_logActorTrace`, `logTime`) are strictly read-only observers that inspect properties without altering transitions, scale, opacity, or the compositor's skipped-effects set.
*(Pinned by: Node parity tests `tests/dispatcher_test.js::"dispatcher (modern): actor operation sequences are identical with logging on and off across all scenarios"` and `tests/legacy_dispatcher_test.js::"dispatcher (legacy): actor operation sequences are identical with logging on and off across all scenarios"`)*

