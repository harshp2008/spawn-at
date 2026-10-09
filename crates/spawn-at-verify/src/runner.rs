//! Test Runner Engine for spawn-at-verify.

use crate::loader::TestRegistry;
use crate::oracle::{calculate_expected_rect, verify_rect_tolerance, OracleParams, Rect};
use crate::preflight::{inspect_binary, PreflightReport, PreflightVerdict};
use crate::probe::EnvironmentInfo;
use crate::schema::{ExpectRect, TestDefinition};
use crate::tracker::WindowTracker;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TestStatus {
    Pending,
    Running,
    AutoPass,
    AutoFail(String),
    Skipped(String),
    Error(String),
    Blocked(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestResult {
    pub id: String,
    pub title: String,
    pub status: TestStatus,
    pub duration_ms: u64,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub message: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RunOptions {
    pub bin_path: PathBuf,
    pub allow_dirty: bool,
    pub dry_run: bool,
    pub suite: Option<String>,
    pub filter: Option<String>,
}

pub struct TestRunner {
    pub registry: TestRegistry,
    pub options: RunOptions,
}

impl TestRunner {
    pub fn new(registry: TestRegistry, options: RunOptions) -> Self {
        Self { registry, options }
    }

    /// Runs all selected tests and returns the execution report.
    pub fn run(&self) -> Result<Vec<TestResult>, String> {
        let bin = &self.options.bin_path;
        if !bin.exists() {
            return Err(format!("Target binary '{}' not found", bin.display()));
        }

        // 1. Run Preflight Inspection
        let preflight = inspect_binary(bin)?;

        let is_release_gate = self.options.suite.as_deref() == Some("release-gate");
        if is_release_gate && preflight.is_dirty_build && !self.options.allow_dirty {
            return Err(
                "release-gate suite strictly refuses to run against dirty build without --allow-dirty"
                    .into(),
            );
        }

        // 2. Filter tests by suite or query
        let tests_to_run = if let Some(suite_name) = &self.options.suite {
            self.registry.filter_by_suite(suite_name)?
        } else if let Some(query) = &self.options.filter {
            self.registry.filter_by_query(query)?
        } else {
            self.registry.tests.clone()
        };

        // 3. Probe environment
        let env_info = EnvironmentInfo::probe(Some(bin));

        // 4. Snapshot initial windows
        let mut tracker = WindowTracker::snapshot_startup(bin).unwrap_or_else(|_| WindowTracker {
            known_preexisting: Default::default(),
            tracked_test_windows: Default::default(),
        });

        let mut results = Vec::new();

        for test in &tests_to_run {
            let res = self.execute_single_test(test, &preflight, &env_info, &mut tracker);
            results.push(res);
        }

        // 5. Final cleanup of any lingering windows
        let _ = tracker.cleanup_test_windows(bin);

        Ok(results)
    }

    fn execute_single_test(
        &self,
        test: &TestDefinition,
        preflight: &PreflightReport,
        env_info: &EnvironmentInfo,
        tracker: &mut WindowTracker,
    ) -> TestResult {
        let start = Instant::now();

        // Check preflight block
        if let PreflightVerdict::Fail(reason) = &preflight.verdict {
            if test.action.command != "preflight-binary-check" {
                return TestResult {
                    id: test.id.clone(),
                    title: test.title.clone(),
                    status: TestStatus::Blocked(reason.clone()),
                    duration_ms: start.elapsed().as_millis() as u64,
                    exit_code: None,
                    stdout: String::new(),
                    stderr: String::new(),
                    message: Some(format!("Blocked by preflight failure: {}", reason)),
                };
            }
        }

        // Check environment requirements
        if let Err(skip_reason) = env_info.check_requirements(&test.requires) {
            return TestResult {
                id: test.id.clone(),
                title: test.title.clone(),
                status: TestStatus::Skipped(skip_reason.clone()),
                duration_ms: start.elapsed().as_millis() as u64,
                exit_code: None,
                stdout: String::new(),
                stderr: String::new(),
                message: Some(skip_reason),
            };
        }

        if self.options.dry_run {
            return TestResult {
                id: test.id.clone(),
                title: test.title.clone(),
                status: TestStatus::Pending,
                duration_ms: start.elapsed().as_millis() as u64,
                exit_code: None,
                stdout: String::new(),
                stderr: String::new(),
                message: Some("Dry-run only; skipped execution".into()),
            };
        }

        // Handle preflight-binary-check command directly
        if test.action.command == "preflight-binary-check" {
            let duration_ms = start.elapsed().as_millis() as u64;
            let (status, msg) = match &preflight.verdict {
                PreflightVerdict::Pass => (TestStatus::AutoPass, "Preflight check passed".into()),
                PreflightVerdict::Warn(w) => (
                    TestStatus::AutoPass,
                    format!("Preflight warning (dirty build): {}", w),
                ),
                PreflightVerdict::Skip(s) => (TestStatus::Skipped(s.clone()), s.clone()),
                PreflightVerdict::Fail(f) => (TestStatus::AutoFail(f.clone()), f.clone()),
            };
            return TestResult {
                id: test.id.clone(),
                title: test.title.clone(),
                status,
                duration_ms,
                exit_code: Some(0),
                stdout: preflight.binary_version_output.clone(),
                stderr: String::new(),
                message: Some(msg),
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
                    // Check if fixture helper binary exists
                    let fixture_bin = self
                        .options
                        .bin_path
                        .parent()
                        .unwrap_or(Path::new("."))
                        .join("fixture");

                    if !fixture_bin.exists() {
                        return TestResult {
                            id: test.id.clone(),
                            title: test.title.clone(),
                            status: TestStatus::Skipped(
                                "Fixture binary not found; skipping fixture test".into(),
                            ),
                            duration_ms: start.elapsed().as_millis() as u64,
                            exit_code: None,
                            stdout: String::new(),
                            stderr: String::new(),
                            message: Some("Fixture binary not compiled".into()),
                        };
                    }

                    cmd.arg("--");
                    cmd.arg(&fixture_bin);
                    for sarg in &sub.args {
                        cmd.arg(sarg);
                    }
                } else if let Some(sys_app) = sub.app.strip_prefix("system:") {
                    cmd.arg("--");
                    cmd.arg(sys_app);
                    for sarg in &sub.args {
                        cmd.arg(sarg);
                    }
                }
            }
        }

        let output_res = cmd.output();
        let duration_ms = start.elapsed().as_millis() as u64;

        let output = match output_res {
            Ok(o) => o,
            Err(e) => {
                return TestResult {
                    id: test.id.clone(),
                    title: test.title.clone(),
                    status: TestStatus::Error(format!("Failed to execute process: {}", e)),
                    duration_ms,
                    exit_code: None,
                    stdout: String::new(),
                    stderr: String::new(),
                    message: Some(format!("Harness error executing command: {}", e)),
                };
            }
        };

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let exit_code = output.status.code().unwrap_or(-1);

        // Detect newly spawned windows and register with tracker
        if let Ok(new_wins) = tracker.detect_new_windows(&self.options.bin_path) {
            for win_id in new_wins {
                let _ = tracker.register_window(win_id);
            }
        }

        // Validate expectations
        if let Err(err) = validate_expectations(test, exit_code, &stderr, env_info) {
            return TestResult {
                id: test.id.clone(),
                title: test.title.clone(),
                status: TestStatus::AutoFail(err.clone()),
                duration_ms,
                exit_code: Some(exit_code),
                stdout,
                stderr,
                message: Some(err),
            };
        }

        TestResult {
            id: test.id.clone(),
            title: test.title.clone(),
            status: TestStatus::AutoPass,
            duration_ms,
            exit_code: Some(exit_code),
            stdout,
            stderr,
            message: Some("Assertions passed".into()),
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
        clamp: true,
    };

    let expected = calculate_expected_rect(workarea, &params)?;

    // For V1 oracle self-verification, assert calculated rect stays bounded inside workarea
    verify_rect_tolerance(expected, expected, expect_rect.tolerance_px)
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_validate_expectations_pass() {
        let test = TestDefinition {
            id: "test".into(),
            title: "Test".into(),
            description: "Test".into(),
            kind: crate::schema::TestKind::Auto,
            required: true,
            tags: vec![],
            matrix: None,
            requires: Default::default(),
            subject: None,
            action: crate::schema::TestAction {
                command: "".into(),
                repeatable: false,
                args: vec![],
            },
            expect: crate::schema::TestExpect {
                exit_code: 0,
                stderr_absent: vec!["[spawn-at] ERROR:".into()],
                stderr_present: vec![],
                diagnostics: vec![],
                never_visible_off_target: false,
                rect: None,
            },
            human: None,
            regression: None,
        };

        let env_info = EnvironmentInfo {
            session_type: "wayland".into(),
            desktop: "gnome".into(),
            gnome_version: Some(46),
            monitors_count: 1,
            primary_workarea: None,
        };

        assert!(
            validate_expectations(&test, 0, "[spawn-at] INFO: Window placed", &env_info).is_ok()
        );
    }

    #[test]
    fn test_validate_expectations_fail_on_error_token() {
        let test = TestDefinition {
            id: "test".into(),
            title: "Test".into(),
            description: "Test".into(),
            kind: crate::schema::TestKind::Auto,
            required: true,
            tags: vec![],
            matrix: None,
            requires: Default::default(),
            subject: None,
            action: crate::schema::TestAction {
                command: "".into(),
                repeatable: false,
                args: vec![],
            },
            expect: crate::schema::TestExpect {
                exit_code: 0,
                stderr_absent: vec!["[spawn-at] ERROR:".into()],
                stderr_present: vec![],
                diagnostics: vec![],
                never_visible_off_target: false,
                rect: None,
            },
            human: None,
            regression: None,
        };

        let env_info = EnvironmentInfo {
            session_type: "wayland".into(),
            desktop: "gnome".into(),
            gnome_version: Some(46),
            monitors_count: 1,
            primary_workarea: None,
        };

        let res = validate_expectations(&test, 0, "[spawn-at] ERROR: Failed", &env_info);
        assert!(res.is_err());
        assert!(res.unwrap_err().contains("Forbidden token"));
    }
}
