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
// 6. Real Trace Drop-in Fixture Test (Requirement 7)
// =======================================================================

#[test]
fn test_real_trace_drop_in_fixture() {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let fixture_path = Path::new(manifest_dir).join("tests/fixtures/real_trace.log");

    if !fixture_path.exists() {
        println!(
            "NOTE: Drop-in real trace fixture '{}' not present yet. Skipping verification until captured and supplied.",
            fixture_path.display()
        );
        return;
    }

    let raw = std::fs::read_to_string(&fixture_path).expect("failed to read real_trace.log");
    let analyser = TraceAnalyser::new();
    let report = analyser.analyse(&raw);

    println!("{}", report.summary());
    assert!(
        report.is_pass(),
        "Real-trace drop-in fixture failed verification:\n{}",
        report.summary()
    );
    assert!(
        !report.events.is_empty(),
        "Real-trace drop-in fixture contained no parsed events"
    );
}
