//! Runs `execute_single_test` against `/bin/echo` and a fake extension to check that logging is
//! enabled before each run, and that a missing trace is recorded with its reason. The "trace"
//! is the real recording in `tests/fixtures/real_trace.log`, so no timing is invented.

use super::engine::TestRunner;
use super::models::{AutoStatus, FinalStatus, RunOptions, TestResult};
use crate::ext_logging::FakeExtension;
use crate::loader::TestRegistry;
use crate::log_source::NoOpLogSource;
use crate::preflight::{PreflightReport, PreflightVerdict};
use crate::probe::EnvironmentInfo;
use crate::schema::TestDefinition;
use crate::trace::TraceState;
use crate::tracker::WindowTracker;
use std::path::PathBuf;

/// Window id of the fixture window in the recording.
const REAL_WINDOW: u64 = 870905755;

/// The recorded log, line by line.
fn real_trace_lines() -> Vec<&'static str> {
    include_str!("../../tests/fixtures/real_trace.log")
        .lines()
        .collect()
}

/// A trace-dependent spec whose action prints `claim` as its whole stdout when run through
/// `/bin/echo` (the action command is echo's first argument).
fn spec_claiming(claim: &str) -> TestDefinition {
    toml::from_str(&format!(
        r#"
id = "cloak/capture-test"
category = "animation-and-cloak"
title = "t"
description = "d"
kind = "auto"
required = true
tags = []
[requires]
session = ["any"]
backend = ["any"]
gnome = "any"
monitors_min = 1
scale = "any"
apps = []
[action]
command = '{}'
repeatable = false
args = []
[expect]
exit_code = 0
stderr_absent = []
stderr_present = []
diagnostics = []
never_visible_off_target = true
"#,
        claim
    ))
    .expect("spec parses")
}

/// A runner over `/bin/echo` whose log source and logging control are `ext`.
fn runner_with(ext: &FakeExtension) -> TestRunner {
    TestRunner::new(
        TestRegistry {
            tests: vec![],
            suites: std::collections::HashMap::new(),
        },
        RunOptions {
            bin_path: PathBuf::from("/bin/echo"),
            ..Default::default()
        },
    )
    .with_log_source(Box::new(ext.clone()))
    .with_logging_control(Box::new(ext.clone()))
}

/// Executes `test` once against a passing preflight.
fn run_once(runner: &TestRunner, test: &TestDefinition) -> TestResult {
    let preflight = PreflightReport {
        verdict: PreflightVerdict::Pass,
        binary_version_output: String::new(),
        detected_commit: None,
        repo_head: None,
        repo_dirty: false,
        is_dirty_build: false,
    };
    let env = EnvironmentInfo {
        session_type: "wayland".into(),
        desktop: "gnome".into(),
        gnome_version: Some(46),
        monitors_count: 1,
        primary_workarea: None,
        independent_workarea: None,
    };
    let mut tracker = WindowTracker {
        known_preexisting: Default::default(),
        tracked_test_windows: Default::default(),
        owned: Default::default(),
    };
    runner.execute_single_test(test, &preflight, &env, &mut tracker)
}

/// Asserts the run passed on its evidence: plain pass, trace analysed.
fn assert_full_pass(res: &TestResult) {
    assert_eq!(res.auto_status, AutoStatus::AutoPass, "{:?}", res.message);
    assert_eq!(res.final_status, FinalStatus::Pass);
}

/// Asserts the run passed only with its trace unchecked, and that the message says why.
fn assert_trace_unchecked(res: &TestResult, reason_part: &str) {
    assert_eq!(res.auto_status, AutoStatus::AutoPassTraceUnchecked);
    assert_eq!(res.final_status, FinalStatus::PassTraceUnchecked);
    let msg = res.message.as_deref().unwrap_or("");
    assert!(
        msg.contains("the trace was not analysed"),
        "message was {}",
        msg
    );
    assert!(msg.contains(reason_part), "message was {}", msg);
}

/// The window claim echo prints for the recording's window.
fn claim_real() -> String {
    format!(r#"{{"window_id":{}}}"#, REAL_WINDOW)
}

/// Logging on: the slice holds the recording and the result's trace is analysed, no note.
#[test]
fn test_normal_trace_is_captured_and_analysed() {
    let ext = FakeExtension::new(true, &real_trace_lines());
    let res = run_once(&runner_with(&ext), &spec_claiming(&claim_real()));
    assert_eq!(res.claimed_window_id(), Some(REAL_WINDOW));
    assert_eq!(res.trace_note, None);
    assert!(matches!(res.trace_state(), TraceState::Analysed(_)));
    assert_full_pass(&res);
}

/// Logging was off when the run began (an extension reload): it is switched on before the
/// run, so the slice is not empty. Without that call the same fake yields an empty slice.
#[test]
fn test_logging_off_at_run_time_is_re_enabled_before_the_run() {
    let ext = FakeExtension::new(false, &real_trace_lines());
    assert_eq!(ext.enable_calls(), 0);
    let res = run_once(&runner_with(&ext), &spec_claiming(&claim_real()));
    assert_eq!(ext.enable_calls(), 1);
    assert!(ext.is_enabled());
    assert!(matches!(res.trace_state(), TraceState::Analysed(_)));
    assert_full_pass(&res);
}

/// A TUI session keeps running tests while the extension is reloaded between them: every run
/// enables logging again, so the run after the reload still gets its trace.
#[test]
fn test_logging_that_resets_mid_session_is_re_enabled_for_the_next_run() {
    let ext = FakeExtension::new(true, &real_trace_lines());
    let runner = runner_with(&ext);
    let test = spec_claiming(&claim_real());

    let first = run_once(&runner, &test);
    assert!(matches!(first.trace_state(), TraceState::Analysed(_)));
    assert_full_pass(&first);

    ext.reload();
    assert!(!ext.is_enabled());

    let second = run_once(&runner, &test);
    assert_eq!(ext.enable_calls(), 2, "enabled once per run");
    assert!(matches!(second.trace_state(), TraceState::Analysed(_)));
    assert_full_pass(&second);
}

/// The extension reloads and logging cannot be turned back on: the run after the reload has
/// no trace and is "trace unchecked", not a pass.
#[test]
fn test_reset_mid_session_with_enable_failure_is_trace_unchecked() {
    let ext = FakeExtension::new(true, &real_trace_lines());
    let runner = runner_with(&ext);
    let test = spec_claiming(&claim_real());
    assert_full_pass(&run_once(&runner, &test));

    ext.reload();
    ext.set_enable_error(Some("extension is not running"));
    let res = run_once(&runner, &test);
    assert_trace_unchecked(&res, "extension is not running");
}

/// Logging cannot be turned on: the slice is empty, and the result carries the failure as the
/// reason for the missing trace.
#[test]
fn test_logging_that_cannot_be_enabled_records_the_reason() {
    let ext = FakeExtension::new(false, &real_trace_lines());
    ext.set_enable_error(Some("gdbus: no such object"));
    let res = run_once(&runner_with(&ext), &spec_claiming(&claim_real()));
    let note = res.trace_note.as_deref().expect("a note");
    assert!(note.contains("could not be enabled"), "note was {}", note);
    let state = res.trace_state();
    let why = state.reason().expect("no trace");
    assert!(why.contains("log slice is empty"), "reason was {}", why);
    assert!(why.contains("gdbus: no such object"), "reason was {}", why);
    assert_trace_unchecked(&res, "gdbus: no such object");
}

/// No-op log source (not GNOME): the slice is empty and the note says there is no log source.
#[test]
fn test_noop_log_source_records_no_log_source() {
    let ext = FakeExtension::new(true, &real_trace_lines());
    let runner = runner_with(&ext).with_log_source(Box::new(NoOpLogSource));
    let res = run_once(&runner, &spec_claiming(&claim_real()));
    let note = res.trace_note.as_deref().expect("a note");
    assert!(note.starts_with("no log source:"), "note was {}", note);
    assert_eq!(res.log_slice.as_deref(), Some(""));
    assert!(res
        .trace_state()
        .reason()
        .unwrap()
        .contains("no log source"));
    assert_trace_unchecked(&res, "no log source");
}

/// The slice holds the recording, but the fixture claims a different window: nothing of that
/// window was captured.
#[test]
fn test_slice_with_only_another_windows_events_is_not_the_claimed_trace() {
    let ext = FakeExtension::new(true, &real_trace_lines());
    let res = run_once(&runner_with(&ext), &spec_claiming(r#"{"window_id":999}"#));
    assert_eq!(res.claimed_window_id(), Some(999));
    let state = res.trace_state();
    let why = state.reason().expect("window 999 has no events");
    assert!(
        why.contains("no event for window 999"),
        "reason was {}",
        why
    );
    assert_trace_unchecked(&res, "no event for window 999");
}

/// Logging is on but the slice is empty (nothing was logged in the run's time window): the
/// pass is "trace unchecked", never clean.
#[test]
fn test_empty_slice_with_logging_on_is_trace_unchecked() {
    let ext = FakeExtension::new(true, &[]);
    let res = run_once(&runner_with(&ext), &spec_claiming(&claim_real()));
    assert_eq!(res.trace_note, None);
    assert_trace_unchecked(&res, "log slice is empty");
}

/// A placement test: no trace dependency (`never_visible_off_target` false, id outside `cloak/`).
fn placement_spec() -> TestDefinition {
    let mut spec = spec_claiming(&claim_real());
    spec.id = "placement/capture-test".into();
    spec.expect.never_visible_off_target = false;
    spec
}

/// Logging is enabled before every executed test, not only trace-dependent ones: a placement
/// test that starts with logging off gets it switched on and passes plainly.
#[test]
fn test_placement_test_with_logging_off_is_re_enabled() {
    let ext = FakeExtension::new(false, &real_trace_lines());
    let res = run_once(&runner_with(&ext), &placement_spec());
    assert_eq!(ext.enable_calls(), 1);
    assert!(ext.is_enabled());
    assert_eq!(res.trace_note, None);
    assert_full_pass(&res);
}

/// A placement test whose logging cannot be enabled keeps its normal verdict (it does not need
/// a trace) and carries the reason in `trace_note`.
#[test]
fn test_placement_test_with_enable_failure_keeps_its_verdict_and_shows_reason() {
    let ext = FakeExtension::new(false, &real_trace_lines());
    ext.set_enable_error(Some("gdbus: no such object"));
    let res = run_once(&runner_with(&ext), &placement_spec());
    assert_eq!(ext.enable_calls(), 1);
    assert_full_pass(&res);
    let note = res.trace_note.as_deref().expect("a note");
    assert!(note.contains("gdbus: no such object"), "note was {}", note);
    assert_eq!(res.message.as_deref(), Some("Assertions passed"));
}

/// A control that takes 300 ms to enable logging, to show that time is not part of a run.
struct SlowLogging(FakeExtension);

impl crate::ext_logging::LoggingControl for SlowLogging {
    /// Sleeps 300 ms, then enables the wrapped fake.
    fn ensure_enabled(
        &self,
        log: &dyn crate::log_source::LogSource,
    ) -> crate::ext_logging::LoggingOutcome {
        std::thread::sleep(std::time::Duration::from_millis(300));
        self.0.ensure_enabled(log)
    }
}

/// Enabling logging (here 300 ms) is excluded from `duration_ms`: the action is `/bin/echo`, so
/// the recorded duration is far below the enable time. Fails if `start` is not moved forward by
/// the time `ensure_enabled` took.
#[test]
fn test_enable_time_is_excluded_from_duration_ms() {
    let ext = FakeExtension::new(false, &real_trace_lines());
    let runner = runner_with(&ext).with_logging_control(Box::new(SlowLogging(ext.clone())));
    let began = std::time::Instant::now();
    let res = run_once(&runner, &placement_spec());
    assert!(began.elapsed() >= std::time::Duration::from_millis(300));
    assert!(res.duration_ms < 250, "duration_ms was {}", res.duration_ms);
    assert_full_pass(&res);
}

/// Wraps a fake extension and counts `trace_captured` calls.
struct CountingCaptured(
    FakeExtension,
    std::sync::Arc<std::sync::atomic::AtomicUsize>,
);

impl crate::ext_logging::LoggingControl for CountingCaptured {
    /// Delegates to the wrapped fake.
    fn ensure_enabled(
        &self,
        log: &dyn crate::log_source::LogSource,
    ) -> crate::ext_logging::LoggingOutcome {
        self.0.ensure_enabled(log)
    }

    /// Counts the call.
    fn trace_captured(&self) {
        self.1.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }
}

/// A run whose slice holds the window's trace calls `trace_captured` once; a run whose log is
/// off (empty slice) does not. Fails if the memory were cleared without a captured trace.
#[test]
fn test_captured_trace_notifies_logging_control_and_empty_slice_does_not() {
    use std::sync::{atomic::AtomicUsize, atomic::Ordering, Arc};
    let hits = Arc::new(AtomicUsize::new(0));
    let ext = FakeExtension::new(false, &real_trace_lines());
    let runner = runner_with(&ext)
        .with_logging_control(Box::new(CountingCaptured(ext.clone(), hits.clone())));
    run_once(&runner, &placement_spec());
    assert_eq!(hits.load(Ordering::SeqCst), 1);

    let off = FakeExtension::new(false, &real_trace_lines());
    off.set_enable_error(Some("gdbus: no such object"));
    let hits2 = Arc::new(AtomicUsize::new(0));
    let runner = runner_with(&off)
        .with_logging_control(Box::new(CountingCaptured(off.clone(), hits2.clone())));
    run_once(&runner, &placement_spec());
    assert_eq!(hits2.load(Ordering::SeqCst), 0);
}

/// A scratch directory holding a fake `spawn-at` (a shell script) and a dummy `fixture` file next
/// to it, removed on drop. The script answers `query ...` with `[]` (no windows) and `spawn ...`
/// with `stderr_text` on stderr and exit code `exit_code`.
struct FakeBin {
    dir: PathBuf,
}

impl FakeBin {
    /// Creates the directory and the script for one test (`name` keeps parallel tests apart).
    fn new(name: &str, exit_code: i32, stderr_text: &str) -> Self {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!(
            "spawn-at-verify-fakebin-{}-{}",
            std::process::id(),
            name
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let script = format!(
            "#!/bin/sh\ncase \"$1\" in\n  query) echo '[]'; exit 0;;\n  spawn) echo '{}' >&2; exit {};;\nesac\nexit 0\n",
            stderr_text, exit_code
        );
        let bin = dir.join("spawn-at");
        std::fs::write(&bin, script).unwrap();
        std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).unwrap();
        std::fs::write(dir.join("fixture"), "").unwrap();
        Self { dir }
    }

    /// Path of the fake `spawn-at`.
    fn bin(&self) -> PathBuf {
        self.dir.join("spawn-at")
    }
}

impl Drop for FakeBin {
    /// Removes the scratch directory.
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// A `spawn` spec with the fixture subject that expects `exit_code` and (when not empty) the
/// stderr text `present`.
fn spec_expecting(exit_code: i32, present: &str) -> TestDefinition {
    toml::from_str(&format!(
        r#"
id = "placement/expected-failure"
category = "placement"
title = "t"
description = "d"
kind = "auto"
required = true
[subject]
app = "fixture"
profile = "plain"
args = ["--title", "T"]
[action]
command = "spawn"
repeatable = false
args = ["--anchor", "center", "--size", "0", "150"]
[expect]
exit_code = {}
stderr_absent = []
stderr_present = [{}]
diagnostics = []
never_visible_off_target = false
"#,
        exit_code,
        if present.is_empty() {
            String::new()
        } else {
            format!("'{}'", present)
        }
    ))
    .unwrap()
}

/// Runs `spec` against the fake binary and returns the result.
fn run_fake(fake: &FakeBin, spec: &TestDefinition) -> TestResult {
    let ext = FakeExtension::new(false, &[]);
    let mut runner = runner_with(&ext);
    runner.options.bin_path = fake.bin();
    run_once(&runner, spec)
}

/// (c) The expected non-zero exit with the expected stderr passes, without attributing a window.
#[test]
fn test_expected_failure_with_expected_exit_and_stderr_passes() {
    let fake = FakeBin::new("c", 2, "[spawn-at] ERROR: Invalid size");
    let res = run_fake(&fake, &spec_expecting(2, "ERROR: Invalid size"));
    assert_eq!(res.auto_status, AutoStatus::AutoPass, "{:?}", res.message);
}

/// (a) A spec expecting exit 2 but getting exit 0 (the CLI accepted the size) FAILS with an exit
/// code mismatch.
#[test]
fn test_expected_nonzero_exit_but_got_zero_fails() {
    let fake = FakeBin::new("a", 0, "");
    let res = run_fake(&fake, &spec_expecting(2, ""));
    assert_eq!(res.auto_status, AutoStatus::AutoFail);
    let msg = res.message.unwrap_or_default();
    assert!(
        msg.contains("Exit code mismatch: expected 2, got 0"),
        "{}",
        msg
    );
}

/// (b) An unexpected non-zero exit FAILS: expected 0 got 3, and expected 2 got 1.
#[test]
fn test_unexpected_nonzero_exit_fails() {
    let fake = FakeBin::new("b1", 3, "boom");
    let res = run_fake(&fake, &spec_expecting(0, ""));
    assert_eq!(res.auto_status, AutoStatus::AutoFail);
    let msg = res.message.unwrap_or_default();
    assert!(
        msg.contains("Exit code mismatch: expected 0, got 3"),
        "{}",
        msg
    );

    let fake = FakeBin::new("b2", 1, "boom");
    let res = run_fake(&fake, &spec_expecting(2, ""));
    assert_eq!(res.auto_status, AutoStatus::AutoFail);
    let msg = res.message.unwrap_or_default();
    assert!(
        msg.contains("Exit code mismatch: expected 2, got 1"),
        "{}",
        msg
    );
}

/// The expected exit with the WRONG stderr FAILS on the missing text, not on attribution.
#[test]
fn test_expected_exit_with_missing_stderr_fails() {
    let fake = FakeBin::new("c2", 2, "something else");
    let res = run_fake(&fake, &spec_expecting(2, "ERROR: Invalid size"));
    assert_eq!(res.auto_status, AutoStatus::AutoFail);
    let msg = res.message.unwrap_or_default();
    assert!(msg.contains("Required token or pattern"), "{}", msg);
}
