//! Theme styling, color mapping, and status indicators.

use crate::runner::{AutoStatus, FinalStatus, HumanVerdictKind, TestResult};
use crate::schema::{TestDefinition, TestKind};
use ratatui::style::Color;

/// Orange used for "passed, but its trace was not captured"; yellow stays for geometry unchecked.
pub const TRACE_UNCHECKED: Color = Color::Rgb(255, 140, 0);

/// Pink used for "PASS (flash)": the placement is right but the window was shown before it was
/// ready or changed after the reveal. Distinct from the orange and yellow of the unchecked passes
/// and from the red of a failure.
pub const FLASH: Color = Color::Rgb(255, 105, 180);

/// List symbol of "PASS (flash)": a single-width glyph, unlike the lightning-bolt emoji.
pub const FLASH_SYMBOL: &str = "✱";

/// Maps test identifier to its primary category designation.
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

/// Pads string with trailing spaces to match requested width.
pub fn pad_str(s: &str, width: usize) -> String {
    let count = s.chars().count();
    if count >= width {
        s.to_string()
    } else {
        format!("{}{}", s, " ".repeat(width - count))
    }
}

/// Truncates test ID to fit within maximum available column width.
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

/// Maps test status to corresponding display symbol and color styling.
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
            } else if r.is_flash_pass() {
                // Placement passed, but the window flashed: not the plain green check.
                (FLASH_SYMBOL, FLASH)
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
            } else if r.final_status == FinalStatus::PassUnchecked
                || r.auto_status == AutoStatus::AutoPassUnchecked
            {
                // Passed, but the geometry was not verified: not the plain green check.
                ("!", Color::Yellow)
            } else if r.final_status == FinalStatus::PassTraceUnchecked
                || r.auto_status == AutoStatus::AutoPassTraceUnchecked
            {
                // Passed, but its trace was not captured: a different colour from geometry unchecked.
                ("!", TRACE_UNCHECKED)
            } else if r.final_status == FinalStatus::Pass || r.auto_status == AutoStatus::AutoPass {
                ("✔", Color::Green)
            } else {
                ("●", Color::DarkGray)
            }
        }
    }
}
