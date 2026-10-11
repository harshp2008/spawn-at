//! Regression tests: the analyser and `TestResult` must measure the window the fixture named in
//! stdout, never a leftover window from an earlier run that the whole-second log slice picked up.
//!
//! The scenario mirrors the observed `cloak/delayed-paint/delay-500` drawer: two complete spawn
//! cycles of the same class in one slice, a leftover window A whose CLOAK..UNCLOAK is 31.8 ms and
//! the real window B whose CLOAK..UNCLOAK is 29.1 ms (888.9 - 859.8), plus RAISE/ACTIVATE of an
//! unrelated third window.

use spawn_at_verify::runner::{AutoStatus, FinalStatus, HumanVerdict, TestResult};
use spawn_at_verify::schema::TestKind;
use spawn_at_verify::trace::TraceAnalyser;
use std::collections::BTreeSet;

const WIN_A: u64 = 870906415; // leftover window from the previous run
const WIN_B: u64 = 870906416; // the window the fixture reports in stdout
const WIN_C: u64 = 870900001; // an unrelated window (terminal / desktop)
const BASE: f64 = 27_000_000.0;

/// One log line carrying a `win=[...]` block, in the real journal format.
fn win_line(ms: f64, tag: &str, id: u64, rest: &str) -> String {
    format!(
        "[spawn-at-time] +{:.2}ms | {} win=[id={} pid=1438838 class=\"verify.cloak.delayedpaint\" app=\"\" type=NORMAL proto=Wayland]{}",
        BASE + ms,
        tag,
        id,
        rest
    )
}

/// `ARM_SPAWN` carries only the class: it has no window id.
fn arm_line(ms: f64) -> String {
    format!(
        "[spawn-at-time] +{:.2}ms | ARM_SPAWN target=\"verify.cloak.delayedpaint\"",
        BASE + ms
    )
}

/// Builds the two-cycle slice. `leftover_uncloak_ms` is when window A is uncloaked
/// (126.3 in the observed run, A's CLOAK is at 94.5).
fn two_cycle_slice(leftover_uncloak_ms: f64) -> String {
    let frame = " opacity=0 frame=(50,90,500x350)";
    let mut l = vec![
        arm_line(0.0),
        win_line(9.3, "WINDOW_CREATED", WIN_A, ""),
        win_line(94.3, "ACTOR_MAP", WIN_A, ""),
        win_line(94.5, "CLOAK", WIN_A, frame),
        win_line(
            leftover_uncloak_ms,
            "UNCLOAK",
            WIN_A,
            " opacity=255 frame=(710,385,500x350)",
        ),
        win_line(150.0, "RAISE", WIN_C, " opacity=255 frame=(0,40,1920x1040)"),
        arm_line(314.8),
        win_line(321.4, "WINDOW_CREATED", WIN_B, ""),
        win_line(859.6, "ACTOR_MAP", WIN_B, ""),
        win_line(859.8, "CLOAK", WIN_B, frame),
        win_line(
            888.9,
            "UNCLOAK",
            WIN_B,
            " opacity=255 frame=(710,385,500x350)",
        ),
        win_line(
            900.0,
            "ACTIVATE",
            WIN_C,
            " opacity=255 frame=(0,40,1920x1040)",
        ),
        arm_line(1200.0), // a later run's ARM_SPAWN inside the same slice
    ];
    l.push(String::new());
    l.join("\n")
}

/// Distinct window ids that appear in `events`.
fn window_ids(events: &[spawn_at_verify::trace::TraceEvent]) -> BTreeSet<u64> {
    events.iter().filter_map(|e| e.window_id).collect()
}

/// Builds a passing automatic `TestResult` whose stdout is `stdout` and whose log slice is `slice`.
fn result_with(stdout: &str, slice: String) -> TestResult {
    TestResult {
        id: "cloak/delayed-paint/delay-500".into(),
        title: "App paints slowly (500 ms)".into(),
        kind: TestKind::Auto,
        required: false,
        auto_status: AutoStatus::AutoPass,
        human_verdict: HumanVerdict::default(),
        final_status: FinalStatus::Pass,
        disagreement: false,
        duration_ms: 623,
        exit_code: Some(0),
        stdout: stdout.into(),
        stderr: String::new(),
        message: None,
        log_slice: Some(slice),
    }
}

/// Without a target the analyser sees both cycles and reports the larger cloak (31.8 ms from the
/// leftover window A) - this pins the pre-fix behaviour so the scoped tests below are meaningful.
#[test]
fn test_unscoped_analysis_credits_leftover_window_cloak() {
    let report = TraceAnalyser::new().analyse(&two_cycle_slice(126.3));
    assert_eq!(
        window_ids(&report.events),
        BTreeSet::from([WIN_A, WIN_B, WIN_C])
    );
    let ms = report.cloak_duration_ms.expect("cloak measured");
    assert!((ms - 31.8).abs() < 0.01, "unscoped max cloak was {}", ms);
}

/// With target B only B's events survive, cloak is B's 29.1 ms, and the leftover cycle's
/// id-less ARM_SPAWN (at +0.0) and the later run's ARM_SPAWN (at +1200.0) are dropped.
#[test]
fn test_target_window_is_the_one_measured() {
    let report = TraceAnalyser::new()
        .with_target_window_id(WIN_B)
        .analyse(&two_cycle_slice(126.3));
    assert_eq!(window_ids(&report.events), BTreeSet::from([WIN_B]));
    let ms = report.cloak_duration_ms.expect("cloak measured");
    assert!((ms - 29.1).abs() < 0.01, "scoped cloak was {}", ms);
    let tags: Vec<&str> = report.events.iter().map(|e| e.tag.as_str()).collect();
    assert_eq!(
        tags,
        vec![
            "ARM_SPAWN",
            "WINDOW_CREATED",
            "ACTOR_MAP",
            "CLOAK",
            "UNCLOAK"
        ],
        "only window B's own spawn cycle may remain"
    );
    assert!(
        (report.events[0].elapsed_ms - (BASE + 314.8)).abs() < 0.01,
        "kept ARM_SPAWN must be B's (+314.8), got {}",
        report.events[0].elapsed_ms
    );
    assert_eq!(report.lifecycles.len(), 1, "exactly one lifecycle for B");
}

/// Whatever the leftover window's cloak was (5 ms, 31.8 ms, 500 ms), B's measurement is unchanged.
#[test]
fn test_leftover_window_cannot_change_cloak_duration() {
    for leftover_uncloak in [99.5, 126.3, 594.5] {
        let report = TraceAnalyser::new()
            .with_target_window_id(WIN_B)
            .analyse(&two_cycle_slice(leftover_uncloak));
        let ms = report.cloak_duration_ms.expect("cloak measured");
        assert!(
            (ms - 29.1).abs() < 0.01,
            "leftover uncloak at {} changed B's cloak to {}",
            leftover_uncloak,
            ms
        );
    }
}

/// `TestResult` scopes to the `window_id` printed on stdout: the drawer, RESULT row and
/// description panel all go through `trace_report()` / `cloak_duration_ms()`.
#[test]
fn test_result_measures_window_named_in_stdout() {
    let stdout = format!(
        r#"{{"window_id":{},"x":710,"y":385,"w":500,"h":350}}"#,
        WIN_B
    );
    let res = result_with(&stdout, two_cycle_slice(126.3));
    assert_eq!(res.claimed_window_id(), Some(WIN_B));
    let ms = res.cloak_duration_ms().expect("cloak measured");
    assert!((ms - 29.1).abs() < 0.01, "result cloak was {}", ms);
    let report = res.trace_report().unwrap();
    assert_eq!(window_ids(&report.events), BTreeSet::from([WIN_B]));
}

/// If stdout names a window that never appears in the slice, nothing is measured - the result
/// must not fall back to measuring some other window of the class.
#[test]
fn test_result_naming_absent_window_measures_nothing() {
    let res = result_with(r#"{"window_id":999}"#, two_cycle_slice(126.3));
    assert_eq!(res.cloak_duration_ms(), None);
    assert!(res.trace_report().unwrap().events.is_empty());
}

/// An explicit `window_id: null` names no window, so the whole slice is analysed unscoped.
#[test]
fn test_result_with_null_window_id_falls_back_to_unscoped() {
    let res = result_with(r#"{"window_id":null}"#, two_cycle_slice(126.3));
    assert_eq!(res.claimed_window_id(), None);
    let ms = res.cloak_duration_ms().expect("cloak measured");
    assert!(
        (ms - 31.8).abs() < 0.01,
        "unscoped fallback cloak was {}",
        ms
    );
}
