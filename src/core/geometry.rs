//! # Pure Mathematical Geometry Engine
//!
//! This module provides a platform-agnostic, pure mathematical coordinate and layout engine
//! with zero I/O, D-Bus, or compositor dependencies.
//!
//! ## Coordinate System Conventions
//! - **Origin `(0, 0)`**: Top-left of the primary display or virtual bounding box.
//! - **Axes**: X increases to the right, Y increases downwards.
//! - **Coordinate Spaces**: Coordinates can be absolute (global desktop bounds spanning multi-monitor setups) or relative.
//! - **Sign Conventions**: Coordinates (`x`, `y`) are signed `i32` to allow for negative monitor offsets, while dimensions (`width`, `height`) are unsigned `u32` to prevent impossible sizes.

use serde::{Deserialize, Serialize};
use crate::core::types::PlacementPayload;

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

/// Defines the origin point for window placement relative to a bounding workarea.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
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

/// Defines the boundary reference area for placement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum, Default)]
pub enum Area {
    #[default]
    Workarea,
    Screen,
}

/// Defines which corner of the window aligns with the target coordinate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum, Default)]
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
    /// The target coordinate aligns with the window's center.
    Center,
}

/// Input parameters for calculating final target window geometry.
#[derive(Debug, Clone, Default)]
pub struct GeometryParams {
    /// Explicit absolute screen coordinates (X, Y)
    pub pos: Option<(i32, i32)>,
    /// Relative offset from mouse cursor (X, Y)
    pub offset: Option<(i32, i32)>,
    /// Target window dimensions (Width, Height)
    pub size: Option<(u32, u32)>,
    /// Boundary distance from the top edge
    pub bound_top: Option<i32>,
    /// Boundary distance from the bottom edge
    pub bound_bottom: Option<i32>,
    /// Boundary distance from the left edge
    pub bound_left: Option<i32>,
    /// Boundary distance from the right edge
    pub bound_right: Option<i32>,
    /// Universal margin applied to all bounded edges
    pub margin: Option<i32>,
}

/// Target geometry for the window to be spawned.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct TargetGeometry {
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
    pub min_x: Option<i32>,
    pub max_x: Option<i32>,
    pub min_y: Option<i32>,
    pub max_y: Option<i32>,
}

/// Resolves the target workarea rectangle from a list of workareas, pointer coordinates,
/// and a monitor selector string (`<INDEX>`, `"cursor"`, or `"primary"`).
///
/// - If `monitor` is `"cursor"`, locates the workarea containing `(px, py)`.
///   Falls back to `workareas[0]` if the cursor is outside all workareas.
/// - If `monitor` is a numeric index (e.g. `"0"`, `"1"`), selects `workareas[index]`.
///   Falls back to `workareas[0]` if out of bounds.
/// - Defaults to `workareas[0]` (the primary monitor) for `"primary"` or any other input.
///
/// # Examples
/// ```
/// use spawn_at::core::geometry::{Rect, resolve_workarea};
/// let m1 = Rect { x: 0, y: 0, width: 1920, height: 1080 };
/// let m2 = Rect { x: 1920, y: 0, width: 2560, height: 1440 };
/// let workareas = vec![m1, m2];
/// 
/// assert_eq!(resolve_workarea(&workareas, (0, 0), "1").unwrap(), m2);
/// assert_eq!(resolve_workarea(&workareas, (2000, 500), "cursor").unwrap(), m2);
/// ```
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

/// Computes the top-left origin coordinates `(x, y)` for a window of size `(win_w, win_h)`
/// anchored inside `workarea` with a given `margin`.
///
/// # Examples
/// ```
/// use spawn_at::core::geometry::{Rect, Anchor, apply_anchor};
/// let wa = Rect { x: 0, y: 0, width: 1920, height: 1080 };
/// let (x, y) = apply_anchor(wa, 800, 600, Anchor::Center, 16);
/// assert_eq!(x, (1920 - 800) / 2);
/// assert_eq!(y, (1080 - 600) / 2);
/// ```
pub fn apply_anchor(
    workarea: Rect,
    win_w: u32,
    win_h: u32,
    anchor: Anchor,
    margin: i32,
) -> (i32, i32) {
    // Cast widths safely to i32 for coordinate math
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
        Anchor::Cursor => {
            (workarea.x, workarea.y)
        }
    }
}

/// Computes the top-left origin coordinates `(x, y)` when placing a window of size
/// `(win_w, win_h)` such that the specified `pivot` point aligns with `(target_x, target_y)`.
///
/// # Examples
/// ```
/// use spawn_at::core::geometry::{Pivot, apply_pivot};
/// let (x, y) = apply_pivot(500, 400, 200, 100, Pivot::Center);
/// assert_eq!((x, y), (400, 350));
/// ```
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
        Pivot::Center => (target_x - w / 2, target_y - h / 2),
    }
}

/// Clamps `rect.x` and `rect.y` so that the window stays strictly within the bounds,
/// accounting for a global `margin` and specific directional boundaries.
///
/// If `rect` is larger than the boundaries minus margins, it will pin to the top-left
/// `(bounds.x + left_bound, bounds.y + top_bound)` to prevent hiding the title bar.
///
/// # Examples
/// ```
/// use spawn_at::core::geometry::{Rect, clamp_to_bounds};
/// let bounds = Rect { x: 0, y: 0, width: 1920, height: 1080 };
/// let rect = Rect { x: 2000, y: 100, width: 400, height: 300 };
/// let clamped = clamp_to_bounds(rect, bounds, 0, 0, 0, 0, 10);
/// assert_eq!(clamped.x, 1920 - 400 - 10);
/// ```
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

    // If max_x is less than min_x, the window is too wide for the monitor minus margins.
    // In this case, we anchor to min_x to keep the left edge (and title bar) visible.
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

/// Pure mathematical calculation of target window geometry.
///
/// - If `params.pos` is specified, uses absolute coordinates.
/// - Otherwise, derives base coordinates from `cursor` (falling back to (0, 0)) + `params.offset`.
/// - If margin/boundary constraints are set and `monitors` are provided, clamps coordinates
///   within the active monitor containing the target and computes boundary limits.
pub fn calculate(
    params: &GeometryParams,
    cursor: Option<(i32, i32)>,
    monitors: &[Rect],
) -> TargetGeometry {
    // 1. Determine base target coordinates
    let (mut target_x, mut target_y) = if let Some((px, py)) = params.pos {
        (px, py)
    } else {
        let (cx, cy) = cursor.unwrap_or((0, 0));
        let (ox, oy) = params.offset.unwrap_or((0, 0));
        (cx + ox, cy + oy)
    };

    // 2. Determine target dimensions (0 implies client default / unconstrained)
    let (w, h) = params.size.unwrap_or((0, 0));

    // 3. Optional screen boundary clamping
    let has_bounds = params.margin.is_some()
        || params.bound_top.is_some()
        || params.bound_bottom.is_some()
        || params.bound_left.is_some()
        || params.bound_right.is_some();

    let mut min_x = None;
    let mut max_x = None;
    let mut min_y = None;
    let mut max_y = None;

    if has_bounds && !monitors.is_empty() {
        // Resolve active monitor by mimicking cursor behavior with the target coordinates
        let active_monitor = resolve_workarea(monitors, (target_x, target_y), "cursor").unwrap_or(monitors[0]);

        let bound_top = params.bound_top.unwrap_or(0);
        let bound_bottom = params.bound_bottom.unwrap_or(0);
        let bound_left = params.bound_left.unwrap_or(0);
        let bound_right = params.bound_right.unwrap_or(0);
        let margin = params.margin.unwrap_or(0);

        let top = bound_top + margin;
        let bottom = bound_bottom + margin;
        let left = bound_left + margin;
        let right = bound_right + margin;

        min_x = Some(active_monitor.x + left);
        max_x = Some(active_monitor.x + (active_monitor.width as i32) - right);
        min_y = Some(active_monitor.y + top);
        max_y = Some(active_monitor.y + (active_monitor.height as i32) - bottom);

        let clamped = clamp_to_bounds(
            Rect { x: target_x, y: target_y, width: w, height: h },
            active_monitor,
            bound_top,
            bound_bottom,
            bound_left,
            bound_right,
            margin,
        );
        target_x = clamped.x;
        target_y = clamped.y;
    }

    TargetGeometry {
        x: target_x,
        y: target_y,
        w,
        h,
        min_x,
        max_x,
        min_y,
        max_y,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_apply_anchor_center() {
        let wa = Rect { x: 0, y: 0, width: 1920, height: 1080 };
        let (x, y) = apply_anchor(wa, 800, 600, Anchor::Center, 16);
        assert_eq!(x, (1920 - 800) / 2);
        assert_eq!(y, (1080 - 600) / 2);
    }

    #[test]
    fn test_apply_anchor_corners() {
        let wa = Rect { x: 100, y: 50, width: 1920, height: 1080 };
        let margin = 20;
        let win_w = 400;
        let win_h = 300;

        assert_eq!(apply_anchor(wa, win_w, win_h, Anchor::TopLeft, margin), (120, 70));
        assert_eq!(apply_anchor(wa, win_w, win_h, Anchor::TopRight, margin), (100 + 1920 - 400 - 20, 70));
        assert_eq!(apply_anchor(wa, win_w, win_h, Anchor::BottomLeft, margin), (120, 50 + 1080 - 300 - 20));
        assert_eq!(apply_anchor(wa, win_w, win_h, Anchor::BottomRight, margin), (100 + 1920 - 400 - 20, 50 + 1080 - 300 - 20));
        assert_eq!(apply_anchor(wa, win_w, win_h, Anchor::Top, margin), (100 + (1920 - 400) / 2, 70));
        assert_eq!(apply_anchor(wa, win_w, win_h, Anchor::Bottom, margin), (100 + (1920 - 400) / 2, 50 + 1080 - 300 - 20));
        assert_eq!(apply_anchor(wa, win_w, win_h, Anchor::Left, margin), (120, 50 + (1080 - 300) / 2));
        assert_eq!(apply_anchor(wa, win_w, win_h, Anchor::Right, margin), (100 + 1920 - 400 - 20, 50 + (1080 - 300) / 2));
    }

    #[test]
    fn test_apply_pivot() {
        let target_x = 500;
        let target_y = 400;
        let win_w = 200;
        let win_h = 100;

        assert_eq!(apply_pivot(target_x, target_y, win_w, win_h, Pivot::TopLeft), (500, 400));
        assert_eq!(apply_pivot(target_x, target_y, win_w, win_h, Pivot::TopRight), (300, 400));
        assert_eq!(apply_pivot(target_x, target_y, win_w, win_h, Pivot::BottomLeft), (500, 300));
        assert_eq!(apply_pivot(target_x, target_y, win_w, win_h, Pivot::BottomRight), (300, 300));
        assert_eq!(apply_pivot(target_x, target_y, win_w, win_h, Pivot::Center), (400, 350));
    }

    #[test]
    fn test_clamp_to_bounds_within() {
        let wa = Rect { x: 0, y: 0, width: 1920, height: 1080 };
        let r = Rect { x: 100, y: 100, width: 500, height: 400 };
        let clamped = clamp_to_bounds(r, wa, 0, 0, 0, 0, 16);
        assert_eq!(clamped, r);
    }

    #[test]
    fn test_clamp_to_bounds_outside() {
        let wa = Rect { x: 0, y: 0, width: 1920, height: 1080 };
        let r = Rect { x: 2000, y: -50, width: 500, height: 400 };
        let clamped = clamp_to_bounds(r, wa, 0, 0, 0, 0, 16);
        assert_eq!(clamped, Rect { x: 1920 - 500 - 16, y: 16, width: 500, height: 400 });
    }

    #[test]
    fn test_clamp_to_bounds_oversized() {
        let wa = Rect { x: 0, y: 0, width: 1000, height: 800 };
        let r = Rect { x: 500, y: 500, width: 1200, height: 900 };
        let clamped = clamp_to_bounds(r, wa, 0, 0, 0, 0, 16);
        assert_eq!(clamped, Rect { x: 16, y: 16, width: 1200, height: 900 });
    }

    #[test]
    fn test_resolve_workarea() {
        let wa1 = Rect { x: 0, y: 0, width: 1920, height: 1080 };
        let wa2 = Rect { x: 1920, y: 0, width: 2560, height: 1440 };
        let workareas = vec![wa1, wa2];

        assert_eq!(resolve_workarea(&workareas, (0, 0), "0").unwrap(), wa1);
        assert_eq!(resolve_workarea(&workareas, (0, 0), "1").unwrap(), wa2);
        assert_eq!(resolve_workarea(&workareas, (0, 0), "99").unwrap(), wa1);
        assert_eq!(resolve_workarea(&workareas, (0, 0), "primary").unwrap(), wa1);
        assert_eq!(resolve_workarea(&workareas, (500, 300), "cursor").unwrap(), wa1);
        assert_eq!(resolve_workarea(&workareas, (2200, 500), "cursor").unwrap(), wa2);
        assert_eq!(resolve_workarea(&workareas, (9999, 9999), "cursor").unwrap(), wa1);
    }
    
    #[test]
    fn test_calculate_with_explicit_pos() {
        let params = GeometryParams {
            pos: Some((500, 300)),
            size: Some((800, 600)),
            ..Default::default()
        };
        let geom = calculate(&params, None, &[]);
        assert_eq!(geom, TargetGeometry { x: 500, y: 300, w: 800, h: 600, ..Default::default() });
    }

    #[test]
    fn test_calculate_with_cursor_and_offset() {
        let params = GeometryParams {
            offset: Some((50, -20)),
            size: Some((400, 300)),
            ..Default::default()
        };
        let geom = calculate(&params, Some((1000, 500)), &[]);
        assert_eq!(geom, TargetGeometry { x: 1050, y: 480, w: 400, h: 300, ..Default::default() });
    }

    #[test]
    fn test_calculate_with_boundary_limits() {
        let wa = Rect { x: 100, y: 50, width: 1920, height: 1080 };
        let params = GeometryParams {
            pos: Some((200, 200)),
            size: Some((800, 600)),
            margin: Some(40),
            bound_top: Some(10),
            ..Default::default()
        };
        let geom = calculate(&params, None, &[wa]);
        assert_eq!(geom.min_x, Some(100 + 40));
        assert_eq!(geom.max_x, Some(100 + 1920 - 40));
        assert_eq!(geom.min_y, Some(50 + 40 + 10));
        assert_eq!(geom.max_y, Some(50 + 1080 - 40));
    }
}


#[derive(Debug, Clone, Default)]
pub struct PlacementParams {
    pub pos: Option<(i32, i32)>,
    pub offset: Option<(i32, i32)>,
    pub anchor: Option<Anchor>,
    pub pivot: Pivot,
    pub size: Option<(u32, u32)>,
    pub margin: i32,
    pub margin_top: Option<i32>,
    pub margin_bottom: Option<i32>,
    pub margin_left: Option<i32>,
    pub margin_right: Option<i32>,
    pub area: Option<Area>,
    pub cursor_pos: Option<(i32, i32)>,
    pub workarea: Rect,
}

pub fn calculate_placement(params: PlacementParams, current_w: u32, current_h: u32) -> PlacementPayload {
    let intended_w = params.size.map(|s| s.0).unwrap_or(current_w);
    let intended_h = params.size.map(|s| s.1).unwrap_or(current_h);

    let wa = params.workarea;
    let margin_top = params.margin_top.unwrap_or(params.margin);
    let margin_bottom = params.margin_bottom.unwrap_or(params.margin);
    let margin_left = params.margin_left.unwrap_or(params.margin);
    let margin_right = params.margin_right.unwrap_or(params.margin);

    let offset_x = params.offset.map(|o| o.0).unwrap_or(0);
    let offset_y = params.offset.map(|o| o.1).unwrap_or(0);

    let (pivot_u, pivot_v, screen_anchor_x, screen_anchor_y) = if let Some(anchor) = params.anchor {
        match anchor {
            Anchor::Center => (
                0.5,
                0.5,
                wa.x + (wa.width as i32) / 2,
                wa.y + (wa.height as i32) / 2,
            ),
            Anchor::TopLeft => (
                0.0,
                0.0,
                wa.x + margin_left,
                wa.y + margin_top,
            ),
            Anchor::TopRight => (
                1.0,
                0.0,
                wa.x + (wa.width as i32) - margin_right,
                wa.y + margin_top,
            ),
            Anchor::BottomLeft => (
                0.0,
                1.0,
                wa.x + margin_left,
                wa.y + (wa.height as i32) - margin_bottom,
            ),
            Anchor::BottomRight => (
                1.0,
                1.0,
                wa.x + (wa.width as i32) - margin_right,
                wa.y + (wa.height as i32) - margin_bottom,
            ),
            Anchor::Top => (
                0.5,
                0.0,
                wa.x + (wa.width as i32) / 2,
                wa.y + margin_top,
            ),
            Anchor::Bottom => (
                0.5,
                1.0,
                wa.x + (wa.width as i32) / 2,
                wa.y + (wa.height as i32) - margin_bottom,
            ),
            Anchor::Left => (
                0.0,
                0.5,
                wa.x + margin_left,
                wa.y + (wa.height as i32) / 2,
            ),
            Anchor::Right => (
                1.0,
                0.5,
                wa.x + (wa.width as i32) - margin_right,
                wa.y + (wa.height as i32) / 2,
            ),
            Anchor::Cursor => {
                let (ax, ay) = if let Some(cursor) = params.cursor_pos {
                    (cursor.0, cursor.1)
                } else {
                    (wa.x + margin_left, wa.y + margin_top)
                };
                let (pu, pv) = match params.pivot {
                    Pivot::TopLeft => (0.0, 0.0),
                    Pivot::TopRight => (1.0, 0.0),
                    Pivot::BottomLeft => (0.0, 1.0),
                    Pivot::BottomRight => (1.0, 1.0),
                    Pivot::Center => (0.5, 0.5),
                };
                (pu, pv, ax, ay)
            }
        }
    } else {
        let (pu, pv) = match params.pivot {
            Pivot::TopLeft => (0.0, 0.0),
            Pivot::TopRight => (1.0, 0.0),
            Pivot::BottomLeft => (0.0, 1.0),
            Pivot::BottomRight => (1.0, 1.0),
            Pivot::Center => (0.5, 0.5),
        };

        let (ax, ay) = if let Some(pos) = params.pos {
            (pos.0, pos.1)
        } else if let Some(cursor) = params.cursor_pos {
            (cursor.0, cursor.1)
        } else {
            (wa.x + margin_left, wa.y + margin_top)
        };

        (pu, pv, ax, ay)
    };

    PlacementPayload {
        intended_w,
        intended_h,
        screen_anchor_x,
        screen_anchor_y,
        pivot_u,
        pivot_v,
        offset_x,
        offset_y,
        area: params.area.map(|a| match a {
            Area::Workarea => "workarea".to_string(),
            Area::Screen => "screen".to_string(),
        }),
        margin_top,
        margin_bottom,
        margin_left,
        margin_right,
    }
}
