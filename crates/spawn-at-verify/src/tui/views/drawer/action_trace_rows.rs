//! RESULT rows of a version 2 result: what the extension logged for each action of the test
//! (`ActionTrace`), in place of the version 1 rows that describe a `spawn` slice.

use crate::runner::TestResult;
use crate::trace::action::{ActionTrace, TraceStatus, FLASH_CAVEAT, NO_CLOAK_NOTE};
use crate::tui::views::builder::DrawerBuilder;
use ratatui::style::{Color, Modifier, Style};

const GREY: Color = Color::Rgb(200, 200, 200);

/// The action whose cloak the Cloak and Budget rows describe: the first with a cloak phase, else
/// the first action.
fn headline_trace(r: &TestResult) -> Option<&ActionTrace> {
    let all = &r.flash_info.action_traces;
    all.iter()
        .find(|t| t.has_cloak_phase())
        .or_else(|| all.first())
}

/// Pushes Cloak, Budget, one Trace row per action, the inventory sentence of each command, the lag
/// of the first line, and the flash note. Reasons are the exact strings of [`TraceStatus::reason`].
pub fn push_action_trace_rows(builder: &mut DrawerBuilder, r: &TestResult) {
    let Some(head) = headline_trace(r) else {
        return;
    };
    let grey = Style::default().fg(GREY);
    let dim = Style::default()
        .fg(Color::DarkGray)
        .add_modifier(Modifier::ITALIC);
    builder.push_wrapped_kv("Cloak", &head.cloak_text(), grey);
    let budget = match head.cloak_to_reveal_ms {
        Some(ms) if ms <= 100.0 => "100 ms (provisional)   ✔ within budget".to_string(),
        Some(_) => "100 ms (provisional)   ! over provisional budget".to_string(),
        None => format!("n/a (no cloak phase for {})", head.command),
    };
    builder.push_wrapped_kv("Budget", &budget, grey);
    for t in &r.flash_info.action_traces {
        let style = match &t.status {
            TraceStatus::Captured => grey,
            TraceStatus::NotApplicable => dim,
            _ => Style::default().fg(Color::Yellow),
        };
        builder.push_wrapped_kv("Trace", &t.headline(), style);
        builder.push_wrapped_kv("Extension logs", &t.logs, dim);
        if let Some(ms) = t.lead_in_ms {
            builder.push_wrapped_kv(
                "First line",
                &format!("{ms:.1} ms after the action started (action #{})", t.index),
                grey,
            );
        }
    }
    let transform_flash = r.flash_info.flash_basis.is_some();
    if transform_flash {
        builder.push_wrapped_kv("Flash note", FLASH_CAVEAT, dim);
    } else if r
        .flash_info
        .action_traces
        .iter()
        .any(|t| t.command == "focus" && t.status == TraceStatus::Captured)
    {
        builder.push_wrapped_kv("Flash", NO_CLOAK_NOTE, dim);
    }
}

/// Pushes the TIMELINE of a version 2 result: per action a header (command, target, reason or
/// event count), then one row per event with its offset from the first line of that action. The
/// raw view shows the journal lines as logged. Only lines of runner-owned windows exist here.
pub fn push_action_timelines(builder: &mut DrawerBuilder, r: &TestResult, compact: bool) {
    let grey = Style::default().fg(GREY);
    let dim = Style::default()
        .fg(Color::DarkGray)
        .add_modifier(Modifier::ITALIC);
    let several_windows = r
        .flash_info
        .action_traces
        .iter()
        .flat_map(|t| t.events.iter().map(|e| e.window_id))
        .collect::<std::collections::BTreeSet<_>>()
        .len()
        > 1;
    for t in &r.flash_info.action_traces {
        builder.push_wrapped_kv(
            "Action",
            &t.headline(),
            if t.events.is_empty() { dim } else { grey },
        );
        if t.events.is_empty() {
            continue;
        }
        builder.push_wrapped_kv("Offsets", "ms after the first line of this action", dim);
        for e in &t.events {
            if compact {
                let window = if several_windows {
                    format!(" [{}]", e.window)
                } else {
                    String::new()
                };
                builder.push_wrapped_kv(
                    &format!("+{:.2} ms", e.offset_ms),
                    format!("{}{} {}", e.tag, window, e.detail).trim_end(),
                    grey,
                );
            } else {
                builder.push_wrapped_kv("", &e.raw, grey);
            }
        }
    }
}
