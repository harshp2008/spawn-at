//! Markdown session report rendering.

use super::models::*;
use crate::runner::TestResult;

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

    md.push_str("## 3. Detailed Test Results by Category\n\n");

    let mut by_category: std::collections::BTreeMap<&str, Vec<&TestResult>> =
        std::collections::BTreeMap::new();
    for r in &report.results {
        by_category.entry(r.category()).or_default().push(r);
    }

    for (cat, tests) in by_category {
        let cat_title = match cat {
            "basics" => "Basics",
            "placement" => "Placement",
            "transforms" => "Transforms",
            "focus" => "Focus",
            "animation-and-cloak" => "Animation and Cloak",
            "queries" => "Queries",
            "robustness" => "Robustness",
            "platform" => "Platform",
            other => other,
        };
        md.push_str(&format!("### {}\n\n", cat_title));
        md.push_str("| ID | Kind | Req | Auto | Human | Final | Duration |\n");
        md.push_str("| :--- | :--- | :---: | :---: | :---: | :---: | :---: |\n");

        for r in tests {
            let req_str = if r.required { "Yes" } else { "No" };
            let human_str = format!("{:?}", r.human_verdict.verdict);
            md.push_str(&format!(
                "| `{}` | `{:?}` | {} | `{:?}` | `{}` | **`{:?}`** | {}ms |\n",
                r.id, r.kind, req_str, r.auto_status, human_str, r.final_status, r.duration_ms
            ));
        }
        md.push('\n');
    }

    md
}
