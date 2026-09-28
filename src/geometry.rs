#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowInfo {
    pub id: String,
    pub wm_class: String,
    pub title: String,
}

/// Finds the monitor that contains the cursor. If none found, returns the first one,
/// or a default if monitors is empty.
pub fn find_active_monitor(cursor_x: i32, cursor_y: i32, monitors: &[Rect]) -> &Rect {
    for monitor in monitors {
        if cursor_x >= monitor.x
            && cursor_x < monitor.x + monitor.width
            && cursor_y >= monitor.y
            && cursor_y < monitor.y + monitor.height
        {
            return monitor;
        }
    }
    // Fallback to first monitor if out of bounds, or a dummy if none
    monitors.first().expect("No monitors found")
}

/// Clamps the given rectangle (x, y, w, h) strictly within the monitor bounds,
/// considering the provided bounds/margins.
/// Handles multi-monitor containment (window coordinates never cross into neighboring monitors).
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
    let mut max_x = monitor.x + monitor.width - right - w;
    let mut max_y = monitor.y + monitor.height - bottom - h;

    // Ensure we don't end up with max < min. If window is bigger than monitor, min wins.
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
    fn test_find_active_monitor() {
        let m1 = Rect {
            x: 0,
            y: 0,
            width: 1920,
            height: 1080,
        };
        let m2 = Rect {
            x: 1920,
            y: 0,
            width: 1920,
            height: 1080,
        };
        let monitors = vec![m1, m2];

        assert_eq!(find_active_monitor(100, 100, &monitors), &m1);
        assert_eq!(find_active_monitor(2000, 100, &monitors), &m2);

        // Out of bounds, falls back to first
        assert_eq!(find_active_monitor(-10, -10, &monitors), &m1);
    }

    #[test]
    fn test_clamp_to_monitor() {
        let monitor = Rect {
            x: 1920,
            y: 0,
            width: 1920,
            height: 1080,
        };

        // Window inside bounds
        let (cx, cy) = clamp_to_monitor(2000, 100, 800, 600, &monitor, 0, 0, 0, 0, 0);
        assert_eq!((cx, cy), (2000, 100));

        // Window crossing right boundary
        let (cx, cy) = clamp_to_monitor(3500, 100, 800, 600, &monitor, 0, 0, 0, 0, 0);
        // max_x = 1920 + 1920 - 800 = 3040
        assert_eq!((cx, cy), (3040, 100));

        // Crossing left boundary
        let (cx, cy) = clamp_to_monitor(1800, 100, 800, 600, &monitor, 0, 0, 0, 0, 0);
        assert_eq!((cx, cy), (1920, 100));

        // With margins
        let (cx, cy) = clamp_to_monitor(1800, 100, 800, 600, &monitor, 10, 20, 30, 40, 5);
        // min_x = 1920 + 30 + 5 = 1955
        // min_y = 0 + 10 + 5 = 15 (though y is 100, so it will be 100)
        assert_eq!(cx, 1955);
        assert_eq!(cy, 100);

        // Window bigger than monitor with margins
        let (cx, cy) = clamp_to_monitor(1920, 0, 2000, 1080, &monitor, 0, 0, 0, 0, 0);
        assert_eq!((cx, cy), (1920, 0)); // since max is applied before min, min wins
    }
}
