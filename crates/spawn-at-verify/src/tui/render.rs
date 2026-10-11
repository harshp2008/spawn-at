//! Headless rendering of the detail drawer for recorded real runs.
//!
//! Used by `spawn-at-verify render` and by tests. Only data recorded from real runs and embedded
//! from `tests/fixtures/` is rendered; nothing is invented.

use super::app::TuiApp;
use super::views::{build_drawer_content, draw_ui};
use crate::report::SessionReport;
use crate::runner::{RunOptions, TestResult};
use ratatui::{backend::TestBackend, Terminal};
use std::path::{Path, PathBuf};
use unicode_width::UnicodeWidthStr;

/// Recorded session reports embedded at compile time: (file name, JSON text).
const FIXTURES: &[(&str, &str)] = &[
    (
        "real_placement_run.json",
        include_str!("../../tests/fixtures/real_placement_run.json"),
    ),
    (
        "real_delay500_run.json",
        include_str!("../../tests/fixtures/real_delay500_run.json"),
    ),
];

/// One recorded real case that can be rendered.
pub struct RecordedCase {
    /// Test id, e.g. `cloak/delayed-paint/delay-500`.
    pub id: String,
    /// Fixture file the recording came from.
    pub fixture: &'static str,
    /// The recorded result (stdout, log slice, statuses) exactly as saved.
    pub result: TestResult,
}

/// Returns every recorded case in fixture order.
pub fn recorded_cases() -> Vec<RecordedCase> {
    let mut cases = Vec::new();
    for (name, text) in FIXTURES {
        let report: SessionReport = serde_json::from_str(text)
            .unwrap_or_else(|e| panic!("embedded fixture {} is invalid: {}", name, e));
        for result in report.results {
            cases.push(RecordedCase {
                id: result.id.clone(),
                fixture: name,
                result,
            });
        }
    }
    cases
}

/// Finds a case by exact test id, or by a suffix that identifies exactly one case
/// (e.g. `delay-500`).
pub fn find_case(name: &str) -> Result<RecordedCase, String> {
    let mut matches: Vec<RecordedCase> = recorded_cases()
        .into_iter()
        .filter(|c| c.id == name || c.id.ends_with(&format!("/{}", name)))
        .collect();
    if let Some(i) = matches.iter().position(|c| c.id == name) {
        return Ok(matches.swap_remove(i));
    }
    match matches.len() {
        1 => Ok(matches.remove(0)),
        0 => Err(format!(
            "no recorded case matches '{}'; available: {}",
            name,
            recorded_cases()
                .iter()
                .map(|c| c.id.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        )),
        _ => Err(format!("'{}' is ambiguous", name)),
    }
}

/// Renders the detail drawer for `case` at `width` x `height` and returns the screen as text,
/// one terminal row per line with trailing spaces removed.
///
/// `scroll` is the drawer's scroll offset in rows. Returns an error if any drawer content line
/// or any rendered row is wider than `width`.
pub fn render_case(
    verify_dir: &Path,
    case: &RecordedCase,
    width: u16,
    height: u16,
    scroll: u16,
) -> Result<String, String> {
    // A binary path that does not exist keeps TuiApp::new from querying the live desktop.
    let options = RunOptions {
        bin_path: PathBuf::from("spawn-at-verify-render-no-binary"),
        ..Default::default()
    };
    let mut app = TuiApp::new(verify_dir, options)?;
    // `selected_index` indexes the filtered, sorted test list (not the registry order).
    let idx = app
        .filtered_tests()
        .iter()
        .position(|t| t.id == case.id)
        .ok_or_else(|| format!("test '{}' is not in the verify/ registry", case.id))?;
    app.selected_index = idx;
    app.fixture_exists = true;
    app.detail_drawer_open = true;
    app.detail_scroll = scroll;
    app.results.insert(case.id.clone(), case.result.clone());
    let test = app
        .selected_test()
        .cloned()
        .filter(|t| t.id == case.id)
        .ok_or_else(|| format!("selected test is not '{}'", case.id))?;

    // Drawer content must fit inside the drawer's borders (width - 2).
    let (_lines, plain) = build_drawer_content(&app, &test, width as usize, true);
    let inner = (width as usize).saturating_sub(2);
    for line in plain.lines() {
        if line.width() > inner {
            return Err(format!(
                "drawer line is {} columns, wider than the inner width {}: {:?}",
                line.width(),
                inner,
                line
            ));
        }
    }

    let mut terminal = Terminal::new(TestBackend::new(width, height)).map_err(|e| e.to_string())?;
    terminal
        .draw(|f| draw_ui(f, &app))
        .map_err(|e| e.to_string())?;
    let buf = terminal.backend().buffer();
    let mut out = String::new();
    for y in 0..height {
        let mut row = String::new();
        for x in 0..width {
            row.push_str(buf[(x, y)].symbol());
        }
        let row = row.trim_end();
        if row.width() > width as usize {
            return Err(format!(
                "rendered row {} is {} columns, wider than {}: {:?}",
                y,
                row.width(),
                width,
                row
            ));
        }
        out.push_str(row);
        out.push('\n');
    }
    Ok(out)
}
