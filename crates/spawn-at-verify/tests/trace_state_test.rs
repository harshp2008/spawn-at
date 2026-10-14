//! `assess_trace` must tell "analysed" from "no trace captured": an empty or foreign log slice
//! is never a clean trace. The normal-trace case uses the real recording in
//! `tests/fixtures/real_trace.log`; no timing is invented.

use spawn_at_verify::trace::{assess_trace, TraceState};
use std::path::Path;

/// Window id of the fixture window in `real_trace.log`.
const REAL_WINDOW: u64 = 870905755;

/// The recorded journal/session log.
fn real_trace() -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/real_trace.log");
    std::fs::read_to_string(path).expect("real_trace.log is checked in")
}

/// A slice with no content at all is "not captured", with the reason naming the empty slice.
#[test]
fn test_empty_slice_is_not_captured() {
    for empty in ["", "  \n\n \t\n"] {
        let state = assess_trace(Some(empty), Some(REAL_WINDOW), None);
        let why = state.reason().expect("empty slice has no trace");
        assert!(why.contains("log slice is empty"), "reason was {}", why);
        assert!(state.report().is_none());
    }
}

/// No slice at all (the log source failed) is "not captured" too.
#[test]
fn test_missing_slice_is_not_captured() {
    let state = assess_trace(None, Some(REAL_WINDOW), None);
    assert_eq!(state.reason(), Some("no log slice was captured"));
}

/// The run-time note (a no-op log source) is appended to the reason.
#[test]
fn test_noop_log_source_note_is_part_of_the_reason() {
    let note = "no log source: XDG_CURRENT_DESKTOP is unset";
    let state = assess_trace(Some(""), Some(REAL_WINDOW), Some(note));
    let why = state.reason().unwrap();
    assert!(why.ends_with(note), "reason was {}", why);
    let state = assess_trace(None, None, Some(note));
    assert!(state.reason().unwrap().ends_with(note));
}

/// A slice that holds only another window's events must not be reported as this window's trace.
#[test]
fn test_slice_with_only_another_windows_events_is_not_captured() {
    let state = assess_trace(Some(&real_trace()), Some(999), None);
    let why = state.reason().expect("window 999 is not in the recording");
    assert!(
        why.contains("no event for window 999"),
        "reason was {}",
        why
    );
}

/// Lines that name no window (SET_LOGGING, ARM_SPAWN) are not a window's trace.
#[test]
fn test_slice_with_only_windowless_lines_is_not_captured() {
    let slice = "[spawn-at-time] +0.00ms | SET_LOGGING enabled=true\n\
                 [spawn-at-time] +10.00ms | ARM_SPAWN target=\"fixture\"";
    let state = assess_trace(Some(slice), None, None);
    let why = state.reason().unwrap();
    assert!(
        why.contains("no event naming a window"),
        "reason was {}",
        why
    );
}

/// The recorded trace, scoped to its own window, is analysed and has a measured cloak time.
#[test]
fn test_normal_trace_is_analysed() {
    let state = assess_trace(Some(&real_trace()), Some(REAL_WINDOW), None);
    let TraceState::Analysed(report) = state else {
        panic!("the recording holds events for window {}", REAL_WINDOW);
    };
    assert!(!report.events.is_empty());
    assert!(report.cloak_duration_ms.is_some());
}
