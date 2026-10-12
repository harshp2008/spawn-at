//! Headless rendering of the detail drawer for recorded real runs.
//!
//! Used by `spawn-at-verify render` and by tests. Only data recorded from real runs and embedded
//! from `tests/fixtures/` is rendered; nothing is invented.

use super::app::TuiApp;
use super::views::{build_drawer_content, draw_ui};
use crate::probe::EnvironmentInfo;
use crate::report::{EnvironmentFingerprint, SessionReport};
use crate::runner::{RunOptions, TestResult};
use ratatui::{backend::TestBackend, Terminal};
use std::path::{Path, PathBuf};
use unicode_width::UnicodeWidthStr;

/// Recorded session reports embedded at compile time: (file name, JSON text, label).
///
/// The label distinguishes several recordings of the same test: a recording with a label is
/// named `<id>@<label>`, one without is named by its test id alone.
const FIXTURES: &[(&str, &str, Option<&str>)] = &[
    (
        "real_placement_run.json",
        include_str!("../../tests/fixtures/real_placement_run.json"),
        None,
    ),
    (
        "real_delay500_run.json",
        include_str!("../../tests/fixtures/real_delay500_run.json"),
        None,
    ),
    (
        "real_delay500_probed_run.json",
        include_str!("../../tests/fixtures/real_delay500_probed_run.json"),
        Some("probed"),
    ),
    (
        "real_delay0_no_trace_run.json",
        include_str!("../../tests/fixtures/real_delay0_no_trace_run.json"),
        Some("no-trace"),
    ),
    (
        "real_placement_top_left_run.json",
        include_str!("../../tests/fixtures/real_placement_top_left_run.json"),
        None,
    ),
    (
        "real_placement_center_200x150_fail_run.json",
        include_str!("../../tests/fixtures/real_placement_center_200x150_fail_run.json"),
        None,
    ),
];

/// One recorded real case that can be rendered.
pub struct RecordedCase {
    /// Name used with `--case`: the test id, or `<id>@<label>` for a labelled recording.
    pub name: String,
    /// Test id, e.g. `cloak/delayed-paint/delay-500`.
    pub id: String,
    /// Fixture file the recording came from.
    pub fixture: &'static str,
    /// The recorded result (stdout, log slice, statuses) exactly as saved.
    pub result: TestResult,
    /// The environment fingerprint recorded with it, including the workarea measurements for
    /// recordings made after they were added.
    pub environment: EnvironmentFingerprint,
}

impl RecordedCase {
    /// The environment as the run saw it, rebuilt from the recording: nothing is probed live, so
    /// a recording without workarea measurements stays without them and its geometry is judged
    /// (and marked) accordingly.
    pub fn environment_info(&self) -> EnvironmentInfo {
        EnvironmentInfo {
            session_type: self.environment.session_type.clone(),
            desktop: self.environment.desktop.clone(),
            gnome_version: self.environment.gnome_version,
            monitors_count: self.environment.monitors_count,
            primary_workarea: self.environment.primary_workarea,
            independent_workarea: self.environment.independent_workarea,
        }
    }
}

/// Returns every recorded case in fixture order.
pub fn recorded_cases() -> Vec<RecordedCase> {
    let mut cases = Vec::new();
    for (file, text, label) in FIXTURES {
        let report: SessionReport = serde_json::from_str(text)
            .unwrap_or_else(|e| panic!("embedded fixture {} is invalid: {}", file, e));
        for result in report.results {
            let name = match label {
                Some(l) => format!("{}@{}", result.id, l),
                None => result.id.clone(),
            };
            cases.push(RecordedCase {
                name,
                id: result.id.clone(),
                fixture: file,
                result,
                environment: report.environment.clone(),
            });
        }
    }
    cases
}

/// Finds a case by exact name, or by a suffix of its name that identifies exactly one case
/// (e.g. `delay-500` or `delay-500@probed`).
pub fn find_case(name: &str) -> Result<RecordedCase, String> {
    let mut matches: Vec<RecordedCase> = recorded_cases()
        .into_iter()
        .filter(|c| c.name == name || c.name.ends_with(&format!("/{}", name)))
        .collect();
    if let Some(i) = matches.iter().position(|c| c.name == name) {
        return Ok(matches.swap_remove(i));
    }
    match matches.len() {
        1 => Ok(matches.remove(0)),
        0 => Err(format!(
            "no recorded case matches '{}'; available: {}",
            name,
            recorded_cases()
                .iter()
                .map(|c| c.name.as_str())
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
    // Judge with the environment the run recorded, never a live probe of this machine.
    app.env_info = Some(case.environment_info());
    // The load the run recorded (absent in older recordings), never this machine's load now.
    app.run_load = case
        .environment
        .cpu_count
        .map(|cpus| crate::tui::app::RunLoad {
            start: case.environment.load_avg_start,
            end: case.environment.load_avg_end,
            cpus,
        });
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
