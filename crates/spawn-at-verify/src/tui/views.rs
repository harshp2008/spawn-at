//! Ratatui Views and UI Widgets for spawn-at-verify TUI.

use super::app::{FilterMode, ModalState, TuiApp};
use crate::runner::{FinalStatus, HumanVerdictKind};
use crate::schema::TestKind;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Row, Table, TableState, Wrap},
    Frame,
};

/// Main UI drawing entry point.
pub fn draw_ui(f: &mut Frame, app: &TuiApp) {
    let area = f.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(if !app.fixture_exists { 4 } else { 3 }),
            Constraint::Min(10),
            Constraint::Length(2),
        ])
        .split(area);

    draw_header(f, app, chunks[0]);
    draw_body(f, app, chunks[1]);
    draw_footer(f, app, chunks[2]);

    if let ModalState::HumanVerdict { test_id } = &app.modal {
        draw_verdict_modal(f, app, test_id, area);
    }
}

fn draw_header(f: &mut Frame, app: &TuiApp, area: Rect) {
    let mut lines = Vec::new();

    // Line 1: Title and Environment
    let env_str = if let Some(env) = &app.env_info {
        format!(
            "{} ({}) | GNOME: {} | Monitors: {}",
            env.desktop,
            env.session_type,
            env.gnome_version
                .map(|v| v.to_string())
                .unwrap_or_else(|| "N/A".into()),
            env.monitors_count
        )
    } else {
        "Session probing...".into()
    };

    lines.push(Line::from(vec![
        Span::styled(
            " spawn-at-verify TUI ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" | "),
        Span::styled(env_str, Style::default().fg(Color::Gray)),
    ]));

    // Line 2: Missing fixture banner OR fixture status
    if !app.fixture_exists {
        lines.push(Line::from(vec![
            Span::styled(
                " ⚠ FIXTURE MISSING: ",
                Style::default()
                    .fg(Color::Yellow)
                    .bg(Color::Red)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!(
                    " {} not found. Build with: `cargo build -p spawn-at-verify --bin fixture --features fixture`",
                    app.fixture_path.display()
                ),
                Style::default()
                    .fg(Color::Yellow)
                    .bg(Color::Black)
                    .add_modifier(Modifier::BOLD),
            ),
        ]));
    }

    // Line 3: Filter info
    let filter_style = match app.filter_mode {
        FilterMode::All => Style::default().fg(Color::White),
        FilterMode::HumanOnly => Style::default().fg(Color::Yellow),
        FilterMode::AutoOnly => Style::default().fg(Color::Green),
        FilterMode::FailedOnly => Style::default().fg(Color::Red),
    };

    lines.push(Line::from(vec![
        Span::raw("Filter: ["),
        Span::styled(
            app.filter_mode.label(),
            filter_style.add_modifier(Modifier::BOLD),
        ),
        Span::raw("] (Press 'f' to cycle) | Showing "),
        Span::styled(
            format!(
                "{}/{}",
                app.filtered_tests().len(),
                app.registry.tests.len()
            ),
            Style::default().fg(Color::Cyan),
        ),
        Span::raw(" tests"),
    ]));

    let header_widget = Paragraph::new(lines).block(Block::default().borders(Borders::BOTTOM));
    f.render_widget(header_widget, area);
}

fn draw_body(f: &mut Frame, app: &TuiApp, area: Rect) {
    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
        .split(area);

    draw_test_table(f, app, main_chunks[0]);
    draw_detail_drawer(f, app, main_chunks[1]);
}

fn draw_test_table(f: &mut Frame, app: &TuiApp, area: Rect) {
    let filtered = app.filtered_tests();

    let rows: Vec<Row> = filtered
        .iter()
        .enumerate()
        .map(|(i, test)| {
            let res = app.results.get(&test.id);

            let (status_text, status_style) = if let Some(r) = res {
                if r.disagreement {
                    (
                        "[DISAGREE]",
                        Style::default()
                            .fg(Color::Black)
                            .bg(Color::Yellow)
                            .add_modifier(Modifier::BOLD),
                    )
                } else {
                    match r.final_status {
                        FinalStatus::Pass => (
                            "[PASS]",
                            Style::default()
                                .fg(Color::Green)
                                .add_modifier(Modifier::BOLD),
                        ),
                        FinalStatus::Fail => (
                            "[FAIL]",
                            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                        ),
                        FinalStatus::Skipped => ("[SKIP]", Style::default().fg(Color::Yellow)),
                        FinalStatus::Blocked => ("[BLOCK]", Style::default().fg(Color::DarkGray)),
                        FinalStatus::Error => (
                            "[ERROR]",
                            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                        ),
                        FinalStatus::Flaky => (
                            "[FLKY]",
                            Style::default()
                                .fg(Color::Magenta)
                                .add_modifier(Modifier::BOLD),
                        ),
                        FinalStatus::Pending => ("[PEND]", Style::default().fg(Color::DarkGray)),
                    }
                }
            } else {
                ("[PEND]", Style::default().fg(Color::DarkGray))
            };

            let kind_str = match test.kind {
                TestKind::Auto => "auto",
                TestKind::Human => "human",
                TestKind::Both => "both",
            };

            let req_str = if test.required { "req" } else { "-" };

            let cursor = if i == app.selected_index { ">" } else { " " };

            Row::new(vec![
                Span::styled(format!("{} {}", cursor, status_text), status_style),
                Span::raw(&test.id),
                Span::raw(kind_str),
                Span::raw(req_str),
            ])
        })
        .collect();

    let widths = [
        Constraint::Length(13),
        Constraint::Min(25),
        Constraint::Length(8),
        Constraint::Length(5),
    ];

    let header_row = Row::new(vec!["Status", "Test ID", "Kind", "Req"])
        .style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .bottom_margin(1);

    let table = Table::new(rows, widths)
        .header(header_row)
        .block(Block::default().borders(Borders::ALL).title(" Test Suite "))
        .row_highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        );

    let mut state = TableState::default();
    if !filtered.is_empty() {
        state.select(Some(app.selected_index.min(filtered.len() - 1)));
    }

    f.render_stateful_widget(table, area, &mut state);
}

fn draw_detail_drawer(f: &mut Frame, app: &TuiApp, area: Rect) {
    let test_opt = app.selected_test();

    let mut lines = Vec::new();

    if let Some(test) = test_opt {
        lines.push(Line::from(vec![
            Span::styled(
                "ID: ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(&test.id),
        ]));
        lines.push(Line::from(vec![
            Span::styled(
                "Title: ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(&test.title),
        ]));
        lines.push(Line::from(vec![
            Span::styled("Description: ", Style::default().fg(Color::Cyan)),
            Span::raw(&test.description),
        ]));
        lines.push(Line::raw(""));

        // Preconditions & Subject
        if let Some(sub) = &test.subject {
            lines.push(Line::from(vec![
                Span::styled("Subject App: ", Style::default().fg(Color::Yellow)),
                Span::raw(&sub.app),
            ]));
        }

        // Geometric Bounds Oracle
        if let Some(rect) = &test.expect.rect {
            lines.push(Line::raw(""));
            lines.push(Line::styled(
                "--- Oracle Arithmetic Expectation ---",
                Style::default()
                    .fg(Color::Magenta)
                    .add_modifier(Modifier::BOLD),
            ));
            lines.push(Line::from(format!(
                "Anchor: {:?} | Pivot: {:?} | Tolerance: ±{}px",
                rect.anchor.as_deref().unwrap_or("none"),
                rect.pivot.as_deref().unwrap_or("none"),
                rect.tolerance_px
            )));
        }

        // Human Review Section
        if let Some(human) = &test.human {
            lines.push(Line::raw(""));
            lines.push(Line::styled(
                "--- 👁 Human Visual Review Watch ---",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ));
            lines.push(Line::from(vec![
                Span::styled(
                    "Watch Prompt: ",
                    Style::default().add_modifier(Modifier::UNDERLINED),
                ),
                Span::raw(&human.watch),
            ]));

            let v_str = app
                .results
                .get(&test.id)
                .map(|r| match r.human_verdict.verdict {
                    HumanVerdictKind::Pass => "PASS",
                    HumanVerdictKind::Fail => "FAIL",
                    HumanVerdictKind::Skip => "SKIP",
                    HumanVerdictKind::Flaky => "FLAKY",
                    HumanVerdictKind::Unset => "UNSET (Press 'v' to record)",
                })
                .unwrap_or("UNSET (Press 'v' to record)");

            lines.push(Line::from(vec![
                Span::styled("Human Verdict: ", Style::default().fg(Color::Yellow)),
                Span::styled(v_str, Style::default().add_modifier(Modifier::BOLD)),
            ]));
        }

        // Test Execution Result
        if let Some(res) = app.results.get(&test.id) {
            lines.push(Line::raw(""));
            lines.push(Line::styled(
                "--- Execution Outcome ---",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ));
            lines.push(Line::from(format!(
                "Final: {:?} | Auto: {:?} | Human: {:?} | Duration: {}ms",
                res.final_status, res.auto_status, res.human_verdict.verdict, res.duration_ms
            )));

            if res.disagreement {
                lines.push(Line::styled(
                    "⚠ DISAGREEMENT: Automated outcome and Human visual judgment contradict!",
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                ));
            }

            if let Some(msg) = &res.message {
                lines.push(Line::from(vec![
                    Span::styled("Message: ", Style::default().fg(Color::Red)),
                    Span::raw(msg),
                ]));
            }

            if !res.stderr.is_empty() {
                lines.push(Line::raw(""));
                lines.push(Line::styled(
                    "Stderr Output:",
                    Style::default().fg(Color::DarkGray),
                ));
                for line in res.stderr.lines().take(5) {
                    lines.push(Line::from(format!("  {}", line)));
                }
            }
        }
    } else {
        lines.push(Line::raw("No test selected."));
    }

    let detail_widget = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Test Details & Verification "),
        )
        .wrap(Wrap { trim: true });

    f.render_widget(detail_widget, area);
}

fn draw_footer(f: &mut Frame, app: &TuiApp, area: Rect) {
    let msg = app.status_message.as_deref().unwrap_or(
        "[j/k] Navigate | [r] Run Selected | [a] Run All | [v] Set Verdict | [f] Filter | [q/Ctrl+C] Quit",
    );

    let footer_widget = Paragraph::new(Line::from(vec![
        Span::styled("Commands: ", Style::default().fg(Color::Cyan)),
        Span::raw(msg),
    ]))
    .block(Block::default().borders(Borders::TOP));

    f.render_widget(footer_widget, area);
}

fn draw_verdict_modal(f: &mut Frame, app: &TuiApp, test_id: &str, area: Rect) {
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
                " [P] Pass ",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" - Window positioned cleanly with zero visual flicker"),
        ]),
        Line::from(vec![
            Span::styled(
                " [F] Fail ",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
            Span::raw(" - Visual defect, flicker, flash, or wrong geometry observed"),
        ]),
        Line::from(vec![
            Span::styled(
                " [S] Skip ",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" - Cannot be visually verified in current environment"),
        ]),
        Line::from(vec![
            Span::styled(
                " [K] Flaky ",
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
        .style(Style::default().bg(Color::Black));

    let paragraph = Paragraph::new(text).block(block).wrap(Wrap { trim: true });
    f.render_widget(paragraph, popup_area);
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
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
