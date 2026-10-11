//! Test description pane and line wrapping count helpers.

use crate::runner::{AutoStatus, FinalStatus, HumanVerdictKind};
use crate::schema::TestKind;
use crate::tui::app::{ActivePane, TuiApp};
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Padding, Paragraph, Wrap},
    Frame,
};

/// Computes number of lines required to display wrapped description text within available width.
pub fn count_wrapped_lines(text: &str, max_width: usize) -> usize {
    if max_width == 0 {
        return 1;
    }
    let mut total_lines = 0;
    for raw_line in text.lines() {
        let words: Vec<&str> = raw_line.split_whitespace().collect();
        if words.is_empty() {
            total_lines += 1;
            continue;
        }
        let mut lines = 1;
        let mut current_line_len = 0;
        for word in words {
            let w_len = word.chars().count();
            if current_line_len == 0 {
                current_line_len = w_len;
            } else if current_line_len + 1 + w_len <= max_width {
                current_line_len += 1 + w_len;
            } else {
                lines += 1;
                current_line_len = w_len;
            }
            while current_line_len > max_width {
                lines += 1;
                current_line_len = current_line_len.saturating_sub(max_width);
            }
        }
        total_lines += lines;
    }
    total_lines.max(1)
}

/// Renders the test description and details pane in the middle of the screen.
pub fn draw_details_pane(f: &mut Frame, app: &TuiApp, area: Rect, _is_narrow: bool) {
    let test_opt = app.selected_test();
    let is_focused = app.active_pane == ActivePane::Details;

    let border_color = if is_focused {
        Color::Cyan
    } else {
        Color::DarkGray
    };

    let mut block = Block::default()
        .borders(Borders::ALL)
        .title(" Description ")
        .border_style(Style::default().fg(border_color))
        .padding(Padding::horizontal(1));

    let inner = block.inner(area);
    if inner.height == 0 || inner.width < 10 {
        f.render_widget(block, area);
        return;
    }

    let mut lines = Vec::new();

    if let Some(test) = test_opt {
        let res_opt = app.results.get(&test.id);

        if inner.height == 1 {
            // Minimal height (1 line): show title and ID
            lines.push(Line::from(vec![
                Span::styled(
                    &test.title,
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(" · "),
                Span::styled("ID ", Style::default().fg(Color::DarkGray)),
                Span::styled(&test.id, Style::default().fg(Color::White)),
            ]));
        } else {
            // First line inside: test title in accent color
            lines.push(Line::styled(
                &test.title,
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ));

            let mode_desc = match test.kind {
                TestKind::Auto => "auto · automatic check",
                TestKind::Human => "you · your visual check",
                TestKind::Both => "both · automatic check + your visual review",
            };

            // Blank line following test title
            if inner.height >= 4 {
                lines.push(Line::raw(""));
            }

            // ID line
            lines.push(Line::from(vec![
                Span::styled("ID    ", Style::default().fg(Color::DarkGray)),
                Span::styled(&test.id, Style::default().fg(Color::White)),
            ]));

            // Mode line
            lines.push(Line::from(vec![
                Span::styled("Mode  ", Style::default().fg(Color::DarkGray)),
                Span::styled(mode_desc, Style::default().fg(Color::Gray)),
            ]));
        }

        // Blank line before Description
        lines.push(Line::raw(""));

        // Description
        for desc_line in test.description.lines() {
            lines.push(Line::styled(desc_line, Style::default().fg(Color::Gray)));
        }

        // Blank line before "What to look for:"
        lines.push(Line::raw(""));

        lines.push(Line::styled(
            "What to look for:",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ));
        match test.kind {
            TestKind::Auto => {
                lines.push(Line::styled(
                    "Nothing to watch.",
                    Style::default().fg(Color::DarkGray),
                ));
            }
            TestKind::Human | TestKind::Both => {
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
                        "• Window placed accurately without flicker or jump.",
                        Style::default().fg(Color::White),
                    ));
                }
            }
        }

        // Blank line before Outcome
        lines.push(Line::raw(""));

        lines.push(Line::styled(
            "Outcome:",
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
                AutoStatus::AutoPassUnchecked => Span::styled(
                    "PASS (geometry unchecked)",
                    Style::default()
                        .fg(Color::Yellow)
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

            let auto_sym = match res.auto_status {
                AutoStatus::AutoPass => Span::styled("✔ ", Style::default().fg(Color::Green)),
                AutoStatus::AutoFail | AutoStatus::Error => {
                    Span::styled("✖ ", Style::default().fg(Color::Red))
                }
                AutoStatus::AutoPassUnchecked => {
                    Span::styled("⚠ ", Style::default().fg(Color::Yellow))
                }
                AutoStatus::Running => Span::styled("◐ ", Style::default().fg(Color::Cyan)),
                AutoStatus::Skipped | AutoStatus::Blocked => {
                    Span::styled("⊘ ", Style::default().fg(Color::Magenta))
                }
                _ => Span::styled("● ", Style::default().fg(Color::DarkGray)),
            };

            lines.push(Line::from(vec![
                auto_sym,
                Span::styled("Auto: ", Style::default().fg(Color::White)),
                auto_label,
                Span::styled(
                    format!(" ({} ms)", res.duration_ms),
                    Style::default().fg(Color::DarkGray),
                ),
            ]));

            // Detail line with fixed 2-char symbol slot
            if let Some(msg) = &res.message {
                let is_pass = res.auto_status == AutoStatus::AutoPass
                    && (res.final_status == FinalStatus::Pass
                        || res.final_status == FinalStatus::Pending);
                let is_redundant_pass =
                    is_pass && (msg == "Assertions passed" || msg == "Preflight check passed");
                if !is_redundant_pass {
                    let is_warn = msg.contains("warning")
                        || msg.contains("Warn")
                        || msg.contains("dirty build")
                        || msg.contains("not verified");
                    let detail_style = if is_warn {
                        Style::default().fg(Color::Yellow)
                    } else if res.auto_status == AutoStatus::AutoFail
                        || res.final_status == FinalStatus::Fail
                    {
                        Style::default().fg(Color::LightRed)
                    } else {
                        Style::default().fg(Color::DarkGray)
                    };
                    for msg_line in msg.lines() {
                        lines.push(Line::from(vec![
                            Span::raw("  "),
                            Span::styled("Detail: ", Style::default().fg(Color::DarkGray)),
                            Span::styled(msg_line, detail_style),
                        ]));
                    }
                }
            }

            // Cloak & Trace with fixed 2-char symbol slot
            if let Some(report) = res.trace_report() {
                let trace_part = if report.is_pass() {
                    Span::styled("Trace: 0 violations", Style::default().fg(Color::Green))
                } else {
                    Span::styled(
                        format!("Trace: {} violations", report.violations.len()),
                        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                    )
                };

                if let Some(ms) = report.cloak_duration_ms {
                    lines.push(Line::from(vec![
                        Span::raw("  "),
                        Span::styled(
                            format!("Cloak: {:.1} ms · ", ms),
                            Style::default().fg(Color::White),
                        ),
                        trace_part,
                    ]));
                } else {
                    lines.push(Line::from(vec![Span::raw("  "), trace_part]));
                }
            }

            // Human review outcome with fixed 2-char symbol slot
            if test.kind == TestKind::Human || test.kind == TestKind::Both {
                match res.human_verdict.verdict {
                    HumanVerdictKind::Pass => {
                        lines.push(Line::from(vec![
                            Span::styled("✔ ", Style::default().fg(Color::Green)),
                            Span::styled("You: PASS", Style::default().fg(Color::Green)),
                        ]));
                    }
                    HumanVerdictKind::Fail => {
                        lines.push(Line::from(vec![
                            Span::styled("✖ ", Style::default().fg(Color::Red)),
                            Span::styled("You: FAIL", Style::default().fg(Color::Red)),
                        ]));
                    }
                    HumanVerdictKind::Skip => {
                        lines.push(Line::from(vec![
                            Span::styled("⊘ ", Style::default().fg(Color::DarkGray)),
                            Span::styled("You: SKIPPED", Style::default().fg(Color::DarkGray)),
                        ]));
                    }
                    HumanVerdictKind::Flaky => {
                        lines.push(Line::from(vec![
                            Span::styled("! ", Style::default().fg(Color::Magenta)),
                            Span::styled("You: FLAKY", Style::default().fg(Color::Magenta)),
                        ]));
                    }
                    HumanVerdictKind::Unset => {
                        lines.push(Line::from(vec![
                            Span::styled("? ", Style::default().fg(Color::Yellow)),
                            Span::styled("You: pending review", Style::default().fg(Color::Yellow)),
                        ]));
                    }
                }
            }
        } else {
            lines.push(Line::from(vec![
                Span::styled("● ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    "Not run yet. Press 'r' to execute or 'a' to run all.",
                    Style::default().fg(Color::DarkGray),
                ),
            ]));
        }
    } else {
        lines.push(Line::raw("No test selected."));
    }

    // Clipping and "▼ more" hint
    let total_lines = lines.len();
    let inner_h = inner.height as usize;
    let is_clipped = total_lines > (app.detail_scroll as usize + inner_h);

    if is_clipped {
        block = block.title_bottom(
            Line::from(" ▼ more · d to expand ").style(
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
        );
        let last_visible_idx = (app.detail_scroll as usize + inner_h).saturating_sub(1);
        if last_visible_idx < lines.len() {
            let orig_line = &lines[last_visible_idx];
            let mut spans = orig_line.spans.clone();
            spans.push(Span::styled(
                "   ▼ more · d to expand",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ));
            lines[last_visible_idx] = Line::from(spans);
        }
    }

    let p = Paragraph::new(lines)
        .block(block)
        .scroll((app.detail_scroll, 0))
        .wrap(Wrap { trim: false });

    f.render_widget(p, area);
}
