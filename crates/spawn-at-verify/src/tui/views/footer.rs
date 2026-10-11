//! TUI bottom footer bars and responsive keybinding grids.

use crate::tui::app::{ModalState, TuiApp};
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};
use unicode_width::UnicodeWidthStr;

pub const DRAWER_KEYS: &[(&str, &str)] = &[
    ("y", "Copy"),
    ("w", "Save"),
    ("t", "Raw/Compact"),
    ("j/k", "Scroll"),
    ("?", "Help"),
    ("Esc", "Close"),
];

pub const VERDICT_KEYS: &[(&str, &str)] = &[
    ("p", "Pass"),
    ("x", "Fail"),
    ("s", "Skip"),
    ("k", "Flaky"),
    ("r", "Re-run"),
    ("Esc", "Cancel"),
];

pub fn render_key_grid(entries: &[(&str, &str)], inner_width: usize) -> Vec<Line<'static>> {
    let single_row_len: usize = entries
        .iter()
        .enumerate()
        .map(|(i, (k, l))| {
            let chip_w = k.width() + 2;
            let cell_w = chip_w + 1 + l.width();
            if i > 0 {
                3 + cell_w
            } else {
                cell_w
            }
        })
        .sum();

    let single_row_with_pad = single_row_len + 1;
    if single_row_with_pad <= inner_width && inner_width > 80 {
        let mut spans = vec![Span::raw(" ")];
        for (i, (key, label)) in entries.iter().enumerate() {
            if i > 0 {
                spans.push(Span::raw("   "));
            }
            spans.push(Span::styled(
                format!("[{}]", key),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ));
            spans.push(Span::styled(
                format!(" {}", label),
                Style::default().fg(Color::Rgb(200, 200, 200)),
            ));
        }
        vec![Line::from(spans)]
    } else {
        let max_key_len = entries.iter().map(|(k, _)| k.width()).max().unwrap_or(1);

        let format_chip = |key: &str| -> String {
            let diff = max_key_len.saturating_sub(key.width());
            let pad_left = diff / 2;
            let pad_right = diff - pad_left;
            format!("[{}{}{}]", " ".repeat(pad_left), key, " ".repeat(pad_right))
        };

        let mut col_widths = [0usize; 3];
        for col in 0..3 {
            let mut max_cell_w = 0;
            for row in 0..2 {
                let idx = row * 3 + col;
                if idx < entries.len() {
                    let (key, label) = entries[idx];
                    let chip = format_chip(key);
                    let cell_w = chip.width() + 2 + label.width();
                    if cell_w > max_cell_w {
                        max_cell_w = cell_w;
                    }
                }
            }
            col_widths[col] = max_cell_w;
        }

        let mut lines = Vec::new();
        for row in 0..2 {
            let mut spans = vec![Span::raw(" ")];
            for col in 0..3 {
                let idx = row * 3 + col;
                if idx < entries.len() {
                    if col > 0 {
                        spans.push(Span::raw("   "));
                    }
                    let (key, label) = entries[idx];
                    let chip = format_chip(key);
                    spans.push(Span::styled(
                        chip.clone(),
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    ));
                    spans.push(Span::raw("  "));
                    let cell_w = chip.width() + 2 + label.width();
                    let pad_end = col_widths[col].saturating_sub(cell_w);
                    spans.push(Span::styled(
                        label.to_string(),
                        Style::default().fg(Color::Rgb(200, 200, 200)),
                    ));
                    if pad_end > 0 && col < 2 {
                        spans.push(Span::raw(" ".repeat(pad_end)));
                    }
                }
            }
            lines.push(Line::from(spans));
        }
        lines
    }
}

fn draw_drawer_footer(f: &mut Frame, area: Rect) {
    let block = Block::default().borders(Borders::ALL).border_style(
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    );
    let inner_w = area.width.saturating_sub(2) as usize;
    let lines = render_key_grid(DRAWER_KEYS, inner_w);
    let p = Paragraph::new(lines).block(block);
    f.render_widget(p, area);
}

fn draw_verdict_footer(f: &mut Frame, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )
        .title(" Your verdict: ");
    let inner_w = area.width.saturating_sub(2) as usize;
    let lines = render_key_grid(VERDICT_KEYS, inner_w);
    let p = Paragraph::new(lines).block(block);
    f.render_widget(p, area);
}

fn draw_normal_footer(f: &mut Frame, _app: &TuiApp, area: Rect) {
    let lines = if area.width >= 105 {
        vec![Line::from(vec![
            Span::styled(
                "r",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Run  "),
            Span::styled(
                "a",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" All  "),
            Span::styled(
                "c",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Run Cat    "),
            Span::styled(
                "j/k",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Move  "),
            Span::styled(
                "Tab",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Panel  "),
            Span::styled(
                "Space",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Fold    "),
            Span::styled(
                "Enter/d",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Drawer  "),
            Span::styled(
                "f",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Filter  "),
            Span::styled(
                "?",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Help    "),
            Span::styled(
                "q",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Quit"),
        ])]
    } else {
        vec![
            Line::from(vec![
                Span::styled(
                    "r",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(" Run  "),
                Span::styled(
                    "a",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(" All  "),
                Span::styled(
                    "c",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(" Run Cat  "),
                Span::styled(
                    "j/k",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(" Move  "),
                Span::styled(
                    "Tab",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(" Panel"),
            ]),
            Line::from(vec![
                Span::styled(
                    "Space",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(" Fold  "),
                Span::styled(
                    "Enter/d",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(" Drawer  "),
                Span::styled(
                    "f",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(" Filter  "),
                Span::styled(
                    "?",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(" Help  "),
                Span::styled(
                    "q",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(" Quit"),
            ]),
        ]
    };

    let p = Paragraph::new(lines);
    f.render_widget(p, area);
}

pub fn draw_footer(f: &mut Frame, app: &TuiApp, area: Rect) {
    if matches!(app.modal, ModalState::Help) {
        return;
    }
    if app.detail_drawer_open {
        draw_drawer_footer(f, area);
    } else if app.waiting_for_human.is_some()
        || matches!(app.modal, ModalState::HumanVerdict { .. })
    {
        draw_verdict_footer(f, area);
    } else {
        draw_normal_footer(f, app, area);
    }
}
