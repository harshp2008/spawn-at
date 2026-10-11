use super::app::{ModalState, TuiApp};
use crate::runner::{AutoStatus, FinalStatus, HumanVerdictKind};
use crate::schema::TestKind;
use crate::trace::TraceAnalyser;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Padding, Paragraph, Row, Table, TableState, Wrap},
    Frame,
};

/// Main UI drawing entry point.
pub fn draw_ui(f: &mut Frame, app: &TuiApp) {
    let area = f.area();

    // 1 line top padding, 1 line bottom padding
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),                                       // Top padding
            Constraint::Length(if !app.fixture_exists { 2 } else { 1 }), // Header line (+ banner if fixture missing)
            Constraint::Min(8),                                          // Content panels
            Constraint::Length(1),                                       // Command bar
            Constraint::Length(1),                                       // Bottom padding
        ])
        .split(area);

    draw_header(f, app, chunks[1]);
    draw_body(f, app, chunks[2]);
    draw_footer(f, app, chunks[3]);

    if let ModalState::HumanVerdict { test_id } = &app.modal {
        draw_verdict_modal(f, app, test_id, area);
    }
}

fn draw_header(f: &mut Frame, app: &TuiApp, area: Rect) {
    let mut lines = Vec::new();

    // Fixture missing banner if needed
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
                    " Run `cargo build -p spawn-at-verify --bin fixture --features fixture` ({})",
                    app.fixture_path.display()
                ),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
        ]));
    }

    // Progress counts
    let all_filtered = app.filtered_tests();
    let total_count = all_filtered.len();
    let done_count = all_filtered
        .iter()
        .filter(|t| {
            app.results
                .get(&t.id)
                .map(|r| r.final_status != FinalStatus::Pending)
                .unwrap_or(false)
        })
        .count();

    let waiting_count = all_filtered
        .iter()
        .filter(|t| match t.kind {
            TestKind::Human | TestKind::Both => app
                .results
                .get(&t.id)
                .map(|r| r.human_verdict.verdict == HumanVerdictKind::Unset)
                .unwrap_or(true),
            TestKind::Auto => false,
        })
        .count();

    let filter_label = app.filter_mode.label();

    lines.push(Line::from(vec![
        Span::styled(
            "spawn-at verify",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" · ", Style::default().fg(Color::DarkGray)),
        Span::styled("Filter: ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("{} (f)", filter_label),
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" · ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("{}/{} done", done_count, total_count),
            Style::default()
                .fg(if done_count > 0 {
                    Color::Green
                } else {
                    Color::White
                })
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" · ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("{} waiting for you", waiting_count),
            Style::default()
                .fg(if waiting_count > 0 {
                    Color::Yellow
                } else {
                    Color::DarkGray
                })
                .add_modifier(Modifier::BOLD),
        ),
    ]));

    let header_widget = Paragraph::new(lines);
    f.render_widget(header_widget, area);
}

fn draw_body(f: &mut Frame, app: &TuiApp, area: Rect) {
    if app.detail_drawer_open {
        // Expanded full-screen detail drawer view
        draw_detail_drawer(f, app, area, true);
    } else {
        // Standard split view: Left test suite + Right overview panel
        let main_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
            .split(area);

        draw_test_table_pane(f, app, main_chunks[0]);
        draw_right_panel_default(f, app, main_chunks[1]);
    }
}

fn draw_test_table_pane(f: &mut Frame, app: &TuiApp, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(8),    // Test list
            Constraint::Length(1), // Kinds legend
            Constraint::Length(4), // Result box
        ])
        .split(area);

    draw_test_table(f, app, chunks[0]);
    draw_table_legend(f, chunks[1]);
    draw_result_box(f, app, chunks[2]);
}

fn draw_table_legend(f: &mut Frame, area: Rect) {
    let line = Line::from(vec![
        Span::styled(" Kinds: ", Style::default().fg(Color::DarkGray)),
        Span::styled("auto", Style::default().fg(Color::White)),
        Span::styled(
            " = verified by math & trace · ",
            Style::default().fg(Color::DarkGray),
        ),
        Span::styled("you", Style::default().fg(Color::Yellow)),
        Span::styled(
            " = visual review only · ",
            Style::default().fg(Color::DarkGray),
        ),
        Span::styled("both", Style::default().fg(Color::Cyan)),
        Span::styled(" = auto + visual", Style::default().fg(Color::DarkGray)),
    ]);
    f.render_widget(Paragraph::new(line), area);
}

fn get_test_category(id: &str) -> &'static str {
    if id.starts_with("preflight") {
        "Preflight"
    } else if id.starts_with("placement") {
        "Placement"
    } else if id.starts_with("cloak") {
        "Cloak & Delayed Paint"
    } else if id.starts_with("transform") {
        "Transform"
    } else if id.starts_with("lifecycle") {
        "Lifecycle"
    } else {
        "General"
    }
}

fn draw_test_table(f: &mut Frame, app: &TuiApp, area: Rect) {
    let filtered = app.filtered_tests();

    // Map test definitions to rows with category separator headers
    let mut rows: Vec<Row> = Vec::new();
    let mut selected_row_idx: Option<usize> = None;
    let mut last_category: Option<&str> = None;

    for (test_idx, test) in filtered.iter().enumerate() {
        let category = get_test_category(&test.id);
        if last_category != Some(category) {
            last_category = Some(category);
            let sep_text = format!(
                "── {} ────────────────────────────────────────────────",
                category
            );
            rows.push(
                Row::new(vec![
                    Span::styled(
                        sep_text,
                        Style::default()
                            .fg(Color::DarkGray)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(""),
                    Span::raw(""),
                    Span::raw(""),
                ])
                .style(Style::default().fg(Color::DarkGray)),
            );
        }

        if test_idx == app.selected_index {
            selected_row_idx = Some(rows.len());
        }

        let res = app.results.get(&test.id);

        let status_span = match res {
            None => Span::styled("● not run", Style::default().fg(Color::DarkGray)),
            Some(r) => match test.kind {
                TestKind::Auto => {
                    let is_warn = r
                        .message
                        .as_deref()
                        .map(|m| m.contains("Preflight warning"))
                        .unwrap_or(false);
                    if is_warn {
                        Span::styled(
                            "! warn",
                            Style::default()
                                .fg(Color::Yellow)
                                .add_modifier(Modifier::BOLD),
                        )
                    } else {
                        match r.auto_status {
                            AutoStatus::AutoPass => Span::styled(
                                "✔ pass",
                                Style::default()
                                    .fg(Color::Green)
                                    .add_modifier(Modifier::BOLD),
                            ),
                            AutoStatus::AutoFail => Span::styled(
                                "✖ fail",
                                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                            ),
                            AutoStatus::Skipped => {
                                Span::styled("⊘ skip", Style::default().fg(Color::DarkGray))
                            }
                            AutoStatus::Error => Span::styled(
                                "✖ error",
                                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                            ),
                            AutoStatus::Blocked => {
                                Span::styled("⊘ blocked", Style::default().fg(Color::DarkGray))
                            }
                            AutoStatus::Pending => {
                                Span::styled("● not run", Style::default().fg(Color::DarkGray))
                            }
                            AutoStatus::Running => {
                                Span::styled("⏳ running", Style::default().fg(Color::Cyan))
                            }
                        }
                    }
                }
                TestKind::Both => {
                    let auto_char = match r.auto_status {
                        AutoStatus::AutoPass => "✔",
                        AutoStatus::AutoFail => "✖",
                        _ => "●",
                    };
                    let human_str = match r.human_verdict.verdict {
                        HumanVerdictKind::Unset => {
                            if app.waiting_for_human.as_deref() == Some(&test.id) {
                                "?"
                            } else {
                                "pending"
                            }
                        }
                        HumanVerdictKind::Pass => "✔",
                        HumanVerdictKind::Fail => "✖",
                        HumanVerdictKind::Skip => "⊘",
                        HumanVerdictKind::Flaky => "!",
                    };

                    let auto_style = if r.auto_status == AutoStatus::AutoPass {
                        Style::default().fg(Color::Green)
                    } else if r.auto_status == AutoStatus::AutoFail {
                        Style::default().fg(Color::Red)
                    } else {
                        Style::default().fg(Color::DarkGray)
                    };

                    let human_style = match r.human_verdict.verdict {
                        HumanVerdictKind::Pass => Style::default().fg(Color::Green),
                        HumanVerdictKind::Fail => Style::default().fg(Color::Red),
                        HumanVerdictKind::Skip => Style::default().fg(Color::DarkGray),
                        _ => {
                            if app.waiting_for_human.as_deref() == Some(&test.id) {
                                Style::default()
                                    .fg(Color::Yellow)
                                    .add_modifier(Modifier::BOLD)
                            } else {
                                Style::default().fg(Color::Yellow)
                            }
                        }
                    };

                    Span::styled(
                        format!("auto {} | you: {}", auto_char, human_str),
                        if r.human_verdict.verdict == HumanVerdictKind::Unset {
                            human_style
                        } else {
                            auto_style
                        },
                    )
                }
                TestKind::Human => match r.human_verdict.verdict {
                    HumanVerdictKind::Pass => Span::styled(
                        "✔ pass",
                        Style::default()
                            .fg(Color::Green)
                            .add_modifier(Modifier::BOLD),
                    ),
                    HumanVerdictKind::Fail => Span::styled(
                        "✖ fail",
                        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                    ),
                    HumanVerdictKind::Skip => {
                        Span::styled("⊘ skip", Style::default().fg(Color::DarkGray))
                    }
                    HumanVerdictKind::Flaky => {
                        Span::styled("! flaky", Style::default().fg(Color::Magenta))
                    }
                    HumanVerdictKind::Unset => {
                        if app.waiting_for_human.as_deref() == Some(&test.id) {
                            Span::styled(
                                "? waiting for you",
                                Style::default()
                                    .fg(Color::Yellow)
                                    .add_modifier(Modifier::BOLD),
                            )
                        } else {
                            Span::styled("● pending", Style::default().fg(Color::Yellow))
                        }
                    }
                },
            },
        };

        let kind_span = match test.kind {
            TestKind::Auto => Span::styled("auto", Style::default().fg(Color::DarkGray)),
            TestKind::Human => Span::styled("you", Style::default().fg(Color::Yellow)),
            TestKind::Both => Span::styled("both", Style::default().fg(Color::Cyan)),
        };

        let cursor = if test_idx == app.selected_index {
            ">"
        } else {
            " "
        };

        rows.push(Row::new(vec![
            status_span,
            Span::styled(
                format!("{} {}", cursor, test.id),
                if test_idx == app.selected_index {
                    Style::default().add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                },
            ),
            kind_span,
            Span::raw(&test.title),
        ]));
    }

    let widths = [
        Constraint::Length(20), // Status
        Constraint::Min(26),    // Test ID
        Constraint::Length(6),  // Kind
        Constraint::Min(22),    // Title
    ];

    let header_row = Row::new(vec!["Status", "Test ID", "Kind", "Title"])
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
                .title(" Test Suite ")
                .padding(Padding::horizontal(1)),
        )
        .row_highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        );

    let mut state = TableState::default();
    if let Some(r_idx) = selected_row_idx {
        state.select(Some(r_idx));
    }

    f.render_stateful_widget(table, area, &mut state);
}

fn draw_result_box(f: &mut Frame, app: &TuiApp, area: Rect) {
    let test_opt = app.selected_test();
    let res_opt = test_opt.and_then(|t| app.results.get(&t.id));

    let mut lines = Vec::new();

    if let (Some(test), Some(res)) = (test_opt, res_opt) {
        // Auto status line
        match res.auto_status {
            AutoStatus::AutoPass => {
                let is_dirty_warn = res
                    .message
                    .as_deref()
                    .map(|m| m.contains("Preflight warning"))
                    .unwrap_or(false);

                if is_dirty_warn {
                    lines.push(Line::from(vec![
                        Span::styled(
                            "! Auto: WARN ",
                            Style::default()
                                .fg(Color::Yellow)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(
                            res.message.as_deref().unwrap_or("Warning detected"),
                            Style::default().fg(Color::Yellow),
                        ),
                    ]));
                } else {
                    let mut timing_info = String::new();
                    if let Some(slice) = &res.log_slice {
                        let report = TraceAnalyser::new().analyse(slice);
                        if let Some(ms) = report.cloak_duration_ms {
                            timing_info =
                                format!(" (cloak: {:.1}ms, budget: 100ms) · Trace: PASS", ms);
                        } else if report.is_pass() {
                            timing_info = " · Trace: 0 violations, PASS".into();
                        }
                    }
                    lines.push(Line::from(vec![
                        Span::styled(
                            "✔ Auto: PASS",
                            Style::default()
                                .fg(Color::Green)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(timing_info, Style::default().fg(Color::White)),
                    ]));
                }
            }
            AutoStatus::AutoFail => {
                let reason = res.message.as_deref().unwrap_or("Verification failed");
                lines.push(Line::from(vec![
                    Span::styled(
                        "✖ Auto: FAIL - ",
                        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(reason, Style::default().fg(Color::LightRed)),
                ]));
            }
            AutoStatus::Skipped => {
                lines.push(Line::from(vec![
                    Span::styled("⊘ Auto: SKIPPED - ", Style::default().fg(Color::DarkGray)),
                    Span::styled(
                        res.message.as_deref().unwrap_or("Skipped requirements"),
                        Style::default().fg(Color::Gray),
                    ),
                ]));
            }
            AutoStatus::Error => {
                lines.push(Line::from(vec![
                    Span::styled(
                        "✖ Auto: ERROR - ",
                        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        res.message.as_deref().unwrap_or("Harness execution error"),
                        Style::default().fg(Color::LightRed),
                    ),
                ]));
            }
            AutoStatus::Blocked => {
                lines.push(Line::from(vec![
                    Span::styled("⊘ Auto: BLOCKED - ", Style::default().fg(Color::DarkGray)),
                    Span::styled(
                        res.message.as_deref().unwrap_or("Blocked by preflight"),
                        Style::default().fg(Color::Gray),
                    ),
                ]));
            }
            AutoStatus::Pending => {
                lines.push(Line::styled(
                    "● Auto: Not executed yet",
                    Style::default().fg(Color::DarkGray),
                ));
            }
            AutoStatus::Running => {
                lines.push(Line::styled(
                    "⏳ Auto: Running assertions...",
                    Style::default().fg(Color::Cyan),
                ));
            }
        }

        // Human status line (if relevant)
        if test.kind == TestKind::Human || test.kind == TestKind::Both {
            match res.human_verdict.verdict {
                HumanVerdictKind::Unset => {
                    if app.waiting_for_human.as_deref() == Some(&test.id) {
                        lines.push(Line::from(vec![
                            Span::styled(
                                "? Visual Review: ",
                                Style::default()
                                    .fg(Color::Yellow)
                                    .add_modifier(Modifier::BOLD),
                            ),
                            Span::styled(
                                "Waiting for you! Press 'p' (Pass), 'f' (Fail), 's' (Skip)",
                                Style::default()
                                    .fg(Color::White)
                                    .add_modifier(Modifier::BOLD),
                            ),
                        ]));
                    } else {
                        lines.push(Line::styled(
                            "? Visual Review: Pending execution",
                            Style::default().fg(Color::Yellow),
                        ));
                    }
                }
                HumanVerdictKind::Pass => {
                    lines.push(Line::styled(
                        "✔ Visual Review: PASS",
                        Style::default()
                            .fg(Color::Green)
                            .add_modifier(Modifier::BOLD),
                    ));
                }
                HumanVerdictKind::Fail => {
                    lines.push(Line::styled(
                        "✖ Visual Review: FAIL",
                        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                    ));
                }
                HumanVerdictKind::Skip => {
                    lines.push(Line::styled(
                        "⊘ Visual Review: SKIPPED",
                        Style::default().fg(Color::DarkGray),
                    ));
                }
                HumanVerdictKind::Flaky => {
                    lines.push(Line::styled(
                        "! Visual Review: FLAKY",
                        Style::default().fg(Color::Magenta),
                    ));
                }
            }
        }
    } else {
        lines.push(Line::styled(
            "● Not run yet. Select test and press 'r' to execute or 'a' to run all.",
            Style::default().fg(Color::DarkGray),
        ));
    }

    let border_color = if let Some(res) = res_opt {
        if res.final_status == FinalStatus::Fail
            || res.auto_status == AutoStatus::AutoFail
            || res.final_status == FinalStatus::Error
        {
            Color::Red
        } else if res.final_status == FinalStatus::Pass || res.auto_status == AutoStatus::AutoPass {
            Color::Green
        } else if app.waiting_for_human.is_some() {
            Color::Yellow
        } else {
            Color::DarkGray
        }
    } else {
        Color::DarkGray
    };

    let p = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Result ")
                .border_style(Style::default().fg(border_color))
                .padding(Padding::horizontal(1)),
        )
        .wrap(Wrap { trim: true });

    f.render_widget(p, area);
}

fn draw_right_panel_default(f: &mut Frame, app: &TuiApp, area: Rect) {
    let test_opt = app.selected_test();

    let mut lines = Vec::new();

    if let Some(test) = test_opt {
        let res_opt = app.results.get(&test.id);

        // Plain-language title
        lines.push(Line::styled(
            &test.title,
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ));

        // ID and Kind
        let kind_str = match test.kind {
            TestKind::Auto => "auto (fully verified by math & trace)",
            TestKind::Human => "you (visual inspection required)",
            TestKind::Both => "both (auto checks + visual review)",
        };
        lines.push(Line::from(vec![
            Span::styled("ID: ", Style::default().fg(Color::DarkGray)),
            Span::styled(&test.id, Style::default().fg(Color::White)),
            Span::styled(" · Kind: ", Style::default().fg(Color::DarkGray)),
            Span::styled(kind_str, Style::default().fg(Color::Gray)),
        ]));

        // Description
        for desc_line in test.description.lines() {
            lines.push(Line::styled(desc_line, Style::default().fg(Color::Gray)));
        }
        lines.push(Line::raw(""));

        // What to look for
        lines.push(Line::styled(
            "What to look for:",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ));
        if let Some(human) = &test.human {
            for watch_line in human.watch.lines() {
                lines.push(Line::from(vec![
                    Span::styled("• ", Style::default().fg(Color::Yellow)),
                    Span::styled(
                        watch_line,
                        Style::default()
                            .fg(Color::White)
                            .add_modifier(Modifier::BOLD),
                    ),
                ]));
            }
        } else {
            lines.push(Line::styled(
                "• Window cold-starts with zero visual flicker; cloaked at frame 0 and revealed only when sized and placed.",
                Style::default().fg(Color::White),
            ));
        }
        lines.push(Line::raw(""));

        // Outcome Summary
        lines.push(Line::styled(
            "Outcome Summary:",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ));
        if let Some(res) = res_opt {
            let auto_label = match res.auto_status {
                AutoStatus::AutoPass => Span::styled(
                    "PASS",
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                ),
                AutoStatus::AutoFail => Span::styled(
                    "FAIL",
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                ),
                AutoStatus::Skipped => {
                    Span::styled("SKIPPED", Style::default().fg(Color::DarkGray))
                }
                AutoStatus::Error => Span::styled("ERROR", Style::default().fg(Color::Red)),
                AutoStatus::Blocked => {
                    Span::styled("BLOCKED", Style::default().fg(Color::DarkGray))
                }
                AutoStatus::Pending => Span::styled("PENDING", Style::default().fg(Color::Yellow)),
                AutoStatus::Running => Span::styled("RUNNING", Style::default().fg(Color::Cyan)),
            };
            lines.push(Line::from(vec![
                Span::styled("• Auto assertions: ", Style::default().fg(Color::Gray)),
                auto_label,
                Span::styled(
                    format!(" ({} ms)", res.duration_ms),
                    Style::default().fg(Color::DarkGray),
                ),
            ]));

            if let Some(msg) = &res.message {
                for msg_line in msg.lines() {
                    lines.push(Line::from(vec![
                        Span::styled("• Detail: ", Style::default().fg(Color::Gray)),
                        Span::styled(msg_line, Style::default().fg(Color::LightRed)),
                    ]));
                }
            }

            if let Some(slice) = &res.log_slice {
                let report = TraceAnalyser::new().analyse(slice);
                if let Some(ms) = report.cloak_duration_ms {
                    lines.push(Line::from(vec![
                        Span::styled("• Cloak duration: ", Style::default().fg(Color::Gray)),
                        Span::styled(
                            format!("{:.1} ms", ms),
                            Style::default()
                                .fg(Color::White)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(
                            " (provisional budget: 100 ms)",
                            Style::default().fg(Color::DarkGray),
                        ),
                    ]));
                }
                lines.push(Line::from(vec![
                    Span::styled("• Trace diagnostics: ", Style::default().fg(Color::Gray)),
                    Span::styled(
                        if report.is_pass() {
                            "0 violations recorded"
                        } else {
                            "Violations detected"
                        },
                        Style::default().fg(if report.is_pass() {
                            Color::Green
                        } else {
                            Color::Red
                        }),
                    ),
                ]));
            }
        } else {
            lines.push(Line::styled(
                "• Not executed yet",
                Style::default().fg(Color::DarkGray),
            ));
        }
    } else {
        lines.push(Line::styled(
            "No test selected",
            Style::default().fg(Color::DarkGray),
        ));
    }

    // Clipping check (Item 14 & 16)
    let available_height = area.height.saturating_sub(2) as usize;
    let inner_width = area.width.saturating_sub(4).max(1) as usize;

    let mut total_visual_rows = 0;
    for line in &lines {
        let len: usize = line.spans.iter().map(|s| s.content.chars().count()).sum();
        let rows = if len == 0 {
            1
        } else {
            len.div_ceil(inner_width)
        };
        total_visual_rows += rows;
    }

    let is_clipped = total_visual_rows > available_height;

    let final_lines: Vec<Line> = if is_clipped && available_height > 2 {
        let hint_rows = if 46 <= inner_width { 2 } else { 3 };
        let max_rows = available_height.saturating_sub(hint_rows + 2);
        let mut truncated = Vec::new();
        let mut cur_rows = 0;
        for line in lines {
            let len: usize = line.spans.iter().map(|s| s.content.chars().count()).sum();
            let rows = if len == 0 {
                1
            } else {
                len.div_ceil(inner_width)
            };
            if cur_rows + rows > max_rows {
                break;
            }
            cur_rows += rows;
            truncated.push(line);
        }
        truncated.push(Line::from(vec![Span::styled(
            "▼ more: press Enter or d to expand details/logs",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )]));
        truncated
    } else {
        let mut padded = lines;
        if total_visual_rows + 2 <= available_height {
            padded.push(Line::raw(""));
            padded.push(Line::styled(
                "Press Enter or d for full details, oracle diff & trace logs",
                Style::default().fg(Color::DarkGray),
            ));
        } else if total_visual_rows < available_height {
            padded.push(Line::styled(
                "Press Enter or d for full details, oracle diff & trace logs",
                Style::default().fg(Color::DarkGray),
            ));
        }
        padded
    };

    let p = Paragraph::new(final_lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Test Overview ")
                .padding(Padding::horizontal(1)),
        )
        .wrap(Wrap { trim: true });

    f.render_widget(p, area);
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

        // 2. GEOMETRIC ORACLE EXPECTED VS ACTUAL
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

        // 3. TRACE DIAGNOSTICS & TIMELINE
        lines.push(Line::styled(
            "--- 2. Compositor Actor Trace Diagnostics ---",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ));
        if let Some(res) = res_opt {
            if let Some(slice) = &res.log_slice {
                let report = TraceAnalyser::new().analyse(slice);
                if report.is_pass() {
                    lines.push(Line::from(vec![
                        Span::styled("✓ Trace Clean: ", Style::default().fg(Color::Green)),
                        Span::raw(
                            "Zero cloak or geometry violations recorded by compositor extension.",
                        ),
                    ]));
                } else {
                    for v in &report.violations {
                        lines.push(Line::styled(
                            format!("  ⚠ {:?}: {}", v.kind, v.message),
                            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                        ));
                    }
                }

                if let Some(ms) = report.cloak_duration_ms {
                    lines.push(Line::from(format!(
                        "Measured Cloak Duration: {:.2}ms (Provisional budget: 100ms)",
                        ms
                    )));
                }

                lines.push(Line::styled(
                    "Trace Event Timeline:",
                    Style::default().fg(Color::DarkGray),
                ));
                for l in slice.lines().take(12) {
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
                for line in res.stderr.lines().take(12) {
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
                for line in res.stdout.lines().take(6) {
                    lines.push(Line::from(format!("  {}", line)));
                }
            }
        } else {
            lines.push(Line::raw("Output not yet available."));
        }
        lines.push(Line::raw(""));

        // 5. HUMAN VISUAL REVIEW
        if let Some(human) = &test.human {
            lines.push(Line::styled(
                "--- 4. Visual Review Watch Prompt ---",
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
                    HumanVerdictKind::Unset => "PENDING",
                })
                .unwrap_or("PENDING");

            lines.push(Line::from(vec![
                Span::styled("Human Verdict: ", Style::default().fg(Color::Yellow)),
                Span::styled(v_str, Style::default().add_modifier(Modifier::BOLD)),
            ]));
            lines.push(Line::raw(""));
        }

        // 6. METADATA
        lines.push(Line::styled(
            "--- 5. Spec Details ---",
            Style::default()
                .fg(Color::Blue)
                .add_modifier(Modifier::BOLD),
        ));
        lines.push(Line::from(format!(
            "Action: {} {:?}",
            test.action.command, test.action.args
        )));
        if let Some(sub) = &test.subject {
            lines.push(Line::from(format!("Subject: {} {:?}", sub.app, sub.args)));
        }
    } else {
        lines.push(Line::raw("No test selected."));
    }

    let total_lines = lines.len();
    let current_line = (app.detail_scroll as usize + 1).min(total_lines.max(1));

    let title = format!(
        " Test Details & Logs (line {}/{} · j/k scroll, Esc/Enter close) ",
        current_line, total_lines
    );

    let detail_widget = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(title)
                .border_style(if is_expanded {
                    Style::default().fg(Color::Cyan)
                } else {
                    Style::default()
                })
                .padding(Padding::horizontal(1)),
        )
        .scroll((app.detail_scroll, 0))
        .wrap(Wrap { trim: true });

    f.render_widget(detail_widget, area);
}

fn draw_footer(f: &mut Frame, app: &TuiApp, area: Rect) {
    let msg = if app.waiting_for_human.is_some() {
        "p: Pass · f: Fail · s: Skip · r: Re-run · Esc: Cancel · q: Quit"
    } else if app.detail_drawer_open {
        "j/k: Scroll · Esc/Enter/d: Close drawer · q: Quit"
    } else {
        "r: Run · a: Run all · Enter/d: Drawer · f: Filter · v: Verdict · q: Quit"
    };

    let footer_widget = Paragraph::new(Line::from(vec![
        Span::styled(
            " Keys: ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(msg, Style::default().fg(Color::White)),
    ]));

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
