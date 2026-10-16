//! The event blocks of a TIMELINE: an event-name line with its offset right-aligned as `+N.N ms`,
//! then indented, column-aligned key/value rows, with one blank row between events. Both the spawn
//! timeline and the per-action timeline of an existing-window test are drawn by
//! [`push_event_blocks`], so the two cannot drift.

use super::timeline_format::ParsedTimelineEvent;
use crate::tui::views::builder::DrawerBuilder;
use crate::tui::views::constants::{DRAWER_INDENT, DRAWER_RIGHT_PAD};
use crate::tui::views::wrapping::wrap_value_tokens;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use unicode_width::UnicodeWidthStr;

/// Draws `trace_events` (offsets already relative to the first of them) into `builder`.
pub fn push_event_blocks(builder: &mut DrawerBuilder, trace_events: &[ParsedTimelineEvent]) {
    let max_a = trace_events
        .iter()
        .map(|e| format!("+{:.1} ms", e.dt_ms).width())
        .max()
        .unwrap_or(9)
        .max(9);

    let col_b_x = DRAWER_INDENT + max_a + 2;
    let detail_indent = col_b_x + 2;

    // Detail labels share the drawer-wide label column width.
    for (lbl, _) in trace_events.iter().flat_map(|e| e.details.iter()) {
        builder.note_label(lbl);
    }
    let label_col_w = builder.label_w;
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
            let lbl_fmt = builder.format_label(lbl);
            let val_lines = wrap_value_tokens(val, detail_content_w);
            for (line_idx, val_line) in val_lines.into_iter().enumerate() {
                if line_idx == 0 {
                    let det_plain = format!("{}{}{}", indent_detail_str, lbl_fmt, val_line);
                    let det_line = Line::from(vec![
                        Span::raw(indent_detail_str.clone()),
                        Span::styled(lbl_fmt.clone(), Style::default().fg(Color::DarkGray)),
                        Span::styled(val_line, Style::default().fg(Color::Rgb(200, 200, 200))),
                    ]);
                    builder.push(det_line, det_plain);
                } else {
                    let indent_cont = " ".repeat(detail_val_x);
                    let det_plain = format!("{}{}", indent_cont, val_line);
                    let det_line = Line::from(vec![
                        Span::raw(indent_cont),
                        Span::styled(val_line, Style::default().fg(Color::Rgb(200, 200, 200))),
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
