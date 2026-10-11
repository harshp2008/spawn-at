//! Ratatui Views and UI Widgets for spawn-at-verify TUI.

use super::app::{FilterMode, ModalState, TuiApp};
use crate::runner::{AutoStatus, FinalStatus, HumanVerdictKind};
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
    if app.detail_drawer_open {
        // Expanded full-screen detail drawer view
        draw_detail_drawer(f, app, area, true);
    } else {
        // Standard split view: left test suite + right detail drawer
        let main_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
            .split(area);

        draw_test_table_pane(f, app, main_chunks[0]);
        draw_detail_drawer(f, app, main_chunks[1], false);
    }
}

fn draw_test_table_pane(f: &mut Frame, app: &TuiApp, area: Rect) {
    let selected_test = app.selected_test();
    let selected_result = selected_test.and_then(|t| app.results.get(&t.id));
    let has_failure_reason = selected_result
        .map(|r| {
            r.final_status == FinalStatus::Fail
                || r.auto_status == AutoStatus::AutoFail
                || r.message.is_some()
        })
        .unwrap_or(false);

    if has_failure_reason {
        let left_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(8), Constraint::Length(4)])
            .split(area);

        draw_test_table(f, app, left_chunks[0]);
        draw_failure_summary_box(f, app, selected_result.unwrap(), left_chunks[1]);
    } else {
        draw_test_table(f, app, area);
    }
}

fn draw_failure_summary_box(
    f: &mut Frame,
    _app: &TuiApp,
    res: &crate::runner::TestResult,
    area: Rect,
) {
    let reason_line = if let Some(msg) = &res.message {
        msg.lines().next().unwrap_or(msg)
    } else if !res.stderr.is_empty() {
        res.stderr.lines().next().unwrap_or(&res.stderr)
    } else {
        "Test failed"
    };

    let p = Paragraph::new(Line::from(vec![
        Span::styled(
            "✖ Reason: ",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            reason_line,
            Style::default()
                .fg(Color::LightRed)
                .add_modifier(Modifier::BOLD),
        ),
    ]))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Failure Summary ")
            .border_style(Style::default().fg(Color::Red)),
    )
    .wrap(Wrap { trim: true });

    f.render_widget(p, area);
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
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Test Suite ([Enter/d] Details) "),
        )
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

fn draw_detail_drawer(f: &mut Frame, app: &TuiApp, area: Rect, is_expanded: bool) {
    let test_opt = app.selected_test();

    let mut lines = Vec::new();

    if let Some(test) = test_opt {
        let res_opt = app.results.get(&test.id);

        // 1. FAILURE REASON FIRST
        if let Some(res) = res_opt {
            if res.final_status == FinalStatus::Fail
                || res.auto_status == AutoStatus::AutoFail
                || res.final_status == FinalStatus::Error
            {
                lines.push(Line::styled(
                    "✖ FAILURE REASON:",
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                ));
                let reason_text = res
                    .message
                    .as_deref()
                    .unwrap_or("Test execution failed without specific message");
                for line in reason_text.lines() {
                    lines.push(Line::from(vec![
                        Span::styled("  ", Style::default()),
                        Span::styled(
                            line,
                            Style::default()
                                .fg(Color::LightRed)
                                .add_modifier(Modifier::BOLD),
                        ),
                    ]));
                }
                lines.push(Line::raw(""));
            }
        }

        // 2. ORACLE EXPECTED VS ACTUAL GEOMETRY
        lines.push(Line::styled(
            "--- 1. Geometric Oracle Comparison ---",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ));
        if let Some(rect) = &test.expect.rect {
            lines.push(Line::from(format!(
                "Expected: anchor={:?}, pivot={:?}, size={:?}, margin={:?}, tolerance=±{}px",
                rect.anchor.as_deref().unwrap_or("none"),
                rect.pivot.as_deref().unwrap_or("none"),
                rect.size,
                rect.margin,
                rect.tolerance_px
            )));
        } else {
            lines.push(Line::raw(
                "Expected: (No geometric rect expectation defined)",
            ));
        }

        if let Some(res) = res_opt {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&res.stdout) {
                if let (Some(x), Some(y), Some(w), Some(h)) = (
                    val.get("x").and_then(|v| v.as_i64()),
                    val.get("y").and_then(|v| v.as_i64()),
                    val.get("w").and_then(|v| v.as_u64()),
                    val.get("h").and_then(|v| v.as_u64()),
                ) {
                    lines.push(Line::from(vec![
                        Span::styled(
                            "Actual Geometry: ",
                            Style::default()
                                .fg(Color::White)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(
                            format!(
                                "pos=({}, {}), size=({}x{}), source={}",
                                x,
                                y,
                                w,
                                h,
                                val.get("source")
                                    .and_then(|s| s.as_str())
                                    .unwrap_or("claim")
                            ),
                            Style::default().fg(Color::Green),
                        ),
                    ]));
                }
            }
        }
        lines.push(Line::raw(""));

        // 3. TRACE VIOLATIONS / WARNINGS
        lines.push(Line::styled(
            "--- 2. Compositor Actor Trace Diagnostics ---",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ));
        if let Some(res) = res_opt {
            if let Some(slice) = &res.log_slice {
                let violations: Vec<&str> = slice
                    .lines()
                    .filter(|l| {
                        l.contains("VIOLATION")
                            || l.contains("WARNING")
                            || l.contains("REJECT")
                            || l.contains("WATCHDOG")
                    })
                    .collect();

                if violations.is_empty() {
                    lines.push(Line::from(vec![
                        Span::styled("✓ Trace Clean: ", Style::default().fg(Color::Green)),
                        Span::raw(
                            "Zero cloak or geometry violations recorded by compositor extension.",
                        ),
                    ]));
                } else {
                    for v in violations {
                        lines.push(Line::styled(
                            format!("  ⚠ {}", v),
                            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                        ));
                    }
                }
                lines.push(Line::styled(
                    "Trace Timeline (Recent Events):",
                    Style::default().fg(Color::DarkGray),
                ));
                for l in slice.lines().take(6) {
                    lines.push(Line::from(format!("  {}", l)));
                }
            } else {
                lines.push(Line::raw("No trace slice captured for this run."));
            }
        } else {
            lines.push(Line::raw("Test has not been executed yet."));
        }
        lines.push(Line::raw(""));

        // 4. CAPTURED STDOUT / STDERR
        lines.push(Line::styled(
            "--- 3. Captured Output (Stdout & Stderr) ---",
            Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
        ));
        if let Some(res) = res_opt {
            if !res.stderr.is_empty() {
                lines.push(Line::styled(
                    "Stderr Output:",
                    Style::default().fg(Color::LightRed),
                ));
                for line in res.stderr.lines().take(10) {
                    lines.push(Line::from(format!("  {}", line)));
                }
            } else {
                lines.push(Line::styled(
                    "Stderr: (empty)",
                    Style::default().fg(Color::DarkGray),
                ));
            }

            if !res.stdout.is_empty() {
                lines.push(Line::styled(
                    "Stdout Output:",
                    Style::default().fg(Color::DarkGray),
                ));
                for line in res.stdout.lines().take(5) {
                    lines.push(Line::from(format!("  {}", line)));
                }
            }
        } else {
            lines.push(Line::raw("Output not yet available."));
        }
        lines.push(Line::raw(""));

        // 5. HUMAN VISUAL REVIEW WATCH & VERDICT
        if let Some(human) = &test.human {
            lines.push(Line::styled(
                "--- 4. 👁 Human Visual Review Watch ---",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ));
            lines.push(Line::from(vec![
                Span::styled(
                    "Watch Prompt: ",
                    Style::default().add_modifier(Modifier::UNDERLINED),
                ),
                Span::raw(&human.watch),
            ]));

            let v_str = res_opt
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
            lines.push(Line::raw(""));
        }

        // 6. TEST METADATA & ACTIONS
        lines.push(Line::styled(
            "--- 5. Test Metadata & Action ---",
            Style::default()
                .fg(Color::Blue)
                .add_modifier(Modifier::BOLD),
        ));
        lines.push(Line::from(format!(
            "ID: {} | Kind: {:?} | Required: {}",
            test.id, test.kind, test.required
        )));
        lines.push(Line::from(format!("Title: {}", test.title)));
        lines.push(Line::from(format!("Description: {}", test.description)));
        if let Some(sub) = &test.subject {
            lines.push(Line::from(format!(
                "Subject App: {} | Args: {:?}",
                sub.app, sub.args
            )));
        }
        lines.push(Line::from(format!(
            "Action: {} {:?}",
            test.action.command, test.action.args
        )));
        if let Some(res) = res_opt {
            lines.push(Line::from(format!(
                "Outcome: Final={:?}, Auto={:?}, Duration={}ms, ExitCode={:?}",
                res.final_status, res.auto_status, res.duration_ms, res.exit_code
            )));
            if res.disagreement {
                lines.push(Line::styled(
                    "⚠ DISAGREEMENT: Automated outcome and Human visual judgment contradict!",
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                ));
            }
        }
    } else {
        lines.push(Line::raw("No test selected."));
    }

    let title_suffix = if is_expanded {
        format!(
            " [FULL VIEW - Line: {} | Enter/d/Esc: return, j/k: scroll] ",
            app.detail_scroll
        )
    } else {
        format!(
            " [Line: {} | Enter/d: expand, [/]: scroll] ",
            app.detail_scroll
        )
    };

    let title = format!(" Test Details & Diagnostics{} ", title_suffix);

    let detail_widget = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(title)
                .border_style(if is_expanded {
                    Style::default().fg(Color::Cyan)
                } else {
                    Style::default()
                }),
        )
        .scroll((app.detail_scroll, 0))
        .wrap(Wrap { trim: true });

    f.render_widget(detail_widget, area);
}

fn draw_footer(f: &mut Frame, app: &TuiApp, area: Rect) {
    let msg = app.status_message.as_deref().unwrap_or(
        "[j/k] Navigate | [r] Run | [a] Run All | [v] Visual Verdict | [d/Enter] Details | [f] Filter | [q] Quit",
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
