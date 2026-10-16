//! The per-action timeline is drawn by the same parser and block renderer as the spawn timeline:
//! golden text of the owner's real transform and focus lines, one layout for both paths, value
//! humanising, the pid in the header, blank rows between events, wrapping at the spawn width, and
//! the Trace-row wording.

use super::create_test_app;
use spawn_at_verify::runner::{AutoStatus, FinalStatus, HumanVerdict, TestResult};
use spawn_at_verify::trace::action::{
    build_action_trace, owned_events, ActionSpec, ActionTrace, OwnedWindow, TraceStatus,
};
use spawn_at_verify::trace::flash::FlashInfo;
use spawn_at_verify::tui::app::TuiApp;
use spawn_at_verify::tui::views::build_drawer_content;

const TRANSFORM: &str = include_str!("../fixtures/real_transform_existing_window_trace.log");
const FOCUS: &str = include_str!("../fixtures/real_focus_existing_window_trace.log");
const GOLDEN_TRANSFORM: &str = include_str!("../fixtures/golden_transform_timeline.txt");
const GOLDEN_FOCUS: &str = include_str!("../fixtures/golden_focus_timeline.txt");

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

fn trace_of(slice: &str, command: &str, target: &str, id: u64, index: usize) -> ActionTrace {
    let events = owned_events(slice, &owned());
    let spec = ActionSpec {
        index,
        command,
        target: Some(target),
        window_id: Some(id),
    };
    build_action_trace(&spec, &events, &owned(), None, None)
}

fn result_with(app: &TuiApp, traces: Vec<ActionTrace>) -> TestResult {
    let test = app.selected_test().unwrap();
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
        message: None,
        log_slice: None,
        trace_note: None,
        final_rect: None,
        flash_info: FlashInfo {
            action_traces: traces,
            ..Default::default()
        },
        wait_info: Default::default(),
    }
}

/// The drawer text from the TIMELINE heading on, at width `w`.
fn timeline(res: TestResult, w: usize, compact: bool) -> String {
    let mut app = create_test_app();
    let test = app.selected_test().unwrap().clone();
    app.results.insert(test.id.clone(), res);
    let plain = build_drawer_content(&app, &test, w, compact).1;
    let at = plain.find(" ── TIMELINE").expect("TIMELINE heading");
    let mut out: Vec<&str> = plain[at..].lines().map(str::trim_end).collect();
    while out.last() == Some(&"") {
        out.pop();
    }
    out.join("\n") + "\n"
}

fn rendered(slice: &str, command: &str, target: &str, id: u64) -> String {
    let app = create_test_app();
    let res = result_with(&app, vec![trace_of(slice, command, target, id, 0)]);
    timeline(res, 100, true)
}

/// The real transform lines, through the shared renderer, equal the stored text. Regenerate with
/// `UPDATE_GOLDEN=1` after a deliberate change and read the diff.
#[test]
fn test_golden_transform_timeline() {
    let got = rendered(TRANSFORM, "transform", "w", 3210662173);
    if std::env::var("UPDATE_GOLDEN").is_ok() {
        std::fs::write(
            concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/fixtures/golden_transform_timeline.txt"
            ),
            &got,
        )
        .unwrap();
    }
    assert_eq!(got, GOLDEN_TRANSFORM, "{got}");
}

/// The real focus lines, likewise.
#[test]
fn test_golden_focus_timeline() {
    let got = rendered(FOCUS, "focus", "a", 3210662165);
    if std::env::var("UPDATE_GOLDEN").is_ok() {
        std::fs::write(
            concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/fixtures/golden_focus_timeline.txt"
            ),
            &got,
        )
        .unwrap();
    }
    assert_eq!(got, GOLDEN_FOCUS, "{got}");
}

/// The lines of `plain` after the TIMELINE heading and, for the action path, its one caption.
fn blocks(timeline: &str) -> Vec<String> {
    timeline
        .lines()
        .skip(1)
        .filter(|l| !l.contains("offsets count from the first line"))
        .map(str::to_string)
        .collect()
}

/// A spawn result and an action result built from the same lines draw the same event blocks, to
/// the column: this is what stops the two paths drifting apart again.
#[test]
fn test_spawn_and_action_timelines_have_identical_blocks() {
    let app = create_test_app();
    let action = timeline(
        result_with(&app, vec![trace_of(FOCUS, "focus", "a", 3210662165, 0)]),
        100,
        true,
    );
    let mut spawn = result_with(&app, vec![]);
    spawn.log_slice = Some(FOCUS.to_string());
    spawn.stdout = r#"{"window_id":3210662165}"#.into();
    let spawned = timeline(spawn, 100, true);
    assert!(
        spawned.lines().next().unwrap().contains("pid 48203"),
        "{spawned}"
    );
    assert_eq!(blocks(&action), blocks(&spawned), "{action}\n{spawned}");
    assert_eq!(
        action.lines().next(),
        spawned.lines().next(),
        "same heading, pid included"
    );
}

/// Floats are shown to one decimal, a value nobody maps passes through unchanged, an enum-like
/// reason is worded as the spawn view words it, and no field is dropped.
#[test]
fn test_values_are_humanised_like_spawn_and_nothing_is_dropped() {
    let line = r#"[spawn-at-time] +10.00ms | READY_WAIT_RESOLVED win=[id=3210662173 pid=48217 class="verify.owned.w" app="" type=NORMAL proto=Wayland] outcome=reached elapsed_ms=2.7 waited_ms=2.5540000200271606 reason=AT_TARGET kind=NativeWayland preFrame=(200,240,400x300) bufferRect=(1,2,3x4) odd=Thing_1"#;
    let app = create_test_app();
    let res = result_with(&app, vec![trace_of(line, "transform", "w", 3210662173, 0)]);
    let t = timeline(res, 100, true);
    for want in [
        "elapsed ms    2.7",
        "waited ms     2.6",
        "reason        at target",
        "kind          NativeWayland",
        "pre-frame     200,240 400×300",
        "buffer rect   1,2 3×4",
        "odd           Thing_1",
        "outcome       reached",
    ] {
        assert!(t.contains(want), "missing '{want}' in:\n{t}");
    }
}

/// No `pid=` (or `pid=-1`) in the lines: the heading has the window only, never a made-up pid.
#[test]
fn test_missing_pid_is_left_out_of_the_heading() {
    let none = FOCUS.replace("pid=48203", "pid=-1");
    let app = create_test_app();
    let t = timeline(
        result_with(&app, vec![trace_of(&none, "focus", "a", 3210662165, 0)]),
        100,
        true,
    );
    let head = t.lines().next().unwrap();
    assert!(head.contains("window 3210662165"), "{head}");
    assert!(!head.contains("pid"), "{head}");
    assert!(!t.contains("pid ?"), "{t}");
}

/// A blank row between consecutive events and none at the end; and a long value wraps inside the
/// same value column the spawn view uses.
#[test]
fn test_blank_rows_between_events_and_wrapping_at_the_spawn_width() {
    let t = rendered(TRANSFORM, "transform", "w", 3210662173);
    let lines: Vec<&str> = t.lines().collect();
    let tag_rows: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, l)| l.trim_start().starts_with('+'))
        .map(|(i, _)| i)
        .collect();
    assert_eq!(tag_rows.len(), 16);
    for r in &tag_rows[1..] {
        assert_eq!(lines[r - 1], "", "blank row before event at line {r}");
    }
    assert!(!t.ends_with("\n\n"));
    let long = format!(
        r#"[spawn-at-time] +1.00ms | BATCH_STEP win=[id=3210662173 pid=1 class="c" app="" type=NORMAL proto=Wayland] note={}"#,
        "word-".repeat(40)
    );
    let app = create_test_app();
    let narrow = timeline(
        result_with(&app, vec![trace_of(&long, "transform", "w", 3210662173, 0)]),
        60,
        true,
    );
    assert!(
        narrow.lines().all(|l| l.chars().count() <= 60),
        "a line is wider than the drawer:\n{narrow}"
    );
    assert!(narrow.matches("word-").count() >= 40, "{narrow}");
}

fn trace_row(res: TestResult) -> String {
    let mut app = create_test_app();
    let test = app.selected_test().unwrap().clone();
    app.results.insert(test.id.clone(), res);
    let plain = build_drawer_content(&app, &test, 110, true).1;
    plain
        .lines()
        .find(|l| l.trim_start().starts_with("Trace "))
        .unwrap_or_else(|| panic!("no Trace row:\n{plain}"))
        .trim()
        .to_string()
}

/// Trace-row wording: clean (with the event count), a violation, and the reasons for the cases
/// where the checks cannot be applied; no label with one action, a label with several.
#[test]
fn test_trace_row_wording() {
    let app = create_test_app();
    let clean = trace_row(result_with(
        &app,
        vec![trace_of(TRANSFORM, "transform", "w", 3210662173, 0)],
    ));
    assert!(
        clean.starts_with("Trace") && clean.ends_with("✔ clean, no violations · 16 events"),
        "{clean}"
    );

    let late = format!(
        "{TRANSFORM}[spawn-at-time] +2579316.00ms | ANCHOR_CORRECTION win=[id=3210662173 pid=48217 class=\"verify.owned.w\" app=\"\" type=NORMAL proto=Wayland] size=(300x200) from=(1604,56) to=(1600,56)\n"
    );
    let bad = trace_row(result_with(
        &app,
        vec![trace_of(&late, "transform", "w", 3210662173, 0)],
    ));
    assert!(
        bad.contains("✖ ANCHOR_CORRECTION: size or position changed after the reveal · 17 events"),
        "{bad}"
    );

    let focus = trace_row(result_with(
        &app,
        vec![trace_of(FOCUS, "focus", "a", 3210662165, 0)],
    ));
    assert!(
        focus.contains("checks not applied: this command has no cloak phase · 2 events"),
        "{focus}"
    );
    assert!(!focus.contains("clean"), "{focus}");

    let spec = |status| {
        let s = ActionSpec {
            index: 0,
            command: "close",
            target: Some("a"),
            window_id: Some(1),
        };
        build_action_trace(&s, &[], &owned(), Some(status), None)
    };
    let na = trace_row(result_with(&app, vec![spec(TraceStatus::NotApplicable)]));
    assert!(
        na.ends_with("not applicable: the extension logs nothing for this command"),
        "{na}"
    );
    let none = {
        let s = ActionSpec {
            index: 0,
            command: "focus",
            target: Some("a"),
            window_id: Some(1),
        };
        build_action_trace(&s, &[], &owned(), Some(TraceStatus::NoLines), None)
    };
    let nl = trace_row(result_with(&app, vec![none]));
    assert!(
        nl.ends_with("no lines for this window in the slice"),
        "{nl}"
    );

    // Two actions: each Trace row carries its label.
    let both = result_with(
        &app,
        vec![
            trace_of(FOCUS, "focus", "a", 3210662165, 0),
            trace_of(TRANSFORM, "transform", "w", 3210662173, 1),
        ],
    );
    let mut app2 = create_test_app();
    let test = app2.selected_test().unwrap().clone();
    app2.results.insert(test.id.clone(), both);
    let plain = build_drawer_content(&app2, &test, 110, true).1;
    assert!(
        plain.contains("#0 focus on 'a': checks not applied"),
        "{plain}"
    );
    assert!(
        plain.contains("#1 transform on 'w': ✔ clean, no violations"),
        "{plain}"
    );
    assert!(!plain.contains("event(s)"), "the old count wording is gone");
    assert!(!plain.contains("Offsets "), "no separate Offsets row");
}
