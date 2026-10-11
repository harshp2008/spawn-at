//! Drawer TIMELINE section rendering.

use super::timeline_format::parse_compact_timeline;
use crate::runner::TestResult;
use crate::schema::TestDefinition;
use crate::trace::TraceAnalyser;
use crate::tui::views::builder::DrawerBuilder;
use crate::tui::views::constants::{DRAWER_INDENT, DRAWER_RIGHT_PAD};
use crate::tui::views::wrapping::wrap_value_tokens;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use unicode_width::UnicodeWidthStr;

/// Renders the TIMELINE section showing lifecycle events and timing deltas.
pub fn render_timeline_section(
    builder: &mut DrawerBuilder,
    _test: &TestDefinition,
    res_opt: Option<&TestResult>,
    compact: bool,
) {
    let trace_report = res_opt
        .and_then(|r| r.log_slice.as_ref())
        .map(|s| TraceAnalyser::new().analyse(s));
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
            let max_a = trace_events
                .iter()
                .map(|e| format!("+{:.1} ms", e.dt_ms).width())
                .max()
                .unwrap_or(9)
                .max(9);

            let col_b_x = DRAWER_INDENT + max_a + 2;
            let detail_indent = col_b_x + 2;

            let max_human_lbl_w = trace_events
                .iter()
                .flat_map(|e| e.details.iter())
                .map(|(lbl, _)| lbl.width())
                .max()
                .unwrap_or(10)
                .min(16);

            let label_col_w = max_human_lbl_w + 2;
            let detail_val_x = detail_indent + label_col_w;
            let detail_content_w = builder
                .inner_width
                .saturating_sub(detail_val_x + DRAWER_RIGHT_PAD);
            let event_count = trace_events.len();

            for (ev_idx, ev) in trace_events.iter().enumerate() {
                let dt_str = format!("+{:.1} ms", ev.dt_ms);
                let is_bold = matches!(
                    ev.tag.as_str(),
                    "CLOAK" | "UNCLOAK" | "COMMIT_RESOLVED" | "ACTOR_MAP" | "FALLBACK_USED"
                );

                let ev_style = if is_bold {
                    Style::default()
                        .fg(Color::Rgb(200, 200, 200))
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::Rgb(200, 200, 200))
                };

                let dt_pad = format!("{:>width$}", dt_str, width = max_a);
                let indent_str = " ".repeat(DRAWER_INDENT);
                let plain = format!("{}{}  {}", indent_str, dt_pad, ev.tag);
                let line = Line::from(vec![
                    Span::raw(indent_str),
                    Span::styled(dt_pad, Style::default().fg(Color::DarkGray)),
                    Span::raw("  "),
                    Span::styled(ev.tag.clone(), ev_style),
                ]);
                builder.push(line, plain);

                let indent_detail_str = " ".repeat(detail_indent);
                for (lbl, val) in &ev.details {
                    let lbl_fmt = format!("{:<width$}", lbl, width = label_col_w);
                    let val_lines = wrap_value_tokens(val, detail_content_w);
                    for (line_idx, val_line) in val_lines.into_iter().enumerate() {
                        if line_idx == 0 {
                            let det_plain = format!("{}{}{}", indent_detail_str, lbl_fmt, val_line);
                            let det_line = Line::from(vec![
                                Span::raw(indent_detail_str.clone()),
                                Span::styled(lbl_fmt.clone(), Style::default().fg(Color::DarkGray)),
                                Span::styled(
                                    val_line,
                                    Style::default().fg(Color::Rgb(200, 200, 200)),
                                ),
                            ]);
                            builder.push(det_line, det_plain);
                        } else {
                            let indent_cont = " ".repeat(detail_val_x);
                            let det_plain = format!("{}{}", indent_cont, val_line);
                            let det_line = Line::from(vec![
                                Span::raw(indent_cont),
                                Span::styled(
                                    val_line,
                                    Style::default().fg(Color::Rgb(200, 200, 200)),
                                ),
                            ]);
                            builder.push(det_line, det_plain);
                        }
                    }
                }

                // Exactly 1 blank row between consecutive events in compact view
                if ev_idx + 1 < event_count {
                    builder.push_blank();
                }
            }
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
    builder.push_blank();
}
