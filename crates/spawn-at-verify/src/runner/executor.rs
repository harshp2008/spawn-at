//! Single-test execution, process invocation, log capture, and attribution.

use super::engine::TestRunner;
use super::models::{resolve_final_status, AutoStatus, HumanVerdict, TestResult};
use super::validation::{
    build_action_command_args, expects_no_window, extract_claimed_window_id, needs_trace,
    pass_assessment, settle_final_rect, validate_expectations, validate_geometry_final, Settle,
};
use crate::oracle::SETTLE_MS;
use crate::preflight::{PreflightReport, PreflightVerdict};
use crate::probe::EnvironmentInfo;
use crate::schema::TestDefinition;
use crate::trace::{assess_trace, TraceState};
use crate::tracker::{query_window_ids, query_window_records, query_window_rect, WindowTracker};
use std::collections::HashSet;
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant, SystemTime};

impl TestRunner {
    /// Executes an individual verification test case, managing timeouts, logs, and attribution.
    pub fn execute_single_test(
        &self,
        test: &TestDefinition,
        preflight: &PreflightReport,
        env_info: &EnvironmentInfo,
        tracker: &mut WindowTracker,
    ) -> TestResult {
        let mut start = Instant::now();
        let start_secs = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        // Check preflight block
        if let PreflightVerdict::Fail(reason) = &preflight.verdict {
            if test.action.command != "preflight-binary-check" {
                let (final_status, disagreement) =
                    resolve_final_status(test.kind, AutoStatus::Blocked, &HumanVerdict::default());
                return TestResult {
                    id: test.id.clone(),
                    title: test.title.clone(),
                    kind: test.kind,
                    required: test.required,
                    auto_status: AutoStatus::Blocked,
                    human_verdict: HumanVerdict::default(),
                    final_status,
                    disagreement,
                    duration_ms: start.elapsed().as_millis() as u64,
                    exit_code: None,
                    stdout: String::new(),
                    stderr: String::new(),
                    message: Some(format!("Blocked by preflight failure: {}", reason)),
                    log_slice: None,
                    trace_note: None,
                    final_rect: None,
                };
            }
        }

        // Check environment requirements
        if let Err(skip_reason) = env_info.check_requirements(&test.requires) {
            let (final_status, disagreement) =
                resolve_final_status(test.kind, AutoStatus::Skipped, &HumanVerdict::default());
            return TestResult {
                id: test.id.clone(),
                title: test.title.clone(),
                kind: test.kind,
                required: test.required,
                auto_status: AutoStatus::Skipped,
                human_verdict: HumanVerdict::default(),
                final_status,
                disagreement,
                duration_ms: start.elapsed().as_millis() as u64,
                exit_code: None,
                stdout: String::new(),
                stderr: String::new(),
                message: Some(skip_reason),
                log_slice: None,
                trace_note: None,
                final_rect: None,
            };
        }

        if self.options.dry_run {
            let (final_status, disagreement) =
                resolve_final_status(test.kind, AutoStatus::Pending, &HumanVerdict::default());
            return TestResult {
                id: test.id.clone(),
                title: test.title.clone(),
                kind: test.kind,
                required: test.required,
                auto_status: AutoStatus::Pending,
                human_verdict: HumanVerdict::default(),
                final_status,
                disagreement,
                duration_ms: start.elapsed().as_millis() as u64,
                exit_code: None,
                stdout: String::new(),
                stderr: String::new(),
                message: Some("Dry-run only; skipped execution".into()),
                log_slice: None,
                trace_note: None,
                final_rect: None,
            };
        }

        // Handle preflight-binary-check command directly
        if test.action.command == "preflight-binary-check" {
            let duration_ms = start.elapsed().as_millis() as u64;
            let (auto_status, msg) = match &preflight.verdict {
                PreflightVerdict::Pass => (AutoStatus::AutoPass, "Preflight check passed".into()),
                PreflightVerdict::Warn(w) => (
                    AutoStatus::AutoPass,
                    format!("Preflight warning (dirty build): {}", w),
                ),
                PreflightVerdict::Skip(s) => (AutoStatus::Skipped, s.clone()),
                PreflightVerdict::Fail(f) => (AutoStatus::AutoFail, f.clone()),
            };

            let (final_status, disagreement) =
                resolve_final_status(test.kind, auto_status, &HumanVerdict::default());

            return TestResult {
                id: test.id.clone(),
                title: test.title.clone(),
                kind: test.kind,
                required: test.required,
                auto_status,
                human_verdict: HumanVerdict::default(),
                final_status,
                disagreement,
                duration_ms,
                exit_code: Some(0),
                stdout: preflight.binary_version_output.clone(),
                stderr: String::new(),
                message: Some(msg),
                log_slice: None,
                trace_note: None,
                final_rect: None,
            };
        }

        // Build command to execute
        let fixture_bin = self
            .options
            .bin_path
            .parent()
            .unwrap_or(Path::new("."))
            .join("fixture");

        // If subject is fixture, verify fixture binary exists before running
        if test.action.command == "spawn" {
            if let Some(sub) = &test.subject {
                if sub.app == "fixture" && !fixture_bin.exists() {
                    let (final_status, disagreement) = resolve_final_status(
                        test.kind,
                        AutoStatus::Error,
                        &HumanVerdict::default(),
                    );
                    return TestResult {
                        id: test.id.clone(),
                        title: test.title.clone(),
                        kind: test.kind,
                        required: test.required,
                        auto_status: AutoStatus::Error,
                        human_verdict: HumanVerdict::default(),
                        final_status,
                        disagreement,
                        duration_ms: start.elapsed().as_millis() as u64,
                        exit_code: None,
                        stdout: String::new(),
                        stderr: String::new(),
                        message: Some(format!(
                            "Fixture binary not found at {}; run `cargo build -p spawn-at-verify --bin fixture --features fixture`",
                            fixture_bin.display()
                        )),
                        log_slice: None,
                        trace_note: None, final_rect: None,
                    };
                }
            }
        }

        // The extension's timing log has no state getter and starts off again after a reload, so
        // it is enabled (idempotent) before every executed test, not only the ones that need a
        // trace. Time spent doing so is not part of the run's duration. A log that is not known
        // to be on is recorded next to the result; only tests that need a trace (`needs_trace`)
        // lose their plain pass over it, the others keep their verdict.
        let began = Instant::now();
        let trace_note = self.logging.ensure_enabled(&*self.log_source).note();
        start += began.elapsed();

        let cmd_args = build_action_command_args(test, &fixture_bin);
        let mut cmd = Command::new(&self.options.bin_path);
        cmd.args(cmd_args);

        // Fresh window snapshot immediately before each test's action
        let pre_action_snapshot: HashSet<u64> = query_window_ids(&self.options.bin_path)
            .unwrap_or_default()
            .into_iter()
            .collect();

        let output_res = crate::process::execute_with_timeout(cmd, self.options.step_timeout);
        let duration_ms = start.elapsed().as_millis() as u64;
        let end_secs = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(start_secs);

        // Capture log slice for this run's time window
        let log_slice = self.log_source.capture_slice(start_secs, end_secs).ok();

        let output = match output_res {
            Ok(o) => o,
            Err(e) => {
                let (final_status, disagreement) =
                    resolve_final_status(test.kind, AutoStatus::Error, &HumanVerdict::default());
                return TestResult {
                    id: test.id.clone(),
                    title: test.title.clone(),
                    kind: test.kind,
                    required: test.required,
                    auto_status: AutoStatus::Error,
                    human_verdict: HumanVerdict::default(),
                    final_status,
                    disagreement,
                    duration_ms,
                    exit_code: None,
                    stdout: String::new(),
                    stderr: String::new(),
                    message: Some(format!("Harness error executing command: {}", e)),
                    log_slice,
                    trace_note: trace_note.clone(),
                    final_rect: None,
                };
            }
        };

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let exit_code = output.exit_code.unwrap_or(-1);

        // Attribute test-spawned windows strictly belonging to this test.
        // It must come only from that test's own spawn (the window id from the SpawnClaimed
        // signal or the claim result, or the fixture's own app id), never "any new window since the snapshot".
        let mut attributed_id: Option<u64> = None;
        if test.action.command == "spawn" && !expects_no_window(test, exit_code) {
            let claimed_id = extract_claimed_window_id(&stdout, &stderr);
            let subject_app = test.subject.as_ref().map(|s| s.app.as_str());
            if let Ok(records) = query_window_records(&self.options.bin_path) {
                if let Ok(outcome) = tracker.attribute_action_windows(
                    &pre_action_snapshot,
                    &records,
                    None,
                    subject_app,
                    claimed_id,
                ) {
                    attributed_id = outcome.registered_id;
                    if let Some(err_msg) = outcome.error_message {
                        // A wrong exit code is the first thing to report: with it the missing
                        // window is a consequence, not the cause.
                        let err_msg = if exit_code != test.expect.exit_code {
                            format!(
                                "Exit code mismatch: expected {}, got {} ({})",
                                test.expect.exit_code, exit_code, err_msg
                            )
                        } else {
                            err_msg
                        };
                        let (final_status, disagreement) = resolve_final_status(
                            test.kind,
                            AutoStatus::AutoFail,
                            &HumanVerdict::default(),
                        );
                        return TestResult {
                            id: test.id.clone(),
                            title: test.title.clone(),
                            kind: test.kind,
                            required: test.required,
                            auto_status: AutoStatus::AutoFail,
                            human_verdict: HumanVerdict::default(),
                            final_status,
                            disagreement,
                            duration_ms,
                            exit_code: Some(exit_code),
                            stdout,
                            stderr,
                            message: Some(err_msg),
                            log_slice,
                            trace_note: trace_note.clone(),
                            final_rect: None,
                        };
                    }
                }
            }
        }

        let transform_target_id = if test.action.command == "transform" {
            test.action.args.windows(2).find_map(|w| {
                if w[0] == "--id" {
                    w[1].parse::<u64>().ok()
                } else {
                    None
                }
            })
        } else {
            None
        };
        let target_id = attributed_id.or(transform_target_id);

        // `spawn-at` can exit with the rectangle the window had before the app's late resize;
        // when that printed rectangle fails the check, keep the window alive for SETTLE_MS and
        // read where it ended up. A matching print waits for nothing. `duration_ms` above is
        // not extended by the wait. The log slice is taken again so it covers the wait and any
        // correction the extension made during it.
        let settle_id = extract_claimed_window_id(&stdout, &stderr).or(target_id);
        let settle = settle_final_rect(test, &stdout, log_slice.as_deref(), env_info, || {
            std::thread::sleep(Duration::from_millis(SETTLE_MS));
            settle_id.and_then(|id| query_window_rect(&self.options.bin_path, id))
        });
        let final_rect = match settle {
            Settle::Read(r) => Some(r),
            Settle::NotNeeded | Settle::Unreadable => None,
        };
        let log_slice = if settle == Settle::NotNeeded {
            log_slice
        } else {
            let settled_secs = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(end_secs);
            self.log_source
                .capture_slice(start_secs, settled_secs)
                .ok()
                .or(log_slice)
        };

        // A trace for the run's window proves the timing log is on, so a remembered echo timeout
        // no longer applies.
        if assess_trace(
            log_slice.as_deref(),
            extract_claimed_window_id(&stdout, &stderr).or(target_id),
            None,
        )
        .report()
        .is_some()
        {
            self.logging.trace_captured();
        }

        // Validate expectations
        let validation = validate_expectations(test, exit_code, &stderr, env_info).and_then(|()| {
            validate_geometry_final(test, &stdout, final_rect, log_slice.as_deref(), env_info)
        });
        // Geometry caveats worth recording next to a pass.
        let mut pass_message: String;
        let auto_status = match validation {
            Ok(check) => {
                let (status, message) = pass_assessment(check.as_ref());
                pass_message = message;
                status
            }
            Err(err) => {
                // Without a re-read the verdict is on the printed rectangle; say so.
                let err = if settle == Settle::Unreadable {
                    format!(
                        "{} (the window could not be re-read after {} ms; judged on the printed rectangle)",
                        err, SETTLE_MS
                    )
                } else {
                    err
                };
                let (final_status, disagreement) =
                    resolve_final_status(test.kind, AutoStatus::AutoFail, &HumanVerdict::default());
                return TestResult {
                    id: test.id.clone(),
                    title: test.title.clone(),
                    kind: test.kind,
                    required: test.required,
                    auto_status: AutoStatus::AutoFail,
                    human_verdict: HumanVerdict::default(),
                    final_status,
                    disagreement,
                    duration_ms,
                    exit_code: Some(exit_code),
                    stdout,
                    stderr,
                    message: Some(err),
                    log_slice,
                    trace_note: trace_note.clone(),
                    final_rect,
                };
            }
        };

        // A test that depends on the trace needs one for its window: violations fail it, and a
        // missing trace downgrades a plain pass to "trace unchecked" instead of passing it as clean.
        let window = extract_claimed_window_id(&stdout, &stderr).or(target_id);
        let auto_status = if matches!(
            auto_status,
            AutoStatus::AutoPass | AutoStatus::AutoPassUnchecked
        ) && needs_trace(test)
        {
            match assess_trace(log_slice.as_deref(), window, trace_note.as_deref()) {
                TraceState::Analysed(report) => {
                    if test.expect.never_visible_off_target && !report.is_pass() {
                        let err_msg = format!("Trace verification failed: {:?}", report.violations);
                        let (final_status, disagreement) = resolve_final_status(
                            test.kind,
                            AutoStatus::AutoFail,
                            &HumanVerdict::default(),
                        );
                        return TestResult {
                            id: test.id.clone(),
                            title: test.title.clone(),
                            kind: test.kind,
                            required: test.required,
                            auto_status: AutoStatus::AutoFail,
                            human_verdict: HumanVerdict::default(),
                            final_status,
                            disagreement,
                            duration_ms,
                            exit_code: Some(exit_code),
                            stdout,
                            stderr,
                            message: Some(err_msg),
                            log_slice,
                            trace_note: trace_note.clone(),
                            final_rect,
                        };
                    }
                    auto_status
                }
                TraceState::NotCaptured(why) => {
                    pass_message =
                        format!("{}, but the trace was not analysed: {}", pass_message, why);
                    if auto_status == AutoStatus::AutoPass {
                        AutoStatus::AutoPassTraceUnchecked
                    } else {
                        auto_status
                    }
                }
            }
        } else {
            auto_status
        };

        let (final_status, disagreement) =
            resolve_final_status(test.kind, auto_status, &HumanVerdict::default());

        TestResult {
            id: test.id.clone(),
            title: test.title.clone(),
            kind: test.kind,
            required: test.required,
            auto_status,
            human_verdict: HumanVerdict::default(),
            final_status,
            disagreement,
            duration_ms,
            exit_code: Some(exit_code),
            stdout,
            stderr,
            message: Some(pass_message),
            log_slice,
            trace_note,
            final_rect,
        }
    }
}
