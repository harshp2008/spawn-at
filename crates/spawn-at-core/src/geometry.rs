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

/// Defines the boundary reference area for placement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "clap", derive(clap::ValueEnum))]
pub enum Area {
    #[default]
    Workarea,
    Screen,
}

/// Defines which corner of the window aligns with the target coordinate.
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
    /// The target coordinate aligns with the window's center.
    Center,
}

/// Input parameters for calculating final target window geometry.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
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

/// Declarative parameters describing spatial positioning, alignment, and bounding box constraints.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlacementParams {
    pub pos: Option<(i32, i32)>,
    pub offset: Option<(i32, i32)>,
    pub anchor: Option<Anchor>,
    pub pivot: Option<Pivot>,
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

/// Resolves the target workarea rectangle from a list of workareas, pointer coordinates,
/// and a monitor selector string (`<INDEX>`, `"cursor"`, or `"primary"`).
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

/// Computes the top-left origin coordinates `(x, y)` when placing a window of size
/// `(win_w, win_h)` such that the specified `pivot` point aligns with `(target_x, target_y)`.
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

/// Pure mathematical calculation of target window geometry.
pub fn calculate(
    params: &GeometryParams,
    cursor: Option<(i32, i32)>,
    monitors: &[Rect],
) -> TargetGeometry {
    let (mut target_x, mut target_y) = if let Some((px, py)) = params.pos {
        (px, py)
    } else {
        let (cx, cy) = cursor.unwrap_or((0, 0));
        let (ox, oy) = params.offset.unwrap_or((0, 0));
        (cx + ox, cy + oy)
    };

    let (w, h) = params.size.unwrap_or((0, 0));

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

/// Diagnostic messages generated during geometry resolution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GeometryDiagnostic {
    /// Window size exceeds the available workarea space after margins.
    Oversized { w: u32, h: u32 },
    /// Window origin coordinates were adjusted due to boundary margin clamping.
    Repositioned {
        raw_x: i32,
        raw_y: i32,
        clamped_x: i32,
        clamped_y: i32,
    },
    /// Window size is smaller than typical toolkit constraints.
    SubMinimumSize { w: u32, h: u32 },
}

/// Evaluates spatial placement parameters against the bounding workarea and returns any diagnostics.
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
                    Anchor::Center => (
                        wa.x + (wa.width as i32) / 2,
                        wa.y + (wa.height as i32) / 2,
                    ),
                    Anchor::TopLeft => (
                        wa.x + ml,
                        wa.y + mt,
                    ),
                    Anchor::TopRight => (
                        wa.x + (wa.width as i32) - mr,
                        wa.y + mt,
                    ),
                    Anchor::BottomLeft => (
                        wa.x + ml,
                        wa.y + (wa.height as i32) - mb,
                    ),
                    Anchor::BottomRight => (
                        wa.x + (wa.width as i32) - mr,
                        wa.y + (wa.height as i32) - mb,
                    ),
                    Anchor::Top => (
                        wa.x + (wa.width as i32) / 2,
                        wa.y + mt,
                    ),
                    Anchor::Bottom => (
                        wa.x + (wa.width as i32) / 2,
                        wa.y + (wa.height as i32) - mb,
                    ),
                    Anchor::Left => (
                        wa.x + ml,
                        wa.y + (wa.height as i32) / 2,
                    ),
                    Anchor::Right => (
                        wa.x + (wa.width as i32) - mr,
                        wa.y + (wa.height as i32) / 2,
                    ),
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
            } else if let Some(cursor) = params.cursor_pos {
                (cursor.0, cursor.1)
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

    #[test]
    fn test_diagnostics_oversized() {
        let wa = Rect { x: 0, y: 0, width: 1920, height: 1080 };
        let params = PlacementParams {
            size: Some((3000, 2000)),
            anchor: Some(Anchor::BottomRight),
            pivot: Some(Pivot::BottomRight),
            margin: 20,
            workarea: wa,
            ..Default::default()
        };
        let diags = check_geometry_diagnostics(&params);
        assert_eq!(diags, vec![GeometryDiagnostic::Oversized { w: 3000, h: 2000 }]);
    }

    #[test]
    fn test_diagnostics_sub_minimum() {
        let wa = Rect { x: 0, y: 0, width: 1920, height: 1080 };
        let params = PlacementParams {
            size: Some((30, 20)),
            anchor: Some(Anchor::BottomRight),
            pivot: Some(Pivot::BottomRight),
            margin: 20,
            workarea: wa,
            ..Default::default()
        };
        let diags = check_geometry_diagnostics(&params);
        assert_eq!(diags, vec![GeometryDiagnostic::SubMinimumSize { w: 30, h: 20 }]);
    }

    #[test]
    fn test_diagnostics_repositioned() {
        let wa = Rect { x: 0, y: 0, width: 1920, height: 1080 };
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
        let wa = Rect { x: 100, y: 200, width: 1920, height: 1080 };
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
        let bounds = Rect { x: -1920, y: 0, width: 1920, height: 1080 }; // Left monitor
        let normal_rect = Rect { x: -1000, y: 200, width: 500, height: 400 };

        // Inside bounds: untouched
        let clamped = clamp_to_bounds(normal_rect, bounds, 0, 0, 0, 0, 0);
        assert_eq!(clamped, normal_rect);

        // Overflow left: clamped to bounds.x (-1920)
        let overflow_left = Rect { x: -2500, y: 200, width: 500, height: 400 };
        let clamped_left = clamp_to_bounds(overflow_left, bounds, 0, 0, 0, 0, 10);
        assert_eq!(clamped_left.x, -1920 + 10);

        // Overflow right: clamped to bounds.x + width - win_w
        let overflow_right = Rect { x: 500, y: 200, width: 500, height: 400 };
        let clamped_right = clamp_to_bounds(overflow_right, bounds, 0, 0, 0, 0, 10);
        assert_eq!(clamped_right.x, -1920 + 1920 - 10 - 500); // -510

        // Window larger than workarea (2500px wide in 1920px monitor)
        let huge_rect = Rect { x: 0, y: 0, width: 2500, height: 1500 };
        let clamped_huge = clamp_to_bounds(huge_rect, bounds, 0, 0, 0, 0, 16);
        // max_x < min_x -> pins to min_x
        assert_eq!(clamped_huge.x, -1920 + 16);
        assert_eq!(clamped_huge.y, 0 + 16);
    }

    #[test]
    fn test_resolve_workarea_multi_monitor_and_errors() {
        let monitors = vec![
            Rect { x: 0, y: 0, width: 1920, height: 1080 },        // Monitor 0: Primary
            Rect { x: 1920, y: 0, width: 2560, height: 1440 },     // Monitor 1: Right
            Rect { x: -1920, y: 0, width: 1920, height: 1080 },    // Monitor 2: Left
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
        assert_eq!(resolve_workarea(&monitors, (0, 0), "primary").unwrap(), monitors[0]);
        assert_eq!(resolve_workarea(&monitors, (0, 0), "0").unwrap(), monitors[0]);
        assert_eq!(resolve_workarea(&monitors, (0, 0), "1").unwrap(), monitors[1]);
        assert_eq!(resolve_workarea(&monitors, (0, 0), "2").unwrap(), monitors[2]);

        // Out-of-bounds index falls back to primary monitor (monitors[0])
        assert_eq!(resolve_workarea(&monitors, (0, 0), "99").unwrap(), monitors[0]);
        // Unrecognized string falls back to primary monitor
        assert_eq!(resolve_workarea(&monitors, (0, 0), "invalid_spec").unwrap(), monitors[0]);
        // Empty monitor list returns Err
        assert!(resolve_workarea(&[], (0, 0), "primary").is_err());
    }

    #[test]
    fn test_50_permutations_anchor_cross_pivot_matrix() {
        let wa = Rect { x: 0, y: 0, width: 1920, height: 1080 };
        let anchors = [
            Anchor::Center, Anchor::TopLeft, Anchor::TopRight,
            Anchor::BottomLeft, Anchor::BottomRight, Anchor::Top,
            Anchor::Bottom, Anchor::Left, Anchor::Right, Anchor::Cursor,
        ];
        let pivots = [
            Pivot::TopLeft, Pivot::TopRight, Pivot::BottomLeft,
            Pivot::BottomRight, Pivot::Center,
        ];

        let mut count = 0;
        for anchor in &anchors {
            for pivot in &pivots {
                let (ax, ay) = apply_anchor(wa, 800, 600, *anchor, 20);
                let (px, py) = apply_pivot(ax, ay, 800, 600, *pivot);
                assert!(px >= -10000 && px <= 10000);
                assert!(py >= -10000 && py <= 10000);
                count += 1;
            }
        }
        assert_eq!(count, 50);
    }
}
