//! Cross-check test comparing spawn-at-verify independent geometric oracle
//! with spawn-at-core geometry engine across the full anchor x pivot matrix.

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

#[test]
fn test_oracle_vs_core_cross_check_matrix() {
    let anchors = [
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

    let pivots = [
        "top-left",
        "top-right",
        "bottom-left",
        "bottom-right",
        "center",
    ];

    let margins = [0, 16, 24];
    let clamp_modes = [true, false];
    let workarea = OracleRect::new(0, 40, 1920, 1040);
    let core_wa = CoreRect {
        x: 0,
        y: 40,
        width: 1920,
        height: 1040,
    };
    let win_size = (400, 300);

    let mut total_compared = 0;
    let mut mismatches = Vec::new();

    for anchor in &anchors {
        for pivot in &pivots {
            for &margin in &margins {
                for &clamp in &clamp_modes {
                    total_compared += 1;

                    let oracle_params = OracleParams {
                        anchor: Some(anchor.to_string()),
                        pivot: Some(pivot.to_string()),
                        explicit_pos: None,
                        size: win_size,
                        margin,
                        clamp,
                    };

                    let oracle_rect = calculate_expected_rect(workarea, &oracle_params)
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
                        workarea: core_wa,
                        clamp,
                    };

                    let core_rect =
                        calculate_rect_from_placement(&core_params, win_size.0, win_size.1);
                    let mapped_core = core_to_oracle_rect(core_rect);

                    if oracle_rect != mapped_core {
                        mismatches.push(format!(
                            "Mismatch for anchor='{}', pivot='{}', margin={}, clamp={}: oracle=[{}, {}, {}x{}], core=[{}, {}, {}x{}]",
                            anchor, pivot, margin, clamp,
                            oracle_rect.x, oracle_rect.y, oracle_rect.w, oracle_rect.h,
                            mapped_core.x, mapped_core.y, mapped_core.w, mapped_core.h
                        ));
                    }
                }
            }
        }
    }

    println!(
        "Cross-check complete: {} configurations tested. Mismatches: {}",
        total_compared,
        mismatches.len()
    );

    if !mismatches.is_empty() {
        for m in mismatches.iter().take(10) {
            println!("  ! {}", m);
        }
    }

    // We do not silently fail or panic yet; let's see the comparison in cargo test
    assert!(
        mismatches.is_empty(),
        "Found {} mismatches out of {} tested",
        mismatches.len(),
        total_compared
    );
}
