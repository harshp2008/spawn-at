//! # Geometry Engine
//!
//! A pure mathematical engine with zero compositor or OS dependencies for screen
//! coordinate calculations, multi-monitor / workarea resolution, anchor placement,
//! and boundary clamping.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Anchor {
    Center,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

/// Resolves the target workarea rectangle from a list of workareas, pointer coordinates,
/// and a monitor selector string (`<INDEX>`, `"cursor"`, or `"primary"`).
///
/// - If `monitor` is `"cursor"`, locates the workarea containing `(px, py)`.
///   Falls back to `workareas[0]` if the cursor is outside all workareas.
/// - If `monitor` is a numeric index (e.g. `"0"`, `"1"`), selects `workareas[index]`.
///   Falls back to `workareas[0]` if out of bounds.
/// - Defaults to `workareas[0]` (the primary monitor) for `"primary"` or any other input.
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
            if px >= wa.x && px < wa.x + wa.w && py >= wa.y && py < wa.y + wa.h {
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
    win_w: i32,
    win_h: i32,
    anchor: Anchor,
    margin: i32,
) -> (i32, i32) {
    match anchor {
        Anchor::Center => {
            let x = workarea.x + (workarea.w - win_w) / 2;
            let y = workarea.y + (workarea.h - win_h) / 2;
            (x, y)
        }
        Anchor::TopLeft => {
            let x = workarea.x + margin;
            let y = workarea.y + margin;
            (x, y)
        }
        Anchor::TopRight => {
            let x = workarea.x + workarea.w - win_w - margin;
            let y = workarea.y + margin;
            (x, y)
        }
        Anchor::BottomLeft => {
            let x = workarea.x + margin;
            let y = workarea.y + workarea.h - win_h - margin;
            (x, y)
        }
        Anchor::BottomRight => {
            let x = workarea.x + workarea.w - win_w - margin;
            let y = workarea.y + workarea.h - win_h - margin;
            (x, y)
        }
    }
}

/// Clamps `rect.x` and `rect.y` so that the window stays strictly within the workarea
/// bounds, accounting for `margin`.
///
/// - Bounds: `[workarea.x + margin, workarea.x + workarea.w - rect.w - margin]` and
///           `[workarea.y + margin, workarea.y + workarea.h - rect.h - margin]`.
/// - If `rect.w > workarea.w - 2 * margin` or `rect.h > workarea.h - 2 * margin`,
///   pins the coordinate to `(workarea.x + margin, workarea.y + margin)` to prevent
///   hiding the title bar or window headers.
pub fn clamp_rect(rect: Rect, workarea: Rect, margin: i32) -> Rect {
    let min_x = workarea.x + margin;
    let max_x = workarea.x + workarea.w - rect.w - margin;
    let min_y = workarea.y + margin;
    let max_y = workarea.y + workarea.h - rect.h - margin;

    let clamped_x = if rect.w > workarea.w - 2 * margin {
        min_x
    } else {
        rect.x.clamp(min_x, max_x)
    };

    let clamped_y = if rect.h > workarea.h - 2 * margin {
        min_y
    } else {
        rect.y.clamp(min_y, max_y)
    };

    Rect {
        x: clamped_x,
        y: clamped_y,
        w: rect.w,
        h: rect.h,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_apply_anchor_center() {
        let wa = Rect {
            x: 0,
            y: 0,
            w: 1920,
            h: 1080,
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
            w: 1920,
            h: 1080,
        };
        let margin = 20;
        let win_w = 400;
        let win_h = 300;

        // TopLeft
        assert_eq!(
            apply_anchor(wa, win_w, win_h, Anchor::TopLeft, margin),
            (100 + 20, 50 + 20)
        );

        // TopRight
        assert_eq!(
            apply_anchor(wa, win_w, win_h, Anchor::TopRight, margin),
            (100 + 1920 - 400 - 20, 50 + 20)
        );

        // BottomLeft
        assert_eq!(
            apply_anchor(wa, win_w, win_h, Anchor::BottomLeft, margin),
            (100 + 20, 50 + 1080 - 300 - 20)
        );

        // BottomRight
        assert_eq!(
            apply_anchor(wa, win_w, win_h, Anchor::BottomRight, margin),
            (100 + 1920 - 400 - 20, 50 + 1080 - 300 - 20)
        );
    }

    #[test]
    fn test_clamp_rect_within_bounds() {
        let wa = Rect {
            x: 0,
            y: 0,
            w: 1920,
            h: 1080,
        };
        let r = Rect {
            x: 100,
            y: 100,
            w: 500,
            h: 400,
        };
        let clamped = clamp_rect(r, wa, 16);
        assert_eq!(clamped, r);
    }

    #[test]
    fn test_clamp_rect_outside_bounds() {
        let wa = Rect {
            x: 0,
            y: 0,
            w: 1920,
            h: 1080,
        };
        let r = Rect {
            x: 2000,
            y: -50,
            w: 500,
            h: 400,
        };
        let clamped = clamp_rect(r, wa, 16);
        assert_eq!(
            clamped,
            Rect {
                x: 1920 - 500 - 16,
                y: 16,
                w: 500,
                h: 400
            }
        );
    }

    #[test]
    fn test_clamp_rect_oversized_pins_topleft() {
        let wa = Rect {
            x: 0,
            y: 0,
            w: 1000,
            h: 800,
        };
        let r = Rect {
            x: 500,
            y: 500,
            w: 1200,
            h: 900,
        };
        let clamped = clamp_rect(r, wa, 16);
        assert_eq!(
            clamped,
            Rect {
                x: 16,
                y: 16,
                w: 1200,
                h: 900
            }
        );
    }

    #[test]
    fn test_resolve_workarea() {
        let wa1 = Rect {
            x: 0,
            y: 0,
            w: 1920,
            h: 1080,
        };
        let wa2 = Rect {
            x: 1920,
            y: 0,
            w: 2560,
            h: 1440,
        };
        let workareas = vec![wa1, wa2];

        // By index
        assert_eq!(resolve_workarea(&workareas, (0, 0), "0").unwrap(), wa1);
        assert_eq!(resolve_workarea(&workareas, (0, 0), "1").unwrap(), wa2);
        assert_eq!(resolve_workarea(&workareas, (0, 0), "99").unwrap(), wa1);

        // By primary
        assert_eq!(resolve_workarea(&workareas, (0, 0), "primary").unwrap(), wa1);

        // By cursor
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
}
