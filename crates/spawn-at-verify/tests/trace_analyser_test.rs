//! Comprehensive test suite for the trace event parser and lifecycle verification engine.

use spawn_at_verify::trace::{TraceAnalyser, TraceRect, ViolationKind, SCOPE_LIMITATION_NOTICE};
use std::path::Path;

// =======================================================================
// 1. Passing Scenario Traces (Requirement 5)
// =======================================================================

#[test]
fn test_scenario_visible_passes() {
    let raw = r#"
+10.00ms | CLOAK win=[id=1001 pid=2001 class="test" app="test" type=NORMAL proto=Wayland] opacity=0 frame=(10,10,300x200)
+25.00ms | AFTER_SET_SIZE win=[id=1001 pid=2001 class="test" app="test" type=NORMAL proto=Wayland] opacity=0 frame=(10,10,500x400)
+35.00ms | COMMIT_RESOLVED win=[id=1001 pid=2001 class="test" app="test" type=NORMAL proto=Wayland] reason=exact finalFrame=(500x400) target=(500x400) safe=(nullxnull)
+42.00ms | AFTER_SET_POSITION_ANCHORED win=[id=1001 pid=2001 class="test" app="test" type=NORMAL proto=Wayland] opacity=0 frame=(100,100,500x400)
+50.00ms | UNCLOAK win=[id=1001 pid=2001 class="test" app="test" type=NORMAL proto=Wayland] opacity=255 frame=(100,100,500x400)
"#;

    let analyser = TraceAnalyser::new()
        .with_target_window_id(1001)
        .with_target_geometry(TraceRect {
            x: 100,
            y: 100,
            w: 500,
            h: 400,
        });

    let report = analyser.analyse(raw);
    assert!(
        report.is_pass(),
        "Visible scenario should pass, got: {:?}",
        report.violations
    );
    assert_eq!(report.violations.len(), 0);
    assert_eq!(report.warnings.len(), 0);
    assert_eq!(report.cloak_duration_ms, Some(40.0));
}

#[test]
fn test_scenario_covered_passes() {
    let raw = r#"
+12.00ms | CLOAK win=[id=1002 pid=2002 class="covered" app="covered" type=NORMAL proto=Wayland] opacity=0 frame=(50,50,400x300)
+20.00ms | AFTER_SET_SIZE win=[id=1002 pid=2002 class="covered" app="covered" type=NORMAL proto=Wayland] opacity=0 frame=(50,50,600x450)
+32.00ms | COMMIT_RESOLVED win=[id=1002 pid=2002 class="covered" app="covered" type=NORMAL proto=Wayland] reason=exact finalFrame=(600x450)
+38.00ms | AFTER_SET_POSITION_ANCHORED win=[id=1002 pid=2002 class="covered" app="covered" type=NORMAL proto=Wayland] opacity=0 frame=(200,150,600x450)
+45.00ms | UNCLOAK win=[id=1002 pid=2002 class="covered" app="covered" type=NORMAL proto=Wayland] opacity=255 frame=(200,150,600x450)
"#;

    let analyser = TraceAnalyser::new().with_target_window_id(1002);
    let report = analyser.analyse(raw);
    assert!(
        report.is_pass(),
        "Covered scenario should pass, got: {:?}",
        report.violations
    );
}

#[test]
fn test_scenario_minimized_passes_and_checks_pre_reveal_unminimize() {
    let raw = r#"
+15.00ms | CLOAK win=[id=1003 pid=2003 class="min" app="min" type=NORMAL proto=Wayland] opacity=0 frame=(0,0,300x200)
+28.00ms | AFTER_SET_SIZE win=[id=1003 pid=2003 class="min" app="min" type=NORMAL proto=Wayland] opacity=0 frame=(0,0,450x350)
+40.00ms | COMMIT_RESOLVED win=[id=1003 pid=2003 class="min" app="min" type=NORMAL proto=Wayland] reason=exact finalFrame=(450x350)
+48.00ms | PRE_REVEAL_UNMINIMIZE win=[id=1003 pid=2003 class="min" app="min" type=NORMAL proto=Wayland] unminimizing prior to uncloak
+52.00ms | AFTER_SET_POSITION_ANCHORED win=[id=1003 pid=2003 class="min" app="min" type=NORMAL proto=Wayland] opacity=0 frame=(150,150,450x350)
+60.00ms | UNCLOAK win=[id=1003 pid=2003 class="min" app="min" type=NORMAL proto=Wayland] opacity=255 frame=(150,150,450x350)
"#;

    let analyser = TraceAnalyser::new()
        .with_target_window_id(1003)
        .with_require_pre_reveal_unminimize(true);

    let report = analyser.analyse(raw);
    assert!(
        report.is_pass(),
        "Minimized scenario with PRE_REVEAL_UNMINIMIZE should pass: {:?}",
        report.violations
    );
}

#[test]
fn test_scenario_covered_with_deferred_focus_tolerates_extra_events() {
    let raw = r#"
+5.00ms | FOCUS_DEFERRED win=[id=1004 pid=2004 class="app" app="app" type=NORMAL proto=Wayland] windowId=1004
+12.00ms | CLOAK win=[id=1004 pid=2004 class="app" app="app" type=NORMAL proto=Wayland] opacity=0 frame=(10,10,400x300)
+20.00ms | BATCH_STEP win=[id=1004 pid=2004 class="app" app="app" type=NORMAL proto=Wayland] inst=SetSize
+22.00ms | AFTER_SET_SIZE win=[id=1004 pid=2004 class="app" app="app" type=NORMAL proto=Wayland] opacity=0 frame=(10,10,700x500)
+34.00ms | COMMIT_RESOLVED win=[id=1004 pid=2004 class="app" app="app" type=NORMAL proto=Wayland] reason=exact finalFrame=(700x500)
+40.00ms | BATCH_STEP win=[id=1004 pid=2004 class="app" app="app" type=NORMAL proto=Wayland] inst=SetPositionAnchored
+44.00ms | AFTER_SET_POSITION_ANCHORED win=[id=1004 pid=2004 class="app" app="app" type=NORMAL proto=Wayland] opacity=0 frame=(50,50,700x500)
+48.00ms | RAISE win=[id=1004 pid=2004 class="app" app="app" type=NORMAL proto=Wayland] opacity=0 frame=(50,50,700x500)
+52.00ms | ACTIVATE win=[id=1004 pid=2004 class="app" app="app" type=NORMAL proto=Wayland] opacity=0 frame=(50,50,700x500)
+58.00ms | UNCLOAK win=[id=1004 pid=2004 class="app" app="app" type=NORMAL proto=Wayland] opacity=255 frame=(50,50,700x500)
"#;

    let analyser = TraceAnalyser::new().with_target_window_id(1004);
    let report = analyser.analyse(raw);
    assert!(
        report.is_pass(),
        "Covered scenario with deferred focus should pass despite intervening events: {:?}",
        report.violations
    );
}

#[test]
fn test_scenario_delayed_paint_passes_with_multiple_commit_samples() {
    let raw = r#"
+10.00ms | CLOAK win=[id=1005 pid=2005 class="delayed" app="delayed" type=NORMAL proto=Wayland] opacity=0 frame=(100,100,400x300)
+20.00ms | AFTER_SET_SIZE win=[id=1005 pid=2005 class="delayed" app="delayed" type=NORMAL proto=Wayland] opacity=0 frame=(100,100,600x400)
+30.00ms | COMMIT_SAMPLE win=[id=1005 pid=2005 class="delayed" app="delayed" type=NORMAL proto=Wayland] source=size-change frame=(400x300) last=(400x300)
+60.00ms | COMMIT_SAMPLE win=[id=1005 pid=2005 class="delayed" app="delayed" type=NORMAL proto=Wayland] source=size-change frame=(400x300) last=(400x300)
+110.00ms | COMMIT_SAMPLE win=[id=1005 pid=2005 class="delayed" app="delayed" type=NORMAL proto=Wayland] source=after-update frame=(600x400) last=(400x300)
+112.00ms | COMMIT_RESOLVED win=[id=1005 pid=2005 class="delayed" app="delayed" type=NORMAL proto=Wayland] reason=exact finalFrame=(600x400) target=(600x400)
+118.00ms | AFTER_SET_POSITION_ANCHORED win=[id=1005 pid=2005 class="delayed" app="delayed" type=NORMAL proto=Wayland] opacity=0 frame=(300,200,600x400)
+125.00ms | UNCLOAK win=[id=1005 pid=2005 class="delayed" app="delayed" type=NORMAL proto=Wayland] opacity=255 frame=(300,200,600x400)
"#;

    let analyser = TraceAnalyser::new().with_target_window_id(1005);
    let report = analyser.analyse(raw);
    assert!(
        report.is_pass(),
        "Delayed paint scenario with commit samples should pass: {:?}",
        report.violations
    );
    assert_eq!(report.cloak_duration_ms, Some(115.0));
}

#[test]
fn test_scenario_remaximize_passes() {
    let raw = r#"
+10.00ms | CLOAK win=[id=1006 pid=2006 class="max" app="max" type=NORMAL proto=Wayland] opacity=0 frame=(0,0,1920x1080)
+22.00ms | AFTER_SET_SIZE win=[id=1006 pid=2006 class="max" app="max" type=NORMAL proto=Wayland] opacity=0 frame=(0,0,800x600)
+35.00ms | COMMIT_RESOLVED win=[id=1006 pid=2006 class="max" app="max" type=NORMAL proto=Wayland] reason=exact finalFrame=(800x600)
+42.00ms | AFTER_SET_POSITION_ANCHORED win=[id=1006 pid=2006 class="max" app="max" type=NORMAL proto=Wayland] opacity=0 frame=(200,100,800x600)
+50.00ms | UNCLOAK win=[id=1006 pid=2006 class="max" app="max" type=NORMAL proto=Wayland] opacity=255 frame=(200,100,800x600)
+75.00ms | BATCH_STEP win=[id=1006 pid=2006 class="max" app="max" type=NORMAL proto=Wayland] inst=Maximize
"#;

    let analyser = TraceAnalyser::new().with_target_window_id(1006);
    let report = analyser.analyse(raw);
    assert!(
        report.is_pass(),
        "Remaximize scenario should pass: {:?}",
        report.violations
    );
}

// =======================================================================
// 2. Adversarial & Invariant Violation Traces
// =======================================================================

#[test]
fn test_violation_off_target_visibility() {
    let raw = r#"
+10.00ms | CLOAK win=[id=2001] opacity=0 frame=(10,10,300x200)
+20.00ms | AFTER_SET_SIZE win=[id=2001] opacity=255 frame=(10,10,500x400)
+30.00ms | COMMIT_RESOLVED win=[id=2001] reason=exact
+40.00ms | AFTER_SET_POSITION_ANCHORED win=[id=2001] opacity=0 frame=(100,100,500x400)
+50.00ms | UNCLOAK win=[id=2001] opacity=255 frame=(100,100,500x400)
"#;

    let analyser = TraceAnalyser::new().with_target_window_id(2001);
    let report = analyser.analyse(raw);
    assert!(!report.is_pass());
    assert!(report
        .violations
        .iter()
        .any(|v| v.kind == ViolationKind::OffTargetVisibility));
}

/// Feeds a trace where UNCLOAK (+30ms) precedes COMMIT_RESOLVED (+40ms) on window 2002.
/// Asserts `report.is_pass()` is false and records `ViolationKind::OrderConstraintViolation`.
/// Fails if the analyzer permits revealing a surface before Wayland commits its buffer.
#[test]
fn test_violation_order_uncloak_before_commit() {
    let raw = r#"
+10.00ms | CLOAK win=[id=2002] opacity=0 frame=(10,10,300x200)
+20.00ms | AFTER_SET_SIZE win=[id=2002] opacity=0 frame=(10,10,500x400)
+30.00ms | UNCLOAK win=[id=2002] opacity=255 frame=(100,100,500x400)
+40.00ms | COMMIT_RESOLVED win=[id=2002] reason=exact
"#;

    let analyser = TraceAnalyser::new().with_target_window_id(2002);
    let report = analyser.analyse(raw);
    assert!(!report.is_pass());
    assert!(report
        .violations
        .iter()
        .any(|v| v.kind == ViolationKind::OrderConstraintViolation));
}

#[test]
fn test_violation_order_uncloak_before_pre_reveal_unminimize() {
    let raw = r#"
+10.00ms | CLOAK win=[id=2003] opacity=0 frame=(10,10,300x200)
+20.00ms | AFTER_SET_SIZE win=[id=2003] opacity=0 frame=(10,10,500x400)
+30.00ms | COMMIT_RESOLVED win=[id=2003] reason=exact
+40.00ms | UNCLOAK win=[id=2003] opacity=255 frame=(100,100,500x400)
+50.00ms | PRE_REVEAL_UNMINIMIZE win=[id=2003] unminimizing
"#;

    let analyser = TraceAnalyser::new().with_target_window_id(2003);
    let report = analyser.analyse(raw);
    assert!(!report.is_pass());
    assert!(report
        .violations
        .iter()
        .any(|v| v.kind == ViolationKind::OrderConstraintViolation));
}

#[test]
fn test_violation_multiple_reveal() {
    let raw = r#"
+10.00ms | CLOAK win=[id=2004] opacity=0 frame=(10,10,300x200)
+20.00ms | AFTER_SET_SIZE win=[id=2004] opacity=0 frame=(10,10,500x400)
+30.00ms | COMMIT_RESOLVED win=[id=2004] reason=exact
+40.00ms | UNCLOAK win=[id=2004] opacity=255 frame=(100,100,500x400)
+50.00ms | UNCLOAK win=[id=2004] opacity=255 frame=(100,100,500x400)
"#;

    let analyser = TraceAnalyser::new().with_target_window_id(2004);
    let report = analyser.analyse(raw);
    assert!(!report.is_pass());
    assert!(report
        .violations
        .iter()
        .any(|v| v.kind == ViolationKind::MultipleReveal));
}

#[test]
fn test_violation_lingering_cloak() {
    let raw = r#"
+10.00ms | CLOAK win=[id=2005] opacity=0 frame=(10,10,300x200)
+20.00ms | AFTER_SET_SIZE win=[id=2005] opacity=0 frame=(10,10,500x400)
+30.00ms | COMMIT_RESOLVED win=[id=2005] reason=exact
"#;

    let analyser = TraceAnalyser::new().with_target_window_id(2005);
    let report = analyser.analyse(raw);
    assert!(!report.is_pass());
    assert!(report
        .violations
        .iter()
        .any(|v| v.kind == ViolationKind::LingeringCloak));
}

#[test]
fn test_lingering_cloak_disarmed_by_anchor_released_is_not_violation() {
    let raw = r#"
+10.00ms | CLOAK win=[id=2006] opacity=0 frame=(10,10,300x200)
+20.00ms | ANCHOR_RELEASED win=[id=2006] reason=destroyed
"#;

    let analyser = TraceAnalyser::new().with_target_window_id(2006);
    let report = analyser.analyse(raw);
    assert!(report.is_pass());
}

// =======================================================================
// 3. Timing Budgets (Requirement 4)
// =======================================================================

/// Feeds a trace with CLOAK at +10ms and UNCLOAK at +300ms (duration 290.0 ms against 200.0 ms budget).
/// Asserts `report.is_pass()` is true, violations is 0, warnings has 1 entry ("exceeded warning budget"), and cloak duration is exactly 290.0 ms.
/// Fails if over-budget duration fails the test or if cloak duration arithmetic drifts.
#[test]
fn test_timing_over_budget_generates_warning_not_failure() {
    let raw = r#"
+10.00ms | CLOAK win=[id=3001] opacity=0 frame=(10,10,300x200)
+50.00ms | AFTER_SET_SIZE win=[id=3001] opacity=0 frame=(10,10,500x400)
+200.00ms | COMMIT_RESOLVED win=[id=3001] reason=exact
+280.00ms | AFTER_SET_POSITION_ANCHORED win=[id=3001] opacity=0 frame=(100,100,500x400)
+300.00ms | UNCLOAK win=[id=3001] opacity=255 frame=(100,100,500x400)
"#;

    // Cloak duration is 290ms. Budget is 200ms.
    let analyser = TraceAnalyser::new()
        .with_target_window_id(3001)
        .with_warning_budget_ms(200.0);

    let report = analyser.analyse(raw);
    assert!(
        report.is_pass(),
        "Over-budget duration must be a warning, NOT a failure"
    );
    assert_eq!(report.violations.len(), 0);
    assert_eq!(report.warnings.len(), 1);
    assert!(report.warnings[0].contains("exceeded warning budget"));
    assert_eq!(report.cloak_duration_ms, Some(290.0));
}

#[test]
fn test_watchdog_deadline_reached_is_hard_failure() {
    let raw = r#"
+10.00ms | CLOAK win=[id=3002] opacity=0 frame=(10,10,300x200)
+6010.00ms | [spawn-at] Watchdog deadline reached for cloaked actor; uncloaking unconditionally.
+6015.00ms | UNCLOAK win=[id=3002] opacity=255 frame=(10,10,300x200)
"#;

    let analyser = TraceAnalyser::new().with_target_window_id(3002);
    let report = analyser.analyse(raw);
    assert!(
        !report.is_pass(),
        "Watchdog deadline must produce hard failure"
    );
    assert!(report
        .violations
        .iter()
        .any(|v| v.kind == ViolationKind::WatchdogExpiration));
}

// =======================================================================
// 4. Window ID Filtering & Deduplication (Requirement 6)
// =======================================================================

#[test]
fn test_filter_by_window_id_ignores_other_windows() {
    let raw = r#"
+10.00ms | CLOAK win=[id=4001] opacity=0 frame=(10,10,300x200)
+15.00ms | CLOAK win=[id=9999] opacity=255 frame=(0,0,100x100)
+25.00ms | AFTER_SET_SIZE win=[id=4001] opacity=0 frame=(10,10,500x400)
+30.00ms | UNCLOAK win=[id=9999] opacity=255 frame=(0,0,100x100)
+35.00ms | COMMIT_RESOLVED win=[id=4001] reason=exact
+45.00ms | AFTER_SET_POSITION_ANCHORED win=[id=4001] opacity=0 frame=(100,100,500x400)
+50.00ms | UNCLOAK win=[id=4001] opacity=255 frame=(100,100,500x400)
"#;

    let analyser = TraceAnalyser::new().with_target_window_id(4001);
    let report = analyser.analyse(raw);
    assert!(
        report.is_pass(),
        "Window 9999's bad events must be ignored: {:?}",
        report.violations
    );
    assert_eq!(report.events.len(), 5);
    assert!(report
        .events
        .iter()
        .all(|e| e.window_id == Some(4001) || e.window_id.is_none()));
}

#[test]
fn test_deduplication_between_journal_and_session_log() {
    let raw = r#"
[2026-10-10T21:30:00.100Z] +10.00ms | CLOAK win=[id=5001] opacity=0 frame=(10,10,300x200)
gnome-shell[1234]: [spawn-at-time] +10.00ms | CLOAK win=[id=5001] opacity=0 frame=(10,10,300x200)
[2026-10-10T21:30:00.200Z] +20.00ms | AFTER_SET_SIZE win=[id=5001] opacity=0 frame=(10,10,500x400)
Oct 11 02:40:12 host gnome-shell[1234]: [spawn-at-time] +20.00ms | AFTER_SET_SIZE win=[id=5001] opacity=0 frame=(10,10,500x400)
[2026-10-10T21:30:00.300Z] +30.00ms | COMMIT_RESOLVED win=[id=5001] reason=exact
gnome-shell[1234]: [spawn-at-time] +30.00ms | COMMIT_RESOLVED win=[id=5001] reason=exact
[2026-10-10T21:30:00.400Z] +40.00ms | AFTER_SET_POSITION_ANCHORED win=[id=5001] opacity=0 frame=(100,100,500x400)
gnome-shell[1234]: [spawn-at-time] +40.00ms | AFTER_SET_POSITION_ANCHORED win=[id=5001] opacity=0 frame=(100,100,500x400)
[2026-10-10T21:30:00.500Z] +50.00ms | UNCLOAK win=[id=5001] opacity=255 frame=(100,100,500x400)
gnome-shell[1234]: [spawn-at-time] +50.00ms | UNCLOAK win=[id=5001] opacity=255 frame=(100,100,500x400)
"#;

    let analyser = TraceAnalyser::new().with_target_window_id(5001);
    let report = analyser.analyse(raw);
    assert!(
        report.is_pass(),
        "Duplicate lines should deduplicate cleanly: {:?}",
        report.violations
    );
    // 5 distinct events, even though 10 raw lines were provided
    assert_eq!(report.events.len(), 5);
}

// =======================================================================
// 5. Scope Documentation & Limitation Notice (Requirement 1)
// =======================================================================

#[test]
fn test_scope_notice_in_report_summary() {
    let raw = r#"
+10.00ms | CLOAK win=[id=6001] opacity=0 frame=(10,10,300x200)
+20.00ms | AFTER_SET_SIZE win=[id=6001] opacity=0 frame=(10,10,500x400)
+30.00ms | COMMIT_RESOLVED win=[id=6001] reason=exact
+40.00ms | UNCLOAK win=[id=6001] opacity=255 frame=(100,100,500x400)
"#;

    let analyser = TraceAnalyser::new().with_target_window_id(6001);
    let report = analyser.analyse(raw);
    let summary = report.summary();

    assert!(summary.contains(SCOPE_LIMITATION_NOTICE));
    assert!(summary.contains("verifies the extension's own logged opacity"));
    assert!(summary.contains("cannot prove absence of flicker between samples"));
    assert!(summary.contains("real visual flicker evidence is a V6 / human-review concern"));
}

// =======================================================================
// 6. Real Trace Golden Fixture Test (Requirement 2 & 7)
// =======================================================================

#[test]
fn test_real_trace_golden_assertions() {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let fixture_path = Path::new(manifest_dir).join("tests/fixtures/real_trace.log");

    assert!(
        fixture_path.exists(),
        "Required golden fixture '{}' must exist",
        fixture_path.display()
    );

    let raw = std::fs::read_to_string(&fixture_path).expect("failed to read real_trace.log");
    let analyser = TraceAnalyser::new().with_target_window_id(870905755);
    let report = analyser.analyse(&raw);

    // 0 violations
    assert!(
        report.is_pass(),
        "Real-trace fixture failed with violations:\n{}",
        report.summary()
    );
    assert_eq!(report.violations.len(), 0);

    // 3 placement lifecycles
    assert_eq!(
        report.lifecycles.len(),
        3,
        "Expected exactly 3 placement lifecycles in real trace"
    );

    // 3 reveals (UNCLOAK)
    let uncloak_events: Vec<_> = report
        .events
        .iter()
        .filter(|e| e.tag == "UNCLOAK")
        .collect();
    assert_eq!(uncloak_events.len(), 3, "Expected exactly 3 UNCLOAK events");

    // Final frames: (100,140,400x300), (200,240,400x300), (300,340,400x300)
    assert_eq!(
        report.lifecycles[0].final_frame,
        Some(TraceRect {
            x: 100,
            y: 140,
            w: 400,
            h: 300
        }),
        "Lifecycle #0 final frame mismatch"
    );
    assert_eq!(
        report.lifecycles[1].final_frame,
        Some(TraceRect {
            x: 200,
            y: 240,
            w: 400,
            h: 300
        }),
        "Lifecycle #1 final frame mismatch"
    );
    assert_eq!(
        report.lifecycles[2].final_frame,
        Some(TraceRect {
            x: 300,
            y: 340,
            w: 400,
            h: 300
        }),
        "Lifecycle #2 final frame mismatch"
    );

    // SNAPSHOT_CREATED in the 2nd lifecycle
    assert!(
        report.lifecycles[1]
            .events
            .iter()
            .any(|e| e.tag == "SNAPSHOT_CREATED"),
        "Lifecycle #1 must contain SNAPSHOT_CREATED"
    );

    // SNAPSHOT_SKIPPED reason="minimized" in the 3rd lifecycle
    assert!(
        report.lifecycles[2]
            .events
            .iter()
            .any(|e| e.tag == "SNAPSHOT_SKIPPED" && e.extra.contains("reason=\"minimized\"")),
        "Lifecycle #2 must contain SNAPSHOT_SKIPPED reason=\"minimized\""
    );

    // PRE_REVEAL_UNMINIMIZE before the 3rd UNCLOAK
    let lc2_events = &report.lifecycles[2].events;
    let unminimize_idx = lc2_events
        .iter()
        .position(|e| e.tag == "PRE_REVEAL_UNMINIMIZE")
        .expect("Lifecycle #2 must contain PRE_REVEAL_UNMINIMIZE");
    let uncloak_idx = lc2_events
        .iter()
        .position(|e| e.tag == "UNCLOAK")
        .expect("Lifecycle #2 must contain UNCLOAK");
    assert!(
        unminimize_idx < uncloak_idx,
        "PRE_REVEAL_UNMINIMIZE (#{}) must precede UNCLOAK (#{}) in lifecycle #2",
        unminimize_idx,
        uncloak_idx
    );

    // No watchdog in trace
    assert!(
        !report
            .events
            .iter()
            .any(|e| e.tag == "WATCHDOG_EXPIRED" || e.raw.contains("Watchdog deadline reached")),
        "No watchdog expiration must occur in real trace"
    );

    // ANCHOR_RELEASED reason=EXTERNAL_CLOSE at the end
    let last_event = report.events.last().expect("Events must not be empty");
    assert_eq!(
        last_event.tag, "ANCHOR_RELEASED",
        "Last event must be ANCHOR_RELEASED"
    );
    assert!(
        last_event.extra.contains("reason=EXTERNAL_CLOSE"),
        "Last event reason must be EXTERNAL_CLOSE, got: {}",
        last_event.extra
    );

    // Measured cloak->uncloak durations are all under the 100ms warning budget
    assert_eq!(report.warnings.len(), 0);
    for lc in &report.lifecycles {
        let dur = lc
            .cloak_duration_ms
            .expect("Cloak duration must be measured");
        assert!(
            dur < 100.0,
            "Lifecycle #{} cloak duration ({:.2}ms) should be under 100ms",
            lc.index,
            dur
        );
    }
}

/// The extension's `LATE_RESIZE_CORRECTED` event (a window re-anchored after a resize that landed
/// between the 4C check and the reveal) sits between `UNCLOAK_4C` and `UNCLOAK`. It is not a
/// violation, not a warning and does not change the cloak duration (CLOAK at +10 ms, UNCLOAK at
/// +62 ms = 52 ms): the analyser must not read it as a reveal, a second cloak or an order break.
#[test]
fn test_late_resize_corrected_event_is_not_a_violation() {
    let raw = r#"
+10.00ms | CLOAK win=[id=1003 pid=2003 class="late" app="late" type=NORMAL proto=Wayland] opacity=0 frame=(50,90,400x300)
+22.00ms | COMMIT_RESOLVED win=[id=1003 pid=2003 class="late" app="late" type=NORMAL proto=Wayland] reason=SIZE_SETTLED finalFrame=(400x300) target=(200x150) safe=(200x150)
+25.00ms | AFTER_SET_POSITION_ANCHORED win=[id=1003 pid=2003 class="late" app="late" type=NORMAL proto=Wayland] opacity=0 frame=(1504,764,400x300)
+30.00ms | UNCLOAK_4C win=[id=1003 pid=2003 class="late" app="late" type=NORMAL proto=Wayland] positioned=(400x300) actual=(400x300) needsReanchor=false
+48.00ms | LATE_RESIZE_CORRECTED win=[id=1003 pid=2003 class="late" app="late" type=NORMAL proto=Wayland] positioned=(400x300) actual=(200x150)
+48.20ms | APPLY_ANCHOR win=[id=1003 pid=2003 class="late" app="late" type=NORMAL proto=Wayland] eff=(200x150) bounds=(0,40,1920x1040) final=(1704,914)
+48.40ms | POSITION_SET win=[id=1003 pid=2003 class="late" app="late" type=NORMAL proto=Wayland] preFrame=(1504,764,200x150) postFrame=(1704,914,200x150) bufferRect=(1704,914,200x150) actor=(1704,914) delta=(200,150)
+62.00ms | UNCLOAK win=[id=1003 pid=2003 class="late" app="late" type=NORMAL proto=Wayland] opacity=255 frame=(1704,914,200x150)
"#;

    let analyser = TraceAnalyser::new()
        .with_target_window_id(1003)
        .with_target_geometry(TraceRect {
            x: 1704,
            y: 914,
            w: 200,
            h: 150,
        });
    let report = analyser.analyse(raw);
    assert!(
        report.is_pass(),
        "a late-resize correction must not be a violation, got: {:?}",
        report.violations
    );
    assert_eq!(report.violations.len(), 0);
    assert_eq!(report.warnings.len(), 0);
    assert_eq!(report.cloak_duration_ms, Some(52.0));
}

/// A lifecycle shaped like the loaded gate-(b) failures: revealed 30 ms after the CLOAK
/// with the window at 400x300, then an `ANCHOR_CORRECTION` 43.9 ms after the `UNCLOAK`. The
/// analyser warns "placement corrected after reveal (+43.9 ms)" and it is a warning only: no
/// violation, `is_pass()` still true. The warning is in the report's list and the lifecycle's.
#[test]
fn test_correction_after_reveal_is_a_warning_with_its_delay() {
    let raw = r#"
+10.00ms | CLOAK win=[id=1007 pid=2007 class="post" app="post" type=NORMAL proto=Wayland] opacity=0 frame=(50,90,400x300)
+22.00ms | COMMIT_RESOLVED win=[id=1007 pid=2007 class="post" app="post" type=NORMAL proto=Wayland] reason=SIZE_SETTLED finalFrame=(400x300) target=(200x150) safe=(200x150)
+25.00ms | AFTER_SET_POSITION_ANCHORED win=[id=1007 pid=2007 class="post" app="post" type=NORMAL proto=Wayland] opacity=0 frame=(760,764,400x300)
+30.00ms | UNCLOAK_4C win=[id=1007 pid=2007 class="post" app="post" type=NORMAL proto=Wayland] positioned=(400x300) actual=(400x300) needsReanchor=false
+40.00ms | UNCLOAK win=[id=1007 pid=2007 class="post" app="post" type=NORMAL proto=Wayland] opacity=255 frame=(760,764,400x300)
+40.10ms | ANCHOR_ARMED win=[id=1007 pid=2007 class="post" app="post" type=NORMAL proto=Wayland]
+83.90ms | ANCHOR_CORRECTION win=[id=1007 pid=2007 class="post" app="post" type=NORMAL proto=Wayland] size=(300x200) from=(760,764) to=(810,864)
"#;
    let report = TraceAnalyser::new()
        .with_target_window_id(1007)
        .analyse(raw);
    assert!(report.is_pass(), "{:?}", report.violations);
    assert_eq!(
        report.warnings,
        vec!["placement corrected after reveal (+43.9 ms)".to_string()]
    );
    assert_eq!(report.lifecycles.len(), 1);
    assert_eq!(report.lifecycles[0].warnings, report.warnings);
}

/// Negative direction: a correction before the reveal (`LATE_RESIZE_CORRECTED`, then
/// `ANCHOR_CORRECTION` between `UNCLOAK_4C` and `UNCLOAK`) gives no warning, and a trace with no
/// correction at all gives none.
#[test]
fn test_correction_before_reveal_is_not_a_warning() {
    let before = r#"
+10.00ms | CLOAK win=[id=1008 pid=2008 class="pre" app="pre" type=NORMAL proto=Wayland] opacity=0 frame=(50,90,400x300)
+22.00ms | COMMIT_RESOLVED win=[id=1008 pid=2008 class="pre" app="pre" type=NORMAL proto=Wayland] reason=SIZE_SETTLED finalFrame=(400x300) target=(200x150) safe=(200x150)
+25.00ms | AFTER_SET_POSITION_ANCHORED win=[id=1008 pid=2008 class="pre" app="pre" type=NORMAL proto=Wayland] opacity=0 frame=(1504,764,400x300)
+30.00ms | UNCLOAK_4C win=[id=1008 pid=2008 class="pre" app="pre" type=NORMAL proto=Wayland] positioned=(400x300) actual=(400x300) needsReanchor=false
+48.00ms | LATE_RESIZE_CORRECTED win=[id=1008 pid=2008 class="pre" app="pre" type=NORMAL proto=Wayland] positioned=(400x300) actual=(200x150)
+48.50ms | ANCHOR_CORRECTION win=[id=1008 pid=2008 class="pre" app="pre" type=NORMAL proto=Wayland] size=(200x150) from=(1504,764) to=(1704,914)
+62.00ms | UNCLOAK win=[id=1008 pid=2008 class="pre" app="pre" type=NORMAL proto=Wayland] opacity=255 frame=(1704,914,200x150)
+62.10ms | ANCHOR_ARMED win=[id=1008 pid=2008 class="pre" app="pre" type=NORMAL proto=Wayland]
"#;
    let report = TraceAnalyser::new()
        .with_target_window_id(1008)
        .analyse(before);
    assert!(report.is_pass(), "{:?}", report.violations);
    assert!(report.warnings.is_empty(), "{:?}", report.warnings);

    let none = before
        .lines()
        .filter(|l| !l.contains("ANCHOR_CORRECTION"))
        .collect::<Vec<_>>()
        .join("\n");
    let report = TraceAnalyser::new()
        .with_target_window_id(1008)
        .analyse(&none);
    assert!(report.warnings.is_empty(), "{:?}", report.warnings);
}
