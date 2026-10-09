//! Report Writer and Session Artifacts (`report.json`, `report.md`, and log slices).

use crate::preflight::PreflightReport;
use crate::probe::EnvironmentInfo;
use crate::runner::{FinalStatus, TestResult};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentFingerprint {
    pub hostname: String,
    pub session_type: String,
    pub desktop: String,
    pub gnome_version: Option<u32>,
    pub monitors_count: usize,
    pub binary_path: String,
    pub binary_commit: Option<String>,
    pub is_dirty_build: bool,
    pub generated_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequiredSkippedItem {
    pub id: String,
    pub title: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualificationReport {
    pub supported: bool,
    pub total_required: usize,
    pub passed_required: usize,
    pub failed_required: usize,
    pub skipped_required: Vec<RequiredSkippedItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionReport {
    pub environment: EnvironmentFingerprint,
    pub qualification: QualificationReport,
    pub results: Vec<TestResult>,
}

/// Redacts window titles from text unless `include_titles` is true.
pub fn redact_titles(text: &str, include_titles: bool) -> String {
    if include_titles {
        return text.to_string();
    }

    // Common patterns where titles appear in spawn-at output:
    // e.g. "title: \"My Window\"", "--title", "Verify-..."
    let re = regex::Regex::new(r#"(?i)(--title\s+["']?)[^"'\s]+(["']?)|("title"\s*:\s*")[^"]+(")"#)
        .unwrap();

    re.replace_all(text, "$1[REDACTED]$2").to_string()
}

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

/// Builds the SessionReport data structure.
pub fn build_session_report(
    bin_path: &Path,
    preflight: &PreflightReport,
    env_info: &EnvironmentInfo,
    results: &[TestResult],
    include_titles: bool,
) -> SessionReport {
    let now_secs = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let hostname = std::env::var("HOSTNAME")
        .or_else(|_| std::env::var("HOST"))
        .unwrap_or_else(|_| "localhost".to_string());

    let sanitized_results: Vec<TestResult> = results
        .iter()
        .map(|r| {
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
            binary_path: bin_path.display().to_string(),
            binary_commit: preflight.detected_commit.clone(),
            is_dirty_build: preflight.is_dirty_build,
            generated_at: now_secs,
        },
        qualification,
        results: sanitized_results,
    }
}

/// Writes `report.json`, `report.md`, and log slices into `verify-results/<timestamp>-<host>/`.
pub fn write_report_artifacts(
    output_base: &Path,
    report: &SessionReport,
) -> Result<PathBuf, String> {
    let dir_name = format!(
        "{}-{}",
        report.environment.generated_at, report.environment.hostname
    );
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

/// Formats the markdown report.
pub fn render_markdown_report(report: &SessionReport) -> String {
    let env = &report.environment;
    let qual = &report.qualification;

    let dirty_badge = if env.is_dirty_build {
        "**[DIRTY BUILD]**"
    } else {
        "Clean"
    };

    let qual_badge = if qual.supported {
        "✅ **SUPPORTED (100% Required Passed)**"
    } else {
        "❌ **NOT QUALIFIED / UNSUPPORTED**"
    };

    let mut md = format!(
        "# Verification Session Report\n\n\
         ## 1. Environment Fingerprint\n\n\
         | Property | Value |\n\
         | :--- | :--- |\n\
         | **Host** | `{}` |\n\
         | **Session Type** | `{}` |\n\
         | **Desktop** | `{}` |\n\
         | **GNOME Version** | `{}` |\n\
         | **Monitors** | `{}` |\n\
         | **Binary** | `{}` |\n\
         | **Binary Commit** | `{}` ({}) |\n\
         | **Platform Qualification** | {} |\n\n\
         ## 2. Platform Qualification Summary\n\n\
         - **Required Tests Total:** {}\n\
         - **Required Tests Passed:** {}\n\
         - **Required Tests Failed:** {}\n\
         - **Required Tests Skipped:** {}\n\n",
        env.hostname,
        env.session_type,
        env.desktop,
        env.gnome_version
            .map(|v| v.to_string())
            .unwrap_or_else(|| "N/A".into()),
        env.monitors_count,
        env.binary_path,
        env.binary_commit.as_deref().unwrap_or("unknown"),
        dirty_badge,
        qual_badge,
        qual.total_required,
        qual.passed_required,
        qual.failed_required,
        qual.skipped_required.len()
    );

    if !qual.skipped_required.is_empty() {
        md.push_str("### ⚠️ Required-But-Skipped Tests (Prevents Qualification)\n\n");
        md.push_str("| Test ID | Title | Skip Reason |\n| :--- | :--- | :--- |\n");
        for sk in &qual.skipped_required {
            md.push_str(&format!("| `{}` | {} | {} |\n", sk.id, sk.title, sk.reason));
        }
        md.push('\n');
    }

    md.push_str("## 3. Detailed Test Results\n\n");
    md.push_str("| ID | Kind | Req | Auto | Human | Final | Duration |\n");
    md.push_str("| :--- | :--- | :---: | :---: | :---: | :---: | :---: |\n");

    for r in &report.results {
        let req_str = if r.required { "Yes" } else { "No" };
        let human_str = format!("{:?}", r.human_verdict.verdict);
        md.push_str(&format!(
            "| `{}` | `{:?}` | {} | `{:?}` | `{}` | **`{:?}`** | {}ms |\n",
            r.id, r.kind, req_str, r.auto_status, human_str, r.final_status, r.duration_ms
        ));
    }

    md
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runner::{AutoStatus, HumanVerdict};
    use crate::schema::TestKind;

    #[test]
    fn test_skipped_is_never_a_pass() {
        let results = vec![
            TestResult {
                id: "preflight/binary-head".into(),
                title: "Preflight".into(),
                kind: TestKind::Auto,
                required: true,
                auto_status: AutoStatus::AutoPass,
                human_verdict: HumanVerdict::default(),
                final_status: FinalStatus::Pass,
                disagreement: false,
                duration_ms: 5,
                exit_code: Some(0),
                stdout: "".into(),
                stderr: "".into(),
                message: None,
                log_slice: None,
            },
            TestResult {
                id: "cloak/delayed-paint".into(),
                title: "Cloak test".into(),
                kind: TestKind::Both,
                required: true,
                auto_status: AutoStatus::Skipped,
                human_verdict: HumanVerdict::default(),
                final_status: FinalStatus::Skipped,
                disagreement: false,
                duration_ms: 0,
                exit_code: None,
                stdout: "".into(),
                stderr: "".into(),
                message: Some("Wayland only".into()),
                log_slice: None,
            },
        ];

        let qual = evaluate_qualification(&results);
        assert_eq!(qual.total_required, 2);
        assert_eq!(qual.passed_required, 1);
        assert_eq!(qual.skipped_required.len(), 1);
        // RULE: Skipped test prevents platform support qualification
        assert!(!qual.supported);
    }

    #[test]
    fn test_title_redaction() {
        let unredacted = "args: --title MyPrivateDocumentWindow --app-id org.test";
        let redacted = redact_titles(unredacted, false);
        assert!(redacted.contains("[REDACTED]"));
        assert!(!redacted.contains("MyPrivateDocumentWindow"));

        let preserved = redact_titles(unredacted, true);
        assert!(preserved.contains("MyPrivateDocumentWindow"));
    }
}
