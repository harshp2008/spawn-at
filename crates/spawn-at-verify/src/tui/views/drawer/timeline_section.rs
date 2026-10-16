//! Drawer TIMELINE section rendering.

use super::timeline_format::parse_compact_timeline;
use crate::runner::TestResult;
use crate::schema::TestDefinition;
use crate::tui::views::builder::DrawerBuilder;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

/// Renders the TIMELINE section showing lifecycle events and timing deltas.
pub fn render_timeline_section(
    builder: &mut DrawerBuilder,
    _test: &TestDefinition,
    res_opt: Option<&TestResult>,
    compact: bool,
) {
    let trace_report = res_opt.and_then(|r| r.trace_report());
    // 3. Section: TIMELINE
    let mut win_id_opt = None;
    let mut win_pid_opt = None;
    if let Some(ref rep) = trace_report {
        for ev in &rep.events {
            if win_id_opt.is_none() {
                win_id_opt = ev.window_id;
            }
            if let Some(m) = &ev.meta {
                if win_pid_opt.is_none() {
                    win_pid_opt = m.pid;
                }
            }
        }
    }

    let timeline_extra = match (win_id_opt, win_pid_opt) {
        (Some(id), Some(pid)) => Some(format!("window {} · pid {}", id, pid)),
        (Some(id), None) => Some(format!("window {}", id)),
        _ => None,
    };
    // A version 2 result has one timeline per action, already limited to runner-owned windows.
    if let Some(r) = res_opt.filter(|r| !r.flash_info.action_traces.is_empty()) {
        let extra = super::action_trace_rows::timeline_heading_extra(r);
        builder.push_heading("TIMELINE", extra.as_deref());
        super::action_trace_rows::push_action_timelines(builder, r, compact);
        return;
    }
    builder.push_heading("TIMELINE", timeline_extra.as_deref());

    if compact {
        let trace_events = parse_compact_timeline(res_opt, trace_report.as_ref());
        if trace_events.is_empty() {
            builder.push_kv(
                "Trace",
                vec![Span::styled(
                    "no trace captured",
                    Style::default()
                        .fg(Color::DarkGray)
                        .add_modifier(Modifier::ITALIC),
                )],
                "no trace captured",
            );
        } else {
            super::timeline_blocks::push_event_blocks(builder, &trace_events);
        }
    } else {
        // Raw view
        if let Some(r) = res_opt {
            if let Some(slice) = &r.log_slice {
                for line in slice.lines() {
                    let clean = line.trim();
                    if !clean.is_empty() {
                        builder.push(
                            Line::from(vec![
                                Span::raw("  "),
                                Span::styled(
                                    clean.to_string(),
                                    Style::default().fg(Color::Rgb(200, 200, 200)),
                                ),
                            ]),
                            format!("  {}", clean),
                        );
                    }
                }
            } else {
                builder.push_kv(
                    "Trace",
                    vec![Span::styled(
                        "no trace slice available",
                        Style::default()
                            .fg(Color::DarkGray)
                            .add_modifier(Modifier::ITALIC),
                    )],
                    "no trace slice available",
                );
            }
        } else {
            builder.push_kv(
                "Trace",
                vec![Span::styled(
                    "no trace captured",
                    Style::default()
                        .fg(Color::DarkGray)
                        .add_modifier(Modifier::ITALIC),
                )],
                "no trace captured",
            );
        }
    }
}
