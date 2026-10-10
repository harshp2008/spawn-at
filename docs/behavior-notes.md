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

### Straddling Monitors
When a window's target geometry spans multiple monitors in unbounded mode:
- `spawn-at` prints an `INFO` diagnostic indicating that the window straddles monitors (naming the intersecting monitors).
- Coordinates are never altered or snapped.

### Mutter Compositor Boundaries
- Mutter enforces an internal minimum visibility constraint (approximately 75px of the window surface must remain on an active monitor). If an unbounded coordinate places a window almost entirely off-screen, Mutter will snap the window back just enough to maintain this minimum visible handle.

---

## 4. Monitor Selection Defaults

When `--monitor` is omitted:

1. **`spawn` Command**:
   - Defaults to the primary monitor (index `0` or monitor flagged `is_primary`).
   - If `--anchor cursor` is specified, uses the monitor containing the current cursor position.

2. **`transform` Command**:
   - Defaults to the window's **current monitor**—specifically the monitor with the largest pixel overlap area with the window's current rectangle.
   - If the window does not overlap any monitor (e.g. positioned off-screen), falls back to the primary monitor.

---

## 5. Query Windows and Round-Trip Equivalence

In `spawn-at query windows`:
- `GEOMETRY` always displays global screen coordinates `(X, Y)`.
- `REL` displays the position relative to the window's current monitor's work area by default.
- `query windows --area screen`: `REL` displays the position relative to the window's current monitor's full screen rectangle.
- `query windows --area unbounded`: `REL` repeats the `GEOMETRY` desktop coordinates.
- JSON output includes `x`, `y`, `rel: { x, y }`, and `"area"`.

### Round-Trip Guarantee
Reading the `REL` values from `query windows` and passing them to `transform -p X Y` reproduces the exact same screen position:
1. **Workarea Mode**: `query windows` (REL `(X, Y)`) &rarr; `transform -p X Y` &rarr; same position.
2. **Screen Mode**: `query windows --area screen` (REL `(X, Y)`) &rarr; `transform --area screen -p X Y` &rarr; same position.
3. **Unbounded Mode**: `query windows --area unbounded` (REL / GEOMETRY `(X, Y)`) &rarr; `transform --area unbounded -p X Y` &rarr; same position.
