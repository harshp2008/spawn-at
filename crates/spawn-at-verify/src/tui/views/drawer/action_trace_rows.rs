//! RESULT rows and TIMELINE of a version 2 result: what the extension logged for each action of the
//! test (`ActionTrace`), in place of the version 1 rows that describe a `spawn` slice. The
//! timeline is drawn by the same parser and the same block renderer as the spawn timeline
//! (`timeline_format::parse_timeline_lines`, `timeline_blocks::push_event_blocks`).

use super::timeline_blocks::push_event_blocks;
use super::timeline_format::parse_timeline_lines;
use crate::runner::TestResult;
use crate::trace::action::{ActionTrace, ActionVerdict, TraceStatus, FLASH_CAVEAT, NO_CLOAK_NOTE};
use crate::tui::views::builder::DrawerBuilder;
use ratatui::style::{Color, Modifier, Style};

const GREY: Color = Color::Rgb(200, 200, 200);

fn grey() -> Style {
    Style::default().fg(GREY)
}

fn dim() -> Style {
    Style::default()
        .fg(Color::DarkGray)
        .add_modifier(Modifier::ITALIC)
}

/// The action whose cloak the Cloak and Budget rows describe: the first with a cloak phase, else
/// the first action.
fn headline_trace(r: &TestResult) -> Option<&ActionTrace> {
    let all = &r.flash_info.action_traces;
    all.iter()
        .find(|t| t.has_cloak_phase())
        .or_else(|| all.first())
}

/// Pushes Cloak, Budget, one Trace row per action (verdict wording as for spawn, plus the event
/// count; the action label only when more than one action ran) and the lag of the first line.
/// Reasons are the exact strings of [`TraceStatus::reason`]. The flash rows follow, from
/// [`push_flash_note`].
pub fn push_action_trace_rows(builder: &mut DrawerBuilder, r: &TestResult) {
    let Some(head) = headline_trace(r) else {
        return;
    };
    builder.push_wrapped_kv("Cloak", &head.cloak_text(), grey());
    let budget = match head.cloak_to_reveal_ms {
        Some(ms) if ms <= 100.0 => "100 ms (provisional)   ✔ within budget".to_string(),
        Some(_) => "100 ms (provisional)   ! over provisional budget".to_string(),
        None => format!("n/a (no cloak phase for {})", head.command),
    };
    builder.push_wrapped_kv("Budget", &budget, grey());
    let labelled = r.flash_info.action_traces.len() > 1;
    for t in &r.flash_info.action_traces {
        let style = match (&t.status, t.verdict()) {
            (_, ActionVerdict::Violations(_)) => Style::default().fg(Color::Red),
            (TraceStatus::Captured, ActionVerdict::Clean) => grey(),
            (TraceStatus::Captured, _) | (TraceStatus::NotApplicable, _) => dim(),
            _ => Style::default().fg(Color::Yellow),
        };
        builder.push_wrapped_kv("Trace", &t.trace_text(labelled), style);
        // Only a command that logs some of its work needs its inventory said; a captured trace
        // and a command that logs nothing (the exact "not applicable" reason) say it already.
        if t.status == TraceStatus::NoLines {
            builder.push_wrapped_kv("Extension logs", &t.logs, dim());
        }
        if let Some(ms) = t.lead_in_ms {
            builder.push_wrapped_kv(
                "First line",
                &format!("{ms:.1} ms after the action started"),
                grey(),
            );
        }
    }
}

/// Under the Flash row: the caveat of a transform's flash result on one wrapped row, or, for a
/// focus, why there is no flash analysis.
pub fn push_flash_note(builder: &mut DrawerBuilder, r: &TestResult) {
    if r.flash_info.flash_basis.is_some() {
        builder.push_wrapped_kv("", FLASH_CAVEAT, dim());
    } else if r
        .flash_info
        .action_traces
        .iter()
        .any(|t| t.command == "focus" && t.status == TraceStatus::Captured)
    {
        builder.push_wrapped_kv("Flash", NO_CLOAK_NOTE, dim());
    }
}

/// The TIMELINE heading suffix: the first window id and its pid when the captured lines have
/// one (omitted cleanly otherwise).
pub fn timeline_heading_extra(r: &TestResult) -> Option<String> {
    let t = r
        .flash_info
        .action_traces
        .iter()
        .find(|t| t.window_id.is_some())?;
    let id = t.window_id?;
    let pid = r.flash_info.action_traces.iter().find_map(|t| t.pid());
    Some(match pid {
        Some(pid) => format!("window {id} · pid {pid}"),
        None => format!("window {id}"),
    })
}

/// Pushes the TIMELINE of a version 2 result: one caption (offsets count from the first line of
/// each action), then per action its label when several ran and its event blocks, drawn exactly
/// like a spawn's. An action with no lines shows its reason. The raw view shows the lines as
/// logged. Only lines of runner-owned windows exist here.
pub fn push_action_timelines(builder: &mut DrawerBuilder, r: &TestResult, compact: bool) {
    let labelled = r.flash_info.action_traces.len() > 1;
    if compact {
        builder.push_wrapped_kv(
            "",
            "offsets count from the first line of each action",
            dim(),
        );
    }
    for t in &r.flash_info.action_traces {
        if labelled || t.events.is_empty() {
            builder.push_wrapped_kv(
                "Action",
                &t.trace_text(true),
                if t.events.is_empty() { dim() } else { grey() },
            );
        }
        if t.events.is_empty() {
            continue;
        }
        if compact {
            let parsed = parse_timeline_lines(t.events.iter().map(|e| e.raw.as_str()));
            push_event_blocks(builder, &parsed);
            builder.push_blank();
        } else {
            for e in &t.events {
                builder.push_wrapped_kv("", &e.raw, grey());
            }
        }
    }
}
