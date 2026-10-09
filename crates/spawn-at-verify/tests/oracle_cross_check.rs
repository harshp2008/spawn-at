//! Cross-check test comparing spawn-at-verify independent geometric oracle
//! with spawn-at-core geometry engine across the full anchor x pivot matrix,
//! including offset-origin and negative-origin workareas.

use spawn_at_core::geometry::{
    calculate_rect_from_placement, Anchor, Area, Pivot, PlacementParams as CoreParams,
    Rect as CoreRect,
};
use spawn_at_verify::oracle::{calculate_expected_rect, OracleParams, Rect as OracleRect};

fn core_to_oracle_rect(r: CoreRect) -> OracleRect {
    OracleRect::new(r.x, r.y, r.width, r.height)
}

fn map_anchor(a_str: &str) -> Anchor {
    match a_str {
        "center" => Anchor::Center,
        "top-left" => Anchor::TopLeft,
        "top-right" => Anchor::TopRight,
        "bottom-left" => Anchor::BottomLeft,
        "bottom-right" => Anchor::BottomRight,
        "top" => Anchor::Top,
        "bottom" => Anchor::Bottom,
        "left" => Anchor::Left,
        "right" => Anchor::Right,
        "cursor" => Anchor::Cursor,
        other => panic!("Unknown anchor {}", other),
    }
}

fn map_pivot(p_str: &str) -> Pivot {
    match p_str {
        "top-left" => Pivot::TopLeft,
        "top-right" => Pivot::TopRight,
        "bottom-left" => Pivot::BottomLeft,
        "bottom-right" => Pivot::BottomRight,
        "center" => Pivot::Center,
        other => panic!("Unknown pivot {}", other),
    }
}

/// The 9 screen/workarea-relative anchors.
/// Note: The 10th anchor defined in Phase 0 / README is `Anchor::Cursor` (pointer anchor).
/// It is tested separately in `test_oracle_vs_core_cursor_anchor_cross_check` because
/// cursor anchoring depends on dynamic pointer input (`cursor_pos`) rather than static
/// workarea bounds.
const SCREEN_ANCHORS: [&str; 9] = [
    "top-left",
    "top",
    "top-right",
    "left",
    "center",
    "right",
    "bottom-left",
    "bottom",
    "bottom-right",
];

const PIVOTS: [&str; 5] = [
    "top-left",
    "top-right",
    "bottom-left",
    "bottom-right",
    "center",
];

#[test]
fn test_oracle_vs_core_cross_check_matrix() {
    let margins = [0, 16, 24];
    let clamp_modes = [true, false];

    // Tested workareas:
    // 1. Standard primary monitor: 1920x1040 at (0, 40)
    // 2. Offset-origin workarea: 1600x900 at (200, 100)
    // 3. Negative-origin monitor (left of primary): 1920x1080 at (-1920, 0)
    let workareas = [
        (
            OracleRect::new(0, 40, 1920, 1040),
            CoreRect {
                x: 0,
                y: 40,
                width: 1920,
                height: 1040,
            },
            "primary (0, 40)",
        ),
        (
            OracleRect::new(200, 100, 1600, 900),
            CoreRect {
                x: 200,
                y: 100,
                width: 1600,
                height: 900,
            },
            "offset (200, 100)",
        ),
        (
            OracleRect::new(-1920, 0, 1920, 1080),
            CoreRect {
                x: -1920,
                y: 0,
                width: 1920,
                height: 1080,
            },
            "negative-origin (-1920, 0)",
        ),
    ];

    let win_size = (400, 300);

    let mut total_compared = 0;
    let mut mismatches = Vec::new();

    for (oracle_wa, core_wa, wa_desc) in &workareas {
        for anchor in &SCREEN_ANCHORS {
            for pivot in &PIVOTS {
                for &margin in &margins {
                    for &clamp in &clamp_modes {
                        total_compared += 1;

                        let oracle_params = OracleParams {
                            anchor: Some(anchor.to_string()),
                            pivot: Some(pivot.to_string()),
                            explicit_pos: None,
                            size: win_size,
                            margin,
                            margin_top: None,
                            margin_bottom: None,
                            margin_left: None,
                            margin_right: None,
                            clamp,
                        };

                        let oracle_rect = calculate_expected_rect(*oracle_wa, &oracle_params)
                            .expect("Oracle calculation failed");

                        let core_params = CoreParams {
                            pos: None,
                            offset: None,
                            anchor: Some(map_anchor(anchor)),
                            pivot: Some(map_pivot(pivot)),
                            size: Some(win_size),
                            margin,
                            margin_top: None,
                            margin_bottom: None,
                            margin_left: None,
                            margin_right: None,
                            area: Some(Area::Workarea),
                            cursor_pos: None,
                            workarea: *core_wa,
                            clamp,
                        };

                        let core_rect =
                            calculate_rect_from_placement(&core_params, win_size.0, win_size.1);
                        let mapped_core = core_to_oracle_rect(core_rect);

                        if oracle_rect != mapped_core {
                            mismatches.push(format!(
                                "Mismatch [{wa_desc}] for anchor='{anchor}', pivot='{pivot}', margin={margin}, clamp={clamp}: oracle=[{}, {}, {}x{}], core=[{}, {}, {}x{}]",
                                oracle_rect.x, oracle_rect.y, oracle_rect.w, oracle_rect.h,
                                mapped_core.x, mapped_core.y, mapped_core.w, mapped_core.h
                            ));
                        }
                    }
                }
            }
        }
    }

    println!(
        "Cross-check complete: {} configurations tested across {} work areas. Mismatches: {}",
        total_compared,
        workareas.len(),
        mismatches.len()
    );

    if !mismatches.is_empty() {
        for m in mismatches.iter().take(10) {
            println!("  ! {}", m);
        }
    }

    assert!(
        mismatches.is_empty(),
        "Found {} mismatches out of {} tested",
        mismatches.len(),
        total_compared
    );
}

#[test]
fn test_oracle_vs_core_directional_margins_cross_check() {
    let oracle_wa = OracleRect::new(0, 40, 1920, 1040);
    let core_wa = CoreRect {
        x: 0,
        y: 40,
        width: 1920,
        height: 1040,
    };
    let win_size = (300, 200);

    let directional_settings = [
        (Some(10), Some(20), Some(30), Some(40)),
        (Some(0), Some(50), Some(0), Some(50)),
        (Some(25), None, Some(15), None),
    ];

    let mut mismatches = Vec::new();
    let mut count = 0;

    for (mt, mb, ml, mr) in directional_settings {
        for anchor in &SCREEN_ANCHORS {
            for pivot in &PIVOTS {
                count += 1;
                let oracle_params = OracleParams {
                    anchor: Some(anchor.to_string()),
                    pivot: Some(pivot.to_string()),
                    explicit_pos: None,
                    size: win_size,
                    margin: 16,
                    margin_top: mt,
                    margin_bottom: mb,
                    margin_left: ml,
                    margin_right: mr,
                    clamp: true,
                };

                let oracle_rect = calculate_expected_rect(oracle_wa, &oracle_params)
                    .expect("Oracle calculation failed");

                let core_params = CoreParams {
                    pos: None,
                    offset: None,
                    anchor: Some(map_anchor(anchor)),
                    pivot: Some(map_pivot(pivot)),
                    size: Some(win_size),
                    margin: 16,
                    margin_top: mt,
                    margin_bottom: mb,
                    margin_left: ml,
                    margin_right: mr,
                    area: Some(Area::Workarea),
                    cursor_pos: None,
                    workarea: core_wa,
                    clamp: true,
                };

                let core_rect = calculate_rect_from_placement(&core_params, win_size.0, win_size.1);
                let mapped_core = core_to_oracle_rect(core_rect);

                if oracle_rect != mapped_core {
                    mismatches.push(format!(
                        "Directional mismatch anchor='{anchor}', pivot='{pivot}': oracle={oracle_rect:?}, core={mapped_core:?}"
                    ));
                }
            }
        }
    }

    assert!(
        mismatches.is_empty(),
        "Directional cross-check failed: {} mismatches out of {}",
        mismatches.len(),
        count
    );
}

#[test]
fn test_oracle_vs_core_cursor_anchor_cross_check() {
    // Cross-check for the 10th anchor: Anchor::Cursor
    let cursor_positions = [(500, 400), (0, 0), (1919, 1039), (-500, 300)];
    let win_size = (300, 200);
    let oracle_wa = OracleRect::new(0, 40, 1920, 1040);
    let core_wa = CoreRect {
        x: 0,
        y: 40,
        width: 1920,
        height: 1040,
    };

    for &(cx, cy) in &cursor_positions {
        for pivot in &PIVOTS {
            for &clamp in &[false, true] {
                let oracle_params = OracleParams {
                    anchor: None,
                    pivot: Some(pivot.to_string()),
                    explicit_pos: Some((cx, cy)),
                    size: win_size,
                    margin: 0,
                    margin_top: None,
                    margin_bottom: None,
                    margin_left: None,
                    margin_right: None,
                    clamp,
                };

                let oracle_rect = calculate_expected_rect(oracle_wa, &oracle_params)
                    .expect("Oracle calculation failed");

                let core_params = CoreParams {
                    pos: None,
                    offset: None,
                    anchor: Some(Anchor::Cursor),
                    pivot: Some(map_pivot(pivot)),
                    size: Some(win_size),
                    margin: 0,
                    margin_top: None,
                    margin_bottom: None,
                    margin_left: None,
                    margin_right: None,
                    area: Some(Area::Workarea),
                    cursor_pos: Some((cx, cy)),
                    workarea: core_wa,
                    clamp,
                };

                let core_rect = calculate_rect_from_placement(&core_params, win_size.0, win_size.1);
                let mapped_core = core_to_oracle_rect(core_rect);

                assert_eq!(
                    oracle_rect, mapped_core,
                    "Cursor anchor mismatch at ({cx}, {cy}) with pivot='{pivot}', clamp={clamp}"
                );
            }
        }
    }
}

#[test]
fn test_oracle_vs_core_default_pivot_cross_check() {
    // Cross-check for default pivot (no --pivot specified, params.pivot = None)
    // for all 9 screen anchors across multiple workareas, margins, and clamp modes.
    let margins = [0, 16, 24];
    let clamp_modes = [true, false];
    let win_size = (400, 300);

    let workareas = [
        (
            OracleRect::new(0, 40, 1920, 1040),
            CoreRect {
                x: 0,
                y: 40,
                width: 1920,
                height: 1040,
            },
            "primary (0, 40)",
        ),
        (
            OracleRect::new(200, 100, 1600, 900),
            CoreRect {
                x: 200,
                y: 100,
                width: 1600,
                height: 900,
            },
            "offset (200, 100)",
        ),
        (
            OracleRect::new(-1920, 0, 1920, 1080),
            CoreRect {
                x: -1920,
                y: 0,
                width: 1920,
                height: 1080,
            },
            "negative-origin (-1920, 0)",
        ),
    ];

    let mut checked_cases = 0;

    for (oracle_wa, core_wa, wa_name) in &workareas {
        for margin in &margins {
            for &clamp in &clamp_modes {
                for anchor_str in &SCREEN_ANCHORS {
                    let oracle_params = OracleParams {
                        anchor: Some(anchor_str.to_string()),
                        pivot: None, // Default pivot
                        explicit_pos: None,
                        size: win_size,
                        margin: *margin,
                        margin_top: None,
                        margin_bottom: None,
                        margin_left: None,
                        margin_right: None,
                        clamp,
                    };

                    let oracle_rect = calculate_expected_rect(*oracle_wa, &oracle_params)
                        .expect("Oracle calculation failed");

                    let core_params = CoreParams {
                        pos: None,
                        offset: None,
                        anchor: Some(map_anchor(anchor_str)),
                        pivot: None, // Default pivot
                        size: Some(win_size),
                        margin: *margin,
                        margin_top: None,
                        margin_bottom: None,
                        margin_left: None,
                        margin_right: None,
                        area: Some(Area::Workarea),
                        cursor_pos: None,
                        workarea: *core_wa,
                        clamp,
                    };

                    let core_rect =
                        calculate_rect_from_placement(&core_params, win_size.0, win_size.1);
                    let mapped_core = core_to_oracle_rect(core_rect);

                    assert_eq!(
                        oracle_rect, mapped_core,
                        "Default pivot mismatch on {wa_name}: anchor='{anchor_str}', margin={margin}, clamp={clamp}"
                    );
                    checked_cases += 1;
                }
            }
        }
    }

    assert_eq!(
        checked_cases, 162,
        "Expected 162 default-pivot cross-check cases (9 anchors x 3 workareas x 3 margins x 2 clamp modes)"
    );
}
