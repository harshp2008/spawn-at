//! Markdown session report rendering.

use super::models::*;
use crate::runner::TestResult;
use crate::trace::flash::Visibility;

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
         - **Required Tests Skipped:** {}\n\
         - **Tests Passed With Flash:** {} ({} with a certain flash, {} with only possible ones; display {})\n\n",
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
        qual.skipped_required.len(),
        report.results.iter().filter(|r| r.is_flash_pass()).count(),
        report
            .results
            .iter()
            .filter(|r| r.is_flash_pass() && r.flash_info.flash_certain > 0)
            .count(),
        report
            .results
            .iter()
            .filter(|r| r.is_flash_pass() && r.flash_info.flash_certain == 0)
            .count(),
        match (env.refresh_hz, &env.refresh_source) {
            (Some(hz), Some(src)) => format!("{:.0} Hz, {}", hz, src),
            _ => "refresh rate not recorded".to_string(),
        }
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
            let final_str = if r.is_flash_pass() {
                "Pass (flash)".to_string()
            } else {
                format!("{:?}", r.final_status)
            };
            md.push_str(&format!(
                "| `{}` | `{:?}` | {} | `{:?}` | `{}` | **`{}`** | {}ms |\n",
                r.id, r.kind, req_str, r.auto_status, human_str, final_str, r.duration_ms
            ));
        }
        md.push('\n');
    }

    if !report.repeat_runs.is_empty() {
        md.push_str("## 4. Repeat Runs\n\n");
        md.push_str(
            "`results` above holds the first run of each repeated test; every run is listed here.\n\n",
        );
        for rr in &report.repeat_runs {
            md.push_str(&format!("### `{}` ({} runs)\n\n", rr.id, rr.runs.len()));
            md.push_str("| Run | Auto | Final | Duration | Cloak |\n| :---: | :---: | :---: | :---: | :---: |\n");
            for (i, r) in rr.runs.iter().enumerate() {
                let cloak = r
                    .cloak_duration_ms()
                    .map(|c| format!("{:.1} ms", c))
                    .unwrap_or_else(|| "n/a".into());
                md.push_str(&format!(
                    "| {} | `{:?}` | `{:?}` | {}ms | {} |\n",
                    i + 1,
                    r.auto_status,
                    r.final_status,
                    r.duration_ms,
                    cloak
                ));
            }
            md.push('\n');
        }
    }

    // Warnings never change a verdict; they are listed so a pass that needed a re-read or whose
    // window was moved after the reveal is not mistaken for a clean one.
    let mut warned: Vec<(String, Vec<String>)> = Vec::new();
    for r in &report.results {
        let w = r.run_warnings();
        if !w.is_empty() {
            warned.push((r.id.clone(), w));
        }
    }
    for rr in &report.repeat_runs {
        for (i, r) in rr.runs.iter().enumerate() {
            let w = r.run_warnings();
            if !w.is_empty() {
                warned.push((format!("{} (repeat run {})", rr.id, i + 1), w));
            }
        }
    }
    if !warned.is_empty() {
        md.push_str("## 5. Warnings\n\n");
        md.push_str("| Test | Warning |\n| :--- | :--- |\n");
        for (id, ws) in &warned {
            for w in ws {
                md.push_str(&format!("| `{}` | {} |\n", id, w));
            }
        }
        md.push('\n');
    }

    // Flash events, certain or not, with the evidence they rest on. A run counted as a flash has
    // at least one certain event; the others are listed so a near-miss is not lost.
    let mut flash_rows: Vec<String> = Vec::new();
    let mut add_flash = |id: String, r: &TestResult| {
        for e in &r.flash_info.flash_events {
            flash_rows.push(format!(
                "| `{}` | {:?} | {:?} | {:.1} | {} | {} | {} |\n",
                id,
                e.kind,
                e.source,
                e.lead_ms,
                if e.numeric {
                    "yes"
                } else {
                    "no (not above the uncertainty or threshold)"
                },
                match (e.numeric, e.visibility) {
                    (false, _) | (_, None) => "-".to_string(),
                    (true, Some(Visibility::Certain)) => "certain".to_string(),
                    (true, Some(Visibility::Possible)) => {
                        format!("possible ({:.0}%)", e.visible_probability * 100.0)
                    }
                },
                e.detail
            ));
        }
    };
    for r in &report.results {
        add_flash(r.id.clone(), r);
    }
    for rr in &report.repeat_runs {
        for (i, r) in rr.runs.iter().enumerate() {
            add_flash(format!("{} (repeat run {})", rr.id, i + 1), r);
        }
    }
    if !flash_rows.is_empty() {
        md.push_str("## 6. Flash\n\n");
        md.push_str(
            "| Test | Kind | Source | Lead (ms) | Counted | Visible on this screen | Detail |\n| :--- | :--- | :--- | ---: | :--- | :--- | :--- |\n",
        );
        for row in &flash_rows {
            md.push_str(row);
        }
        md.push('\n');
    }

    // The extension's trace of each action of a version 2 test (commands on an existing window):
    // only lines of windows the runner launched, offsets counted from the first line of the action.
    let traced: Vec<&TestResult> = report
        .results
        .iter()
        .filter(|r| !r.flash_info.action_traces.is_empty())
        .collect();
    if !traced.is_empty() {
        md.push_str("## 7. Extension trace of actions\n\n");
        md.push_str(&format!(
            "No pixels are captured. Flash results are {}.\n\n",
            crate::trace::action::FLASH_CAVEAT
        ));
        for r in traced {
            md.push_str(&format!("### `{}`\n\n", r.id));
            for t in &r.flash_info.action_traces {
                md.push_str(&format!(
                    "- {} (extension logs: {}; cloak: {})\n",
                    t.headline(),
                    t.logs,
                    t.cloak_text()
                ));
                for e in &t.events {
                    md.push_str(&format!(
                        "  - +{:.2} ms `{}` ({}) {}\n",
                        e.offset_ms, e.tag, e.window, e.detail
                    ));
                }
            }
            md.push('\n');
        }
    }

    md
}
