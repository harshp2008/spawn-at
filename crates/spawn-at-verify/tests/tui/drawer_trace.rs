//! RESULT section when the log slice holds no trace for the window: "not analysed", never a
//! clean mark, and never the run's wall-clock duration passed off as a cloak time.

use super::create_test_app;
use spawn_at_verify::runner::{AutoStatus, FinalStatus, HumanVerdict, TestResult};
use spawn_at_verify::tui::views::build_drawer_content;

/// Window id the fixture claims in stdout.
const WINDOW: u64 = 870906691;
/// Run duration distinctive enough to spot if it leaks into the Cloak row.
const WALL_CLOCK_MS: u64 = 162;

/// A passed result for the selected test, claiming `WINDOW`, with the given slice and note.
fn passed_result(
    app: &spawn_at_verify::tui::app::TuiApp,
    slice: Option<&str>,
    note: Option<&str>,
) -> (spawn_at_verify::schema::TestDefinition, TestResult) {
    let test = app.selected_test().unwrap().clone();
    let res = TestResult {
        id: test.id.clone(),
        title: test.title.clone(),
        kind: test.kind,
        required: test.required,
        auto_status: AutoStatus::AutoPass,
        human_verdict: HumanVerdict::default(),
        final_status: FinalStatus::Pass,
        disagreement: false,
        duration_ms: WALL_CLOCK_MS,
        exit_code: Some(0),
        stdout: format!(r#"{{"window_id":{}}}"#, WINDOW),
        stderr: String::new(),
        message: None,
        log_slice: slice.map(String::from),
        trace_note: note.map(String::from),
    };
    (test, res)
}

/// The compact drawer's plain text for `res`.
fn drawer(
    app: &mut spawn_at_verify::tui::app::TuiApp,
    test: &spawn_at_verify::schema::TestDefinition,
    res: TestResult,
) -> String {
    app.results.insert(test.id.clone(), res);
    build_drawer_content(app, test, 100, true).1
}

/// The row of `plain` whose label is `label`.
fn row<'a>(plain: &'a str, label: &str) -> &'a str {
    plain
        .lines()
        .find(|l| l.trim_start().starts_with(label))
        .unwrap_or_else(|| panic!("no {} row in:\n{}", label, plain))
}

/// Asserts the Trace/Cloak/Budget rows say nothing was measured.
fn assert_not_analysed(plain: &str) {
    assert!(
        row(plain, "Trace").contains("not analysed (no trace captured)"),
        "Trace row:\n{}",
        plain
    );
    assert!(row(plain, "Cloak").ends_with("not measured"), "{}", plain);
    assert!(row(plain, "Budget").ends_with("not measured"), "{}", plain);
    assert!(
        !plain.contains("clean, no violations"),
        "an absent trace must not read as clean:\n{}",
        plain
    );
    assert!(
        !plain.contains("over provisional budget") && !plain.contains("within budget"),
        "no budget verdict without a measurement:\n{}",
        plain
    );
    assert!(
        !plain.contains(&format!("{}.0 ms", WALL_CLOCK_MS)),
        "the run duration must not appear as a cloak time:\n{}",
        plain
    );
}

/// An empty slice: not analysed, with the empty-slice reason on its own row.
#[test]
fn test_drawer_empty_slice_shows_not_analysed() {
    let mut app = create_test_app();
    let (test, res) = passed_result(&app, Some(""), None);
    let plain = drawer(&mut app, &test, res);
    assert_not_analysed(&plain);
    assert!(
        row(&plain, "Reason").contains("log slice is empty"),
        "{}",
        plain
    );
}

/// No slice and a no-op log source note: the note is shown as the reason.
#[test]
fn test_drawer_noop_log_source_shows_reason() {
    let mut app = create_test_app();
    let note = "no log source: XDG_CURRENT_DESKTOP is unset";
    let (test, res) = passed_result(&app, None, Some(note));
    let plain = drawer(&mut app, &test, res);
    assert_not_analysed(&plain);
    assert!(row(&plain, "Reason").contains(note), "{}", plain);
}

/// A slice that only holds another window's events: not analysed, reason names the window.
#[test]
fn test_drawer_slice_with_only_another_windows_events_shows_not_analysed() {
    let mut app = create_test_app();
    let other = "[spawn-at-time] +10.00ms | CLOAK win=[id=870906689 pid=1 class=\"\" app=\"\" type=DROPDOWN_MENU proto=Wayland] opacity=0 frame=(0,0,10x10)\n\
                 [spawn-at-time] +40.00ms | UNCLOAK win=[id=870906689 pid=1 class=\"\" app=\"\" type=DROPDOWN_MENU proto=Wayland] opacity=255 frame=(0,0,10x10)";
    let (test, res) = passed_result(&app, Some(other), None);
    let plain = drawer(&mut app, &test, res);
    assert_not_analysed(&plain);
    assert!(
        row(&plain, "Reason").contains(&format!("no event for window {}", WINDOW)),
        "{}",
        plain
    );
}

/// A passing trace-dependent run with no trace: the verdict reads "Pass (trace unchecked)", not
/// the green "✔ Pass", and the Trace row still says "not analysed".
#[test]
fn test_drawer_trace_unchecked_verdict() {
    let mut app = create_test_app();
    let (test, mut res) = passed_result(
        &app,
        Some(""),
        Some("no log source: XDG_CURRENT_DESKTOP is unset"),
    );
    res.auto_status = AutoStatus::AutoPassTraceUnchecked;
    res.final_status = FinalStatus::PassTraceUnchecked;
    let plain = drawer(&mut app, &test, res);
    assert!(
        row(&plain, "Verdict").contains("⚠ Pass (trace unchecked)"),
        "{}",
        plain
    );
    assert!(!plain.contains("✔ Pass"), "{}", plain);
    assert_not_analysed(&plain);
}

/// The list symbol for a trace-unchecked pass is `!` in orange, which is neither the green
/// check of a full pass nor the yellow of a geometry-unchecked pass.
#[test]
fn test_trace_unchecked_symbol_and_colour_are_distinct() {
    use spawn_at_verify::tui::views::theme::{status_symbol_and_color, TRACE_UNCHECKED};
    let app = create_test_app();
    let (test, mut res) = passed_result(&app, Some(""), None);
    let (full_sym, full_colour) = status_symbol_and_color(&test, Some(&res), false);
    assert_eq!((full_sym, full_colour), ("✔", ratatui::style::Color::Green));

    res.auto_status = AutoStatus::AutoPassUnchecked;
    res.final_status = FinalStatus::PassUnchecked;
    let (_, geometry_colour) = status_symbol_and_color(&test, Some(&res), false);

    res.auto_status = AutoStatus::AutoPassTraceUnchecked;
    res.final_status = FinalStatus::PassTraceUnchecked;
    let (sym, colour) = status_symbol_and_color(&test, Some(&res), false);
    assert_eq!((sym, colour), ("!", TRACE_UNCHECKED));
    assert_ne!(colour, geometry_colour);
    assert_ne!(colour, full_colour);
}

/// A test that does not need a trace, with logging that could not be enabled: the verdict stays
/// a plain Pass and the Reason row shows why there is no trace.
#[test]
fn test_drawer_non_trace_test_keeps_pass_and_shows_reason() {
    let mut app = create_test_app();
    let note = "extension timing log could not be enabled: gdbus: no such object";
    let (test, res) = passed_result(&app, Some(""), Some(note));
    let plain = drawer(&mut app, &test, res);
    assert!(row(&plain, "Verdict").contains("✔ Pass"), "{}", plain);
    assert!(!plain.contains("trace unchecked"), "{}", plain);
    // The reason wraps over several rows; compare it with the whitespace collapsed.
    let flat = plain.split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(
        row(&plain, "Reason").contains("log slice is empty"),
        "{}",
        plain
    );
    assert!(flat.contains(note), "{}", plain);
}

/// The echo-timeout note is shown in the Reason row of every affected drawer: here two results
/// in a row (as for two tests after one timed out) both carry the identical full sentence.
#[test]
fn test_drawer_echo_timeout_note_shown_for_every_affected_result() {
    let note = spawn_at_verify::ext_logging::echo_timeout_note();
    for _ in 0..2 {
        let mut app = create_test_app();
        let (test, res) = passed_result(&app, None, Some(&note));
        let plain = drawer(&mut app, &test, res);
        // The 100-column drawer wraps the long sentence, so compare with whitespace collapsed.
        let flat = plain.split_whitespace().collect::<Vec<_>>().join(" ");
        assert!(
            row(&plain, "Reason").contains("SetLogging(true) succeeded"),
            "{}",
            plain
        );
        assert!(flat.contains(&note), "{}", plain);
    }
}

/// A `both` test whose automatic check failed and which the human marked Pass has final status
/// Fail, so the Verdict row must say Fail (and that the human disagreed), never `✔ Pass`.
#[test]
fn test_drawer_auto_fail_with_human_pass_shows_fail() {
    let mut app = create_test_app();
    let (test, mut res) = passed_result(&app, None, None);
    res.kind = spawn_at_verify::schema::TestKind::Both;
    res.auto_status = AutoStatus::AutoFail;
    res.final_status = FinalStatus::Fail;
    res.disagreement = true;
    res.human_verdict = HumanVerdict {
        verdict: spawn_at_verify::runner::HumanVerdictKind::Pass,
        note: None,
    };
    let mut test = test;
    test.kind = spawn_at_verify::schema::TestKind::Both;
    let plain = drawer(&mut app, &test, res);
    let verdict = row(&plain, "Verdict");
    assert!(verdict.contains("✖ Fail"), "{}", plain);
    assert!(!verdict.contains("✔ Pass"), "{}", plain);
    assert!(verdict.contains("marked Pass"), "{}", plain);
}
