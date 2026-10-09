use spawn_at_core::geometry::{
    apply_anchor, apply_pivot, calculate_rect_from_placement, Anchor, Area, Pivot, PlacementParams,
    Rect,
};
use std::fs;
use std::path::PathBuf;

#[derive(serde::Deserialize)]
struct GoldenFile {
    vectors: Vec<GoldenVector>,
}

#[derive(serde::Deserialize)]
struct GoldenVector {
    name: String,
    workarea: RectJson,
    window_size: SizeJson,
    anchor: Option<String>,
    pivot: Option<String>,
    pos: Option<[i32; 2]>,
    offset: Option<[i32; 2]>,
    margin: i32,
    margin_top: Option<i32>,
    margin_bottom: Option<i32>,
    margin_left: Option<i32>,
    margin_right: Option<i32>,
    clamp: bool,
    expected: RectJson,
    #[allow(dead_code)]
    arithmetic: String,
}

#[derive(serde::Deserialize)]
struct RectJson {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
}

#[derive(serde::Deserialize)]
struct SizeJson {
    width: u32,
    height: u32,
}

fn map_anchor(s: &str) -> Anchor {
    match s {
        "top" => Anchor::Top,
        "bottom" => Anchor::Bottom,
        "left" => Anchor::Left,
        "right" => Anchor::Right,
        "center" => Anchor::Center,
        "top-left" => Anchor::TopLeft,
        "top-right" => Anchor::TopRight,
        "bottom-left" => Anchor::BottomLeft,
        "bottom-right" => Anchor::BottomRight,
        "cursor" => Anchor::Cursor,
        other => panic!("Unknown anchor: {}", other),
    }
}

fn map_pivot(s: &str) -> Pivot {
    match s {
        "top-left" => Pivot::TopLeft,
        "top-right" => Pivot::TopRight,
        "bottom-left" => Pivot::BottomLeft,
        "bottom-right" => Pivot::BottomRight,
        "center" => Pivot::Center,
        other => panic!("Unknown pivot: {}", other),
    }
}

#[test]
fn test_core_reads_shared_golden_placement_vectors() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let golden_path = manifest_dir.join("../../verify/golden/placement.json");
    let content = fs::read_to_string(&golden_path)
        .unwrap_or_else(|e| panic!("Failed to read golden vectors at {:?}: {}", golden_path, e));

    let parsed: GoldenFile = serde_json::from_str(&content)
        .unwrap_or_else(|e| panic!("Failed to parse golden placement JSON: {}", e));

    for vec in &parsed.vectors {
        let wa = Rect {
            x: vec.workarea.x,
            y: vec.workarea.y,
            width: vec.workarea.width,
            height: vec.workarea.height,
        };

        let params = PlacementParams {
            pos: vec.pos.map(|p| (p[0], p[1])),
            offset: vec.offset.map(|o| (o[0], o[1])),
            anchor: vec.anchor.as_deref().map(map_anchor),
            pivot: vec.pivot.as_deref().map(map_pivot),
            size: Some((vec.window_size.width, vec.window_size.height)),
            margin: vec.margin,
            margin_top: vec.margin_top,
            margin_bottom: vec.margin_bottom,
            margin_left: vec.margin_left,
            margin_right: vec.margin_right,
            area: Some(Area::Workarea),
            cursor_pos: None,
            workarea: wa,
            clamp: vec.clamp,
        };

        let calculated =
            calculate_rect_from_placement(&params, vec.window_size.width, vec.window_size.height);

        assert_eq!(
            calculated.x, vec.expected.x,
            "Golden vector '{}' failed on x (arithmetic: {})",
            vec.name, vec.arithmetic
        );
        assert_eq!(
            calculated.y, vec.expected.y,
            "Golden vector '{}' failed on y (arithmetic: {})",
            vec.name, vec.arithmetic
        );
        assert_eq!(
            calculated.width, vec.expected.width,
            "Golden vector '{}' failed on width",
            vec.name
        );
        assert_eq!(
            calculated.height, vec.expected.height,
            "Golden vector '{}' failed on height",
            vec.name
        );
    }
}

#[test]
fn test_core_odd_and_even_sizes_exhaustive() {
    let window_sizes = [
        (367, 514), // odd width, even height
        (401, 301), // odd width, odd height
        (513, 513), // odd width, odd height
        (1, 1),     // minimal odd
        (400, 300), // even width, even height
    ];

    let workareas = [
        // Even width, even height
        Rect {
            x: 0,
            y: 40,
            width: 1920,
            height: 1040,
        },
        // Odd width, odd height
        Rect {
            x: 0,
            y: 40,
            width: 1921,
            height: 1039,
        },
        // Negative origin, even dimensions
        Rect {
            x: -1920,
            y: 0,
            width: 1920,
            height: 1080,
        },
    ];

    let anchors = [
        Anchor::TopLeft,
        Anchor::Top,
        Anchor::TopRight,
        Anchor::Left,
        Anchor::Center,
        Anchor::Right,
        Anchor::BottomLeft,
        Anchor::Bottom,
        Anchor::BottomRight,
    ];

    let margins = [0, 16, 24];

    for wa in &workareas {
        for &(w, h) in &window_sizes {
            for &anchor in &anchors {
                for &margin in &margins {
                    // 1. Verify default pivot: calculate_rect_from_placement matches apply_anchor
                    let default_params = PlacementParams {
                        pos: None,
                        offset: None,
                        anchor: Some(anchor),
                        pivot: None,
                        size: Some((w, h)),
                        margin,
                        margin_top: None,
                        margin_bottom: None,
                        margin_left: None,
                        margin_right: None,
                        area: Some(Area::Workarea),
                        cursor_pos: None,
                        workarea: *wa,
                        clamp: false,
                    };
                    let rect = calculate_rect_from_placement(&default_params, w, h);
                    let (expected_x, expected_y) = apply_anchor(*wa, w, h, anchor, margin);
                    assert_eq!(
                        (rect.x, rect.y),
                        (expected_x, expected_y),
                        "Mismatch for anchor {:?}, win={}x{}, wa={}x{}@({}, {}), margin={}",
                        anchor,
                        w,
                        h,
                        wa.width,
                        wa.height,
                        wa.x,
                        wa.y,
                        margin
                    );

                    // 2. Explicit center pivot: centering rule puts extra pixel on right/bottom
                    let (center_anchor_x, center_anchor_y) =
                        (wa.x + (wa.width as i32) / 2, wa.y + (wa.height as i32) / 2);
                    let (p_x, p_y) =
                        apply_pivot(center_anchor_x, center_anchor_y, w, h, Pivot::Center);
                    // For even workarea, explicit center pivot must equal floor((wa - win) / 2)
                    if wa.width % 2 == 0 {
                        let expected_floor_x = wa.x + (wa.width as i32 - w as i32) / 2;
                        assert_eq!(
                            p_x, expected_floor_x,
                            "Explicit pivot X mismatch for win_w={}, wa_w={}",
                            w, wa.width
                        );
                    }
                    if wa.height % 2 == 0 {
                        let expected_floor_y = wa.y + (wa.height as i32 - h as i32) / 2;
                        assert_eq!(
                            p_y, expected_floor_y,
                            "Explicit pivot Y mismatch for win_h={}, wa_h={}",
                            h, wa.height
                        );
                    }
                }

                // 3. Directional margins: verify edge centering centers on whole area
                let directional_params = PlacementParams {
                    pos: None,
                    offset: None,
                    anchor: Some(anchor),
                    pivot: None,
                    size: Some((w, h)),
                    margin: 0,
                    margin_top: Some(10),
                    margin_bottom: Some(20),
                    margin_left: Some(30),
                    margin_right: Some(40),
                    area: Some(Area::Workarea),
                    cursor_pos: None,
                    workarea: *wa,
                    clamp: false,
                };
                let dir_rect = calculate_rect_from_placement(&directional_params, w, h);
                match anchor {
                    Anchor::Top | Anchor::Bottom | Anchor::Center => {
                        let expected_center_x = wa.x + (wa.width as i32 - w as i32) / 2;
                        assert_eq!(
                            dir_rect.x, expected_center_x,
                            "Directional margin horizontal centering must use whole area width"
                        );
                    }
                    _ => {}
                }
                match anchor {
                    Anchor::Left | Anchor::Right | Anchor::Center => {
                        let expected_center_y = wa.y + (wa.height as i32 - h as i32) / 2;
                        assert_eq!(
                            dir_rect.y, expected_center_y,
                            "Directional margin vertical centering must use whole area height"
                        );
                    }
                    _ => {}
                }
            }
        }
    }
}
