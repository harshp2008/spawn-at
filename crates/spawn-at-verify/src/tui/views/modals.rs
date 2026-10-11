//! Modal overlays: keyboard shortcuts help and human verdict review dialog.

use crate::tui::app::TuiApp;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Padding, Paragraph, Wrap},
    Frame,
};

pub fn draw_help_modal(f: &mut Frame, app: &TuiApp, area: Rect) {
    let popup_area = centered_rect(72, 92, area);
    f.render_widget(Clear, popup_area);

    let mut lines = Vec::new();

    if app.detail_drawer_open {
        lines.push(Line::styled(
            "DRAWER KEYS & COMMANDS",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ));
        lines.push(Line::from(vec![
            Span::styled("  Copy/Save: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                "y",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Copy full text · "),
            Span::styled(
                "w",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Save text to file (.log.txt)"),
        ]));
        lines.push(Line::from(vec![
            Span::styled("  View:      ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                "t",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Toggle raw/compact · "),
            Span::styled(
                "?",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Help · "),
            Span::styled(
                "Esc",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Close drawer"),
        ]));
        lines.push(Line::from(vec![
            Span::styled("  Scroll:    ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                "j/k",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Up/down (also ↑/↓) · "),
            Span::styled(
                "PgUp/PgDn",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Page · "),
            Span::styled(
                "g/Shift+G",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Top/bottom"),
        ]));
        lines.push(Line::raw(""));
        lines.push(Line::styled(
            "MAIN KEYS & COMMANDS",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ));
        lines.push(Line::from(vec![
            Span::styled("  Run:       ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                "r",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Run selected · "),
            Span::styled(
                "a",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Run all · "),
            Span::styled(
                "c",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Run category"),
        ]));
        lines.push(Line::from(vec![
            Span::styled("  Navigate:  ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                "j/k",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Move selection · "),
            Span::styled(
                "Tab",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Switch panel · "),
            Span::styled(
                "Space",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Fold/unfold"),
        ]));
        lines.push(Line::from(vec![
            Span::styled("  View:      ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                "Enter/d",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Full drawer · "),
            Span::styled(
                "f",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Filter mode · "),
            Span::styled(
                "Ctrl+f",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Category filter"),
        ]));
        lines.push(Line::from(vec![
            Span::styled("  Quit:      ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                "q",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Quit · "),
            Span::styled(
                "Ctrl+c",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Emergency quit & kill test windows"),
        ]));
    } else {
        lines.push(Line::styled(
            "KEYS & COMMANDS",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ));
        lines.push(Line::from(vec![
            Span::styled("  Run:       ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                "r",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Run selected · "),
            Span::styled(
                "a",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Run all · "),
            Span::styled(
                "c",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Run category"),
        ]));
        lines.push(Line::from(vec![
            Span::styled("  Navigate:  ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                "j/k",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Move (or scroll details) · "),
            Span::styled(
                "Tab",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Switch panel · "),
            Span::styled(
                "Space",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Fold/unfold · "),
            Span::styled(
                "g/Shift+G",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Top/bottom"),
        ]));
        lines.push(Line::from(vec![
            Span::styled("  View:      ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                "Enter/d",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Full drawer · "),
            Span::styled(
                "f",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Filter mode · "),
            Span::styled(
                "Ctrl+f",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Category filter · "),
            Span::styled(
                "?",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Help"),
        ]));
        lines.push(Line::from(vec![
            Span::styled("  Verdict:   ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                "p",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Pass · "),
            Span::styled(
                "x",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Fail · "),
            Span::styled(
                "s",
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Skip · "),
            Span::styled(
                "k",
                Style::default()
                    .fg(Color::Magenta)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Flaky · "),
            Span::styled(
                "Esc",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Cancel"),
        ]));
        lines.push(Line::from(vec![
            Span::styled("  Quit:      ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                "q",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Quit · "),
            Span::styled(
                "Ctrl+c",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Emergency quit & kill test windows"),
        ]));
    }

    lines.push(Line::raw(""));
    lines.push(Line::styled(
        "STATUS SYMBOLS",
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    ));
    lines.push(Line::raw(
        "  ✔ Pass   ✖ Fail   - Skipped   ! Flaky   · Pending",
    ));
    lines.push(Line::raw(""));
    lines.push(Line::styled(
        "MODES LEGEND",
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    ));
    lines.push(Line::raw("  auto: automatic check"));
    lines.push(Line::raw("  you: your visual check"));
    lines.push(Line::raw("  both: automatic check + your visual review"));

    let p = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Help · Keys & Legend ")
            .border_style(Style::default().fg(Color::Cyan))
            .padding(Padding::horizontal(1)),
    );
    f.render_widget(p, popup_area);
}

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

pub fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
