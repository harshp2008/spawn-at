//! Independent Geometric Oracle for spawn-at-verify.
//!
//! Follows the documented project specifications for window anchors, pivots,
//! margins, and workarea boundaries strictly without depending on internal
//! implementation code in `spawn-at` or `spawn-at-core`.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
}

impl Rect {
    pub fn new(x: i32, y: i32, w: u32, h: u32) -> Self {
        Self { x, y, w, h }
    }
}

#[derive(Debug, Clone)]
pub struct OracleParams {
    pub anchor: Option<String>,
    pub pivot: Option<String>,
    pub explicit_pos: Option<(i32, i32)>,
    pub size: (u32, u32),
    pub margin: i32,
    pub clamp: bool,
}

impl Default for OracleParams {
    fn default() -> Self {
        Self {
            anchor: None,
            pivot: None,
            explicit_pos: None,
            size: (400, 300),
            margin: 0,
            clamp: true,
        }
    }
}

/// Computes the expected window rectangle given workarea bounds and placement parameters.
pub fn calculate_expected_rect(workarea: Rect, params: &OracleParams) -> Result<Rect, String> {
    let (w, h) = params.size;
    let w_i32 = w as i32;
    let h_i32 = h as i32;

    let (mut x, mut y) = if let Some((px, py)) = params.explicit_pos {
        let pivot_str = params.pivot.as_deref().unwrap_or("center");
        apply_pivot_offset(px, py, w, h, pivot_str)?
    } else {
        let anchor_str = params.anchor.as_deref().unwrap_or("center");
        let m = params.margin;
        let (wa_w, wa_h) = (workarea.w as i32, workarea.h as i32);

        // Documented semantic: Anchor determines reference point on the workarea boundary/margin.
        let (ax, ay) = match anchor_str {
            "top-left" => (workarea.x + m, workarea.y + m),
            "top" | "top-center" => (workarea.x + wa_w / 2, workarea.y + m),
            "top-right" => (workarea.x + wa_w - m, workarea.y + m),
            "left" => (workarea.x + m, workarea.y + wa_h / 2),
            "center" => (workarea.x + wa_w / 2, workarea.y + wa_h / 2),
            "right" => (workarea.x + wa_w - m, workarea.y + wa_h / 2),
            "bottom-left" => (workarea.x + m, workarea.y + wa_h - m),
            "bottom" | "bottom-center" => (workarea.x + wa_w / 2, workarea.y + wa_h - m),
            "bottom-right" => (workarea.x + wa_w - m, workarea.y + wa_h - m),
            other => return Err(format!("Unknown anchor '{}'", other)),
        };

        // If pivot is explicitly given, align window's pivot to anchor point.
        // Otherwise, use matching default pivot for the anchor.
        let pivot_str = if let Some(p) = &params.pivot {
            p.as_str()
        } else {
            anchor_str
        };

        apply_pivot_offset(ax, ay, w, h, pivot_str)?
    };

    if params.clamp {
        let m = params.margin;
        let inner_x = workarea.x + m;
        let inner_y = workarea.y + m;
        let inner_w = workarea.w as i32 - 2 * m;
        let inner_h = workarea.h as i32 - 2 * m;

        if w_i32 <= inner_w {
            let min_x = inner_x;
            let max_x = inner_x + inner_w - w_i32;
            x = x.clamp(min_x, max_x);
        } else {
            x = inner_x;
        }

        if h_i32 <= inner_h {
            let min_y = inner_y;
            let max_y = inner_y + inner_h - h_i32;
            y = y.clamp(min_y, max_y);
        } else {
            y = inner_y;
        }
    }

    Ok(Rect::new(x, y, w, h))
}

fn apply_pivot_offset(px: i32, py: i32, w: u32, h: u32, pivot: &str) -> Result<(i32, i32), String> {
    let w_i32 = w as i32;
    let h_i32 = h as i32;

    match pivot {
        "top-left" => Ok((px, py)),
        "top" | "top-center" => Ok((px - w_i32 / 2, py)),
        "top-right" => Ok((px - w_i32, py)),
        "left" => Ok((px, py - h_i32 / 2)),
        "center" => Ok((px - w_i32 / 2, py - h_i32 / 2)),
        "right" => Ok((px - w_i32, py - h_i32 / 2)),
        "bottom-left" => Ok((px, py - h_i32)),
        "bottom" | "bottom-center" => Ok((px - w_i32 / 2, py - h_i32)),
        "bottom-right" => Ok((px - w_i32, py - h_i32)),
        other => Err(format!("Unknown pivot '{}'", other)),
    }
}

/// Asserts that actual window bounds match expected bounds within the allowed tolerance in pixels.
pub fn verify_rect_tolerance(
    actual: Rect,
    expected: Rect,
    tolerance_px: i32,
) -> Result<(), String> {
    let dx = (actual.x - expected.x).abs();
    let dy = (actual.y - expected.y).abs();
    let dw = (actual.w as i32 - expected.w as i32).abs();
    let dh = (actual.h as i32 - expected.h as i32).abs();

    if dx <= tolerance_px && dy <= tolerance_px && dw <= tolerance_px && dh <= tolerance_px {
        Ok(())
    } else {
        Err(format!(
            "Geometry mismatch (tolerance {}px): expected [{}, {}, {}x{}], actual [{}, {}, {}x{}] (delta: dx={}, dy={}, dw={}, dh={})",
            tolerance_px,
            expected.x, expected.y, expected.w, expected.h,
            actual.x, actual.y, actual.w, actual.h,
            dx, dy, dw, dh
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bottom_right_anchor_with_margin() {
        let workarea = Rect::new(0, 40, 1920, 1040);
        let params = OracleParams {
            anchor: Some("bottom-right".into()),
            pivot: None,
            explicit_pos: None,
            size: (400, 300),
            margin: 16,
            clamp: true,
        };

        let rect = calculate_expected_rect(workarea, &params).unwrap();
        // x = 0 + 1920 - 16 - 400 = 1504
        // y = 40 + 1040 - 16 - 300 = 764
        assert_eq!(rect.x, 1504);
        assert_eq!(rect.y, 764);
        assert_eq!(rect.w, 400);
        assert_eq!(rect.h, 300);
    }

    #[test]
    fn test_center_anchor_with_offset_workarea() {
        let workarea = Rect::new(100, 50, 1800, 1000);
        let params = OracleParams {
            anchor: Some("center".into()),
            pivot: None,
            explicit_pos: None,
            size: (600, 400),
            margin: 0,
            clamp: true,
        };

        let rect = calculate_expected_rect(workarea, &params).unwrap();
        // x = 100 + (1800 - 600) / 2 = 100 + 600 = 700
        // y = 50 + (1000 - 400) / 2 = 50 + 300 = 350
        assert_eq!(rect.x, 700);
        assert_eq!(rect.y, 350);
        assert_eq!(rect.w, 600);
        assert_eq!(rect.h, 400);
    }

    #[test]
    fn test_verify_tolerance_exact_and_within_delta() {
        let expected = Rect::new(500, 400, 300, 200);
        let actual_exact = Rect::new(500, 400, 300, 200);
        let actual_1px = Rect::new(501, 399, 300, 201);
        let actual_far = Rect::new(505, 400, 300, 200);

        assert!(verify_rect_tolerance(actual_exact, expected, 1).is_ok());
        assert!(verify_rect_tolerance(actual_1px, expected, 1).is_ok());
        assert!(verify_rect_tolerance(actual_far, expected, 1).is_err());
    }
}
