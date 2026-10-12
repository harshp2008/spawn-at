//! Mathematical placement calculations, anchor formulas, and pivot offsets.

/// 2D rectangle representing window coordinates and dimensions in oracle calculations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Rect {
    /// Horizontal coordinate (left edge)
    pub x: i32,
    /// Vertical coordinate (top edge)
    pub y: i32,
    /// Width in pixels
    pub w: u32,
    /// Height in pixels
    pub h: u32,
}

impl Rect {
    /// Constructs a new `Rect` with the specified coordinates and dimensions.
    pub fn new(x: i32, y: i32, w: u32, h: u32) -> Self {
        Self { x, y, w, h }
    }
}

/// Configuration parameters provided to the oracle placement calculator.
#[derive(Debug, Clone)]
pub struct OracleParams {
    /// Target anchor position on screen/monitor/workarea.
    pub anchor: Option<String>,
    /// Window pivot alignment point.
    pub pivot: Option<String>,
    /// Explicit target (x, y) coordinates.
    pub explicit_pos: Option<(i32, i32)>,
    /// Placement offset vector (dx, dy).
    pub offset: Option<(i32, i32)>,
    /// Reference bounding area ("workarea", "monitor", "screen").
    pub area: Option<String>,
    /// Current cursor coordinate (x, y).
    pub cursor_pos: Option<(i32, i32)>,
    /// Window client size (width, height).
    pub size: (u32, u32),
    /// Uniform boundary margin in pixels.
    pub margin: i32,
    /// Top margin override.
    pub margin_top: Option<i32>,
    /// Bottom margin override.
    pub margin_bottom: Option<i32>,
    /// Left margin override.
    pub margin_left: Option<i32>,
    /// Right margin override.
    pub margin_right: Option<i32>,
    /// Whether to clamp window geometry within reference area bounds.
    pub clamp: bool,
}

impl Default for OracleParams {
    /// Constructs default oracle parameters with standard (400, 300) window dimensions.
    fn default() -> Self {
        Self {
            anchor: None,
            pivot: None,
            explicit_pos: None,
            offset: None,
            area: None,
            cursor_pos: None,
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

/// Final window size when the app enforces a minimum on one or both axes: `max(requested, min)`
/// per axis; an axis without a declared minimum keeps the requested value.
pub fn final_size_with_min(
    requested: (u32, u32),
    min_w: Option<u32>,
    min_h: Option<u32>,
) -> (u32, u32) {
    (
        requested.0.max(min_w.unwrap_or(0)),
        requested.1.max(min_h.unwrap_or(0)),
    )
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

    let is_unbounded = params.area.as_deref() == Some("unbounded");

    let (mut x, mut y) = if let Some((px, py)) =
        params.explicit_pos.or(if params.anchor.is_none() {
            params.offset
        } else {
            None
        }) {
        let (ref_x, ref_y) = if is_unbounded {
            (0, 0)
        } else {
            (workarea.x, workarea.y)
        };
        let pivot_str = params.pivot.as_deref().unwrap_or("top-left");
        apply_pivot_offset(ref_x + px, ref_y + py, w, h, pivot_str)?
    } else {
        let anchor_str = params.anchor.as_deref().unwrap_or("center");
        let (wa_w, wa_h) = (workarea.w as i32, workarea.h as i32);
        let (ox, oy) = params.offset.unwrap_or((0, 0));

        // Documented semantic: Anchor determines reference point on the workarea boundary/margin.
        let (ax, ay) = match anchor_str {
            "top-left" => (workarea.x + ml + ox, workarea.y + mt + oy),
            "top" | "top-center" => (workarea.x + wa_w / 2 + ox, workarea.y + mt + oy),
            "top-right" => (workarea.x + wa_w - mr + ox, workarea.y + mt + oy),
            "left" => (workarea.x + ml + ox, workarea.y + wa_h / 2 + oy),
            "center" => (workarea.x + wa_w / 2 + ox, workarea.y + wa_h / 2 + oy),
            "right" => (workarea.x + wa_w - mr + ox, workarea.y + wa_h / 2 + oy),
            "bottom-left" => (workarea.x + ml + ox, workarea.y + wa_h - mb + oy),
            "bottom" | "bottom-center" => (workarea.x + wa_w / 2 + ox, workarea.y + wa_h - mb + oy),
            "bottom-right" => (workarea.x + wa_w - mr + ox, workarea.y + wa_h - mb + oy),
            "cursor" => {
                let (cx, cy) = params
                    .cursor_pos
                    .unwrap_or((workarea.x + ml, workarea.y + mt));
                (cx + ox, cy + oy)
            }
            other => return Err(format!("Unknown anchor '{}'", other)),
        };

        // If pivot is explicitly given, align window's pivot to anchor point.
        // Otherwise, use matching default pivot for the anchor.
        if let Some(p) = &params.pivot {
            apply_pivot_offset(ax, ay, w, h, p.as_str())?
        } else {
            // Documented rule: centering is origin + floor((area_size - window_size) / 2);
            // an odd remainder puts the extra pixel on the right/bottom.
            match anchor_str {
                "top" | "top-center" => {
                    (workarea.x + (wa_w - w_i32) / 2 + ox, workarea.y + mt + oy)
                }
                "bottom" | "bottom-center" => (
                    workarea.x + (wa_w - w_i32) / 2 + ox,
                    workarea.y + wa_h - h_i32 - mb + oy,
                ),
                "left" => (workarea.x + ml + ox, workarea.y + (wa_h - h_i32) / 2 + oy),
                "right" => (
                    workarea.x + wa_w - mr - w_i32 + ox,
                    workarea.y + (wa_h - h_i32) / 2 + oy,
                ),
                "center" => (
                    workarea.x + (wa_w - w_i32) / 2 + ox,
                    workarea.y + (wa_h - h_i32) / 2 + oy,
                ),
                "top-left" => (workarea.x + ml + ox, workarea.y + mt + oy),
                "cursor" => {
                    let (cx, cy) = params
                        .cursor_pos
                        .unwrap_or((workarea.x + ml, workarea.y + mt));
                    (cx + ox, cy + oy)
                }
                "top-right" => (workarea.x + wa_w - mr - w_i32 + ox, workarea.y + mt + oy),
                "bottom-left" => (workarea.x + ml + ox, workarea.y + wa_h - mb - h_i32 + oy),
                "bottom-right" => (
                    workarea.x + wa_w - mr - w_i32 + ox,
                    workarea.y + wa_h - mb - h_i32 + oy,
                ),
                other => return Err(format!("Unknown anchor '{}'", other)),
            }
        }
    };

    if !is_unbounded && params.clamp {
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

/// Computes top-left origin coordinates given reference point, window dimensions, and pivot alignment.
fn apply_pivot_offset(px: i32, py: i32, w: u32, h: u32, pivot: &str) -> Result<(i32, i32), String> {
    let w_i32 = w as i32;
    let h_i32 = h as i32;

    match pivot {
        "top-left" => Ok((px, py)),
        "top" | "top-center" => Ok((px - (w_i32 + 1) / 2, py)),
        "top-right" => Ok((px - w_i32, py)),
        "left" => Ok((px, py - (h_i32 + 1) / 2)),
        "center" => Ok((px - (w_i32 + 1) / 2, py - (h_i32 + 1) / 2)),
        "right" => Ok((px - w_i32, py - (h_i32 + 1) / 2)),
        "bottom-left" => Ok((px, py - h_i32)),
        "bottom" | "bottom-center" => Ok((px - (w_i32 + 1) / 2, py - h_i32)),
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
