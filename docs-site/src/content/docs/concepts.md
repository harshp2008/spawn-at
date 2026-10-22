---
title: "Concepts"
---

spawn-at starts a program, or takes a window that is already open, and puts it at the position and
size you give; on GNOME Wayland its extension hides a new window until it is in place, so that it
does not flash at the wrong spot (how far that holds is on [Findings](/spawn-at/findings/)).

## Where a window goes

A placement is worked out from a few inputs. The arithmetic lives in `spawn-at-core`, which has no
display-server code, and every backend uses it.

### The coordinate model

Every number is in whole pixels. `--area` chooses what a position is counted from. With `workarea`
(the default) and `screen`, `-p X Y` is counted from the top-left corner of that rectangle on the
target monitor. With `unbounded`, `-p X Y` is an absolute position in the global desktop space across
all monitors, which is the space of `GEOMETRY` in `query windows`. The anchor, pivot and margin rules
below, and the worked example, all follow from that choice.

### The inputs

**Reference area (`--area`).** The rectangle that positions, anchors, margins and clamping refer to:

| Value | Meaning |
| :--- | :--- |
| `workarea` (default) | The usable area of the monitor: without the top bar, docks and panels |
| `screen` | The whole monitor |
| `unbounded` | The whole desktop across all monitors, in global coordinates. No margin, no border, no clamp |

**Monitor (`--monitor`).** An index, `cursor`, or `primary`. When omitted, `spawn` uses the primary
monitor (or index 0, with an `INFO` note, if none is flagged primary; or the monitor under the
pointer with `--anchor cursor`). `transform` uses the monitor the window overlaps most, or the
primary monitor if it overlaps none.

**Anchor (`--anchor`).** A point of the area: `center`, `top-left`, `top-right`, `bottom-left`,
`bottom-right`, `top`, `bottom`, `left`, `right`, or `cursor`. Anchor points include the margins:
`top-left` is at the area's left edge plus the left margin, and its top edge plus the top margin.
Edge and centre anchors centre along their axis relative to the whole area, not the area inside the
margins.

**Pivot (`--pivot`).** Which point of the window sits on the anchor: `center`, `top-left`,
`top-right`, `bottom-left`, `bottom-right`. If omitted, each anchor uses its own matching point
(`center` to `center`, `top` to top-centre, corners to the same corner), and `cursor` uses
`top-left`. Without `--anchor` the pivot is `top-left`. This is what the code does (the evidence is
in Sources). The `--anchor` text in `--help` says "defaults to top-left pivot when --pivot is
omitted", which is only true for `cursor` and for no anchor.

**Position (`-p X Y`, or the alias `-o X Y`).** Without `--anchor` it is a position in the area,
counted from the area's own top-left corner with no margin added. With `--anchor` it is an offset
from the anchor. The `INFO` line printed after the placement shows both spaces, for example
`offset (100, 100) in workarea (origin 0, 40) -> screen position (100, 140)`: the 40 is the top
edge of the work area on that machine, not an error.

**Size (`-s W H`).** The window size in pixels.

**Margin (`-m PX`, and `--margin-top`, `--margin-bottom`, `--margin-left`, `--margin-right`).** The
README gives the default as 16 pixels. Per-side margins also answer to `--mt`, `--mb`, `--ml`,
`--mr`. Margins set the boundary that clamping keeps the window inside.

**Clamp (`--clamp true|false`).** On by default: if the result would leave the area minus the
margins, it is moved back inside and an `INFO` note says so. `--clamp false` turns it off.

A window moves only when you ask: with neither `--anchor` nor `-p`/`-o`, no move is sent, and
`--size` alone only resizes. Centring along an axis is `origin + floor((area - window) / 2)`; an odd
remainder puts the extra pixel on the right or bottom.

### Example

For a screen of 1920x1080, a work area of 1920x1040 starting at (0, 40), and the default margin of 16:

| Command | Window's top-left on screen |
| :--- | :--- |
| `spawn-at spawn -p 700 300 app` | (700, 340): the offset is relative to the work area's top-left |
| `spawn-at spawn --area screen -p 700 300 app` | (700, 300) |
| `spawn-at spawn --area unbounded -p 700 300 app` | (700, 300) |
| `spawn-at spawn -p 0 0 app` | (16, 56): clamped to the margin |

### Unbounded mode accepts less

With `--area unbounded` only `-p`/`-o`, `--pivot`, `--size` and `--anchor cursor` are allowed. Any
other `--anchor`, `--clamp`, any margin, and `--monitor` are rejected with an error. GNOME's
compositor, Mutter, keeps about 75 pixels of a window on an active monitor; a window put almost
entirely off-screen is pulled back, and spawn-at reports the real position. That was observed on
GNOME Shell 46, Wayland, one monitor only.

## Windows that end up bigger than asked

Some applications refuse a small size. spawn-at then positions the window using the size it ended
up with, so an anchor such as `bottom-right` still lands at the corner, and it reports that the
size was raised. The README marks this as an observation from development, not a tested guarantee.
The evidence in the repository is for the project's test window, `gnome-text-editor`, `zenity` and
`firefox`. On GNOME this wait costs time: an application already at its minimum size cannot answer a
resize request, so the wait runs to its cap (see below).

## Cloak, and waiting for the app

These words describe what the GNOME extension does on Wayland.

1. `spawn-at` tells the extension where the next window goes, **before** it starts the program. The
   extension does not have to find the window afterwards.
2. When the window is created, the extension hides it (sets its opacity to 0). This is the
   **cloak**.
3. It resizes the window, **waits for the app to answer the size request**, and moves the window.
4. It reveals the window.

Step 3 matters because resizing the frame does not mean the application has drawn into it. The wait
for the application is the **ready-wait**:

- On GNOME 46 with Wayland the extension waits until the app has answered the size request. The
  default cap is 250 ms and the hard limit is 1500 ms (`wait.cap_ms`).
- XWayland and X11 windows, and the legacy extension for GNOME 42 to 44, use the legacy rule: a
  30 ms quiet period, a 120 ms minimum, and a 1500 ms hard cap.
- Setting `wait.strategy` to `legacy` selects that same 30 ms rule; see
  [Configuration](/spawn-at/configuration/).

A window never stays invisible. Registrations expire after 15 seconds (1.2 seconds for wildcard
matches), a window that matches nothing is revealed at once, and an error during placement restores
the opacity. If something goes wrong you get a normally placed window.

When the wait for the window ends with no claim, `spawn` withdraws its own registration at once on the
modern extension (protocol 7). It stays until it expires when the program fails to start, with
`--no-wait`, and on the legacy extension; what that can do is on
[Known limits](/spawn-at/known-limits/#after-a-failed-spawn).

After the reveal, the extension corrects a resize that arrives late by re-anchoring, and keeps a
listener for size changes: when the app resizes itself and the anchored position moves, the frame is
moved back. The listener is dropped when the window closes or one second after the last correction.

Requests go through a queue, so two shortcuts pressed together do not interleave. GTK3 terminals
based on libvte do not lay out their character grid until they get a focus change, so the extension
pulses focus while the window is still hidden.

## What a flash means here

A flash is a window that was shown before the app first painted at its final size, or that changed
size or position after it was shown, as the project's test harness decides it from the Wayland
protocol. No screen capture was done. The rule and its limits are on
[Findings](/spawn-at/findings/#how-a-flash-is-decided).

## Sources

- `README.md` ("How it works", "More internals", "How well it works", "Coordinate Model", "-p vs Anchors")
- `docs/behavior-notes.md`, sections 1 to 4 (areas, anchors, unbounded rules, monitor defaults)
- `docs/CONFIG.md` ("First slice")
- `docs/DESIGN_PLATFORMS.md`, section 4 (reveal control is GNOME-only)
- `docs/DESIGN_READY_WAIT.md`, section 12 (X11 and XWayland keep the legacy rule)
- `docs/TEST_PLAN.md`, sections 6.1 to 6.3 (minimum-size evidence)
- `crates/spawn-at-core/src/geometry.rs`: the pivot match near line 660 (default pivot), and the
  test `test_new_matrix_and_cursor_default_pivot`
- the output of `spawn-at spawn --help`
