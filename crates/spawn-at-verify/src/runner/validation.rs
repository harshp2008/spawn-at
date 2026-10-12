//! Expectation verification, output matching, and action argument building.

use super::models::AutoStatus;
use crate::oracle::check::expected_rect;
use crate::oracle::{check_geometry, GeometryCheck, GeometryStatus, Rect};
use crate::probe::EnvironmentInfo;
use crate::schema::{ExpectRect, TestDefinition, TestKind};
use regex::Regex;
use std::path::Path;

/// Assembles command-line arguments for executing a test action or spawning a fixture.
pub fn build_action_command_args(test: &TestDefinition, fixture_bin: &Path) -> Vec<String> {
    let mut args = Vec::new();
    args.push(test.action.command.clone());
    for arg in &test.action.args {
        args.push(arg.clone());
    }

    if test.action.command == "spawn" {
        if let Some(sub) = &test.subject {
            if sub.app == "fixture" {
                if !args.contains(&"--json".to_string()) {
                    args.push("--json".to_string());
                }
                // If action.args does not already contain --class / -c, forward --app-id from subject.args
                if !args.contains(&"--class".to_string()) && !args.contains(&"-c".to_string()) {
                    for pair in sub.args.windows(2) {
                        if pair[0] == "--app-id" {
                            args.push("--class".to_string());
                            args.push(pair[1].clone());
                            break;
                        }
                    }
                }
                args.push("--".to_string());
                args.push(fixture_bin.to_string_lossy().to_string());
                let mut has_exit_after = false;
                for sarg in &sub.args {
                    if sarg == "--exit-after-ms" {
                        has_exit_after = true;
                    }
                    args.push(sarg.clone());
                }
                // The declared fixture minimums become `--min-width` / `--min-height` unless the
                // spec already spells the flag out in `subject.args`.
                if let Some(mw) = sub.min_width {
                    if !sub.args.iter().any(|a| a == "--min-width") {
                        args.push("--min-width".to_string());
                        args.push(mw.to_string());
                    }
                }
                if let Some(mh) = sub.min_height {
                    if !sub.args.iter().any(|a| a == "--min-height") {
                        args.push("--min-height".to_string());
                        args.push(mh.to_string());
                    }
                }
                // Bounded crash/kill-9 backstop: ensure fixture process self-terminates
                // if test harness crashes or receives SIGKILL mid-execution.
                // For human visual review (Human/Both), allow 5 minutes (300,000ms) for observation;
                // for automated runs, 10,000ms comfortably outlasts the 5s test step timeout.
                if !has_exit_after {
                    args.push("--exit-after-ms".to_string());
                    let timeout = match test.kind {
                        TestKind::Human | TestKind::Both => "300000",
                        TestKind::Auto => "10000",
                    };
                    args.push(timeout.to_string());
                }
            } else if let Some(sys_app) = sub.app.strip_prefix("system:") {
                if !args.contains(&"--json".to_string()) {
                    args.push("--json".to_string());
                }
                args.push("--".to_string());
                args.push(sys_app.to_string());
                for sarg in &sub.args {
                    args.push(sarg.clone());
                }
            }
        }
    }

    args
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

/// Checks that the oracle can compute an expected rectangle for the spec's anchor, pivot and
/// margins (an unknown anchor name is an error in the test definition). It does not compare
/// anything with an observed window: that is [`validate_geometry`].
fn verify_oracle_rect(expect_rect: &ExpectRect, workarea: Rect) -> Result<(), String> {
    let size = match &expect_rect.size {
        Some(s) if s.len() >= 2 => (s[0], s[1]),
        _ => (400, 300),
    };
    expected_rect(expect_rect, workarea, size).map(|_| ())
}

/// Compares the window observed in `stdout` with the oracle's expected rectangle within the
/// spec's tolerance. This is the same comparison the drawer shows
/// ([`crate::oracle::GeometryCheck`]), so the verdict and the table cannot disagree.
///
/// The workarea is the independent one (`env_info.independent_workarea`, read from the window
/// system);
/// the extension's own values (`query layout`, logged bounds) are cross-checks and a fallback when nothing was
/// probed.
///
/// Returns `Ok(None)` when the test has no rectangle expectation, `Ok(Some(check))` otherwise,
/// and `Err` only for a measured mismatch. The caller must look at
/// [`GeometryCheck::status`](crate::oracle::GeometryCheck::status): `Unchecked` (no workarea)
/// and `NotMeasured` (no observed rectangle) are not passes.
pub fn validate_geometry(
    test: &TestDefinition,
    stdout: &str,
    log_slice: Option<&str>,
    env_info: &EnvironmentInfo,
) -> Result<Option<crate::oracle::GeometryCheck>, String> {
    match &test.expect.rect {
        Some(expect) => {
            let check = check_geometry(expect, stdout, log_slice, env_info.workarea_probe());
            check.verdict().map(|()| Some(check))
        }
        None => Ok(None),
    }
}

/// Automated status and result message for a test whose validation did not fail, given its
/// geometry check (`None` when the test states no rectangle expectation).
///
/// - no check, or a measured pass on an independent workarea: `AutoPass`, "Assertions passed";
/// - no workarea at all: `AutoPassUnchecked`, with the message saying geometry was not verified;
/// - otherwise `AutoPass` with a note for each caveat: no rectangle in stdout, a probe/extension
///   workarea conflict, or a workarea supplied by the extension itself.
pub fn pass_assessment(check: Option<&GeometryCheck>) -> (AutoStatus, String) {
    let mut message = String::from("Assertions passed");
    let Some(c) = check else {
        return (AutoStatus::AutoPass, message);
    };
    if c.status() == GeometryStatus::Unchecked {
        message = "Assertions passed, but geometry was not verified: no workarea available from the window system or the extension".into();
        return (AutoStatus::AutoPassUnchecked, message);
    }
    if c.status() == GeometryStatus::NotMeasured {
        message.push_str(" (geometry not measured: no rectangle in stdout)");
    }
    if let Some(w) = c.warning() {
        message.push_str(&format!(" (warning: {})", w));
    }
    if c.workarea_source.is_extension_derived() {
        message.push_str(" (workarea taken from the extension, not independently probed)");
    }
    (AutoStatus::AutoPass, message)
}

/// Whether a passing result for `test` depends on the extension's trace: it asserts that the
/// window is never visible off target, or it is a cloak test (id under `cloak/`).
pub fn needs_trace(test: &TestDefinition) -> bool {
    test.expect.never_visible_off_target || test.id.starts_with("cloak/")
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
