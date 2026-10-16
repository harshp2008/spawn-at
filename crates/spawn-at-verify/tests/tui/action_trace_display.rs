//! How a version 2 result shows the extension's per-action trace: RESULT rows, TIMELINE, the
//! Result panel and the markdown report. The traces are built from the owner's real journal lines
//! (`tests/fixtures/real_*_existing_window_trace.log`), so no timing is invented. The `run` report
//! carries them; the TUI itself still writes no report.

use super::{create_test_app, dump_buffer_to_string};
use ratatui::{backend::TestBackend, Terminal};
use spawn_at_verify::report::render_markdown_report;
use spawn_at_verify::runner::{AutoStatus, FinalStatus, HumanVerdict, TestResult};
use spawn_at_verify::trace::action::{
    build_action_trace, owned_events, ActionSpec, ActionTrace, OwnedWindow, TraceStatus,
};
use spawn_at_verify::trace::flash::{FlashBasis, FlashInfo};
use spawn_at_verify::tui::app::TuiApp;
use spawn_at_verify::tui::views::{build_drawer_content, draw_ui};

const TRANSFORM: &str = include_str!("../fixtures/real_transform_existing_window_trace.log");
const FOCUS: &str = include_str!("../fixtures/real_focus_existing_window_trace.log");

fn owned() -> Vec<OwnedWindow> {
    vec![
        OwnedWindow {
            id: 3210662173,
            name: "w".into(),
        },
        OwnedWindow {
            id: 3210662165,
            name: "a".into(),
        },
    ]
}

fn trace_of(slice: &str, command: &str, target: &str, id: u64) -> ActionTrace {
    let events = owned_events(slice, &owned());
    let spec = ActionSpec {
        index: 0,
        command,
        target: Some(target),
        window_id: Some(id),
    };
    build_action_trace(&spec, &events, &owned(), None, None)
}

fn empty_trace(command: &str, status: TraceStatus) -> ActionTrace {
    let spec = ActionSpec {
        index: 0,
        command,
        target: Some("a"),
        window_id: Some(3210662165),
    };
    build_action_trace(&spec, &[], &owned(), Some(status), None)
}

fn result_with(app: &TuiApp, traces: Vec<ActionTrace>, basis: Option<FlashBasis>) -> TestResult {
    let test = app.selected_test().unwrap();
    let raw: Vec<String> = traces.iter().map(|t| t.raw_lines()).collect();
    TestResult {
        id: test.id.clone(),
        title: test.title.clone(),
        kind: test.kind,
        required: test.required,
        auto_status: AutoStatus::AutoPass,
        human_verdict: HumanVerdict::default(),
        final_status: FinalStatus::Pass,
        disagreement: false,
        duration_ms: 90,
        exit_code: Some(0),
        stdout: String::new(),
        stderr: String::new(),
        message: Some("Assertions passed".into()),
        log_slice: Some(raw.join("\n")).filter(|s| !s.is_empty()),
        trace_note: None,
        final_rect: None,
        flash_info: FlashInfo {
            action_traces: traces,
            flash_basis: basis,
            ..Default::default()
        },
        wait_info: Default::default(),
    }
}

fn drawer(app: &mut TuiApp, res: TestResult, compact: bool) -> String {
    let test = app.selected_test().unwrap().clone();
    app.results.insert(test.id.clone(), res);
    build_drawer_content(app, &test, 110, compact).1
}

fn row<'a>(plain: &'a str, label: &str) -> &'a str {
    plain
        .lines()
        .find(|l| l.trim_start().starts_with(label))
        .unwrap_or_else(|| panic!("no {label} row in:\n{plain}"))
}

/// A transform: Cloak is CLOAK to UNCLOAK_4D_REVEAL (7.0 ms), Budget is judged on it, the Trace
/// row counts the 16 events, the timeline lists them with offsets (UNCLOAK_4D_REVEAL at +7.60 ms),
/// the flash caveat is shown, and the old "not analysed" and "no log slice" texts are gone.
#[test]
fn test_transform_rows_and_timeline() {
    let mut app = create_test_app();
    let t = trace_of(TRANSFORM, "transform", "w", 3210662173);
    let res = result_with(&app, vec![t], Some(FlashBasis::Client));
    let plain = drawer(&mut app, res, true);
    assert!(
        row(&plain, "Cloak").contains("7.0 ms (CLOAK to UNCLOAK_4D_REVEAL)"),
        "{plain}"
    );
    assert!(row(&plain, "Budget").contains("within budget"), "{plain}");
    assert!(
        row(&plain, "Trace").contains("✔ clean, no violations · 16 events"),
        "{plain}"
    );
    assert!(
        plain.contains("informational") && plain.contains("guarantee"),
        "{plain}"
    );
    assert!(plain.contains("no pixels are captured"), "{plain}");
    assert!(!plain.contains("not analysed"), "{plain}");
    assert!(!plain.contains("no log slice was captured"), "{plain}");
    assert!(
        plain
            .lines()
            .any(|l| l.contains("+7.6 ms") && l.contains("UNCLOAK_4D_REVEAL")),
        "{plain}"
    );
    assert!(
        plain
            .lines()
            .any(|l| l.contains("+0.6 ms") && l.ends_with("CLOAK"))
            && plain.contains("frame         100,140 400×300"),
        "{plain}"
    );
}

/// A focus: no cloak phase, so Cloak and Budget say n/a with the command, the flash row says why
/// there is no analysis, and the timeline shows RAISE and ACTIVATE 0.09 ms apart.
#[test]
fn test_focus_rows_say_na_and_timeline_has_raise_and_activate() {
    let mut app = create_test_app();
    let t = trace_of(FOCUS, "focus", "a", 3210662165);
    let res = result_with(&app, vec![t], None);
    let plain = drawer(&mut app, res, true);
    assert!(
        row(&plain, "Cloak").contains("n/a (no cloak phase for focus)"),
        "{plain}"
    );
    assert!(row(&plain, "Budget").contains("n/a"), "{plain}");
    assert!(
        row(&plain, "Flash").contains("no flash analysis: this command has no cloak phase"),
        "{plain}"
    );
    assert!(row(&plain, "Trace").contains("· 2 events"), "{plain}");
    assert!(
        plain
            .lines()
            .any(|l| l.contains("+0.0 ms") && l.contains("RAISE")),
        "{plain}"
    );
    assert!(
        plain
            .lines()
            .any(|l| l.contains("+0.1 ms") && l.contains("ACTIVATE")),
        "{plain}"
    );
}

/// Every reason is the exact text, in the Trace row, with the inventory sentence under it.
#[test]
fn test_each_reason_is_shown_in_the_trace_row() {
    let cases = [
        (
            "close",
            TraceStatus::NotApplicable,
            "not applicable: the extension logs nothing for this command",
        ),
        (
            "focus",
            TraceStatus::NoExtension {
                backend: "sway".into(),
            },
            "no extension on this backend (sway)",
        ),
        (
            "focus",
            TraceStatus::CaptureFailed {
                why: "journalctl exited with error: x".into(),
            },
            "capture failed: journalctl exited with error: x",
        ),
        (
            "focus",
            TraceStatus::NoLines,
            "no lines for this window in the slice",
        ),
    ];
    for (command, status, text) in cases {
        let mut app = create_test_app();
        let res = result_with(&app, vec![empty_trace(command, status)], None);
        let plain = drawer(&mut app, res, true);
        assert!(row(&plain, "Trace").contains(text), "{text}:\n{plain}");
        assert!(row(&plain, "Cloak").contains("n/a"), "{plain}");
        // The inventory sentence is only added where lines were expected and absent.
        assert_eq!(
            plain.contains("Extension logs"),
            text == "no lines for this window in the slice",
            "{plain}"
        );
        assert!(!plain.contains("not analysed"), "{plain}");
        assert!(!plain.contains("no log slice was captured"), "{plain}");
    }
}

/// The raw view shows the journal lines as logged, and nothing but the runner's window.
#[test]
fn test_raw_timeline_shows_the_logged_lines() {
    let mut app = create_test_app();
    let t = trace_of(FOCUS, "focus", "a", 3210662165);
    let res = result_with(&app, vec![t], None);
    let plain = drawer(&mut app, res, false);
    assert!(plain.contains("RAISE win=[id=3210662165"), "{plain}");
    assert!(plain.contains("ACTIVATE win=[id=3210662165"), "{plain}");
}

/// The Result panel shows one line per action with the cloak time where there is one.
#[test]
fn test_result_panel_has_a_trace_line_per_action() {
    let mut app = create_test_app();
    app.fixture_exists = true;
    let t = trace_of(TRANSFORM, "transform", "w", 3210662173);
    let res = result_with(&app, vec![t], None);
    let test = app.selected_test().unwrap().clone();
    app.results.insert(test.id.clone(), res);
    let mut terminal = Terminal::new(TestBackend::new(200, 34)).unwrap();
    terminal.draw(|f| draw_ui(f, &app)).unwrap();
    let rendered = dump_buffer_to_string(&terminal, 200, 34);
    assert!(
        rendered.contains("Cloak: 7.0 ms · Trace: 0 violations · 16 events"),
        "{rendered}"
    );
    assert!(!rendered.contains("event(s)"), "{rendered}");
}

/// The markdown report of `run` lists each action's trace; a report with no traced result has no
/// such section.
#[test]
fn test_markdown_report_lists_action_traces() {
    let app = create_test_app();
    let t = trace_of(TRANSFORM, "transform", "w", 3210662173);
    let res = result_with(&app, vec![t], Some(FlashBasis::Client));
    let report = report_of(std::slice::from_ref(&res));
    let md = render_markdown_report(&report);
    let json = serde_json::to_string(&report).unwrap();
    assert!(
        json.contains("\"action_traces\""),
        "the JSON report carries the traces"
    );
    assert!(json.contains("\"cloak_to_reveal_ms\":7.03"), "{json}");
    assert!(md.contains("## 7. Extension trace of actions"), "{md}");
    assert!(
        md.contains("- #0 transform on 'w': ✔ clean, no violations · 16 events (cloak: 7.0 ms"),
        "{md}"
    );
    assert!(md.contains("Window process id: 48217."), "{md}");
    assert!(md.contains("`UNCLOAK_4D_REVEAL`"), "{md}");
    assert!(
        md.contains("+0.6 ms `CLOAK` opacity 0; frame 100,140 400×300"),
        "{md}"
    );
    assert!(md.contains("no pixels are captured"), "{md}");
    let plain = report_of(&[]);
    assert!(!render_markdown_report(&plain).contains("Extension trace of actions"));
}

/// A session report over `results`, built like `run` builds it.
fn report_of(results: &[TestResult]) -> spawn_at_verify::report::SessionReport {
    let preflight = spawn_at_verify::preflight::PreflightReport {
        repo_head: None,
        detected_commit: None,
        repo_dirty: false,
        is_dirty_build: false,
        verdict: spawn_at_verify::preflight::PreflightVerdict::Pass,
        binary_version_output: String::new(),
    };
    let env = spawn_at_verify::probe::EnvironmentInfo {
        session_type: "wayland".into(),
        desktop: "gnome".into(),
        gnome_version: None,
        monitors_count: 1,
        primary_workarea: None,
        independent_workarea: None,
    };
    spawn_at_verify::report::build_session_report_with_runs(
        std::path::Path::new("bin"),
        &preflight,
        &env,
        results,
        &[],
        false,
    )
}

/// One screen with both: the per-action trace of the selected test (details pane: cloak time and
/// `Trace #0 ...`) and, in the Result box below it, the focus-return lines (the terminal recorded
/// at start, each focus command's exit code and message, the window focused afterwards). The
/// trace comes first (higher on screen), as the details pane sits above the Result box.
#[test]
fn test_screen_shows_the_trace_and_the_focus_return_lines_together() {
    use spawn_at_verify::runner::focus_return::{FocusCommand, FocusReport, WindowRef};
    let mut app = create_test_app();
    app.fixture_exists = true;
    let t = trace_of(TRANSFORM, "transform", "w", 3210662173);
    let res = result_with(&app, vec![t], Some(FlashBasis::Client));
    let test = app.selected_test().unwrap().clone();
    app.results.insert(test.id.clone(), res);
    let term = WindowRef {
        id: 41,
        class: "gnome-terminal".into(),
        title: "term".into(),
    };
    app.focus_reports.insert(
        test.id.clone(),
        FocusReport {
            recorded: Some(term.clone()),
            sent: vec![FocusCommand {
                exit_code: Some(0),
                message: "ok".into(),
            }],
            focused_after: Some(term),
            matches: true,
            ..Default::default()
        },
    );
    let mut terminal = Terminal::new(TestBackend::new(220, 60)).unwrap();
    terminal.draw(|f| draw_ui(f, &app)).unwrap();
    let text = dump_buffer_to_string(&terminal, 220, 60);
    let row_of = |needle: &str| {
        text.lines()
            .position(|l| l.contains(needle))
            .unwrap_or_else(|| panic!("no '{needle}' on screen:\n{text}"))
    };
    let trace = row_of("Cloak: 7.0 ms · Trace: 0 violations · 16 events");
    let terminal_line = row_of("terminal: id 41 class 'gnome-terminal'");
    let cmd = row_of("sent 1x: #1 exit 0 'ok'");
    let after = row_of("afterwards: id 41");
    assert!(trace < terminal_line && terminal_line <= cmd && cmd <= after);
    assert!(text.contains("matches terminal: yes"), "{text}");
}
