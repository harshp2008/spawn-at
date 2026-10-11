//! GEOMETRY table and RESULT "Geometry" row: Expected comes from the oracle, the mark comes from
//! comparing it with Actual within the spec's tolerance, and the runner's verdict agrees.
//!
//! The recorded `placement/bottom-right` run is used: spec bottom-right, 400x300, margin 16,
//! tolerance 1; its log records the workarea 0,40 1920x1040, so Expected is 1504, 764.

use super::{create_app_with_real_fixture, dump_buffer_to_string};
use ratatui::{backend::TestBackend, style::Color, Terminal};
use spawn_at_verify::oracle::Rect;
use spawn_at_verify::probe::EnvironmentInfo;
use spawn_at_verify::runner::validate_geometry;
use spawn_at_verify::tui::app::TuiApp;
use spawn_at_verify::tui::views::{build_drawer_content, draw_ui};

/// Stdout JSON for a window at `x, y` of size `w`x`h`.
fn stdout_at(x: i32, y: i32, w: u32, h: u32) -> String {
    format!(
        r#"{{"window_id":870906106,"x":{},"y":{},"w":{},"h":{}}}"#,
        x, y, w, h
    )
}

/// App whose recorded result has `stdout` replaced; returns the app and the test id.
fn app_with_stdout(stdout: &str) -> (TuiApp, String) {
    let (mut app, test) = create_app_with_real_fixture();
    app.results.get_mut(&test.id).unwrap().stdout = stdout.to_string();
    (app, test.id)
}

/// The drawer text, the GEOMETRY Position row, Size row, and the RESULT Geometry row.
fn rows(app: &TuiApp, id: &str) -> (String, String, String, String) {
    let test = app.registry.get_test(id).unwrap().clone();
    let (_l, plain) = build_drawer_content(app, &test, 100, true);
    let find = |p: &str| {
        plain
            .lines()
            .find(|l| l.starts_with(p))
            .unwrap_or_else(|| panic!("no row {:?} in:\n{}", p, plain))
            .to_string()
    };
    (
        plain.clone(),
        find("  Position"),
        find("  Size"),
        find("  Geometry"),
    )
}

/// Runner verdict for the same recorded data, using a dummy environment (the log records the
/// workarea, so the fallback is not consulted).
fn runner_verdict(app: &TuiApp, id: &str) -> Result<(), String> {
    let test = app.registry.get_test(id).unwrap().clone();
    let res = &app.results[id];
    let env = EnvironmentInfo {
        session_type: "unknown".into(),
        desktop: "unknown".into(),
        gnome_version: None,
        monitors_count: 1,
        primary_workarea: None,
    };
    validate_geometry(&test, &res.stdout, res.log_slice.as_deref(), &env)
}

/// Matching position and size: both rows get ✔, Expected is the oracle's 1504, 764, and the
/// runner agrees.
#[test]
fn test_matching_geometry_shows_check_marks() {
    let (app, id) = app_with_stdout(&stdout_at(1504, 764, 400, 300));
    let (plain, pos, size, result) = rows(&app, &id);
    assert!(
        pos.contains("1504, 764") && pos.contains('✔') && !pos.contains('✘'),
        "{}",
        pos
    );
    assert!(size.contains("400×300") && size.contains('✔'), "{}", size);
    assert!(result.contains("✔ within ±1 px of expected"), "{}", result);
    assert!(!plain.contains('✘'));
    assert!(runner_verdict(&app, &id).is_ok());
}

/// Off by more than the tolerance: Position gets ✘ (Size stays ✔), Expected still shows the
/// oracle's value rather than the observed one, an "Off by" row names the delta, RESULT shows ✘,
/// and the runner fails with the same numbers.
#[test]
fn test_off_by_more_than_tolerance_shows_cross_and_keeps_oracle_expected() {
    let (app, id) = app_with_stdout(&stdout_at(1506, 764, 400, 300));
    let (plain, pos, size, result) = rows(&app, &id);
    assert!(
        pos.contains("1504, 764"),
        "Expected must stay the oracle value: {}",
        pos
    );
    assert!(
        pos.contains("1506, 764") && pos.contains('✘') && !pos.contains('✔'),
        "{}",
        pos
    );
    assert!(size.contains('✔'), "{}", size);
    assert!(
        plain
            .lines()
            .any(|l| l.starts_with("  Off by") && l.contains("dx=+2 dy=+0")),
        "{}",
        plain
    );
    assert!(
        result.contains('✘') && result.contains("dx=+2"),
        "{}",
        result
    );
    let err = runner_verdict(&app, &id).unwrap_err();
    assert!(err.contains("dx=+2"), "{}", err);
}

/// Exactly the tolerance (1 px) in every component still passes, in the table and the runner.
#[test]
fn test_off_by_exactly_tolerance_is_a_pass() {
    let (app, id) = app_with_stdout(&stdout_at(1505, 763, 401, 299));
    let (plain, pos, size, result) = rows(&app, &id);
    assert!(pos.contains('✔') && size.contains('✔'), "{}\n{}", pos, size);
    assert!(result.contains('✔'), "{}", result);
    assert!(!plain.contains('✘'));
    assert!(runner_verdict(&app, &id).is_ok());
}

/// A wrong size alone fails the Size row but not Position.
#[test]
fn test_wrong_size_marks_only_the_size_row() {
    let (app, id) = app_with_stdout(&stdout_at(1504, 764, 420, 300));
    let (_p, pos, size, _r) = rows(&app, &id);
    assert!(pos.contains('✔') && !pos.contains('✘'), "{}", pos);
    assert!(size.contains('✘') && size.contains("420×300"), "{}", size);
}

/// Test not run: Actual is "not measured", no row has a ✔ or ✘, and RESULT says not measured.
/// A workarea is supplied through the probed environment so Expected can still be shown.
#[test]
fn test_not_run_shows_not_measured_and_no_marks() {
    let (mut app, test) = create_app_with_real_fixture();
    app.results.clear();
    let mut env = app.env_info.clone().unwrap();
    env.primary_workarea = Some(Rect::new(0, 40, 1920, 1040));
    app.env_info = Some(env);
    let (plain, pos, size, result) = rows(&app, &test.id);
    assert!(
        pos.contains("1504, 764") && pos.contains("not measured"),
        "{}",
        pos
    );
    assert!(
        size.contains("400×300") && size.contains("not measured"),
        "{}",
        size
    );
    for row in [&pos, &size] {
        assert!(!row.contains('✔') && !row.contains('✘'), "{}", row);
    }
    assert!(result.contains("not measured"), "{}", result);
    assert!(!plain.contains("within"), "no pass claim when not run");
}

/// End to end through the real renderer: a deliberately wrong actual puts a red ✘ on screen,
/// both in the RESULT Geometry row and in the GEOMETRY table, and no ✔ for the position.
#[test]
fn test_wrong_actual_renders_red_cross_in_the_tui() {
    let (mut app, id) = app_with_stdout(&stdout_at(1400, 700, 400, 300));
    app.fixture_exists = true;
    app.detail_drawer_open = true;
    let idx = app
        .filtered_tests()
        .iter()
        .position(|t| t.id == id)
        .unwrap();
    app.selected_index = idx;

    let (w, h) = (100u16, 70u16);
    let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
    terminal.draw(|f| draw_ui(f, &app)).unwrap();
    let text = dump_buffer_to_string(&terminal, w, h);
    assert!(
        text.matches('✘').count() >= 2,
        "expected ✘ in RESULT and GEOMETRY:\n{}",
        text
    );

    let buf = terminal.backend().buffer();
    let mut red_crosses = 0;
    for y in 0..h {
        for x in 0..w {
            let cell = &buf[(x, y)];
            if cell.symbol() == "✘" && cell.fg == Color::Red {
                red_crosses += 1;
            }
        }
    }
    assert!(
        red_crosses >= 2,
        "every ✘ must be red, found {} red",
        red_crosses
    );
    let pos_line = text.lines().find(|l| l.contains("  Position")).unwrap();
    assert!(
        pos_line.contains("1504, 764") && pos_line.contains("1400, 700"),
        "{}",
        pos_line
    );
    assert!(!pos_line.contains('✔'));
}
