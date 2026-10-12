//! Oracle with a fixture minimum: expected size is `max(requested, minimum)` per axis, the
//! position is the anchor for that final size, and far-edge anchors must stay inside the workarea.

use crate::oracle::check::{check_geometry, expected_rect, GeometryStatus, WorkareaProbe};
use crate::oracle::{final_size_with_min, Rect};
use crate::schema::TestDefinition;

/// Workarea 1920x1040 at (0, 40), as in the real recordings.
fn wa() -> Rect {
    Rect::new(0, 40, 1920, 1040)
}

/// A bottom-right spec asking `w`x`h` with margin 16, whose fixture declares a 300x200 minimum.
fn spec(anchor: &str, w: u32, h: u32) -> TestDefinition {
    toml::from_str(&format!(
        r#"
id = "placement/x"
category = "placement"
title = "t"
description = "d"
kind = "auto"

[subject]
app = "fixture"
min_width = 300
min_height = 200

[action]
command = "spawn"
args = []

[expect.rect]
anchor = "{anchor}"
size = [{w}, {h}]
margin = 16
tolerance_px = 1
"#
    ))
    .unwrap()
}

/// `max(requested, min)` per axis, and an axis without a declared minimum is left alone.
#[test]
fn test_final_size_with_min_table() {
    let table: [((u32, u32), Option<u32>, Option<u32>, (u32, u32)); 7] = [
        ((300, 200), Some(300), Some(200), (300, 200)), // exactly at the minimum
        ((200, 500), Some(300), Some(200), (300, 500)), // width below
        ((500, 100), Some(300), Some(200), (500, 200)), // height below
        ((200, 150), Some(300), Some(200), (300, 200)), // both below
        ((50, 50), Some(300), Some(200), (300, 200)),   // far below
        ((3000, 2000), Some(300), Some(200), (3000, 2000)), // above: the request wins
        ((200, 150), None, Some(200), (200, 200)),      // only a height minimum declared
    ];
    for (req, mw, mh, want) in table {
        assert_eq!(
            final_size_with_min(req, mw, mh),
            want,
            "{:?} {:?} {:?}",
            req,
            mw,
            mh
        );
    }
}

/// Expected rectangle for bottom-right, margin 16, fixture minimum 300x200, per request. The
/// position is the anchor for the FINAL size (`workarea edge - margin - final size`), not for the
/// requested one. The overflow row (3000x2000, larger than the workarea) keeps the requested
/// size and, as the calculator clamps, sits at workarea origin plus margin.
#[test]
fn test_expected_rect_uses_the_final_size_table() {
    let table: [((u32, u32), Rect); 6] = [
        ((300, 200), Rect::new(1604, 864, 300, 200)),
        ((200, 500), Rect::new(1604, 564, 300, 500)),
        ((500, 100), Rect::new(1404, 864, 500, 200)),
        ((200, 150), Rect::new(1604, 864, 300, 200)),
        ((50, 50), Rect::new(1604, 864, 300, 200)),
        ((3000, 2000), Rect::new(16, 56, 3000, 2000)),
    ];
    for ((w, h), want) in table {
        let test = spec("bottom-right", w, h);
        let rect = test.effective_expect_rect().unwrap();
        let size = rect.size.as_ref().unwrap();
        let got = expected_rect(&rect, wa(), (size[0], size[1])).unwrap();
        assert_eq!(got, want, "request {}x{}", w, h);
    }
}

/// A spec that declares no fixture minimum is judged exactly as before.
#[test]
fn test_no_declared_minimum_leaves_the_expectation_unchanged() {
    let mut test = spec("bottom-right", 200, 150);
    let subject = test.subject.as_mut().unwrap();
    subject.min_width = None;
    subject.min_height = None;
    let rect = test.effective_expect_rect().unwrap();
    assert_eq!(rect.size, Some(vec![200, 150]));
}

/// Stdout JSON for a window at `x, y` with size `w`x`h`.
fn out(x: i32, y: i32, w: u32, h: u32) -> String {
    format!(r#"{{"window_id":1,"x":{x},"y":{y},"w":{w},"h":{h}}}"#)
}

/// A probe with only an independent workarea.
fn probe() -> WorkareaProbe {
    WorkareaProbe {
        independent: Some(wa()),
        extension: None,
    }
}

/// A 200x150 request on a 300x200-minimum app, bottom-right: the window that settled at 300x200
/// and sits at 1604,864 passes; one left at the position for 200x150 (1704,914) with the right
/// size fails on position; the size actually reached is judged against 300x200, not 200x150.
#[test]
fn test_check_judges_against_the_final_size() {
    let rect = spec("bottom-right", 200, 150)
        .effective_expect_rect()
        .unwrap();
    let ok = check_geometry(&rect, &out(1604, 864, 300, 200), None, probe());
    assert_eq!(ok.status(), GeometryStatus::Pass);
    assert_eq!(ok.expected, Some(Rect::new(1604, 864, 300, 200)));
    let stale = check_geometry(&rect, &out(1604, 664, 300, 200), None, probe());
    assert_eq!(stale.status(), GeometryStatus::Fail);
    let unraised = check_geometry(&rect, &out(1704, 914, 200, 150), None, probe());
    assert_eq!(
        unraised.status(),
        GeometryStatus::Fail,
        "size 200x150 is not max(request, min)"
    );
}

/// Containment: judged for far-edge anchors only, only when the observed size fits inside the
/// margins, and it alone can fail a window whose position and size are within a (here wide)
/// tolerance but which sticks out of the workarea.
#[test]
fn test_far_edge_anchors_must_stay_inside_the_workarea() {
    let mut rect = spec("bottom-right", 400, 300)
        .effective_expect_rect()
        .unwrap();
    rect.tolerance_px = 100;
    let inside = check_geometry(&rect, &out(1504, 764, 400, 300), None, probe());
    assert_eq!(inside.inside_workarea, Some(true));
    assert_eq!(inside.status(), GeometryStatus::Pass);

    // Within 100 px of the expected position but the right edge is at 1990 > 1920.
    let outside = check_geometry(&rect, &out(1590, 764, 400, 300), None, probe());
    assert_eq!(outside.inside_workarea, Some(false));
    assert_eq!(outside.status(), GeometryStatus::Fail);
    let msg = outside.verdict().unwrap_err();
    assert!(msg.contains("not fully inside the workarea"), "{}", msg);

    // A near-edge anchor is not judged for containment.
    let mut top_left = spec("top-left", 400, 300).effective_expect_rect().unwrap();
    top_left.tolerance_px = 2000;
    let c = check_geometry(&top_left, &out(1590, 764, 400, 300), None, probe());
    assert_eq!(c.inside_workarea, None);

    // Overflow case: the observed 3000x2000 does not fit the workarea, so containment does not apply.
    let big = spec("bottom-right", 3000, 2000)
        .effective_expect_rect()
        .unwrap();
    let c = check_geometry(&big, &out(16, 56, 3000, 2000), None, probe());
    assert_eq!(c.inside_workarea, None);
}
