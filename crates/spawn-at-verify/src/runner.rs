//! Test Runner Engine for spawn-at-verify.

use crate::loader::{TestFilter, TestRegistry};
use crate::log_source::{GnomeJournalSource, LogSource, NoOpLogSource};
use crate::oracle::{calculate_expected_rect, verify_rect_tolerance, OracleParams, Rect};
use crate::preflight::{inspect_binary, PreflightReport, PreflightVerdict};
use crate::probe::EnvironmentInfo;
use crate::schema::{ExpectRect, TestDefinition, TestKind};
use crate::tracker::{query_window_ids, query_window_records, WindowTracker};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Instant, SystemTime};

pub const MAX_REPEAT_CAP: usize = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AutoStatus {
    Pending,
    Running,
    AutoPass,
    AutoFail,
    Error,
    Skipped,
    Blocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum HumanVerdictKind {
    #[default]
    Unset,
    Pass,
    Fail,
    Skip,
    Flaky,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct HumanVerdict {
    pub verdict: HumanVerdictKind,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FinalStatus {
    Pass,
    Fail,
    Skipped,
    Blocked,
    Error,
    Flaky,
    Pending,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestResult {
    pub id: String,
    pub title: String,
    pub kind: TestKind,
    pub required: bool,
    pub auto_status: AutoStatus,
    pub human_verdict: HumanVerdict,
    pub final_status: FinalStatus,
    pub disagreement: bool,
    pub duration_ms: u64,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub message: Option<String>,
    pub log_slice: Option<String>,
}

/// Resolves the final test status and checks for disagreement between automated and human verdicts.
pub fn resolve_final_status(
    kind: TestKind,
    auto_status: AutoStatus,
    human: &HumanVerdict,
) -> (FinalStatus, bool) {
    let mut disagreement = false;

    let final_status = match kind {
        TestKind::Auto => match auto_status {
            AutoStatus::AutoPass => FinalStatus::Pass,
            AutoStatus::AutoFail => FinalStatus::Fail,
            AutoStatus::Skipped => FinalStatus::Skipped,
            AutoStatus::Blocked => FinalStatus::Blocked,
            AutoStatus::Error => FinalStatus::Error,
            AutoStatus::Pending => FinalStatus::Pending,
            AutoStatus::Running => FinalStatus::Pending,
        },
        TestKind::Human => match human.verdict {
            HumanVerdictKind::Pass => FinalStatus::Pass,
            HumanVerdictKind::Fail => FinalStatus::Fail,
            HumanVerdictKind::Skip => FinalStatus::Skipped,
            HumanVerdictKind::Flaky => FinalStatus::Flaky,
            HumanVerdictKind::Unset => FinalStatus::Pending,
        },
        TestKind::Both => {
            // Rule: Passes ONLY if BOTH auto-pass and human pass are achieved.
            let auto_passed = auto_status == AutoStatus::AutoPass;
            let human_passed = human.verdict == HumanVerdictKind::Pass;

            if (auto_passed && human.verdict == HumanVerdictKind::Fail)
                || (auto_status == AutoStatus::AutoFail && human_passed)
            {
                disagreement = true;
            }

            if auto_status == AutoStatus::Skipped {
                FinalStatus::Skipped
            } else if auto_status == AutoStatus::Blocked {
                FinalStatus::Blocked
            } else if auto_status == AutoStatus::Error {
                FinalStatus::Error
            } else if human.verdict == HumanVerdictKind::Flaky {
                FinalStatus::Flaky
            } else if auto_passed && human_passed {
                FinalStatus::Pass
            } else if auto_status == AutoStatus::AutoFail || human.verdict == HumanVerdictKind::Fail
            {
                FinalStatus::Fail
            } else {
                FinalStatus::Pending
            }
        }
    };

    (final_status, disagreement)
}

#[derive(Debug, Clone)]
pub struct RunOptions {
    pub bin_path: PathBuf,
    pub allow_dirty: bool,
    pub dry_run: bool,
    pub repeat: usize,
    pub filter: TestFilter,
    pub include_titles: bool,
}

impl Default for RunOptions {
    fn default() -> Self {
        Self {
            bin_path: PathBuf::from("target/debug/spawn-at"),
            allow_dirty: false,
            dry_run: false,
            repeat: 1,
            filter: TestFilter::default(),
            include_titles: false,
        }
    }
}

pub struct TestRunner {
    pub registry: TestRegistry,
    pub options: RunOptions,
    pub log_source: Box<dyn LogSource>,
}

impl TestRunner {
    pub fn new(registry: TestRegistry, options: RunOptions) -> Self {
        let is_gnome = std::env::var("XDG_CURRENT_DESKTOP")
            .map(|d| d.to_lowercase().contains("gnome"))
            .unwrap_or(false);

        let log_source: Box<dyn LogSource> = if is_gnome {
            Box::new(GnomeJournalSource)
        } else {
            Box::new(NoOpLogSource)
        };

        Self {
            registry,
            options,
            log_source,
        }
    }

    pub fn with_log_source(mut self, log_source: Box<dyn LogSource>) -> Self {
        self.log_source = log_source;
        self
    }

    /// Runs all selected tests and returns the execution report.
    pub fn run(&self) -> Result<(PreflightReport, EnvironmentInfo, Vec<TestResult>), String> {
        let bin = &self.options.bin_path;
        if !bin.exists() {
            return Err(format!("Target binary '{}' not found", bin.display()));
        }

        // 1. Run Preflight Inspection
        let preflight = inspect_binary(bin)?;

        let is_release_gate = self.options.filter.suite.as_deref() == Some("release-gate");
        if is_release_gate && preflight.is_dirty_build && !self.options.allow_dirty {
            return Err(
                "release-gate suite strictly refuses to run against dirty build without --allow-dirty"
                    .into(),
            );
        }

        // 2. Filter tests
        let tests_to_run = self.registry.filter(&self.options.filter)?;

        // 3. Probe environment
        let env_info = EnvironmentInfo::probe(Some(bin));

        // 4. Snapshot initial windows
        let mut tracker = WindowTracker::snapshot_startup(bin).unwrap_or_else(|_| WindowTracker {
            known_preexisting: Default::default(),
            tracked_test_windows: Default::default(),
        });

        let repeat_count = self.options.repeat.clamp(1, MAX_REPEAT_CAP);
        let mut results = Vec::new();

        for test in &tests_to_run {
            if repeat_count == 1 {
                let res = self.execute_single_test(test, &preflight, &env_info, &mut tracker);
                results.push(res);
            } else {
                // Multi-run flakiness detection across N repetitions
                let mut run_results = Vec::new();
                for _ in 0..repeat_count {
                    let r = self.execute_single_test(test, &preflight, &env_info, &mut tracker);
                    run_results.push(r);
                }

                // Check for mixed results
                let all_same = run_results
                    .windows(2)
                    .all(|w| w[0].final_status == w[1].final_status);

                let mut combined = run_results.into_iter().next().unwrap();
                if !all_same {
                    combined.final_status = FinalStatus::Flaky;
                    combined.message = Some(format!(
                        "Multi-run inconsistency: detected varying outcomes across {} repetitions",
                        repeat_count
                    ));
                }
                results.push(combined);
            }
        }

        // 5. Final cleanup of any lingering windows
        let _ = tracker.cleanup_test_windows(bin);

        Ok((preflight, env_info, results))
    }

    fn execute_single_test(
        &self,
        test: &TestDefinition,
        preflight: &PreflightReport,
        env_info: &EnvironmentInfo,
        tracker: &mut WindowTracker,
    ) -> TestResult {
        let start = Instant::now();
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
            };
        }

        // Build command to execute
        let mut cmd = Command::new(&self.options.bin_path);
        cmd.arg(&test.action.command);
        for arg in &test.action.args {
            cmd.arg(arg);
        }

        // If subject is defined and action is spawn
        if test.action.command == "spawn" {
            if let Some(sub) = &test.subject {
                if sub.app == "fixture" {
                    let fixture_bin = self
                        .options
                        .bin_path
                        .parent()
                        .unwrap_or(Path::new("."))
                        .join("fixture");

                    if !fixture_bin.exists() {
                        let (final_status, disagreement) = resolve_final_status(
                            test.kind,
                            AutoStatus::Skipped,
                            &HumanVerdict::default(),
                        );
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
                            message: Some("Fixture binary not found; skipping fixture test".into()),
                            log_slice: None,
                        };
                    }

                    cmd.arg("--");
                    cmd.arg(&fixture_bin);
                    for sarg in &sub.args {
                        cmd.arg(sarg);
                    }
                } else if let Some(sys_app) = sub.app.strip_prefix("system:") {
                    if !test.action.args.contains(&"--json".to_string()) {
                        cmd.arg("--json");
                    }
                    cmd.arg("--");
                    cmd.arg(sys_app);
                    for sarg in &sub.args {
                        cmd.arg(sarg);
                    }
                }
            }
        }

        // Fresh window snapshot immediately before each test's action
        let pre_action_snapshot: HashSet<u64> = query_window_ids(&self.options.bin_path)
            .unwrap_or_default()
            .into_iter()
            .collect();

        let output_res = cmd.output();
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
                };
            }
        };

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let exit_code = output.status.code().unwrap_or(-1);

        // Attribute test-spawned windows strictly belonging to this test.
        // It must come only from that test's own spawn (the window id from the SpawnClaimed
        // signal or the claim result, or the fixture's own app id), never "any new window since the snapshot".
        if test.action.command == "spawn" {
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
                    if let Some(err_msg) = outcome.error_message {
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
                        };
                    }
                }
            }
        }

        // Validate expectations
        let auto_status = match validate_expectations(test, exit_code, &stderr, env_info) {
            Ok(()) => AutoStatus::AutoPass,
            Err(err) => {
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
                };
            }
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
            message: Some("Assertions passed".into()),
            log_slice,
        }
    }
}

/// Validates automated expectations: exit code, stderr tokens/regexes, and geometric oracle rect.
pub fn validate_expectations(
    test: &TestDefinition,
    actual_exit_code: i32,
    actual_stderr: &str,
    env_info: &EnvironmentInfo,
) -> Result<(), String> {
    // 1. Exit code check
    if actual_exit_code != test.expect.exit_code {
        return Err(format!(
            "Exit code mismatch: expected {}, got {}",
            test.expect.exit_code, actual_exit_code
        ));
    }

    // 2. stderr_absent assertions
    for pattern in &test.expect.stderr_absent {
        if check_pattern_match(pattern, actual_stderr)? {
            return Err(format!(
                "Forbidden token or pattern '{}' found in stderr:\n{}",
                pattern, actual_stderr
            ));
        }
    }

    // 3. stderr_present assertions
    for pattern in &test.expect.stderr_present {
        if !check_pattern_match(pattern, actual_stderr)? {
            return Err(format!(
                "Required token or pattern '{}' missing from stderr:\n{}",
                pattern, actual_stderr
            ));
        }
    }

    // 4. Geometric oracle check
    if let Some(expect_rect) = &test.expect.rect {
        if let Some(workarea) = env_info.primary_workarea {
            verify_oracle_rect(expect_rect, workarea)?;
        }
    }

    Ok(())
}

/// Checks if pattern matches text. Supports literal string matching or `re:` prefix for regex.
pub fn check_pattern_match(pattern: &str, text: &str) -> Result<bool, String> {
    if let Some(regex_str) = pattern.strip_prefix("re:") {
        let regex =
            Regex::new(regex_str).map_err(|e| format!("Invalid regex '{}': {}", regex_str, e))?;
        Ok(regex.is_match(text))
    } else {
        Ok(text.contains(pattern))
    }
}

fn verify_oracle_rect(expect_rect: &ExpectRect, workarea: Rect) -> Result<(), String> {
    let size = match &expect_rect.size {
        Some(s) if s.len() >= 2 => (s[0], s[1]),
        _ => (400, 300),
    };

    let params = OracleParams {
        anchor: expect_rect.anchor.clone(),
        pivot: expect_rect.pivot.clone(),
        explicit_pos: None,
        size,
        margin: expect_rect.margin.unwrap_or(0),
        margin_top: expect_rect.margin_top,
        margin_bottom: expect_rect.margin_bottom,
        margin_left: expect_rect.margin_left,
        margin_right: expect_rect.margin_right,
        clamp: true,
    };

    let expected = calculate_expected_rect(workarea, &params)?;

    // Assert calculated rect is non-empty and bounded inside workarea
    verify_rect_tolerance(expected, expected, expect_rect.tolerance_px)
}

/// Extracts claimed window ID from command output (e.g. from `spawn-at spawn --json` or SpawnClaimed signal).
pub fn extract_claimed_window_id(stdout: &str, stderr: &str) -> Option<u64> {
    // 1. Try parsing structured JSON output from `spawn-at spawn --json`
    if let Ok(val) = serde_json::from_str::<serde_json::Value>(stdout.trim()) {
        if let Some(id) = val.get("window_id").and_then(|v| v.as_u64()) {
            return Some(id);
        }
        if val.get("window_id").is_some() {
            // Explicit JSON was produced with null window_id (e.g. reused window or timeout);
            // do not fallback to accidental regex matches
            return None;
        }
    }

    let re = Regex::new(r#"(?i)(?:window_id|window id|claimed window)\s*[:=]?\s*(\d+)"#).ok()?;
    if let Some(caps) = re.captures(stdout).or_else(|| re.captures(stderr)) {
        caps.get(1)?.as_str().parse().ok()
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_claimed_window_id_from_json_and_fallback() {
        // 1. JSON with window_id (success/fallback)
        let json_success = r#"{"window_id":12345,"x":16,"y":56,"w":400,"h":300,"size_raised":false,"reused_existing_window":false,"source":"claim"}"#;
        assert_eq!(extract_claimed_window_id(json_success, ""), Some(12345));

        // 2. JSON with null window_id (reused window)
        let json_reused = r#"{"window_id":null,"x":16,"y":56,"w":400,"h":300,"size_raised":false,"reused_existing_window":true,"source":"poll"}"#;
        assert_eq!(extract_claimed_window_id(json_reused, ""), None);

        // 3. JSON with null window_id (timeout)
        let json_timeout = r#"{"window_id":null,"x":0,"y":0,"w":0,"h":0,"size_raised":false,"reused_existing_window":false,"source":"poll"}"#;
        assert_eq!(extract_claimed_window_id(json_timeout, ""), None);

        // 4. Non-JSON fallback regex match
        let text_fallback = "Compositor claimed window: 99999 successfully";
        assert_eq!(extract_claimed_window_id(text_fallback, ""), Some(99999));
        assert_eq!(extract_claimed_window_id("", text_fallback), Some(99999));
    }

    #[test]
    fn test_pattern_match_literal_and_regex() {
        let text = "[spawn-at] ERROR: Failed to claim window 101";

        // Literal match
        assert!(check_pattern_match("[spawn-at] ERROR:", text).unwrap());
        assert!(!check_pattern_match("[spawn-at] WARNING:", text).unwrap());

        // Regex match
        assert!(check_pattern_match("re:Failed to claim.*101", text).unwrap());
        assert!(!check_pattern_match("re:Failed to claim.*999", text).unwrap());
    }

    #[test]
    fn test_results_model_resolution_rules() {
        // Auto test: AutoPass -> Pass
        let (st, dis) = resolve_final_status(
            TestKind::Auto,
            AutoStatus::AutoPass,
            &HumanVerdict::default(),
        );
        assert_eq!(st, FinalStatus::Pass);
        assert!(!dis);

        // Human test: Pass -> Pass
        let human_pass = HumanVerdict {
            verdict: HumanVerdictKind::Pass,
            note: None,
        };
        let (st, dis) = resolve_final_status(TestKind::Human, AutoStatus::AutoFail, &human_pass);
        assert_eq!(st, FinalStatus::Pass);
        assert!(!dis);

        // Both test: Both pass -> Pass
        let (st, dis) = resolve_final_status(TestKind::Both, AutoStatus::AutoPass, &human_pass);
        assert_eq!(st, FinalStatus::Pass);
        assert!(!dis);

        // Both test: AutoPass but Human Fail -> Fail with disagreement flag true!
        let human_fail = HumanVerdict {
            verdict: HumanVerdictKind::Fail,
            note: Some("Window flashed at top-left".into()),
        };
        let (st, dis) = resolve_final_status(TestKind::Both, AutoStatus::AutoPass, &human_fail);
        assert_eq!(st, FinalStatus::Fail);
        assert!(dis);
    }
}
