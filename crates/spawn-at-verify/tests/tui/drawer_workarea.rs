//! Which workarea the geometry check trusts: the probe is primary, the extension's logged bounds
//! are a cross-check, and the RESULT section says so.
//!
//! Recording: `placement/bottom-right` (bottom-right, 400x300, margin 16), logged workarea
//! `0,40 1920x1040`, window observed at 1504, 764.

use super::create_app_with_real_fixture;
use ratatui::style::Color;
use spawn_at_verify::oracle::Rect;
use spawn_at_verify::runner::{AutoStatus, FinalStatus};
use spawn_at_verify::schema::TestKind;
use spawn_at_verify::tui::app::TuiApp;
use spawn_at_verify::tui::views::build_drawer_content;

/// Rows of the drawer for `test` as plain text plus the foreground colours of each row's spans.
fn drawer(
    app: &TuiApp,
    test: &spawn_at_verify::schema::TestDefinition,
) -> Vec<(String, Vec<Color>)> {
    let (lines, plain) = build_drawer_content(app, test, 120, true);
    plain
        .lines()
        .zip(lines.iter())
        .map(|(p, l)| {
            (
                p.to_string(),
                l.spans.iter().filter_map(|s| s.style.fg).collect(),
            )
        })
        .collect()
}

/// First row starting with `prefix`.
fn row<'a>(rows: &'a [(String, Vec<Color>)], prefix: &str) -> &'a (String, Vec<Color>) {
    rows.iter()
        .find(|(t, _)| t.starts_with(prefix))
        .unwrap_or_else(|| panic!("no row {:?} in {:#?}", prefix, rows))
}

/// Replaces the pinned probe workarea (`None` simulates a machine that could not probe one).
fn set_probe(app: &mut TuiApp, wa: Option<Rect>) {
    let mut env = app.env_info.clone().unwrap();
    env.primary_workarea = wa;
    app.env_info = Some(env);
}

/// Probe and log agree: a plain green pass and no warning row.
#[test]
fn test_probe_and_log_agree_is_a_plain_pass_without_warning() {
    let (app, test) = create_app_with_real_fixture();
    let rows = drawer(&app, &test);
    let g = row(&rows, "  Geometry");
    assert!(g.0.contains("✔ within ±1 px of expected"), "{}", g.0);
    assert!(g.1.contains(&Color::Green), "{:?}", g.1);
    assert!(!rows.iter().any(|(t, _)| t.starts_with("  Warning")));
}

/// Probe says 0,0 1920x1000 (bottom-right is 1504, 684) but the extension logged 0,40 1920x1040:
/// the window at 1504, 764 satisfies the extension's belief, not the probe, so the check FAILS,
/// with a warning naming both. Judged on the probe, not on the log.
#[test]
fn test_disagreement_fails_when_window_matches_only_the_extension_workarea() {
    let (mut app, test) = create_app_with_real_fixture();
    set_probe(&mut app, Some(Rect::new(0, 0, 1920, 1000)));
    let rows = drawer(&app, &test);
    let g = row(&rows, "  Geometry");
    assert!(g.0.contains('✘') && g.0.contains("dy=+80"), "{}", g.0);
    assert!(g.1.contains(&Color::Red));
    let w = row(&rows, "  Warning");
    assert!(
        w.0.contains("workarea differs: probe 0,0 1920×1000 vs extension 0,40 1920×1040"),
        "{}",
        w.0
    );
    assert!(w.1.contains(&Color::Yellow));
    let pos = row(&rows, "  Position");
    assert!(
        pos.0.contains("1504, 684"),
        "Expected is under the probe workarea: {}",
        pos.0
    );
}

/// Same disagreement, but the window really is where the PROBE workarea puts it (1504, 684):
/// a pass, still with the warning.
#[test]
fn test_disagreement_passes_when_window_matches_probe_workarea_but_warns() {
    let (mut app, test) = create_app_with_real_fixture();
    set_probe(&mut app, Some(Rect::new(0, 0, 1920, 1000)));
    app.results.get_mut(&test.id).unwrap().stdout =
        r#"{"window_id":870906106,"x":1504,"y":684,"w":400,"h":300}"#.into();
    let rows = drawer(&app, &test);
    assert!(row(&rows, "  Geometry")
        .0
        .contains("✔ within ±1 px of expected"));
    assert!(row(&rows, "  Warning").0.contains("workarea differs"));
}

/// No probe, only the extension's log: the check still runs on the log bounds, but the row says
/// the workarea is extension-derived and is yellow, not the green independent pass.
#[test]
fn test_log_only_is_judged_but_marked_extension_derived() {
    let (mut app, test) = create_app_with_real_fixture();
    set_probe(&mut app, None);
    let rows = drawer(&app, &test);
    let g = row(&rows, "  Geometry");
    assert!(
        g.0.contains("✔ within ±1 px (workarea from extension log, not independently probed)"),
        "{}",
        g.0
    );
    assert!(
        g.1.contains(&Color::Yellow) && !g.1.contains(&Color::Green),
        "{:?}",
        g.1
    );
    // Judged, not skipped: a wrong position still fails on the log workarea.
    app.results.get_mut(&test.id).unwrap().stdout =
        r#"{"window_id":870906106,"x":1500,"y":764,"w":400,"h":300}"#.into();
    let rows = drawer(&app, &test);
    assert!(row(&rows, "  Geometry").0.contains('✘'));
}

/// No workarea from either source: Geometry says unchecked, and an automatic test that passed
/// is shown as "Pass (geometry unchecked)" in yellow, never as the green "✔ Pass".
#[test]
fn test_no_workarea_shows_unchecked_marker_not_a_plain_pass() {
    let (mut app, mut test) = create_app_with_real_fixture();
    set_probe(&mut app, None);
    test.kind = TestKind::Auto;
    let r = app.results.get_mut(&test.id).unwrap();
    r.log_slice = None;
    r.auto_status = AutoStatus::AutoPass;
    r.final_status = FinalStatus::Pass;
    let rows = drawer(&app, &test);
    let g = row(&rows, "  Geometry");
    assert!(
        g.0.contains("⚠ unchecked: no workarea from the probe or the extension log"),
        "{}",
        g.0
    );
    let v = row(&rows, "  Verdict");
    assert!(v.0.contains("⚠ Pass (geometry unchecked)"), "{}", v.0);
    assert!(
        v.1.contains(&Color::Yellow) && !v.1.contains(&Color::Green),
        "{:?}",
        v.1
    );
    assert!(!v.0.contains("✔"));
    // The same result with a probe is a plain green pass.
    set_probe(&mut app, Some(Rect::new(0, 40, 1920, 1040)));
    let rows = drawer(&app, &test);
    assert!(row(&rows, "  Verdict").0.contains("✔ Pass"));
}
