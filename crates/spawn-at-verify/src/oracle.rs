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
    pub margin_top: Option<i32>,
    pub margin_bottom: Option<i32>,
    pub margin_left: Option<i32>,
    pub margin_right: Option<i32>,
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
            margin_top: None,
            margin_bottom: None,
            margin_left: None,
            margin_right: None,
            clamp: true,
        }
    }
}

/// Computes the expected window rectangle given workarea bounds and placement parameters.
pub fn calculate_expected_rect(workarea: Rect, params: &OracleParams) -> Result<Rect, String> {
    let (w, h) = params.size;
    let w_i32 = w as i32;
    let h_i32 = h as i32;

    let mt = params.margin_top.unwrap_or(params.margin);
    let mb = params.margin_bottom.unwrap_or(params.margin);
    let ml = params.margin_left.unwrap_or(params.margin);
    let mr = params.margin_right.unwrap_or(params.margin);

    let (mut x, mut y) = if let Some((px, py)) = params.explicit_pos {
        let pivot_str = params.pivot.as_deref().unwrap_or("top-left");
        apply_pivot_offset(px, py, w, h, pivot_str)?
    } else {
        let anchor_str = params.anchor.as_deref().unwrap_or("center");
        let (wa_w, wa_h) = (workarea.w as i32, workarea.h as i32);

        // Documented semantic: Anchor determines reference point on the workarea boundary/margin.
        let (ax, ay) = match anchor_str {
            "top-left" => (workarea.x + ml, workarea.y + mt),
            "top" | "top-center" => (workarea.x + wa_w / 2, workarea.y + mt),
            "top-right" => (workarea.x + wa_w - mr, workarea.y + mt),
            "left" => (workarea.x + ml, workarea.y + wa_h / 2),
            "center" => (workarea.x + wa_w / 2, workarea.y + wa_h / 2),
            "right" => (workarea.x + wa_w - mr, workarea.y + wa_h / 2),
            "bottom-left" => (workarea.x + ml, workarea.y + wa_h - mb),
            "bottom" | "bottom-center" => (workarea.x + wa_w / 2, workarea.y + wa_h - mb),
            "bottom-right" => (workarea.x + wa_w - mr, workarea.y + wa_h - mb),
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
        let wa_w = workarea.w as i32;
        let wa_h = workarea.h as i32;

        let min_x = workarea.x + ml;
        let mut max_x = workarea.x + wa_w - mr - w_i32;
        if max_x < min_x {
            max_x = min_x;
        }
        x = x.clamp(min_x, max_x);

        let min_y = workarea.y + mt;
        let mut max_y = workarea.y + wa_h - mb - h_i32;
        if max_y < min_y {
            max_y = min_y;
        }
        y = y.clamp(min_y, max_y);
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

    // Hand-computed golden tests:
    // Work area: 1920x1040 at (0, 40)
    // README default margin: 16 (confirmed from README CLI reference: "-m, --margin <PX> Margin on all sides (default 16)")

    #[test]
    fn test_golden_bottom_right_338x93() {
        // Work area: x=0, y=40, w=1920, h=1040. Margin: 16. Window: 338x93.
        // Anchor: bottom-right
        //   ax = workarea.x + workarea.w - margin = 0 + 1920 - 16 = 1904
        //   ay = workarea.y + workarea.h - margin = 40 + 1040 - 16 = 1064
        // Pivot: default bottom-right
        //   x = ax - width = 1904 - 338 = 1566
        //   y = ay - height = 1064 - 93 = 971
        // Clamping checks:
        //   min_x = 0 + 16 = 16, max_x = 1920 - 16 - 338 = 1566 -> x clamped to 1566
        //   min_y = 40 + 16 = 56, max_y = 40 + 1040 - 16 - 93 = 971 -> y clamped to 971
        // Expected: (1566, 971, 338, 93)
        let wa = Rect::new(0, 40, 1920, 1040);
        let params = OracleParams {
            anchor: Some("bottom-right".into()),
            size: (338, 93),
            margin: 16,
            ..Default::default()
        };
        let rect = calculate_expected_rect(wa, &params).unwrap();
        assert_eq!(rect, Rect::new(1566, 971, 338, 93));
    }

    #[test]
    fn test_golden_bottom_right_30x20() {
        // Work area: x=0, y=40, w=1920, h=1040. Margin: 16. Window: 30x20.
        // Anchor: bottom-right
        //   ax = 0 + 1920 - 16 = 1904
        //   ay = 40 + 1040 - 16 = 1064
        // Pivot: bottom-right
        //   x = ax - width = 1904 - 30 = 1874
        //   y = ay - height = 1064 - 20 = 1044
        // Expected: (1874, 1044, 30, 20)
        let wa = Rect::new(0, 40, 1920, 1040);
        let params = OracleParams {
            anchor: Some("bottom-right".into()),
            size: (30, 20),
            margin: 16,
            ..Default::default()
        };
        let rect = calculate_expected_rect(wa, &params).unwrap();
        assert_eq!(rect, Rect::new(1874, 1044, 30, 20));
    }

    #[test]
    fn test_golden_bottom_right_391x368() {
        // Work area: x=0, y=40, w=1920, h=1040. Margin: 16. Window: 391x368.
        // Anchor: bottom-right
        //   ax = 1904, ay = 1064
        // Pivot: bottom-right
        //   x = ax - width = 1904 - 391 = 1513
        //   y = ay - height = 1064 - 368 = 696
        // Expected: (1513, 696, 391, 368)
        let wa = Rect::new(0, 40, 1920, 1040);
        let params = OracleParams {
            anchor: Some("bottom-right".into()),
            size: (391, 368),
            margin: 16,
            ..Default::default()
        };
        let rect = calculate_expected_rect(wa, &params).unwrap();
        assert_eq!(rect, Rect::new(1513, 696, 391, 368));
    }

    #[test]
    fn test_golden_bottom_left_338x93() {
        // Work area: x=0, y=40, w=1920, h=1040. Margin: 16. Window: 338x93.
        // Anchor: bottom-left
        //   ax = workarea.x + margin = 0 + 16 = 16
        //   ay = workarea.y + workarea.h - margin = 40 + 1040 - 16 = 1064
        // Pivot: default bottom-left
        //   x = ax = 16
        //   y = ay - height = 1064 - 93 = 971
        // Expected: (16, 971, 338, 93)
        let wa = Rect::new(0, 40, 1920, 1040);
        let params = OracleParams {
            anchor: Some("bottom-left".into()),
            size: (338, 93),
            margin: 16,
            ..Default::default()
        };
        let rect = calculate_expected_rect(wa, &params).unwrap();
        assert_eq!(rect, Rect::new(16, 971, 338, 93));
    }

    #[test]
    fn test_golden_top_left() {
        // Work area: x=0, y=40, w=1920, h=1040. Margin: 16. Window: 400x300.
        // Anchor: top-left
        //   ax = workarea.x + margin = 0 + 16 = 16
        //   ay = workarea.y + margin = 40 + 16 = 56
        // Pivot: default top-left
        //   x = ax = 16
        //   y = ay = 56
        // Expected: (16, 56, 400, 300)
        let wa = Rect::new(0, 40, 1920, 1040);
        let params = OracleParams {
            anchor: Some("top-left".into()),
            size: (400, 300),
            margin: 16,
            ..Default::default()
        };
        let rect = calculate_expected_rect(wa, &params).unwrap();
        assert_eq!(rect, Rect::new(16, 56, 400, 300));
    }

    #[test]
    fn test_golden_center_with_odd_sizes() {
        // Work area: x=0, y=40, w=1920, h=1040. Margin: 0. Window: 401x301 (odd sizes).
        // Anchor: center
        //   ax = workarea.x + workarea.w / 2 = 0 + 1920 / 2 = 960
        //   ay = workarea.y + workarea.h / 2 = 40 + 1040 / 2 = 560
        // Pivot: center
        //   x = ax - width / 2 = 960 - 401 / 2 = 960 - 200 = 760 (integer arithmetic)
        //   y = ay - height / 2 = 560 - 301 / 2 = 560 - 150 = 410 (integer arithmetic)
        // Expected: (760, 410, 401, 301)
        let wa = Rect::new(0, 40, 1920, 1040);
        let params = OracleParams {
            anchor: Some("center".into()),
            size: (401, 301),
            margin: 0,
            ..Default::default()
        };
        let rect = calculate_expected_rect(wa, &params).unwrap();
        assert_eq!(rect, Rect::new(760, 410, 401, 301));
    }

    #[test]
    fn test_golden_per_side_margins() {
        // Work area: x=0, y=40, w=1920, h=1040. Window: 100x100.
        // Directional margins: top=10, bottom=20, left=30, right=40.
        // Anchor: top-right
        //   ax = workarea.x + workarea.w - margin_right = 0 + 1920 - 40 = 1880
        //   ay = workarea.y + margin_top = 40 + 10 = 50
        // Pivot: top-right
        //   x = ax - width = 1880 - 100 = 1780
        //   y = ay = 50
        // Clamp checks:
        //   min_x = 0 + 30 = 30, max_x = 0 + 1920 - 40 - 100 = 1780 -> x clamped to 1780
        //   min_y = 40 + 10 = 50, max_y = 40 + 1040 - 20 - 100 = 960 -> y clamped to 50
        // Expected: (1780, 50, 100, 100)
        let wa = Rect::new(0, 40, 1920, 1040);
        let params = OracleParams {
            anchor: Some("top-right".into()),
            size: (100, 100),
            margin_top: Some(10),
            margin_bottom: Some(20),
            margin_left: Some(30),
            margin_right: Some(40),
            ..Default::default()
        };
        let rect = calculate_expected_rect(wa, &params).unwrap();
        assert_eq!(rect, Rect::new(1780, 50, 100, 100));
    }

    #[test]
    fn test_golden_negative_origin_monitor() {
        // Monitor placed to the left of primary monitor:
        // Work area: x=-1920, y=0, w=1920, h=1080. Window: 400x300. Margin: 16.
        // Anchor: bottom-right
        //   ax = workarea.x + workarea.w - margin = -1920 + 1920 - 16 = -16
        //   ay = workarea.y + workarea.h - margin = 0 + 1080 - 16 = 1064
        // Pivot: bottom-right
        //   x = ax - width = -16 - 400 = -416
        //   y = ay - height = 1064 - 300 = 764
        // Clamp bounds:
        //   min_x = -1920 + 16 = -1904, max_x = -1920 + 1920 - 16 - 400 = -416 -> x = -416
        //   min_y = 0 + 16 = 16, max_y = 0 + 1080 - 16 - 300 = 764 -> y = 764
        // Expected: (-416, 764, 400, 300)
        let wa = Rect::new(-1920, 0, 1920, 1080);
        let params = OracleParams {
            anchor: Some("bottom-right".into()),
            size: (400, 300),
            margin: 16,
            ..Default::default()
        };
        let rect = calculate_expected_rect(wa, &params).unwrap();
        assert_eq!(rect, Rect::new(-416, 764, 400, 300));
    }

    #[test]
    fn test_golden_offset_origin_workarea() {
        // Work area with non-zero offset: x=200, y=100, w=1600, h=900. Window: 300x200. Margin: 16.
        // Anchor: bottom-left
        //   ax = workarea.x + margin = 200 + 16 = 216
        //   ay = workarea.y + workarea.h - margin = 100 + 900 - 16 = 984
        // Pivot: bottom-left
        //   x = ax = 216
        //   y = ay - height = 984 - 200 = 784
        // Expected: (216, 784, 300, 200)
        let wa = Rect::new(200, 100, 1600, 900);
        let params = OracleParams {
            anchor: Some("bottom-left".into()),
            size: (300, 200),
            margin: 16,
            ..Default::default()
        };
        let rect = calculate_expected_rect(wa, &params).unwrap();
        assert_eq!(rect, Rect::new(216, 784, 300, 200));
    }

    #[test]
    fn test_golden_window_larger_than_workarea_clamp_false() {
        // Window 2000x1200 on 1920x1040 at (0, 40) with clamp=false. Margin: 16.
        // Anchor: center
        //   ax = 0 + 1920 / 2 = 960
        //   ay = 40 + 1040 / 2 = 560
        // Pivot: center
        //   x = ax - width / 2 = 960 - 2000 / 2 = 960 - 1000 = -40
        //   y = ay - height / 2 = 560 - 1200 / 2 = 560 - 600 = -40
        // Clamping is disabled (clamp=false), preserving raw negative coordinates.
        // Expected: (-40, -40, 2000, 1200)
        let wa = Rect::new(0, 40, 1920, 1040);
        let params = OracleParams {
            anchor: Some("center".into()),
            size: (2000, 1200),
            margin: 16,
            clamp: false,
            ..Default::default()
        };
        let rect = calculate_expected_rect(wa, &params).unwrap();
        assert_eq!(rect, Rect::new(-40, -40, 2000, 1200));
    }

    #[test]
    fn test_golden_window_larger_than_workarea_clamp_true() {
        // Window 2000x1200 on 1920x1040 at (0, 40) with clamp=true. Margin: 16.
        // Raw position from center anchor: (-40, -40).
        // Clamping arithmetic:
        //   min_x = 0 + 16 = 16
        //   max_x = 0 + 1920 - 16 - 2000 = -96
        //   Because max_x (-96) < min_x (16), oversized window max_x pins to min_x (16).
        //   Clamped x = (-40).clamp(16, 16) = 16.
        //
        //   min_y = 40 + 16 = 56
        //   max_y = 40 + 1040 - 16 - 1200 = -136
        //   Because max_y (-136) < min_y (56), oversized window max_y pins to min_y (56).
        //   Clamped y = (-40).clamp(56, 56) = 56.
        // Expected: (16, 56, 2000, 1200)
        let wa = Rect::new(0, 40, 1920, 1040);
        let params = OracleParams {
            anchor: Some("center".into()),
            size: (2000, 1200),
            margin: 16,
            clamp: true,
            ..Default::default()
        };
        let rect = calculate_expected_rect(wa, &params).unwrap();
        assert_eq!(rect, Rect::new(16, 56, 2000, 1200));
    }

    #[test]
    fn test_golden_clamp_explicit_pos_0_40() {
        // Live-confirmed case on host machine:
        // Window size: 367x514. Work area: 1920x1040 at (0, 40). Default margin: 16.
        // Command: `transform --pos 0 40`
        //
        // Arithmetic:
        // Work area bounds: wa.x = 0, wa.y = 40, wa.w = 1920, wa.h = 1040.
        // Margins: ml = 16, mr = 16, mt = 16, mb = 16.
        // Requested position (unclamped): px = 0, py = 40.
        // Clamp boundaries (work area minus margin):
        //   min_x = wa.x + ml = 0 + 16 = 16
        //   max_x = wa.x + wa.w - mr - win_w = 0 + 1920 - 16 - 367 = 1537
        //   min_y = wa.y + mt = 40 + 16 = 56
        //   max_y = wa.y + wa.h - mb - win_h = 40 + 1040 - 16 - 514 = 550
        //
        // Clamping:
        //   clamped_x = 0.clamp(16, 1537) = 16
        //   clamped_y = 40.clamp(56, 550) = 56
        // Expected rect: Rect(16, 56, 367, 514)
        let wa = Rect::new(0, 40, 1920, 1040);
        let params = OracleParams {
            explicit_pos: Some((0, 40)),
            size: (367, 514),
            margin: 16,
            clamp: true,
            ..Default::default()
        };
        let rect = calculate_expected_rect(wa, &params).unwrap();
        assert_eq!(rect, Rect::new(16, 56, 367, 514));
    }

    #[test]
    fn test_golden_clamp_explicit_pos_5000_5000() {
        // Live-confirmed case on host machine:
        // Window size: 367x514. Work area: 1920x1040 at (0, 40). Default margin: 16.
        // Command: `transform --pos 5000 5000`
        //
        // Arithmetic:
        // Work area bounds: wa.x = 0, wa.y = 40, wa.w = 1920, wa.h = 1040.
        // Margins: ml = 16, mr = 16, mt = 16, mb = 16.
        // Requested position (unclamped): px = 5000, py = 5000.
        // Clamp boundaries (work area minus margin):
        //   min_x = wa.x + ml = 0 + 16 = 16
        //   max_x = wa.x + wa.w - mr - win_w = 0 + 1920 - 16 - 367 = 1537
        //   min_y = wa.y + mt = 40 + 16 = 56
        //   max_y = wa.y + wa.h - mb - win_h = 40 + 1040 - 16 - 514 = 550
        //
        // Clamping:
        //   clamped_x = 5000.clamp(16, 1537) = 1537
        //   clamped_y = 5000.clamp(56, 550) = 550
        // Expected rect: Rect(1537, 550, 367, 514)
        let wa = Rect::new(0, 40, 1920, 1040);
        let params = OracleParams {
            explicit_pos: Some((5000, 5000)),
            size: (367, 514),
            margin: 16,
            clamp: true,
            ..Default::default()
        };
        let rect = calculate_expected_rect(wa, &params).unwrap();
        assert_eq!(rect, Rect::new(1537, 550, 367, 514));
    }

    #[test]
    fn test_golden_default_pivot_top() {
        // Anchor: "top" with no explicit pivot (pivot = None)
        // Window size: 400x300. Work area: 1920x1040 at (0, 40). Margin: 16.
        //
        // Arithmetic:
        // Screen anchor point for top:
        //   ax = wa.x + wa.w / 2 = 0 + 1920 / 2 = 960
        //   ay = wa.y + mt = 40 + 16 = 56
        // Default pivot for top (centered horizontally along the top margin):
        //   x = ax - win_w / 2 = 960 - 400 / 2 = 760
        //   y = ay = 56
        // Clamping:
        //   min_x = 16, max_x = 1920 - 16 - 400 = 1504 -> 760 is within bounds
        //   min_y = 56, max_y = 40 + 1040 - 16 - 300 = 764 -> 56 is within bounds
        // Expected rect: (760, 56, 400, 300)
        let wa = Rect::new(0, 40, 1920, 1040);
        let params = OracleParams {
            anchor: Some("top".into()),
            pivot: None,
            size: (400, 300),
            margin: 16,
            clamp: true,
            ..Default::default()
        };
        let rect = calculate_expected_rect(wa, &params).unwrap();
        assert_eq!(rect, Rect::new(760, 56, 400, 300));
    }

    #[test]
    fn test_golden_default_pivot_bottom() {
        // Anchor: "bottom" with no explicit pivot (pivot = None)
        // Window size: 400x300. Work area: 1920x1040 at (0, 40). Margin: 16.
        //
        // Arithmetic:
        // Screen anchor point for bottom:
        //   ax = wa.x + wa.w / 2 = 0 + 1920 / 2 = 960
        //   ay = wa.y + wa.h - mb = 40 + 1040 - 16 = 1064
        // Default pivot for bottom (centered horizontally along the bottom margin):
        //   x = ax - win_w / 2 = 960 - 400 / 2 = 760
        //   y = ay - win_h = 1064 - 300 = 764
        // Clamping:
        //   min_x = 16, max_x = 1504 -> 760 is within bounds
        //   min_y = 56, max_y = 764 -> 764 is within bounds
        // Expected rect: (760, 764, 400, 300)
        let wa = Rect::new(0, 40, 1920, 1040);
        let params = OracleParams {
            anchor: Some("bottom".into()),
            pivot: None,
            size: (400, 300),
            margin: 16,
            clamp: true,
            ..Default::default()
        };
        let rect = calculate_expected_rect(wa, &params).unwrap();
        assert_eq!(rect, Rect::new(760, 764, 400, 300));
    }

    #[test]
    fn test_golden_default_pivot_left() {
        // Anchor: "left" with no explicit pivot (pivot = None)
        // Window size: 400x300. Work area: 1920x1040 at (0, 40). Margin: 16.
        //
        // Arithmetic:
        // Screen anchor point for left:
        //   ax = wa.x + ml = 0 + 16 = 16
        //   ay = wa.y + wa.h / 2 = 40 + 1040 / 2 = 560
        // Default pivot for left (centered vertically along the left margin):
        //   x = ax = 16
        //   y = ay - win_h / 2 = 560 - 300 / 2 = 410
        // Clamping:
        //   min_x = 16, max_x = 1504 -> 16 is within bounds
        //   min_y = 56, max_y = 764 -> 410 is within bounds
        // Expected rect: (16, 410, 400, 300)
        let wa = Rect::new(0, 40, 1920, 1040);
        let params = OracleParams {
            anchor: Some("left".into()),
            pivot: None,
            size: (400, 300),
            margin: 16,
            clamp: true,
            ..Default::default()
        };
        let rect = calculate_expected_rect(wa, &params).unwrap();
        assert_eq!(rect, Rect::new(16, 410, 400, 300));
    }

    #[test]
    fn test_golden_default_pivot_right() {
        // Anchor: "right" with no explicit pivot (pivot = None)
        // Window size: 400x300. Work area: 1920x1040 at (0, 40). Margin: 16.
        //
        // Arithmetic:
        // Screen anchor point for right:
        //   ax = wa.x + wa.w - mr = 0 + 1920 - 16 = 1904
        //   ay = wa.y + wa.h / 2 = 40 + 1040 / 2 = 560
        // Default pivot for right (centered vertically along the right margin):
        //   x = ax - win_w = 1904 - 400 = 1504
        //   y = ay - win_h / 2 = 560 - 300 / 2 = 410
        // Clamping:
        //   min_x = 16, max_x = 1504 -> 1504 is within bounds
        //   min_y = 56, max_y = 764 -> 410 is within bounds
        // Expected rect: (1504, 410, 400, 300)
        let wa = Rect::new(0, 40, 1920, 1040);
        let params = OracleParams {
            anchor: Some("right".into()),
            pivot: None,
            size: (400, 300),
            margin: 16,
            clamp: true,
            ..Default::default()
        };
        let rect = calculate_expected_rect(wa, &params).unwrap();
        assert_eq!(rect, Rect::new(1504, 410, 400, 300));
    }

    #[test]
    fn test_golden_default_pivot_all_nine_anchors() {
        // Complete golden check for all 9 anchors with no explicit pivot (pivot = None).
        // Work area: 1920x1040 at (0, 40). Margin: 16. Window size: 400x300.
        //
        // 1. top-left: ax = 16, ay = 56 -> top-left pivot (16, 56)
        // 2. top: ax = 960, ay = 56 -> top pivot (960 - 200, 56) = (760, 56)
        // 3. top-right: ax = 1904, ay = 56 -> top-right pivot (1904 - 400, 56) = (1504, 56)
        // 4. left: ax = 16, ay = 560 -> left pivot (16, 560 - 150) = (16, 410)
        // 5. center: ax = 960, ay = 560 -> center pivot (960 - 200, 560 - 150) = (760, 410)
        // 6. right: ax = 1904, ay = 560 -> right pivot (1904 - 400, 560 - 150) = (1504, 410)
        // 7. bottom-left: ax = 16, ay = 1064 -> bottom-left pivot (16, 1064 - 300) = (16, 764)
        // 8. bottom: ax = 960, ay = 1064 -> bottom pivot (960 - 200, 1064 - 300) = (760, 764)
        // 9. bottom-right: ax = 1904, ay = 1064 -> bottom-right pivot (1904 - 400, 1064 - 300) = (1504, 764)
        let wa = Rect::new(0, 40, 1920, 1040);
        let golden_cases = [
            ("top-left", Rect::new(16, 56, 400, 300)),
            ("top", Rect::new(760, 56, 400, 300)),
            ("top-right", Rect::new(1504, 56, 400, 300)),
            ("left", Rect::new(16, 410, 400, 300)),
            ("center", Rect::new(760, 410, 400, 300)),
            ("right", Rect::new(1504, 410, 400, 300)),
            ("bottom-left", Rect::new(16, 764, 400, 300)),
            ("bottom", Rect::new(760, 764, 400, 300)),
            ("bottom-right", Rect::new(1504, 764, 400, 300)),
        ];

        for (anchor, expected) in golden_cases {
            let params = OracleParams {
                anchor: Some(anchor.into()),
                pivot: None,
                size: (400, 300),
                margin: 16,
                clamp: true,
                ..Default::default()
            };
            let rect = calculate_expected_rect(wa, &params).unwrap();
            assert_eq!(
                rect, expected,
                "Failed golden test for anchor '{}' with default pivot",
                anchor
            );
        }
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
