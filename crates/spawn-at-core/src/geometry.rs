//! # Pure Mathematical Geometry Engine
//!
//! Placement arithmetic and monitor selection on plain rectangles. It does no I/O and has no
//! D-Bus or compositor dependency: callers pass in the rectangles, the pointer position and the
//! cursor position, and get a [`Rect`] or a choice back. It must not query a display server.
//!
//! ## Coordinate System Conventions
//! - **Origin `(0, 0)`**: Top-left of the primary display or virtual bounding box.
//! - **Axes**: X increases to the right, Y increases downwards.
//! - **Coordinate Spaces**: Coordinates can be absolute (global desktop bounds spanning multi-monitor setups) or relative.
//! - **Sign Conventions**: Coordinates (`x`, `y`) are signed `i32` to allow for negative monitor offsets, while dimensions (`width`, `height`) are unsigned `u32` to prevent impossible sizes.
//! - **Units**: pixels, as the caller supplies them; this module does not scale.
//! - **Rounding**: halves are computed with integer division, which truncates toward zero. When
//!   a window fits, an odd leftover pixel therefore goes to the right or bottom side of a centred
//!   window. [`apply_pivot`] with [`Pivot::Center`] rounds up instead, and
//!   [`check_geometry_diagnostics`] rounds with `f64::round`. Details are on each function.
//! - **Overflow**: the sums are plain `i32` arithmetic, not checked (a debug build panics on
//!   overflow, a release build wraps unless overflow checks are enabled). Only the cursor tests in
//!   [`resolve_workarea`] and [`resolve_monitor_from_layout`] use saturating adds.

use serde::{Deserialize, Serialize};

/// A 2D bounding rectangle representing a window, monitor, or workarea boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Rect {
    /// Horizontal position of the top-left corner relative to the global origin.
    pub x: i32,
    /// Vertical position of the top-left corner relative to the global origin.
    pub y: i32,
    /// Width in pixels.
    #[serde(alias = "w")]
    pub width: u32,
    /// Height in pixels.
    #[serde(alias = "h")]
    pub height: u32,
}

/// A [`Rect`] spelled `{ x, y, w, h }` in serialised form, as used by the monitor layout. The
/// `From` impls between the two types copy the four fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LayoutRect {
    /// Horizontal position of the top-left corner.
    pub x: i32,
    /// Vertical position of the top-left corner.
    pub y: i32,
    /// Width in pixels.
    pub w: u32,
    /// Height in pixels.
    pub h: u32,
}

impl From<Rect> for LayoutRect {
    fn from(r: Rect) -> Self {
        Self {
            x: r.x,
            y: r.y,
            w: r.width,
            h: r.height,
        }
    }
}

impl From<LayoutRect> for Rect {
    fn from(r: LayoutRect) -> Self {
        Self {
            x: r.x,
            y: r.y,
            width: r.w,
            height: r.h,
        }
    }
}

/// The distance from each edge of a monitor's screen rectangle to its workarea, in the units of
/// the rectangles it was computed from (see [`MonitorInfo::compute_insets`]). Negative when the
/// workarea reaches past the screen edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LayoutInsets {
    /// Workarea top minus screen top.
    pub top: i32,
    /// Screen right edge minus workarea right edge.
    pub right: i32,
    /// Screen bottom edge minus workarea bottom edge.
    pub bottom: i32,
    /// Workarea left minus screen left.
    pub left: i32,
}

/// One monitor of a [`MonitorLayout`]. Every rectangle is optional because a backend may not
/// report it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MonitorInfo {
    /// Index of the monitor; the numeric selector of [`resolve_monitor_from_layout`] matches this
    /// field, not the position in [`MonitorLayout::monitors`].
    pub index: usize,
    /// Connector name, if known; [`resolve_monitor_from_layout`] matches it ASCII
    /// case-insensitively.
    pub name: Option<String>,
    /// True for the primary monitor. Not enforced: a layout may have none, or several.
    pub primary: bool,
    /// Scale factor if the backend reports one. Not used in this module.
    pub scale: Option<f64>,
    /// The whole monitor rectangle.
    pub screen: Option<LayoutRect>,
    /// The part of the monitor not taken by panels and docks.
    pub workarea: Option<LayoutRect>,
    /// Edge distances between `screen` and `workarea`, if both were known.
    pub insets: Option<LayoutInsets>,
}

impl MonitorInfo {
    /// Computes derived insets from screen and workarea rectangles if both are available;
    /// `None` when either is missing. `top` and `left` are workarea minus screen, `right` and
    /// `bottom` are screen edge minus workarea edge (plain `i32` arithmetic, see the module notes
    /// on overflow).
    pub fn compute_insets(
        screen: Option<LayoutRect>,
        workarea: Option<LayoutRect>,
    ) -> Option<LayoutInsets> {
        let (s, w) = (screen?, workarea?);
        let top = w.y - s.y;
        let left = w.x - s.x;
        let right = (s.x + s.w as i32) - (w.x + w.w as i32);
        let bottom = (s.y + s.h as i32) - (w.y + w.h as i32);
        Some(LayoutInsets {
            top,
            right,
            bottom,
            left,
        })
    }
}

/// The monitors of the desktop, as a serialisable schema.
// At this commit every constructor under src/ sets schema_version to 1.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MonitorLayout {
    /// Version of this schema.
    pub schema_version: u32,
    /// The monitors, in the order the backend gave them.
    pub monitors: Vec<MonitorInfo>,
    /// Free text from the backend; omitted from the serialised form when `None`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// The reference point of a placement inside the bounding area. `Cursor` uses the cursor
/// position instead. [`compute_screen_anchor`] gives the point each variant names; when no
/// [`Pivot`] is set, [`calculate_rect_from_placement`] places the window so that the named
/// corner or edge meets that point, and with a pivot the pivot corner meets it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "clap", derive(clap::ValueEnum))]
pub enum Anchor {
    /// Centers the window inside the target bounding box.
    Center,
    /// Anchors the window to the top-left of the bounding box.
    TopLeft,
    /// Anchors the window to the top-right of the bounding box.
    TopRight,
    /// Anchors the window to the bottom-left of the bounding box.
    BottomLeft,
    /// Anchors the window to the bottom-right of the bounding box.
    BottomRight,
    /// Anchors the window to the top edge of the bounding box.
    Top,
    /// Anchors the window to the bottom edge of the bounding box.
    Bottom,
    /// Anchors the window to the left edge of the bounding box.
    Left,
    /// Anchors the window to the right edge of the bounding box.
    Right,
    /// Anchors the window to the cursor position.
    Cursor,
}

/// Which area bounds a placement. In this module only `Unbounded` has an effect: it makes
/// [`calculate_rect_from_placement`] skip the clamp and measure `pos` from the global origin.
/// `Workarea` and `Screen` are not told apart here; the caller puts the rectangle it chose into
/// [`PlacementParams::workarea`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "clap", derive(clap::ValueEnum))]
pub enum Area {
    /// The monitor area without panels and docks (the default).
    #[default]
    Workarea,
    /// The whole monitor rectangle.
    Screen,
    /// No bounding rectangle: no clamping, and `pos` is absolute.
    Unbounded,
}

impl std::fmt::Display for Area {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Area::Workarea => write!(f, "workarea"),
            Area::Screen => write!(f, "screen"),
            Area::Unbounded => write!(f, "unbounded"),
        }
    }
}

/// Which point of the window is put on the target coordinate. When a pivot is set it overrides
/// the per-anchor alignment in [`calculate_rect_from_placement`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "clap", derive(clap::ValueEnum))]
pub enum Pivot {
    /// The target coordinate aligns with the window's top-left corner.
    #[default]
    TopLeft,
    /// The target coordinate aligns with the window's top-right corner.
    TopRight,
    /// The target coordinate aligns with the window's bottom-left corner.
    BottomLeft,
    /// The target coordinate aligns with the window's bottom-right corner.
    BottomRight,
    /// The target coordinate aligns with the window's center (rounded up, see [`apply_pivot`]).
    Center,
}

// serde default for `PlacementParams::clamp`.
fn default_true() -> bool {
    true
}

/// Everything [`calculate_rect_from_placement`] needs, in pixels. [`PlacementParams::default`]
/// sets nothing, with margin 0, an empty `workarea` and `clamp` true.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlacementParams {
    /// Absolute position of the reference point: added to the workarea origin, or to `(0, 0)` for
    /// [`Area::Unbounded`]. Used only when there is no `anchor`, and it wins over `offset` then.
    pub pos: Option<(i32, i32)>,
    /// With an `anchor`: added to the anchor point. Without one and without `pos`: used like
    /// `pos`. Ignored when both `pos` and `offset` are set and there is no anchor.
    pub offset: Option<(i32, i32)>,
    /// Named reference point of the workarea; takes priority over `pos`.
    pub anchor: Option<Anchor>,
    /// Which point of the window goes on the target coordinate.
    pub pivot: Option<Pivot>,
    /// Requested `(width, height)`; replaces the window size passed to
    /// [`calculate_rect_from_placement`].
    pub size: Option<(u32, u32)>,
    /// Margin on all four sides; a directional margin below replaces it for that side.
    pub margin: i32,
    /// Top margin override.
    pub margin_top: Option<i32>,
    /// Bottom margin override.
    pub margin_bottom: Option<i32>,
    /// Left margin override.
    pub margin_left: Option<i32>,
    /// Right margin override.
    pub margin_right: Option<i32>,
    /// See [`Area`]; `None` is the same as not `Unbounded`.
    pub area: Option<Area>,
    /// Cursor position, used by [`Anchor::Cursor`].
    pub cursor_pos: Option<(i32, i32)>,
    /// The rectangle placements are made in (a workarea, or a screen rectangle chosen by the
    /// caller).
    pub workarea: Rect,
    /// Keep the window inside `workarea` minus the margins. Missing in serialised input means true.
    /// Has no effect for [`Area::Unbounded`].
    #[serde(default = "default_true")]
    pub clamp: bool,
}

impl Default for PlacementParams {
    fn default() -> Self {
        Self {
            pos: None,
            offset: None,
            anchor: None,
            pivot: None,
            size: None,
            margin: 0,
            margin_top: None,
            margin_bottom: None,
            margin_left: None,
            margin_right: None,
            area: None,
            cursor_pos: None,
            workarea: Rect::default(),
            clamp: true,
        }
    }
}

/// Picks one rectangle from `workareas` (in monitor order) by the `monitor` selector, which is
/// trimmed and compared in lower case.
///
/// - `"cursor"`: the first rectangle that contains `pointer` (left and top edges inside, right and
///   bottom edges outside); the first rectangle if none does.
/// - decimal digits: the rectangle at that position in the slice; the first one if the index is
///   out of range.
/// - anything else, including `"primary"` and connector names: the first rectangle. The slice is
///   assumed to start with the primary monitor; nothing here checks that.
///
/// Errors: `Err` only when `workareas` is empty.
pub fn resolve_workarea(
    workareas: &[Rect],
    pointer: (i32, i32),
    monitor: &str,
) -> Result<Rect, String> {
    if workareas.is_empty() {
        return Err("No active workareas provided".to_string());
    }

    let monitor_lower = monitor.trim().to_lowercase();

    if monitor_lower == "cursor" {
        let (px, py) = pointer;
        for wa in workareas {
            let right = wa.x.saturating_add(wa.width as i32);
            let bottom = wa.y.saturating_add(wa.height as i32);
            if px >= wa.x && px < right && py >= wa.y && py < bottom {
                return Ok(*wa);
            }
        }
        // Fallback to primary workarea if cursor is out of bounds
        return Ok(workareas[0]);
    }

    if let Ok(index) = monitor_lower.parse::<usize>() {
        if index < workareas.len() {
            return Ok(workareas[index]);
        }
        return Ok(workareas[0]);
    }

    // Default to primary monitor
    Ok(workareas[0])
}

/// Picks a monitor of `layout` by `monitor_selector`. Returns the monitor and a flag that is true
/// only when it fell back to the first monitor of the list because none is flagged `primary`.
///
/// The selector is trimmed and compared in lower case, and tried in this order:
/// - `"cursor"`: the first monitor whose `screen` (else `workarea`, else an empty rectangle at
///   0,0) contains `pointer` (left and top edges inside, right and bottom outside); otherwise the
///   primary monitor, otherwise the first one (flag true).
/// - decimal digits: the monitor whose `index` field equals the number; if there is none, the
///   first monitor of the list (flag false).
/// - `"primary"`: the primary monitor, else the first one (flag true).
/// - any other text: the monitor whose `name` equals the selector ignoring ASCII case (the
///   selector is not trimmed for this comparison); if none, as for `"primary"`.
///
/// With `None` the result is as for `"primary"`. An empty layout gives `(None, false)`.
pub fn resolve_monitor_from_layout<'a>(
    layout: &'a MonitorLayout,
    pointer: (i32, i32),
    monitor_selector: Option<&str>,
) -> (Option<&'a MonitorInfo>, bool) {
    if layout.monitors.is_empty() {
        return (None, false);
    }

    if let Some(sel) = monitor_selector {
        let sel_lower = sel.trim().to_lowercase();
        if sel_lower == "cursor" {
            let (px, py) = pointer;
            for m in &layout.monitors {
                let rect = m.screen.or(m.workarea).map(Rect::from).unwrap_or_default();
                let right = rect.x.saturating_add(rect.width as i32);
                let bottom = rect.y.saturating_add(rect.height as i32);
                if px >= rect.x && px < right && py >= rect.y && py < bottom {
                    return (Some(m), false);
                }
            }
            // Fallback to primary if cursor is outside all monitors
            if let Some(primary) = layout.monitors.iter().find(|m| m.primary) {
                return (Some(primary), false);
            }
            return (layout.monitors.first(), true);
        }

        if let Ok(idx) = sel_lower.parse::<usize>() {
            if let Some(m) = layout.monitors.iter().find(|m| m.index == idx) {
                return (Some(m), false);
            }
            return (layout.monitors.first(), false);
        }

        if sel_lower == "primary" {
            if let Some(primary) = layout.monitors.iter().find(|m| m.primary) {
                return (Some(primary), false);
            }
            return (layout.monitors.first(), true);
        }

        // Match by connector name (case-insensitive)
        if let Some(m) = layout.monitors.iter().find(|m| {
            m.name
                .as_deref()
                .is_some_and(|n| n.eq_ignore_ascii_case(sel))
        }) {
            return (Some(m), false);
        }

        // Fallback to primary or first
        if let Some(primary) = layout.monitors.iter().find(|m| m.primary) {
            return (Some(primary), false);
        }
        return (layout.monitors.first(), true);
    }

    // When monitor selector is None, spawn defaults to true primary monitor
    if let Some(primary) = layout.monitors.iter().find(|m| m.primary) {
        (Some(primary), false)
    } else {
        (layout.monitors.first(), true)
    }
}

/// Finds the monitor whose `screen` (else `workarea`, else an empty rectangle at 0,0) overlaps
/// `window_rect` in the largest number of pixels (width times height of the intersection). A
/// tie goes to the monitor earlier in the list. `None` when nothing overlaps or the layout is
/// empty. Plain `i32` sums, see the module notes on overflow.
pub fn find_monitor_with_largest_overlap(
    layout: &MonitorLayout,
    window_rect: Rect,
) -> Option<&MonitorInfo> {
    let mut best_m = None;
    let mut best_overlap_area = 0i64;

    for m in &layout.monitors {
        let geom = m.screen.or(m.workarea).map(Rect::from).unwrap_or_default();
        let overlap_w = (window_rect.x + window_rect.width as i32).min(geom.x + geom.width as i32)
            - window_rect.x.max(geom.x);
        let overlap_h = (window_rect.y + window_rect.height as i32)
            .min(geom.y + geom.height as i32)
            - window_rect.y.max(geom.y);
        if overlap_w > 0 && overlap_h > 0 {
            let area = (overlap_w as i64) * (overlap_h as i64);
            if area > best_overlap_area {
                best_overlap_area = area;
                best_m = Some(m);
            }
        }
    }

    best_m
}

/// True if `win_rect` overlaps at least two of the rectangles in `monitors`. Rectangles that only
/// touch the window along an edge do not count (the overlap must be at least 1 px in both
/// directions).
pub fn window_straddles_monitors(win_rect: Rect, monitors: &[Rect]) -> bool {
    let mut count = 0;
    for m in monitors {
        let overlap_w =
            (win_rect.x + win_rect.width as i32).min(m.x + m.width as i32) - win_rect.x.max(m.x);
        let overlap_h =
            (win_rect.y + win_rect.height as i32).min(m.y + m.height as i32) - win_rect.y.max(m.y);
        if overlap_w > 0 && overlap_h > 0 {
            count += 1;
            if count > 1 {
                return true;
            }
        }
    }
    false
}

/// The top-left corner `(x, y)` of a `win_w` x `win_h` window placed against `anchor` inside
/// `workarea`, with the same `margin` on every side.
///
/// Corner and edge anchors keep `margin` from the named edges; centred directions use
/// `(workarea size - window size) / 2` (integer division, truncating toward zero) and ignore the
/// margin. `Anchor::Cursor` has no cursor here and returns the workarea's top-left corner. Does
/// not clamp. Not the function [`calculate_rect_from_placement`] calls; that one uses
/// [`compute_screen_anchor`] and [`apply_pivot`].
pub fn apply_anchor(
    workarea: Rect,
    win_w: u32,
    win_h: u32,
    anchor: Anchor,
    margin: i32,
) -> (i32, i32) {
    let (wa_w, wa_h) = (workarea.width as i32, workarea.height as i32);
    let (w, h) = (win_w as i32, win_h as i32);

    match anchor {
        Anchor::Center => {
            let x = workarea.x + (wa_w - w) / 2;
            let y = workarea.y + (wa_h - h) / 2;
            (x, y)
        }
        Anchor::TopLeft => {
            let x = workarea.x + margin;
            let y = workarea.y + margin;
            (x, y)
        }
        Anchor::TopRight => {
            let x = workarea.x + wa_w - w - margin;
            let y = workarea.y + margin;
            (x, y)
        }
        Anchor::BottomLeft => {
            let x = workarea.x + margin;
            let y = workarea.y + wa_h - h - margin;
            (x, y)
        }
        Anchor::BottomRight => {
            let x = workarea.x + wa_w - w - margin;
            let y = workarea.y + wa_h - h - margin;
            (x, y)
        }
        Anchor::Top => {
            let x = workarea.x + (wa_w - w) / 2;
            let y = workarea.y + margin;
            (x, y)
        }
        Anchor::Bottom => {
            let x = workarea.x + (wa_w - w) / 2;
            let y = workarea.y + wa_h - h - margin;
            (x, y)
        }
        Anchor::Left => {
            let x = workarea.x + margin;
            let y = workarea.y + (wa_h - h) / 2;
            (x, y)
        }
        Anchor::Right => {
            let x = workarea.x + wa_w - w - margin;
            let y = workarea.y + (wa_h - h) / 2;
            (x, y)
        }
        Anchor::Cursor => (workarea.x, workarea.y),
    }
}

/// The point `(x, y)` that `anchor` names on `workarea`. This is a point, not a window origin.
///
/// Corners and edges are inset by the margin of the side they touch (the point moves inward by
/// `margin_left`/`margin_top`, and by `margin_right`/`margin_bottom` from the far edges). A
/// centred coordinate is the workarea origin plus half its size, integer division truncating.
/// `Anchor::Cursor` gives `cursor_pos`, or the top-left point with margins when it is `None`.
pub fn compute_screen_anchor(
    workarea: Rect,
    anchor: Anchor,
    margin_top: i32,
    margin_bottom: i32,
    margin_left: i32,
    margin_right: i32,
    cursor_pos: Option<(i32, i32)>,
) -> (i32, i32) {
    match anchor {
        Anchor::Center => (
            workarea.x + (workarea.width as i32) / 2,
            workarea.y + (workarea.height as i32) / 2,
        ),
        Anchor::TopLeft => (workarea.x + margin_left, workarea.y + margin_top),
        Anchor::TopRight => (
            workarea.x + (workarea.width as i32) - margin_right,
            workarea.y + margin_top,
        ),
        Anchor::BottomLeft => (
            workarea.x + margin_left,
            workarea.y + (workarea.height as i32) - margin_bottom,
        ),
        Anchor::BottomRight => (
            workarea.x + (workarea.width as i32) - margin_right,
            workarea.y + (workarea.height as i32) - margin_bottom,
        ),
        Anchor::Top => (
            workarea.x + (workarea.width as i32) / 2,
            workarea.y + margin_top,
        ),
        Anchor::Bottom => (
            workarea.x + (workarea.width as i32) / 2,
            workarea.y + (workarea.height as i32) - margin_bottom,
        ),
        Anchor::Left => (
            workarea.x + margin_left,
            workarea.y + (workarea.height as i32) / 2,
        ),
        Anchor::Right => (
            workarea.x + (workarea.width as i32) - margin_right,
            workarea.y + (workarea.height as i32) / 2,
        ),
        Anchor::Cursor => cursor_pos.unwrap_or((workarea.x + margin_left, workarea.y + margin_top)),
    }
}

/// The rectangle a window of `win_w` x `win_h` pixels gets under `params` (`params.size`, when
/// set, replaces both numbers).
///
/// Steps, as the code does them:
/// 1. Each margin is its directional override, else `params.margin`.
/// 2. The anchor point: with an `anchor`, [`compute_screen_anchor`] on `params.workarea`; else
///    `pos` (or, if absent, `offset`) added to the workarea origin, or to `(0, 0)` for
///    [`Area::Unbounded`]; else the workarea origin moved in by the left and top margins.
/// 3. The target is the anchor point plus `offset`, but only when there is an `anchor`.
/// 4. The window origin: with a `pivot`, [`apply_pivot`] on the target. With only an `anchor`,
///    each variant is aligned as follows. Centred axes use `(workarea size - window size) / 2`
///    (truncating toward zero) from the workarea origin, plus the offset, and ignore the margins.
///    Right and bottom anchors place the window's right or bottom edge on the target. With
///    neither, the target is the top-left corner.
/// 5. Unless `area` is `Unbounded` or `clamp` is false, [`clamp_to_bounds`] keeps the origin
///    inside the workarea minus the margins (the size is never changed).
///
/// Does not check that the window fits; a window larger than the available space ends up at the
/// top-left limit of the clamp range.
pub fn calculate_rect_from_placement(params: &PlacementParams, win_w: u32, win_h: u32) -> Rect {
    let w = params.size.map(|s| s.0).unwrap_or(win_w);
    let h = params.size.map(|s| s.1).unwrap_or(win_h);

    let wa = params.workarea;
    let margin_top = params.margin_top.unwrap_or(params.margin);
    let margin_bottom = params.margin_bottom.unwrap_or(params.margin);
    let margin_left = params.margin_left.unwrap_or(params.margin);
    let margin_right = params.margin_right.unwrap_or(params.margin);

    let is_unbounded = params.area == Some(Area::Unbounded);

    let (anchor_x, anchor_y) = if let Some(anchor) = params.anchor {
        compute_screen_anchor(
            wa,
            anchor,
            margin_top,
            margin_bottom,
            margin_left,
            margin_right,
            params.cursor_pos,
        )
    } else if let Some(pos) = params.pos.or(params.offset) {
        let (ref_x, ref_y) = if is_unbounded { (0, 0) } else { (wa.x, wa.y) };
        (ref_x + pos.0, ref_y + pos.1)
    } else {
        (wa.x + margin_left, wa.y + margin_top)
    };

    let (ox, oy) = if params.anchor.is_some() {
        params.offset.unwrap_or((0, 0))
    } else {
        (0, 0)
    };
    let target_x = anchor_x + ox;
    let target_y = anchor_y + oy;

    let (x, y) = if let Some(pivot) = params.pivot {
        apply_pivot(target_x, target_y, w, h, pivot)
    } else if let Some(anchor) = params.anchor {
        match anchor {
            Anchor::Top => (wa.x + (wa.width as i32 - w as i32) / 2 + ox, target_y),
            Anchor::Bottom => (
                wa.x + (wa.width as i32 - w as i32) / 2 + ox,
                target_y - (h as i32),
            ),
            Anchor::Left => (target_x, wa.y + (wa.height as i32 - h as i32) / 2 + oy),
            Anchor::Right => (
                target_x - (w as i32),
                wa.y + (wa.height as i32 - h as i32) / 2 + oy,
            ),
            Anchor::Center => (
                wa.x + (wa.width as i32 - w as i32) / 2 + ox,
                wa.y + (wa.height as i32 - h as i32) / 2 + oy,
            ),
            Anchor::TopLeft => (target_x, target_y),
            Anchor::TopRight => (target_x - (w as i32), target_y),
            Anchor::BottomLeft => (target_x, target_y - (h as i32)),
            Anchor::BottomRight => (target_x - (w as i32), target_y - (h as i32)),
            Anchor::Cursor => (target_x, target_y),
        }
    } else {
        (target_x, target_y)
    };

    let raw_rect = Rect {
        x,
        y,
        width: w,
        height: h,
    };
    if !is_unbounded && params.clamp {
        clamp_to_bounds(
            raw_rect,
            wa,
            margin_top,
            margin_bottom,
            margin_left,
            margin_right,
            0,
        )
    } else {
        raw_rect
    }
}

/// The top-left corner `(x, y)` of a `win_w` x `win_h` window whose `pivot` point is at
/// `(target_x, target_y)`. For `Pivot::Center` the half sizes are `(size + 1) / 2`, so an odd size
/// puts the extra pixel on the left and top side.
pub fn apply_pivot(
    target_x: i32,
    target_y: i32,
    win_w: u32,
    win_h: u32,
    pivot: Pivot,
) -> (i32, i32) {
    let (w, h) = (win_w as i32, win_h as i32);
    match pivot {
        Pivot::TopLeft => (target_x, target_y),
        Pivot::TopRight => (target_x - w, target_y),
        Pivot::BottomLeft => (target_x, target_y - h),
        Pivot::BottomRight => (target_x - w, target_y - h),
        Pivot::Center => (target_x - (w + 1) / 2, target_y - (h + 1) / 2),
    }
}

/// Moves `rect` so that it lies inside `bounds` shrunk by the four `bound_*` insets, each plus
/// `global_margin`. Only `x` and `y` change; width and height are kept. When the window is wider
/// (or taller) than the space left, the range collapses to its minimum, so the window sits at the
/// left (or top) limit and sticks out on the right (or bottom).
pub fn clamp_to_bounds(
    rect: Rect,
    bounds: Rect,
    bound_top: i32,
    bound_bottom: i32,
    bound_left: i32,
    bound_right: i32,
    global_margin: i32,
) -> Rect {
    let top = bound_top + global_margin;
    let bottom = bound_bottom + global_margin;
    let left = bound_left + global_margin;
    let right = bound_right + global_margin;

    let min_x = bounds.x + left;
    let min_y = bounds.y + top;
    let mut max_x = bounds.x + (bounds.width as i32) - right - (rect.width as i32);
    let mut max_y = bounds.y + (bounds.height as i32) - bottom - (rect.height as i32);

    if max_x < min_x {
        max_x = min_x;
    }
    if max_y < min_y {
        max_y = min_y;
    }

    let clamped_x = rect.x.clamp(min_x, max_x);
    let clamped_y = rect.y.clamp(min_y, max_y);

    Rect {
        x: clamped_x,
        y: clamped_y,
        width: rect.width,
        height: rect.height,
    }
}

/// A finding of [`check_geometry_diagnostics`]. None of them changes the placement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GeometryDiagnostic {
    /// The requested size is wider than the workarea width minus the left and right margins, or
    /// taller than the height minus the top and bottom margins. `w` and `h` are the requested
    /// width and height in pixels.
    Oversized { w: u32, h: u32 },
    /// The origin the parameters ask for lies outside the range the clamp allows.
    Repositioned {
        /// Requested x of the window origin.
        raw_x: i32,
        /// Requested y of the window origin.
        raw_y: i32,
        /// x after clamping to the allowed range.
        clamped_x: i32,
        /// y after clamping to the allowed range.
        clamped_y: i32,
    },
    /// The requested width is under 100 px or the height under 80 px. `w` and `h` are the
    /// requested width and height in pixels.
    SubMinimumSize { w: u32, h: u32 },
}

/// Checks `params` for the three findings of [`GeometryDiagnostic`]. Returns nothing when
/// `params.size` is `None`.
///
/// `Oversized` is tested first; only when it does not apply is the origin computed and
/// `Repositioned` tested. `SubMinimumSize` (width below 100 or height below 80) is tested
/// independently, so it can accompany `Oversized`.
///
/// This is a separate computation from [`calculate_rect_from_placement`] and differs from it:
/// it ignores `clamp` and `area` (it reports `Repositioned` even if no clamp would run); it takes
/// `pos` as given, not added to the workarea origin; it adds `offset` in every case, where the
/// placement code adds it only with an `anchor`; and it places the pivot or anchor fraction (0, 0.5 or 1) of the size with `f64::round` (halves away
/// from zero), where the placement code uses truncating integer division or `(size + 1) / 2`. For
/// odd sizes the origins can therefore differ by 1 px. Checked by reading both functions, not by
/// a test.
pub fn check_geometry_diagnostics(params: &PlacementParams) -> Vec<GeometryDiagnostic> {
    let mut diags = Vec::new();

    if let Some((w, h)) = params.size {
        let wa = params.workarea;
        let ml = params.margin_left.unwrap_or(params.margin);
        let mr = params.margin_right.unwrap_or(params.margin);
        let mt = params.margin_top.unwrap_or(params.margin);
        let mb = params.margin_bottom.unwrap_or(params.margin);

        let avail_w = wa.width as i32 - ml - mr;
        let avail_h = wa.height as i32 - mt - mb;

        let is_oversized = (w as i32) > avail_w || (h as i32) > avail_h;

        if is_oversized {
            diags.push(GeometryDiagnostic::Oversized { w, h });
        } else {
            let offset_x = params.offset.map(|o| o.0).unwrap_or(0);
            let offset_y = params.offset.map(|o| o.1).unwrap_or(0);

            let (pivot_u, pivot_v) = if let Some(pivot) = params.pivot {
                match pivot {
                    Pivot::TopLeft => (0.0, 0.0),
                    Pivot::TopRight => (1.0, 0.0),
                    Pivot::BottomLeft => (0.0, 1.0),
                    Pivot::BottomRight => (1.0, 1.0),
                    Pivot::Center => (0.5, 0.5),
                }
            } else if let Some(anchor) = params.anchor {
                match anchor {
                    Anchor::Center => (0.5, 0.5),
                    Anchor::TopLeft => (0.0, 0.0),
                    Anchor::TopRight => (1.0, 0.0),
                    Anchor::BottomLeft => (0.0, 1.0),
                    Anchor::BottomRight => (1.0, 1.0),
                    Anchor::Top => (0.5, 0.0),
                    Anchor::Bottom => (0.5, 1.0),
                    Anchor::Left => (0.0, 0.5),
                    Anchor::Right => (1.0, 0.5),
                    Anchor::Cursor => (0.0, 0.0),
                }
            } else {
                (0.0, 0.0)
            };

            let (screen_anchor_x, screen_anchor_y) = if let Some(anchor) = params.anchor {
                match anchor {
                    Anchor::Center => (wa.x + (wa.width as i32) / 2, wa.y + (wa.height as i32) / 2),
                    Anchor::TopLeft => (wa.x + ml, wa.y + mt),
                    Anchor::TopRight => (wa.x + (wa.width as i32) - mr, wa.y + mt),
                    Anchor::BottomLeft => (wa.x + ml, wa.y + (wa.height as i32) - mb),
                    Anchor::BottomRight => (
                        wa.x + (wa.width as i32) - mr,
                        wa.y + (wa.height as i32) - mb,
                    ),
                    Anchor::Top => (wa.x + (wa.width as i32) / 2, wa.y + mt),
                    Anchor::Bottom => {
                        (wa.x + (wa.width as i32) / 2, wa.y + (wa.height as i32) - mb)
                    }
                    Anchor::Left => (wa.x + ml, wa.y + (wa.height as i32) / 2),
                    Anchor::Right => (wa.x + (wa.width as i32) - mr, wa.y + (wa.height as i32) / 2),
                    Anchor::Cursor => {
                        if let Some(cursor) = params.cursor_pos {
                            (cursor.0, cursor.1)
                        } else {
                            (wa.x + ml, wa.y + mt)
                        }
                    }
                }
            } else if let Some(pos) = params.pos {
                (pos.0, pos.1)
            } else {
                (wa.x + ml, wa.y + mt)
            };

            let min_x = wa.x + ml;
            let mut max_x = wa.x + (wa.width as i32) - mr - (w as i32);
            let min_y = wa.y + mt;
            let mut max_y = wa.y + (wa.height as i32) - mb - (h as i32);

            if max_x < min_x {
                max_x = min_x;
            }
            if max_y < min_y {
                max_y = min_y;
            }

            let raw_x = screen_anchor_x - (pivot_u * (w as f64)).round() as i32 + offset_x;
            let raw_y = screen_anchor_y - (pivot_v * (h as f64)).round() as i32 + offset_y;

            let clamped_x = raw_x.clamp(min_x, max_x);
            let clamped_y = raw_y.clamp(min_y, max_y);

            if raw_x != clamped_x || raw_y != clamped_y {
                diags.push(GeometryDiagnostic::Repositioned {
                    raw_x,
                    raw_y,
                    clamped_x,
                    clamped_y,
                });
            }
        }

        if w < 100 || h < 80 {
            diags.push(GeometryDiagnostic::SubMinimumSize { w, h });
        }
    }

    diags
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_apply_anchor_center() {
        let wa = Rect {
            x: 0,
            y: 0,
            width: 1920,
            height: 1080,
        };
        let (x, y) = apply_anchor(wa, 800, 600, Anchor::Center, 16);
        assert_eq!(x, (1920 - 800) / 2);
        assert_eq!(y, (1080 - 600) / 2);
    }

    #[test]
    fn test_apply_anchor_corners() {
        let wa = Rect {
            x: 100,
            y: 50,
            width: 1920,
            height: 1080,
        };
        let margin = 20;
        let win_w = 400;
        let win_h = 300;

        assert_eq!(
            apply_anchor(wa, win_w, win_h, Anchor::TopLeft, margin),
            (120, 70)
        );
        assert_eq!(
            apply_anchor(wa, win_w, win_h, Anchor::TopRight, margin),
            (100 + 1920 - 400 - 20, 70)
        );
        assert_eq!(
            apply_anchor(wa, win_w, win_h, Anchor::BottomLeft, margin),
            (120, 50 + 1080 - 300 - 20)
        );
        assert_eq!(
            apply_anchor(wa, win_w, win_h, Anchor::BottomRight, margin),
            (100 + 1920 - 400 - 20, 50 + 1080 - 300 - 20)
        );
        assert_eq!(
            apply_anchor(wa, win_w, win_h, Anchor::Top, margin),
            (100 + (1920 - 400) / 2, 70)
        );
        assert_eq!(
            apply_anchor(wa, win_w, win_h, Anchor::Bottom, margin),
            (100 + (1920 - 400) / 2, 50 + 1080 - 300 - 20)
        );
        assert_eq!(
            apply_anchor(wa, win_w, win_h, Anchor::Left, margin),
            (120, 50 + (1080 - 300) / 2)
        );
        assert_eq!(
            apply_anchor(wa, win_w, win_h, Anchor::Right, margin),
            (100 + 1920 - 400 - 20, 50 + (1080 - 300) / 2)
        );
    }

    #[test]
    fn test_apply_pivot() {
        let target_x = 500;
        let target_y = 400;
        let win_w = 200;
        let win_h = 100;

        assert_eq!(
            apply_pivot(target_x, target_y, win_w, win_h, Pivot::TopLeft),
            (500, 400)
        );
        assert_eq!(
            apply_pivot(target_x, target_y, win_w, win_h, Pivot::TopRight),
            (300, 400)
        );
        assert_eq!(
            apply_pivot(target_x, target_y, win_w, win_h, Pivot::BottomLeft),
            (500, 300)
        );
        assert_eq!(
            apply_pivot(target_x, target_y, win_w, win_h, Pivot::BottomRight),
            (300, 300)
        );
        assert_eq!(
            apply_pivot(target_x, target_y, win_w, win_h, Pivot::Center),
            (400, 350)
        );
    }

    #[test]
    fn test_clamp_to_bounds_within() {
        let wa = Rect {
            x: 0,
            y: 0,
            width: 1920,
            height: 1080,
        };
        let r = Rect {
            x: 100,
            y: 100,
            width: 500,
            height: 400,
        };
        let clamped = clamp_to_bounds(r, wa, 0, 0, 0, 0, 16);
        assert_eq!(clamped, r);
    }

    #[test]
    fn test_clamp_to_bounds_outside() {
        let wa = Rect {
            x: 0,
            y: 0,
            width: 1920,
            height: 1080,
        };
        let r = Rect {
            x: 2000,
            y: -50,
            width: 500,
            height: 400,
        };
        let clamped = clamp_to_bounds(r, wa, 0, 0, 0, 0, 16);
        assert_eq!(
            clamped,
            Rect {
                x: 1920 - 500 - 16,
                y: 16,
                width: 500,
                height: 400
            }
        );
    }

    #[test]
    fn test_clamp_to_bounds_oversized() {
        let wa = Rect {
            x: 0,
            y: 0,
            width: 1000,
            height: 800,
        };
        let r = Rect {
            x: 500,
            y: 500,
            width: 1200,
            height: 900,
        };
        let clamped = clamp_to_bounds(r, wa, 0, 0, 0, 0, 16);
        assert_eq!(
            clamped,
            Rect {
                x: 16,
                y: 16,
                width: 1200,
                height: 900
            }
        );
    }

    #[test]
    fn test_resolve_workarea() {
        let wa1 = Rect {
            x: 0,
            y: 0,
            width: 1920,
            height: 1080,
        };
        let wa2 = Rect {
            x: 1920,
            y: 0,
            width: 2560,
            height: 1440,
        };
        let workareas = vec![wa1, wa2];

        assert_eq!(resolve_workarea(&workareas, (0, 0), "0").unwrap(), wa1);
        assert_eq!(resolve_workarea(&workareas, (0, 0), "1").unwrap(), wa2);
        assert_eq!(resolve_workarea(&workareas, (0, 0), "99").unwrap(), wa1);
        assert_eq!(
            resolve_workarea(&workareas, (0, 0), "primary").unwrap(),
            wa1
        );
        assert_eq!(
            resolve_workarea(&workareas, (500, 300), "cursor").unwrap(),
            wa1
        );
        assert_eq!(
            resolve_workarea(&workareas, (2200, 500), "cursor").unwrap(),
            wa2
        );
        assert_eq!(
            resolve_workarea(&workareas, (9999, 9999), "cursor").unwrap(),
            wa1
        );
    }

    #[test]
    fn test_diagnostics_oversized() {
        let wa = Rect {
            x: 0,
            y: 0,
            width: 1920,
            height: 1080,
        };
        let params = PlacementParams {
            size: Some((3000, 2000)),
            anchor: Some(Anchor::BottomRight),
            pivot: Some(Pivot::BottomRight),
            margin: 20,
            workarea: wa,
            ..Default::default()
        };
        let diags = check_geometry_diagnostics(&params);
        assert_eq!(
            diags,
            vec![GeometryDiagnostic::Oversized { w: 3000, h: 2000 }]
        );
    }

    #[test]
    fn test_diagnostics_sub_minimum() {
        let wa = Rect {
            x: 0,
            y: 0,
            width: 1920,
            height: 1080,
        };
        let params = PlacementParams {
            size: Some((30, 20)),
            anchor: Some(Anchor::BottomRight),
            pivot: Some(Pivot::BottomRight),
            margin: 20,
            workarea: wa,
            ..Default::default()
        };
        let diags = check_geometry_diagnostics(&params);
        assert_eq!(
            diags,
            vec![GeometryDiagnostic::SubMinimumSize { w: 30, h: 20 }]
        );
    }

    #[test]
    fn test_diagnostics_repositioned() {
        let wa = Rect {
            x: 0,
            y: 0,
            width: 1920,
            height: 1080,
        };
        let params = PlacementParams {
            size: Some((800, 600)),
            anchor: Some(Anchor::BottomRight),
            pivot: Some(Pivot::TopLeft),
            margin: 20,
            workarea: wa,
            ..Default::default()
        };
        let diags = check_geometry_diagnostics(&params);
        assert_eq!(
            diags,
            vec![GeometryDiagnostic::Repositioned {
                raw_x: 1900,
                raw_y: 1060,
                clamped_x: 1100,
                clamped_y: 460
            }]
        );
    }

    // ========================================================================
    // Characterization Tests: Comprehensive Pure Geometry Engine Verification
    // ========================================================================

    #[test]
    fn test_apply_anchor_all_variants_exhaustive() {
        let wa = Rect {
            x: 100,
            y: 200,
            width: 1920,
            height: 1080,
        };
        let (win_w, win_h) = (800, 600);
        let margin = 24;

        // Center
        assert_eq!(
            apply_anchor(wa, win_w, win_h, Anchor::Center, margin),
            (100 + (1920 - 800) / 2, 200 + (1080 - 600) / 2) // (660, 440)
        );

        // TopLeft
        assert_eq!(
            apply_anchor(wa, win_w, win_h, Anchor::TopLeft, margin),
            (100 + 24, 200 + 24) // (124, 224)
        );

        // TopRight
        assert_eq!(
            apply_anchor(wa, win_w, win_h, Anchor::TopRight, margin),
            (100 + 1920 - 800 - 24, 200 + 24) // (1196, 224)
        );

        // BottomLeft
        assert_eq!(
            apply_anchor(wa, win_w, win_h, Anchor::BottomLeft, margin),
            (100 + 24, 200 + 1080 - 600 - 24) // (124, 656)
        );

        // BottomRight
        assert_eq!(
            apply_anchor(wa, win_w, win_h, Anchor::BottomRight, margin),
            (100 + 1920 - 800 - 24, 200 + 1080 - 600 - 24) // (1196, 656)
        );

        // Top
        assert_eq!(
            apply_anchor(wa, win_w, win_h, Anchor::Top, margin),
            (100 + (1920 - 800) / 2, 200 + 24) // (660, 224)
        );

        // Bottom
        assert_eq!(
            apply_anchor(wa, win_w, win_h, Anchor::Bottom, margin),
            (100 + (1920 - 800) / 2, 200 + 1080 - 600 - 24) // (660, 656)
        );

        // Left
        assert_eq!(
            apply_anchor(wa, win_w, win_h, Anchor::Left, margin),
            (100 + 24, 200 + (1080 - 600) / 2) // (124, 440)
        );

        // Right
        assert_eq!(
            apply_anchor(wa, win_w, win_h, Anchor::Right, margin),
            (100 + 1920 - 800 - 24, 200 + (1080 - 600) / 2) // (1196, 440)
        );

        // Cursor fallback returns workarea origin without margin
        assert_eq!(
            apply_anchor(wa, win_w, win_h, Anchor::Cursor, margin),
            (100, 200)
        );
    }

    #[test]
    fn test_apply_pivot_all_variants_exhaustive() {
        let (target_x, target_y) = (500, 400);
        let (win_w, win_h) = (200, 100);

        assert_eq!(
            apply_pivot(target_x, target_y, win_w, win_h, Pivot::TopLeft),
            (500, 400)
        );
        assert_eq!(
            apply_pivot(target_x, target_y, win_w, win_h, Pivot::TopRight),
            (300, 400)
        );
        assert_eq!(
            apply_pivot(target_x, target_y, win_w, win_h, Pivot::BottomLeft),
            (500, 300)
        );
        assert_eq!(
            apply_pivot(target_x, target_y, win_w, win_h, Pivot::BottomRight),
            (300, 300)
        );
        assert_eq!(
            apply_pivot(target_x, target_y, win_w, win_h, Pivot::Center),
            (400, 350)
        );
    }

    #[test]
    fn test_clamp_to_bounds_negative_and_oversized() {
        let bounds = Rect {
            x: -1920,
            y: 0,
            width: 1920,
            height: 1080,
        }; // Left monitor
        let normal_rect = Rect {
            x: -1000,
            y: 200,
            width: 500,
            height: 400,
        };

        // Inside bounds: untouched
        let clamped = clamp_to_bounds(normal_rect, bounds, 0, 0, 0, 0, 0);
        assert_eq!(clamped, normal_rect);

        // Overflow left: clamped to bounds.x (-1920)
        let overflow_left = Rect {
            x: -2500,
            y: 200,
            width: 500,
            height: 400,
        };
        let clamped_left = clamp_to_bounds(overflow_left, bounds, 0, 0, 0, 0, 10);
        assert_eq!(clamped_left.x, -1920 + 10);

        // Overflow right: clamped to bounds.x + width - win_w
        let overflow_right = Rect {
            x: 500,
            y: 200,
            width: 500,
            height: 400,
        };
        let clamped_right = clamp_to_bounds(overflow_right, bounds, 0, 0, 0, 0, 10);
        assert_eq!(clamped_right.x, -1920 + 1920 - 10 - 500); // -510

        // Window larger than workarea (2500px wide in 1920px monitor)
        let huge_rect = Rect {
            x: 0,
            y: 0,
            width: 2500,
            height: 1500,
        };
        let clamped_huge = clamp_to_bounds(huge_rect, bounds, 0, 0, 0, 0, 16);
        // max_x < min_x -> pins to min_x
        assert_eq!(clamped_huge.x, -1920 + 16);
        assert_eq!(clamped_huge.y, 16);
    }

    #[test]
    fn test_resolve_workarea_multi_monitor_and_errors() {
        let monitors = vec![
            Rect {
                x: 0,
                y: 0,
                width: 1920,
                height: 1080,
            }, // Monitor 0: Primary
            Rect {
                x: 1920,
                y: 0,
                width: 2560,
                height: 1440,
            }, // Monitor 1: Right
            Rect {
                x: -1920,
                y: 0,
                width: 1920,
                height: 1080,
            }, // Monitor 2: Left
        ];

        // Cursor resolution
        assert_eq!(
            resolve_workarea(&monitors, (100, 100), "cursor").unwrap(),
            monitors[0]
        );
        assert_eq!(
            resolve_workarea(&monitors, (2500, 500), "cursor").unwrap(),
            monitors[1]
        );
        assert_eq!(
            resolve_workarea(&monitors, (-500, 300), "cursor").unwrap(),
            monitors[2]
        );

        // Primary / Index resolution
        assert_eq!(
            resolve_workarea(&monitors, (0, 0), "primary").unwrap(),
            monitors[0]
        );
        assert_eq!(
            resolve_workarea(&monitors, (0, 0), "0").unwrap(),
            monitors[0]
        );
        assert_eq!(
            resolve_workarea(&monitors, (0, 0), "1").unwrap(),
            monitors[1]
        );
        assert_eq!(
            resolve_workarea(&monitors, (0, 0), "2").unwrap(),
            monitors[2]
        );

        // Out-of-bounds index falls back to primary monitor (monitors[0])
        assert_eq!(
            resolve_workarea(&monitors, (0, 0), "99").unwrap(),
            monitors[0]
        );
        // Unrecognized string falls back to primary monitor
        assert_eq!(
            resolve_workarea(&monitors, (0, 0), "invalid_spec").unwrap(),
            monitors[0]
        );
        // Empty monitor list returns Err
        assert!(resolve_workarea(&[], (0, 0), "primary").is_err());
    }

    #[test]
    fn test_50_permutations_anchor_cross_pivot_matrix() {
        let wa = Rect {
            x: 0,
            y: 0,
            width: 1920,
            height: 1080,
        };
        let anchors = [
            Anchor::Center,
            Anchor::TopLeft,
            Anchor::TopRight,
            Anchor::BottomLeft,
            Anchor::BottomRight,
            Anchor::Top,
            Anchor::Bottom,
            Anchor::Left,
            Anchor::Right,
            Anchor::Cursor,
        ];
        let pivots = [
            Pivot::TopLeft,
            Pivot::TopRight,
            Pivot::BottomLeft,
            Pivot::BottomRight,
            Pivot::Center,
        ];

        let mut count = 0;
        for anchor in &anchors {
            for pivot in &pivots {
                let (ax, ay) = apply_anchor(wa, 800, 600, *anchor, 20);
                let (px, py) = apply_pivot(ax, ay, 800, 600, *pivot);
                assert!((-10000..=10000).contains(&px));
                assert!((-10000..=10000).contains(&py));
                count += 1;
            }
        }
        assert_eq!(count, 50);
    }

    #[test]
    fn test_monitor_layout_compute_insets_and_schema() {
        let screen = LayoutRect {
            x: 0,
            y: 0,
            w: 1920,
            h: 1080,
        };
        let workarea = LayoutRect {
            x: 0,
            y: 40,
            w: 1920,
            h: 1040,
        };
        let insets = MonitorInfo::compute_insets(Some(screen), Some(workarea)).unwrap();
        assert_eq!(
            insets,
            LayoutInsets {
                top: 40,
                right: 0,
                bottom: 0,
                left: 0,
            }
        );

        let layout = MonitorLayout {
            schema_version: 1,
            monitors: vec![MonitorInfo {
                index: 0,
                name: Some("DP-1".to_string()),
                primary: true,
                scale: Some(1.0),
                screen: Some(screen),
                workarea: Some(workarea),
                insets: Some(insets),
            }],
            note: None,
        };

        let json = serde_json::to_string(&layout).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["schema_version"], 1);
        assert_eq!(parsed["monitors"][0]["index"], 0);
        assert_eq!(parsed["monitors"][0]["name"], "DP-1");
        assert_eq!(parsed["monitors"][0]["primary"], true);
        assert_eq!(parsed["monitors"][0]["scale"], 1.0);
        assert_eq!(parsed["monitors"][0]["screen"]["w"], 1920);
        assert_eq!(parsed["monitors"][0]["workarea"]["h"], 1040);
        assert_eq!(parsed["monitors"][0]["insets"]["top"], 40);
        assert!(parsed.get("note").is_none());
    }

    #[test]
    fn test_new_matrix_and_cursor_default_pivot() {
        let wa = Rect {
            x: 0,
            y: 40,
            width: 1920,
            height: 1040,
        };
        let screen = Rect {
            x: 0,
            y: 0,
            width: 1920,
            height: 1080,
        };

        // 1. -p without --anchor defaults to TopLeft pivot relative to workarea top-left (wa.x=0, wa.y=40)
        let params_pos = PlacementParams {
            pos: Some((200, 300)),
            size: Some((400, 300)),
            workarea: wa,
            clamp: true,
            ..Default::default()
        };
        let rect = calculate_rect_from_placement(&params_pos, 400, 300);
        assert_eq!(rect.x, 200);
        assert_eq!(rect.y, 340);

        // 2. -p with --pivot relative to workarea top-left
        let params_pos_pivot = PlacementParams {
            pos: Some((200, 300)),
            pivot: Some(Pivot::Center),
            size: Some((400, 300)),
            workarea: wa,
            clamp: false,
            ..Default::default()
        };
        let rect = calculate_rect_from_placement(&params_pos_pivot, 400, 300);
        assert_eq!(rect.x, 200 - 200); // 0
        assert_eq!(rect.y, 340 - 150); // 190

        // 3. --anchor without --pivot defaults to matching anchor
        let params_anchor_center = PlacementParams {
            anchor: Some(Anchor::Center),
            size: Some((400, 300)),
            workarea: wa,
            clamp: true,
            ..Default::default()
        };
        let rect = calculate_rect_from_placement(&params_anchor_center, 400, 300);
        assert_eq!(rect.x, (1920 - 400) / 2);
        assert_eq!(rect.y, 40 + (1040 - 300) / 2);

        // 4. --anchor cursor without --pivot defaults to TopLeft
        let params_cursor = PlacementParams {
            anchor: Some(Anchor::Cursor),
            cursor_pos: Some((500, 400)),
            size: Some((300, 200)),
            workarea: wa,
            clamp: false,
            ..Default::default()
        };
        let rect = calculate_rect_from_placement(&params_cursor, 300, 200);
        assert_eq!(rect.x, 500);
        assert_eq!(rect.y, 400);

        // 5. --anchor with offset
        let params_anchor_offset = PlacementParams {
            anchor: Some(Anchor::TopLeft),
            offset: Some((25, 35)),
            size: Some((300, 200)),
            margin: 10,
            workarea: wa,
            clamp: false,
            ..Default::default()
        };
        let rect = calculate_rect_from_placement(&params_anchor_offset, 300, 200);
        assert_eq!(rect.x, wa.x + 10 + 25);
        assert_eq!(rect.y, wa.y + 10 + 35);

        // 6. Area::Screen vs Area::Workarea vs Area::Unbounded
        let params_screen = PlacementParams {
            anchor: Some(Anchor::TopLeft),
            size: Some((300, 200)),
            margin: 16,
            area: Some(Area::Screen),
            workarea: screen,
            clamp: true,
            ..Default::default()
        };
        let rect_screen = calculate_rect_from_placement(&params_screen, 300, 200);
        assert_eq!(rect_screen.x, 16);
        assert_eq!(rect_screen.y, 16);

        let params_wa = PlacementParams {
            anchor: Some(Anchor::TopLeft),
            size: Some((300, 200)),
            margin: 16,
            area: Some(Area::Workarea),
            workarea: wa,
            clamp: true,
            ..Default::default()
        };
        let rect_wa = calculate_rect_from_placement(&params_wa, 300, 200);
        assert_eq!(rect_wa.x, 16);
        assert_eq!(rect_wa.y, 40 + 16);

        // Unbounded area: global coordinates, no margin, no clamp
        let params_unbounded = PlacementParams {
            pos: Some((700, 300)),
            size: Some((300, 200)),
            margin: 16,
            area: Some(Area::Unbounded),
            workarea: wa,
            clamp: true,
            ..Default::default()
        };
        let rect_unbounded = calculate_rect_from_placement(&params_unbounded, 300, 200);
        assert_eq!(rect_unbounded.x, 700);
        assert_eq!(rect_unbounded.y, 300);

        // 7. Odd sizes (e.g. 501x333) with Center anchor
        let params_odd = PlacementParams {
            anchor: Some(Anchor::Center),
            size: Some((501, 333)),
            workarea: wa,
            clamp: true,
            ..Default::default()
        };
        let rect_odd = calculate_rect_from_placement(&params_odd, 501, 333);
        assert_eq!(rect_odd.x, (1920 - 501) / 2);
        assert_eq!(rect_odd.y, 40 + (1040 - 333) / 2);
    }

    #[test]
    fn test_resolve_monitor_from_layout_two_monitors_primary_not_index_zero() {
        let layout = MonitorLayout {
            schema_version: 1,
            monitors: vec![
                MonitorInfo {
                    index: 0,
                    name: Some("DP-1".into()),
                    primary: false,
                    scale: Some(1.0),
                    screen: Some(LayoutRect {
                        x: 0,
                        y: 0,
                        w: 1920,
                        h: 1080,
                    }),
                    workarea: Some(LayoutRect {
                        x: 0,
                        y: 40,
                        w: 1920,
                        h: 1040,
                    }),
                    insets: None,
                },
                MonitorInfo {
                    index: 1,
                    name: Some("HDMI-1".into()),
                    primary: true,
                    scale: Some(1.0),
                    screen: Some(LayoutRect {
                        x: 1920,
                        y: 0,
                        w: 1920,
                        h: 1080,
                    }),
                    workarea: Some(LayoutRect {
                        x: 1920,
                        y: 0,
                        w: 1920,
                        h: 1080,
                    }),
                    insets: None,
                },
            ],
            note: None,
        };

        // When --monitor is omitted (None), must pick monitor 1 (HDMI-1) because it is primary
        let (chosen, fallback) = resolve_monitor_from_layout(&layout, (0, 0), None);
        assert_eq!(chosen.unwrap().index, 1);
        assert!(!fallback);

        // When --monitor is explicitly "0", picks index 0
        let (chosen_0, _) = resolve_monitor_from_layout(&layout, (0, 0), Some("0"));
        assert_eq!(chosen_0.unwrap().index, 0);

        // When --monitor is explicitly "primary", picks index 1
        let (chosen_prim, _) = resolve_monitor_from_layout(&layout, (0, 0), Some("primary"));
        assert_eq!(chosen_prim.unwrap().index, 1);
    }

    #[test]
    fn test_resolve_monitor_from_layout_nothing_flagged_primary_falls_back() {
        let layout = MonitorLayout {
            schema_version: 1,
            monitors: vec![
                MonitorInfo {
                    index: 0,
                    name: Some("VGA-1".into()),
                    primary: false,
                    scale: Some(1.0),
                    screen: Some(LayoutRect {
                        x: 0,
                        y: 0,
                        w: 1024,
                        h: 768,
                    }),
                    workarea: Some(LayoutRect {
                        x: 0,
                        y: 0,
                        w: 1024,
                        h: 768,
                    }),
                    insets: None,
                },
                MonitorInfo {
                    index: 1,
                    name: Some("VGA-2".into()),
                    primary: false,
                    scale: Some(1.0),
                    screen: Some(LayoutRect {
                        x: 1024,
                        y: 0,
                        w: 1024,
                        h: 768,
                    }),
                    workarea: Some(LayoutRect {
                        x: 1024,
                        y: 0,
                        w: 1024,
                        h: 768,
                    }),
                    insets: None,
                },
            ],
            note: None,
        };

        // Neither monitor is flagged primary; falls back to index 0 with fallback flag true
        let (chosen, fallback) = resolve_monitor_from_layout(&layout, (0, 0), None);
        assert_eq!(chosen.unwrap().index, 0);
        assert!(fallback);
    }

    #[test]
    fn test_find_monitor_with_largest_overlap() {
        let layout = MonitorLayout {
            schema_version: 1,
            monitors: vec![
                MonitorInfo {
                    index: 0,
                    name: Some("DP-1".into()),
                    primary: true,
                    scale: Some(1.0),
                    screen: Some(LayoutRect {
                        x: 0,
                        y: 0,
                        w: 1920,
                        h: 1080,
                    }),
                    workarea: Some(LayoutRect {
                        x: 0,
                        y: 40,
                        w: 1920,
                        h: 1040,
                    }),
                    insets: None,
                },
                MonitorInfo {
                    index: 1,
                    name: Some("HDMI-1".into()),
                    primary: false,
                    scale: Some(1.0),
                    screen: Some(LayoutRect {
                        x: 1920,
                        y: 0,
                        w: 1920,
                        h: 1080,
                    }),
                    workarea: Some(LayoutRect {
                        x: 1920,
                        y: 0,
                        w: 1920,
                        h: 1080,
                    }),
                    insets: None,
                },
            ],
            note: None,
        };

        // Window mostly on monitor 1 (x: 1800, width: 400 -> 120px on mon 0, 280px on mon 1)
        let win = Rect {
            x: 1800,
            y: 100,
            width: 400,
            height: 300,
        };
        let best = find_monitor_with_largest_overlap(&layout, win).unwrap();
        assert_eq!(best.index, 1);
    }

    #[test]
    fn test_window_straddles_monitors_helper() {
        let monitors = [
            Rect {
                x: 0,
                y: 0,
                width: 1920,
                height: 1080,
            },
            Rect {
                x: 1920,
                y: 0,
                width: 1920,
                height: 1080,
            },
        ];

        let straddling = Rect {
            x: 1800,
            y: 100,
            width: 400,
            height: 300,
        };
        assert!(window_straddles_monitors(straddling, &monitors));

        let single = Rect {
            x: 100,
            y: 100,
            width: 400,
            height: 300,
        };
        assert!(!window_straddles_monitors(single, &monitors));
    }
}
