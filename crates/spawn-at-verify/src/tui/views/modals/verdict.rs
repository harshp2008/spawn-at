//! Human review verdict input dialog modal.

use super::centered_rect;
use crate::tui::app::TuiApp;
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Padding, Paragraph, Wrap},
    Frame,
};

/// Renders the human review verdict input dialog modal.
pub fn draw_verdict_modal(f: &mut Frame, app: &TuiApp, test_id: &str, area: Rect) {
    let popup_area = centered_rect(60, 40, area);
    f.render_widget(Clear, popup_area);

    let test_def = app.registry.get_test(test_id);
    let watch_prompt = test_def
        .and_then(|t| t.human.as_ref())
        .map(|h| h.watch.as_str())
        .unwrap_or("No specific watch prompt");

    let text = vec![
        Line::from(vec![
            Span::styled(
                "Test: ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(test_id),
        ]),
        Line::raw(""),
        Line::styled(
            "Visual Review Watch Instruction:",
            Style::default().fg(Color::Yellow),
        ),
        Line::from(format!("  \"{}\"", watch_prompt)),
        Line::raw(""),
        Line::styled(
            "Select Human Review Verdict:",
            Style::default().add_modifier(Modifier::BOLD),
        ),
        Line::from(vec![
            Span::styled(
                " [p] Pass ",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" - Window positioned cleanly with zero visual flicker"),
        ]),
        Line::from(vec![
            Span::styled(
                " [x] Fail ",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
            Span::raw(" - Visual defect, flicker, flash, or wrong geometry observed"),
        ]),
        Line::from(vec![
            Span::styled(
                " [s] Skip ",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" - Cannot be visually verified in current environment"),
        ]),
        Line::from(vec![
            Span::styled(
                " [k] Flaky ",
                Style::default()
                    .fg(Color::Magenta)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" - Inconsistent visual behavior across multiple runs"),
        ]),
        Line::raw(""),
        Line::styled(
            "Press [Esc] to cancel without recording.",
            Style::default().fg(Color::DarkGray),
        ),
    ];

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Record Human Review Verdict ")
        .style(Style::default().bg(Color::Black))
        .padding(Padding::horizontal(1));

    let paragraph = Paragraph::new(text).block(block).wrap(Wrap { trim: true });
    f.render_widget(paragraph, popup_area);
}
