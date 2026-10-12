//! Session report builder, qualification evaluator, and artifact writer.

use super::markdown::render_markdown_report;
use super::models::*;
use super::redaction::*;
use crate::preflight::PreflightReport;
use crate::probe::EnvironmentInfo;
use crate::runner::{FinalStatus, TestResult};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Computes the qualification assessment.
/// RULE: Skipped is NEVER a pass. 100% of required tests must pass for platform support.
pub fn evaluate_qualification(results: &[TestResult]) -> QualificationReport {
    let mut total_required = 0;
    let mut passed_required = 0;
    let mut failed_required = 0;
    let mut skipped_required = Vec::new();

    for r in results {
        if r.required {
            total_required += 1;
            match r.final_status {
                FinalStatus::Pass => passed_required += 1,
                // A pass whose geometry was not verified does not satisfy a required test.
                FinalStatus::PassUnchecked => failed_required += 1,
                FinalStatus::Skipped => {
                    skipped_required.push(RequiredSkippedItem {
                        id: r.id.clone(),
                        title: r.title.clone(),
                        reason: r
                            .message
                            .clone()
                            .unwrap_or_else(|| "Requirement unmet".into()),
                    });
                }
                _ => failed_required += 1,
            }
        }
    }

    let supported =
        total_required > 0 && passed_required == total_required && skipped_required.is_empty();

    QualificationReport {
        supported,
        total_required,
        passed_required,
        failed_required,
        skipped_required,
    }
}

/// Copy of `r` with window titles redacted from stdout, stderr, message and log slice.
fn sanitize_result(r: &TestResult, include_titles: bool) -> TestResult {
    let mut clone = r.clone();
    clone.stdout = redact_titles(&clone.stdout, include_titles);
    clone.stderr = redact_titles(&clone.stderr, include_titles);
    if let Some(msg) = &clone.message {
        clone.message = Some(redact_titles(msg, include_titles));
    }
    if let Some(slice) = &clone.log_slice {
        clone.log_slice = Some(redact_titles(slice, include_titles));
    }
    clone
}

/// Builds the SessionReport data structure (no repeat runs).
pub fn build_session_report(
    bin_path: &Path,
    preflight: &PreflightReport,
    env_info: &EnvironmentInfo,
    results: &[TestResult],
    include_titles: bool,
) -> SessionReport {
    build_session_report_with_runs(bin_path, preflight, env_info, results, &[], include_titles)
}

/// Builds the SessionReport and also stores every repetition of tests run with `--repeat N`.
///
/// `all_runs` holds, per selected test and in the same order, the result of every repetition
/// (as returned by `TestRunner::run_detailed`). Tests with a single run are not repeated in the
/// report; for the others all runs are stored in `repeat_runs`, redacted like `results`.
pub fn build_session_report_with_runs(
    bin_path: &Path,
    preflight: &PreflightReport,
    env_info: &EnvironmentInfo,
    results: &[TestResult],
    all_runs: &[Vec<TestResult>],
    include_titles: bool,
) -> SessionReport {
    let now_secs = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let hostname = get_real_hostname();

    let sanitized_results: Vec<TestResult> = results
        .iter()
        .map(|r| sanitize_result(r, include_titles))
        .collect();
    let repeat_runs: Vec<RepeatRuns> = all_runs
        .iter()
        .filter(|runs| runs.len() > 1)
        .map(|runs| RepeatRuns {
            id: runs[0].id.clone(),
            runs: runs
                .iter()
                .map(|r| sanitize_result(r, include_titles))
                .collect(),
        })
        .collect();

    let qualification = evaluate_qualification(&sanitized_results);

    SessionReport {
        environment: EnvironmentFingerprint {
            hostname,
            session_type: env_info.session_type.clone(),
            desktop: env_info.desktop.clone(),
            gnome_version: env_info.gnome_version,
            monitors_count: env_info.monitors_count,
            primary_workarea: env_info.primary_workarea,
            independent_workarea: env_info.independent_workarea,
            binary_path: bin_path.display().to_string(),
            binary_commit: preflight.detected_commit.clone(),
            is_dirty_build: preflight.is_dirty_build,
            generated_at: now_secs,
        },
        qualification,
        results: sanitized_results,
        repeat_runs,
    }
}

/// Writes `report.json`, `report.md`, and log slices into `verify-results/<timestamp>-<host>/`.
pub fn write_report_artifacts(
    output_base: &Path,
    report: &SessionReport,
) -> Result<PathBuf, String> {
    let ts = format_sortable_timestamp(
        SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(report.environment.generated_at),
    );
    let dir_name = format!("{}-{}", ts, report.environment.hostname);
    let session_dir = output_base.join(dir_name);
    fs::create_dir_all(&session_dir).map_err(|e| {
        format!(
            "Failed to create report dir {}: {}",
            session_dir.display(),
            e
        )
    })?;

    // 1. report.json
    let json_path = session_dir.join("report.json");
    let json_data = serde_json::to_string_pretty(report)
        .map_err(|e| format!("Failed to serialize report json: {}", e))?;
    fs::write(&json_path, json_data)
        .map_err(|e| format!("Failed to write {}: {}", json_path.display(), e))?;

    // 2. report.md
    let md_path = session_dir.join("report.md");
    let md_data = render_markdown_report(report);
    fs::write(&md_path, md_data)
        .map_err(|e| format!("Failed to write {}: {}", md_path.display(), e))?;

    // 3. slices/
    let slices_dir = session_dir.join("slices");
    fs::create_dir_all(&slices_dir).map_err(|e| {
        format!(
            "Failed to create slices dir {}: {}",
            slices_dir.display(),
            e
        )
    })?;

    for res in &report.results {
        if let Some(log_slice) = &res.log_slice {
            let sanitized_id = res.id.replace('/', "_");
            let slice_file = slices_dir.join(format!("{}.log", sanitized_id));
            let _ = fs::write(slice_file, log_slice);
        }
    }

    Ok(session_dir)
}
