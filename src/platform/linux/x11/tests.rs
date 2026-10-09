//! # Native X11 Driver Tests

use super::*;
use std::time::Duration;

#[test]
fn test_x11_driver_capabilities() {
    let driver = X11Driver;
    assert_eq!(driver.name(), "X11");
    assert!(driver.supports_runtime_transform());
}

#[tokio::test]
async fn test_x11_arm_generates_tokens() {
    use spawn_at_core::driver::{Batch, Entry, FocusIntent, Reveal, Urgency};

    let driver = X11Driver;
    let batch = Batch {
        id: 1,
        entries: vec![Entry {
            key: "test-token-123".into(),
            app_hint: "test-app".into(),
            placement: PlacementParams::default(),
        }],
        reveal: Reveal::Together,
        focus: FocusIntent::Exclusive,
        urgency: Urgency::Normal,
        deadline: Duration::from_millis(5000),
    };

    let armed = driver.arm(batch).await.unwrap();
    assert!(armed
        .launch_env
        .iter()
        .any(|(k, v)| k == "DESKTOP_STARTUP_ID" && v == "test-token-123"));
    assert!(armed
        .launch_env
        .iter()
        .any(|(k, v)| k == "XDG_ACTIVATION_TOKEN" && v == "test-token-123"));
}

#[test]
fn test_calculate_rect_center_anchor() {
    use spawn_at_core::geometry::Anchor;

    let params = PlacementParams {
        anchor: Some(Anchor::Center),
        workarea: Rect {
            x: 0,
            y: 0,
            width: 1920,
            height: 1080,
        },
        ..Default::default()
    };

    let rect = calculate_rect_from_placement(&params, 800, 600);
    assert_eq!(rect.width, 800);
    assert_eq!(rect.height, 600);
    assert_eq!(rect.x, (1920 - 800) / 2); // 560
    assert_eq!(rect.y, (1080 - 600) / 2); // 240
}

#[test]
fn test_calculate_rect_top_left_with_margins() {
    use spawn_at_core::geometry::Anchor;

    let params = PlacementParams {
        anchor: Some(Anchor::TopLeft),
        margin_left: Some(50),
        margin_top: Some(30),
        workarea: Rect {
            x: 100,
            y: 50,
            width: 1920,
            height: 1080,
        },
        ..Default::default()
    };

    let rect = calculate_rect_from_placement(&params, 400, 300);
    assert_eq!(rect.x, 100 + 50); // 150
    assert_eq!(rect.y, 50 + 30);  // 80
    assert_eq!(rect.width, 400);
    assert_eq!(rect.height, 300);
}

#[test]
fn test_calculate_rect_explicit_pos_and_pivot() {
    use spawn_at_core::geometry::Pivot;

    let params = PlacementParams {
        pos: Some((500, 400)),
        pivot: Some(Pivot::Center),
        workarea: Rect {
            x: 0,
            y: 0,
            width: 1920,
            height: 1080,
        },
        ..Default::default()
    };

    let rect = calculate_rect_from_placement(&params, 200, 100);
    assert_eq!(rect.x, 500 - 100); // 400
    assert_eq!(rect.y, 400 - 50);  // 350
    assert_eq!(rect.width, 200);
    assert_eq!(rect.height, 100);
}

#[test]
fn test_compare_x11_and_mechanics_anchor_outputs() {
    use spawn_at_core::geometry::{Anchor, Pivot};
    use crate::platform::linux::gnome::mechanics::calculate_placement;

    let wa = Rect {
        x: 100,
        y: 50,
        width: 1920,
        height: 1080,
    };

    let anchors = [
        Anchor::Center,
        Anchor::TopLeft,
        Anchor::TopRight,
        Anchor::BottomLeft,
        Anchor::BottomRight,
        Anchor::Top,
        Anchor::Bottom,
        Anchor::Left,
        Anchor::Right,
    ];

    let pivots = [
        Pivot::TopLeft,
        Pivot::TopRight,
        Pivot::BottomLeft,
        Pivot::BottomRight,
        Pivot::Center,
    ];

    for anchor in &anchors {
        for pivot in &pivots {
            let params = PlacementParams {
                anchor: Some(*anchor),
                pivot: Some(*pivot),
                margin: 16,
                margin_top: Some(20),
                margin_bottom: Some(25),
                margin_left: Some(30),
                margin_right: Some(35),
                offset: Some((10, -15)),
                workarea: wa,
                ..Default::default()
            };

            let win_w = 600;
            let win_h = 400;

            // 1. Output from native X11 duplicate
            let x11_rect = calculate_rect_from_placement(&params, win_w, win_h);

            // 2. Output from mechanics.rs
            let payload = calculate_placement(&params, win_w, win_h);

            // In GNOME Shell extension, the raw coordinate formula is:
            let ml = params.margin_left.unwrap();
            let mr = params.margin_right.unwrap();
            let mt = params.margin_top.unwrap();
            let mb = params.margin_bottom.unwrap();

            let raw_x = payload.screen_anchor_x
                - (payload.pivot_u * (win_w as f64)).round() as i32
                + payload.offset_x;
            let raw_y = payload.screen_anchor_y
                - (payload.pivot_v * (win_h as f64)).round() as i32
                + payload.offset_y;

            let min_x = wa.x + ml;
            let min_y = wa.y + mt;
            let max_x = (wa.x + (wa.width as i32) - mr - (win_w as i32)).max(min_x);
            let max_y = (wa.y + (wa.height as i32) - mb - (win_h as i32)).max(min_y);

            let extension_x = raw_x.clamp(min_x, max_x);
            let extension_y = raw_y.clamp(min_y, max_y);

            assert_eq!(
                x11_rect.x, extension_x,
                "X11 calculation mismatch with extension for anchor {:?}, pivot {:?}",
                anchor, pivot
            );
            assert_eq!(
                x11_rect.y, extension_y,
                "X11 calculation mismatch with extension for anchor {:?}, pivot {:?}",
                anchor, pivot
            );
        }
    }
}

#[test]
fn test_x11_margin_24_top_right_leaves_24px_not_48px() {
    use spawn_at_core::geometry::Anchor;

    let wa = Rect {
        x: 0,
        y: 0,
        width: 1920,
        height: 1080,
    };
    let params = PlacementParams {
        anchor: Some(Anchor::TopRight),
        margin: 24,
        workarea: wa,
        ..Default::default()
    };

    let win_w = 600;
    let win_h = 400;
    let rect = calculate_rect_from_placement(&params, win_w, win_h);

    // Right edge distance: 1920 - (rect.x + win_w)
    let right_margin = 1920 - (rect.x + (win_w as i32));
    // Top edge distance: rect.y
    let top_margin = rect.y;

    assert_eq!(right_margin, 24, "Right margin must be 24px, not 48px");
    assert_eq!(top_margin, 24, "Top margin must be 24px, not 48px");
}

#[test]
fn test_x11_clamp_false_bypasses_bounds() {
    let params = PlacementParams {
        pos: Some((2500, -500)),
        size: Some((400, 300)),
        workarea: Rect { x: 0, y: 0, width: 1920, height: 1080 },
        clamp: false,
        ..Default::default()
    };
    let rect = calculate_rect_from_placement(&params, 400, 300);
    assert_eq!(rect.x, 2500);
    assert_eq!(rect.y, -500);

    let clamped_params = PlacementParams {
        pos: Some((2500, -500)),
        size: Some((400, 300)),
        workarea: Rect { x: 0, y: 0, width: 1920, height: 1080 },
        clamp: true,
        ..Default::default()
    };
    let clamped_rect = calculate_rect_from_placement(&clamped_params, 400, 300);
    assert_ne!(clamped_rect.x, 2500);
    assert_ne!(clamped_rect.y, -500);
}
