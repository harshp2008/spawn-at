//! # Screen Geometry & Placement Calculation Engine
//!
//! A pure mathematical engine for calculating screen coordinates, mouse cursor offsets,
//! multi-monitor containment, and margin clamping for window cold-starts.
//!
//! This module contains zero I/O, zero D-Bus calls, and zero external subprocess invocations.
//! All environment inputs (pointer coordinates and monitor geometries) are provided
//! by the caller.

use crate::drivers::TargetGeometry;

/// Rectangle representing a screen or monitor bounding box.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
}

impl Rect {
    pub fn clamp_to_bounds(&self, bounds: &Rect, margin: u32) -> Rect {
        let margin = margin as i32;
        let mut clamped = self.clone();

        // Target boundaries
        let min_x = bounds.x + margin;
        let min_y = bounds.y + margin;
        let max_x = bounds.x + bounds.w as i32 - margin - self.w as i32;
        let max_y = bounds.y + bounds.h as i32 - margin - self.h as i32;

        // If the window is larger than bounds (minus margins), anchor it to the top-left (min bounds).
        // Otherwise, clamp between min and max.
        clamped.x = clamped.x.clamp(min_x, max_x.max(min_x));
        clamped.y = clamped.y.clamp(min_y, max_y.max(min_y));

        clamped
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowInfo {
    pub id: String,
    pub wm_class: String,
    pub title: String,
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
    pub bound_top: Option<i32>,
    pub bound_bottom: Option<i32>,
    pub bound_left: Option<i32>,
    pub bound_right: Option<i32>,
    pub margin: Option<i32>,
}

/// Pure mathematical calculation of target window geometry.
///
/// - If `params.pos` is specified, uses absolute coordinates.
/// - Otherwise, derives base coordinates from `cursor` (falling back to (0, 0)) + `params.offset`.
/// - If margin/boundary constraints are set and `monitors` are provided, clamps coordinates
///   within the active monitor containing the target.
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

    if has_bounds && !monitors.is_empty() {
        let active_monitor = find_active_monitor(target_x, target_y, monitors);
        let (cx, cy) = clamp_to_monitor(
            target_x,
            target_y,
            w as i32,
            h as i32,
            active_monitor,
            params.bound_top.unwrap_or(0),
            params.bound_bottom.unwrap_or(0),
            params.bound_left.unwrap_or(0),
            params.bound_right.unwrap_or(0),
            params.margin.unwrap_or(0),
        );
        target_x = cx;
        target_y = cy;
    }

    TargetGeometry {
        x: target_x,
        y: target_y,
        w,
        h,
    }
}

/// Finds the monitor that contains the specified point (X, Y).
/// If out of bounds or none match, returns the first monitor.
pub fn find_active_monitor<'a>(cursor_x: i32, cursor_y: i32, monitors: &'a [Rect]) -> &'a Rect {
    for monitor in monitors {
        if cursor_x >= monitor.x
            && cursor_x < monitor.x + monitor.w as i32
            && cursor_y >= monitor.y
            && cursor_y < monitor.y + monitor.h as i32
        {
            return monitor;
        }
    }
    monitors.first().expect("monitors slice must not be empty")
}

/// Clamps the given rectangle strictly within the monitor bounds,
/// accounting for directional bounds and global margin.
pub fn clamp_to_monitor(
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    monitor: &Rect,
    bound_top: i32,
    bound_bottom: i32,
    bound_left: i32,
    bound_right: i32,
    global_margin: i32,
) -> (i32, i32) {
    let top = bound_top + global_margin;
    let bottom = bound_bottom + global_margin;
    let left = bound_left + global_margin;
    let right = bound_right + global_margin;

    let min_x = monitor.x + left;
    let min_y = monitor.y + top;
    let mut max_x = monitor.x + monitor.w as i32 - right - w;
    let mut max_y = monitor.y + monitor.h as i32 - bottom - h;

    if max_x < min_x {
        max_x = min_x;
    }
    if max_y < min_y {
        max_y = min_y;
    }

    let clamped_x = x.clamp(min_x, max_x);
    let clamped_y = y.clamp(min_y, max_y);

    (clamped_x, clamped_y)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_with_explicit_pos() {
        let params = GeometryParams {
            pos: Some((500, 300)),
            size: Some((800, 600)),
            ..Default::default()
        };
        let geom = calculate(&params, None, &[]);
        assert_eq!(
            geom,
            TargetGeometry {
                x: 500,
                y: 300,
                w: 800,
                h: 600,
            }
        );
    }

    #[test]
    fn test_calculate_with_cursor_and_offset() {
        let params = GeometryParams {
            offset: Some((50, -20)),
            size: Some((400, 300)),
            ..Default::default()
        };
        let geom = calculate(&params, Some((1000, 500)), &[]);
        assert_eq!(
            geom,
            TargetGeometry {
                x: 1050,
                y: 480,
                w: 400,
                h: 300,
            }
        );
    }

    #[test]
    fn test_clamp_to_monitor() {
        let monitor = Rect {
            x: 0,
            y: 0,
            w: 1920,
            h: 1080,
        };

        let (cx, cy) = clamp_to_monitor(2000, 1200, 800, 600, &monitor, 0, 0, 0, 0, 0);
        assert_eq!((cx, cy), (1120, 480));
    }

    #[test]
    fn test_find_active_monitor_multi() {
        let m1 = Rect {
            x: 0,
            y: 0,
            w: 1920,
            h: 1080,
        };
        let m2 = Rect {
            x: 1920,
            y: 0,
            w: 1920,
            h: 1080,
        };
        let monitors = vec![m1, m2];

        assert_eq!(find_active_monitor(500, 500, &monitors), &m1);
        assert_eq!(find_active_monitor(2500, 500, &monitors), &m2);
        assert_eq!(find_active_monitor(5000, 5000, &monitors), &m1); // fallback
    }

    #[test]
    fn test_clamp_to_bounds() {
        let monitor = Rect { x: 0, y: 0, w: 1920, h: 1080 };
        
        // Window inside bounds
        let r1 = Rect { x: 100, y: 100, w: 400, h: 300 };
        let c1 = r1.clamp_to_bounds(&monitor, 10);
        assert_eq!(c1, r1);
        
        // Window outside right bound
        let r2 = Rect { x: 2000, y: 100, w: 400, h: 300 };
        let c2 = r2.clamp_to_bounds(&monitor, 10);
        assert_eq!(c2, Rect { x: 1920 - 400 - 10, y: 100, w: 400, h: 300 });

        // Window outside top bound
        let r3 = Rect { x: 100, y: -500, w: 400, h: 300 };
        let c3 = r3.clamp_to_bounds(&monitor, 10);
        assert_eq!(c3, Rect { x: 100, y: 10, w: 400, h: 300 });
    }
}
