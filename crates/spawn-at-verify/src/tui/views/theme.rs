//! Theme styling, color mapping, and status indicators.

use crate::runner::{AutoStatus, FinalStatus, HumanVerdictKind, TestResult};
use crate::schema::{TestDefinition, TestKind};
use ratatui::style::Color;

pub fn get_test_category(cat_or_id: &str) -> &'static str {
    if cat_or_id.starts_with("preflight") || cat_or_id == "basics" {
        "Preflight"
    } else if cat_or_id.starts_with("placement") || cat_or_id == "placement" {
        "Placement"
    } else if cat_or_id.starts_with("cloak") || cat_or_id == "animation-and-cloak" {
        "Cloak & Delayed Paint"
    } else if cat_or_id.starts_with("transform") || cat_or_id == "transforms" {
        "Transforms"
    } else if cat_or_id.starts_with("focus") {
        "Focus"
    } else if cat_or_id.starts_with("query") || cat_or_id == "queries" {
        "Queries"
    } else if cat_or_id.starts_with("robust")
        || cat_or_id.starts_with("resilience")
        || cat_or_id == "robustness"
    {
        "Robustness"
    } else if cat_or_id.starts_with("platform") {
        "Platform"
    } else {
        "General"
    }
}

pub fn pad_str(s: &str, width: usize) -> String {
    let count = s.chars().count();
    if count >= width {
        s.to_string()
    } else {
        format!("{}{}", s, " ".repeat(width - count))
    }
}

pub fn truncate_id_suffix(id: &str, max_len: usize) -> String {
    let count = id.chars().count();
    if count <= max_len {
        id.to_string()
    } else if max_len <= 1 {
        "…".to_string()
    } else {
        let take = max_len - 1;
        let suffix: String = id.chars().rev().take(take).collect();
        let suffix: String = suffix.chars().rev().collect();
        format!("…{}", suffix)
    }
}

pub fn status_symbol_and_color(
    test: &TestDefinition,
    res: Option<&TestResult>,
    is_waiting: bool,
) -> (&'static str, Color) {
    if is_waiting {
        return ("?", Color::Yellow);
    }
    match res {
        None => ("●", Color::DarkGray),
        Some(r) => {
            if r.auto_status == AutoStatus::Running {
                ("◐", Color::Cyan)
            } else if r.final_status == FinalStatus::Fail
                || r.auto_status == AutoStatus::AutoFail
                || r.human_verdict.verdict == HumanVerdictKind::Fail
                || r.final_status == FinalStatus::Error
                || r.auto_status == AutoStatus::Error
            {
                ("✖", Color::Red)
            } else if r.auto_status == AutoStatus::Skipped
                || r.auto_status == AutoStatus::Blocked
                || r.human_verdict.verdict == HumanVerdictKind::Skip
            {
                ("⊘", Color::Magenta)
            } else if (test.kind == TestKind::Both || test.kind == TestKind::Human)
                && r.auto_status == AutoStatus::AutoPass
                && r.human_verdict.verdict == HumanVerdictKind::Unset
            {
                ("?", Color::Yellow)
            } else if r
                .message
                .as_deref()
                .map(|m| {
                    m.contains("dirty build")
                        || m.contains("Preflight warning")
                        || m.contains("warning")
                })
                .unwrap_or(false)
            {
                ("!", Color::Yellow)
            } else if r.final_status == FinalStatus::Pass || r.auto_status == AutoStatus::AutoPass {
                ("✔", Color::Green)
            } else {
                ("●", Color::DarkGray)
            }
        }
    }
}
