use super::app::{ActivePane, ModalState, TuiApp};
use crate::runner::{AutoStatus, FinalStatus, HumanVerdictKind, TestResult};
use crate::schema::{TestDefinition, TestKind};
use crate::trace::TraceAnalyser;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Padding, Paragraph, Wrap},
    Frame,
};

/// Main UI drawing entry point.
pub fn draw_ui(f: &mut Frame, app: &TuiApp) {
    let area = f.area();

    // Check minimum terminal dimensions (<60x15 per Item D.5)
    if area.width < 60 || area.height < 15 {
        let msg = "Terminal too small — need at least 60x15";
        let paragraph = Paragraph::new(Line::from(vec![Span::styled(
            msg,
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )]))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" spawn-at verify ")
                .border_style(Style::default().fg(Color::Yellow)),
        );
        f.render_widget(paragraph, area);
        return;
    }

    let footer_height = if app.waiting_for_human.is_some() {
        if area.width >= 75 {
            3
        } else {
            4
        }
    } else if area.width >= 105 || area.width < 75 {
        1
    } else {
        2
    };

    let header_height = if !app.fixture_exists { 2 } else { 1 };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(header_height),
            Constraint::Min(6),
            Constraint::Length(footer_height),
        ])
        .split(area);

    draw_header(f, app, chunks[0]);
    draw_body(f, app, chunks[1]);
    draw_footer(f, app, chunks[2]);

    if let ModalState::Help = &app.modal {
        draw_help_modal(f, app, area);
    } else if let ModalState::HumanVerdict { test_id } = &app.modal {
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
            TestKind::Human | TestKind::Both => {
                if app.waiting_for_human.as_deref() == Some(&t.id) {
                    true
                } else if let Some(r) = app.results.get(&t.id) {
                    r.auto_status != AutoStatus::Pending
                        && r.human_verdict.verdict == HumanVerdictKind::Unset
                } else {
                    false
                }
            }
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
        Span::raw("   "),
        Span::styled("Filter: ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            filter_label,
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("   "),
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
        Span::raw("   "),
        Span::styled(
            format!("{} waiting", waiting_count),
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

fn compute_list_items_count(app: &TuiApp) -> usize {
    let filtered = app.filtered_tests();
    let mut count = 0;
    let mut last_cat: Option<&str> = None;
    let mut cat_idx = 0;

    for test in &filtered {
        let cat = test.category();
        if last_cat != Some(cat) {
            if cat_idx > 0 {
                count += 1; // blank line before category heading
            }
            cat_idx += 1;
            count += 1; // category heading line
            last_cat = Some(cat);
        }
        let is_collapsed = app.collapsed_categories.contains(cat)
            || app.collapsed_categories.contains(get_test_category(cat));
        if !is_collapsed {
            count += 1; // test line
        }
    }
    count
}

fn draw_body(f: &mut Frame, app: &TuiApp, area: Rect) {
    if app.detail_drawer_open {
        // Expanded full-screen detail drawer view
        draw_detail_drawer(f, app, area, true);
        return;
    }

    if area.width < 75 {
        // Narrow (<75 cols): list on top sized to its content height (cap ~60% height),
        // details pane BELOW it using remaining rows.
        let list_items_len = compute_list_items_count(app);
        let max_list_h = (area.height * 60) / 100;
        let list_h = ((list_items_len as u16) + 2).min(max_list_h).max(5);
        let result_h = 1;
        let details_h = area.height.saturating_sub(list_h + result_h);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(list_h),
                Constraint::Length(details_h),
                Constraint::Length(result_h),
            ])
            .split(area);

        draw_test_list(f, app, chunks[0]);
        draw_details_pane(f, app, chunks[1], true);
        draw_result_line(f, app, chunks[2]);
    } else {
        // Wide/medium (>=75 cols): list on the left, overview on the right.
        let (left_pct, right_pct) = if area.width >= 110 {
            (52, 48)
        } else {
            (50, 50)
        };
        let main_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(left_pct),
                Constraint::Percentage(right_pct),
            ])
            .split(area);

        // Left pane: list on top, blank line, Result box at bottom
        let left_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(6),    // List block
                Constraint::Length(1), // Blank line / clear separation
                Constraint::Length(3), // Result box
            ])
            .split(main_chunks[0]);

        draw_test_list(f, app, left_chunks[0]);
        // left_chunks[1] is a clear blank line separating list and result
        draw_result_box(f, app, left_chunks[2]);

        // Right pane: overview panel
        draw_details_pane(f, app, main_chunks[1], false);
    }
}

pub fn get_test_category(cat_or_id: &str) -> &'static str {
    if cat_or_id.starts_with("preflight") || cat_or_id == "basics" {
        "Preflight"
    } else if cat_or_id.starts_with("placement") || cat_or_id == "placement" {
        "Placement"
    } else if cat_or_id.starts_with("cloak") || cat_or_id == "animation-and-cloak" {
        "Cloak & Delayed Paint"
    } else if cat_or_id.starts_with("transform") || cat_or_id == "transforms" {
        "Transforms"
    } else if cat_or_id.starts_with("focus") {
        "Focus"
    } else if cat_or_id.starts_with("query") || cat_or_id == "queries" {
        "Queries"
    } else if cat_or_id.starts_with("robust")
        || cat_or_id.starts_with("resilience")
        || cat_or_id == "robustness"
    {
        "Robustness"
    } else if cat_or_id.starts_with("platform") {
        "Platform"
    } else {
        "General"
    }
}

pub fn pad_str(s: &str, width: usize) -> String {
    let count = s.chars().count();
    if count >= width {
        s.to_string()
    } else {
        format!("{}{}", s, " ".repeat(width - count))
    }
}

pub fn truncate_id_suffix(id: &str, max_len: usize) -> String {
    let count = id.chars().count();
    if count <= max_len {
        id.to_string()
    } else if max_len <= 1 {
        "…".to_string()
    } else {
        let take = max_len - 1;
        let suffix: String = id.chars().rev().take(take).collect();
        let suffix: String = suffix.chars().rev().collect();
        format!("…{}", suffix)
    }
}

pub fn status_symbol_and_color(
    test: &TestDefinition,
    res: Option<&TestResult>,
    is_waiting: bool,
) -> (&'static str, Color) {
    if is_waiting {
        return ("?", Color::Yellow);
    }
    match res {
        None => ("●", Color::DarkGray),
        Some(r) => {
            if r.auto_status == AutoStatus::Running {
                ("◐", Color::Cyan)
            } else if r.final_status == FinalStatus::Fail
                || r.auto_status == AutoStatus::AutoFail
                || r.human_verdict.verdict == HumanVerdictKind::Fail
                || r.final_status == FinalStatus::Error
                || r.auto_status == AutoStatus::Error
            {
                ("✖", Color::Red)
            } else if r.auto_status == AutoStatus::Skipped
                || r.auto_status == AutoStatus::Blocked
                || r.human_verdict.verdict == HumanVerdictKind::Skip
            {
                ("⊘", Color::Magenta)
            } else if (test.kind == TestKind::Both || test.kind == TestKind::Human)
                && r.auto_status == AutoStatus::AutoPass
                && r.human_verdict.verdict == HumanVerdictKind::Unset
            {
                ("?", Color::Yellow)
            } else if r
                .message
                .as_deref()
                .map(|m| {
                    m.contains("dirty build")
                        || m.contains("Preflight warning")
                        || m.contains("warning")
                })
                .unwrap_or(false)
            {
                ("!", Color::Yellow)
            } else if r.final_status == FinalStatus::Pass || r.auto_status == AutoStatus::AutoPass {
                ("✔", Color::Green)
            } else {
                ("●", Color::DarkGray)
            }
        }
    }
}

fn draw_test_list(f: &mut Frame, app: &TuiApp, area: Rect) {
    let is_focused = app.active_pane == ActivePane::List;
    let border_color = if is_focused {
        Color::Cyan
    } else {
        Color::DarkGray
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Tests ")
        .border_style(Style::default().fg(border_color));

    let inner = block.inner(area);
    f.render_widget(block, area);

    if inner.height == 0 || inner.width < 10 {
        return;
    }

    let inner_w = inner.width as usize;
    let filtered = app.filtered_tests();

    let mut items: Vec<ListItem> = Vec::new();
    let mut selected_item_idx: Option<usize> = None;
    let mut last_cat: Option<&str> = None;
    let mut cat_idx = 0;

    for (test_idx, test) in filtered.iter().enumerate() {
        let cat = test.category();
        let cat_display = get_test_category(cat);
        let is_collapsed = app.collapsed_categories.contains(cat)
            || app.collapsed_categories.contains(cat_display);

        // Group tests under this category to compute counts
        let cat_tests: Vec<&&TestDefinition> =
            filtered.iter().filter(|t| t.category() == cat).collect();
        let cat_total = cat_tests.len();
        let cat_passed = cat_tests
            .iter()
            .filter(|t| {
                app.results
                    .get(&t.id)
                    .map(|r| {
                        r.final_status == FinalStatus::Pass || r.auto_status == AutoStatus::AutoPass
                    })
                    .unwrap_or(false)
            })
            .count();

        if last_cat != Some(cat) {
            last_cat = Some(cat);
            // One blank line before each category heading (for subsequent categories)
            if cat_idx > 0 {
                items.push(ListItem::new(Line::from("")));
            }
            cat_idx += 1;

            // Category heading = one full-width row, uppercase or accent colour, count right-aligned ("0/4"), NO triangle symbol.
            // If a category is folded show "(4 hidden)" instead.
            let cat_heading_text = format!("  {}", cat_display.to_uppercase());
            let right_text = if is_collapsed {
                format!("({} hidden) ", cat_total)
            } else {
                format!("{}/{} ", cat_passed, cat_total)
            };

            let fill = inner_w
                .saturating_sub(cat_heading_text.chars().count() + right_text.chars().count());
            let heading_line = Line::from(vec![
                Span::styled(
                    cat_heading_text,
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(" ".repeat(fill)),
                Span::styled(
                    right_text,
                    Style::default()
                        .fg(if is_collapsed {
                            Color::Yellow
                        } else {
                            Color::DarkGray
                        })
                        .add_modifier(Modifier::BOLD),
                ),
            ]);
            items.push(ListItem::new(heading_line));
        }

        if is_collapsed {
            // Folded: test rows hidden
            continue;
        }

        let is_selected = test_idx == app.selected_index;
        if is_selected {
            selected_item_idx = Some(items.len());
        }

        let cursor_str = if is_selected { "> " } else { "  " };

        let res = app.results.get(&test.id);
        let is_waiting = app.waiting_for_human.as_deref() == Some(&test.id);
        let (status_sym, status_color) = status_symbol_and_color(test, res, is_waiting);

        let short_name = test.id.rsplit('/').next().unwrap_or(&test.id);

        let kind_str = match test.kind {
            TestKind::Auto => "auto",
            TestKind::Both => "both",
            TestKind::Human => "human",
        };
        let kind_color = match test.kind {
            TestKind::Auto => Color::DarkGray,
            TestKind::Both => Color::Cyan,
            TestKind::Human => Color::Yellow,
        };

        let kind_len = kind_str.chars().count();
        let right_margin = 1;
        let min_gap = 2; // Guaranteed at least 2 spaces between name and kind
        let fixed_overhead = 2 + 1 + 2 + min_gap + kind_len + right_margin;
        let max_name_len = inner_w.saturating_sub(fixed_overhead);

        let (disp_name, name_len) = if short_name.chars().count() > max_name_len {
            if max_name_len <= 1 {
                ("…".to_string(), 1)
            } else {
                let truncated: String = short_name.chars().take(max_name_len - 1).collect();
                (format!("{}…", truncated), max_name_len)
            }
        } else {
            (short_name.to_string(), short_name.chars().count())
        };

        let left_used = 2 + 1 + 2 + name_len;
        let right_used = kind_len + right_margin;
        let flex_spaces = inner_w.saturating_sub(left_used + right_used);

        let row_line = Line::from(vec![
            Span::styled(
                cursor_str,
                if is_selected {
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                },
            ),
            Span::styled(
                status_sym,
                Style::default()
                    .fg(status_color)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::styled(
                disp_name,
                if is_selected {
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White)
                },
            ),
            Span::raw(" ".repeat(flex_spaces)),
            Span::styled(kind_str, Style::default().fg(kind_color)),
            Span::raw(" "),
        ]);

        items.push(ListItem::new(row_line));
    }

    let list_widget = List::new(items).highlight_style(
        Style::default()
            .bg(Color::DarkGray)
            .add_modifier(Modifier::BOLD),
    );

    let mut state = ListState::default();
    state.select(selected_item_idx);
    f.render_stateful_widget(list_widget, inner, &mut state);
}

fn draw_details_pane(f: &mut Frame, app: &TuiApp, area: Rect, _is_narrow: bool) {
    let test_opt = app.selected_test();
    let is_focused = app.active_pane == ActivePane::Details;

    let border_color = if is_focused {
        Color::Cyan
    } else {
        Color::DarkGray
    };

    let title_text = if let Some(test) = test_opt {
        format!(" {} ", test.title)
    } else {
        " Overview ".to_string()
    };

    let mut block = Block::default()
        .borders(Borders::ALL)
        .title(title_text)
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

        // ID line
        lines.push(Line::from(vec![
            Span::styled("ID    ", Style::default().fg(Color::DarkGray)),
            Span::styled(&test.id, Style::default().fg(Color::White)),
        ]));

        // Kind line
        let kind_desc = match test.kind {
            TestKind::Auto => "auto — verified by math & trace",
            TestKind::Human => "you — visual review only",
            TestKind::Both => "both — auto + visual review",
        };
        lines.push(Line::from(vec![
            Span::styled("Kind  ", Style::default().fg(Color::DarkGray)),
            Span::styled(kind_desc, Style::default().fg(Color::Gray)),
        ]));

        lines.push(Line::raw(""));

        // Description: NEVER drop the description on small screens
        for desc_line in test.description.lines() {
            lines.push(Line::styled(desc_line, Style::default().fg(Color::Gray)));
        }

        match test.kind {
            TestKind::Auto => {
                lines.push(Line::from(vec![
                    Span::styled(
                        "What to look for: ",
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled("Nothing to watch.", Style::default().fg(Color::DarkGray)),
                ]));
            }
            TestKind::Human | TestKind::Both => {
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
                        "• Window placed accurately without flicker or jump.",
                        Style::default().fg(Color::White),
                    ));
                }
            }
        }

        lines.push(Line::raw(""));

        // Outcome
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

            // Detail line
            if let Some(msg) = &res.message {
                let is_pass = res.auto_status == AutoStatus::AutoPass
                    && (res.final_status == FinalStatus::Pass
                        || res.final_status == FinalStatus::Pending);
                let is_redundant_pass =
                    is_pass && (msg == "Assertions passed" || msg == "Preflight check passed");
                if !is_redundant_pass {
                    let is_warn = msg.contains("warning")
                        || msg.contains("Warn")
                        || msg.contains("dirty build");
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
                            Span::styled("Detail: ", Style::default().fg(Color::DarkGray)),
                            Span::styled(msg_line, detail_style),
                        ]));
                    }
                }
            }

            // Cloak & Trace
            if let Some(slice) = &res.log_slice {
                let report = TraceAnalyser::new().analyse(slice);
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
                        Span::styled(
                            format!("Cloak: {:.1} ms · ", ms),
                            Style::default().fg(Color::White),
                        ),
                        trace_part,
                    ]));
                } else {
                    lines.push(Line::from(vec![trace_part]));
                }
            }

            // Human review outcome
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
            lines.push(Line::styled(
                "● Not run yet. Press 'r' to execute or 'a' to run all.",
                Style::default().fg(Color::DarkGray),
            ));
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
            Line::from(" ▼ more ").style(
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
                "   ▼ more",
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
        .wrap(Wrap { trim: true });

    f.render_widget(p, area);
}

fn draw_result_line(f: &mut Frame, app: &TuiApp, area: Rect) {
    let test_opt = app.selected_test();
    let res_opt = test_opt.and_then(|t| app.results.get(&t.id));

    let line = if let (Some(test), Some(res)) = (test_opt, res_opt) {
        if app.waiting_for_human.as_deref() == Some(&test.id) {
            let what = test
                .human
                .as_ref()
                .map(|h| h.watch.as_str())
                .unwrap_or("Window placed accurately");
            Line::from(vec![
                Span::styled(
                    "  ? What to look for: ",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(what, Style::default().fg(Color::White)),
            ])
        } else if res.final_status == FinalStatus::Fail || res.auto_status == AutoStatus::AutoFail {
            let reason = res.message.as_deref().unwrap_or("Failed");
            Line::from(vec![
                Span::styled(
                    "  ✖ Result: ",
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("FAIL - {}", reason),
                    Style::default().fg(Color::LightRed),
                ),
            ])
        } else if res.final_status == FinalStatus::Pass || res.auto_status == AutoStatus::AutoPass {
            Line::from(vec![
                Span::styled(
                    "  ✔ Result: ",
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("PASS ({} ms)", res.duration_ms),
                    Style::default().fg(Color::Green),
                ),
            ])
        } else if res.auto_status == AutoStatus::Running {
            Line::from(vec![
                Span::styled(
                    "  ◐ Result: ",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled("Running...", Style::default().fg(Color::Cyan)),
            ])
        } else {
            Line::from(vec![
                Span::styled("  ● Result: ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    "not run yet.  Press r to run, a for all.",
                    Style::default().fg(Color::DarkGray),
                ),
            ])
        }
    } else {
        Line::from(vec![
            Span::styled("  ✔ Result: ", Style::default().fg(Color::Green)),
            Span::styled(
                "not run yet.  Press r to run, a for all.",
                Style::default().fg(Color::White),
            ),
        ])
    };

    let p = Paragraph::new(line);
    f.render_widget(p, area);
}

fn draw_result_box(f: &mut Frame, app: &TuiApp, area: Rect) {
    let test_opt = app.selected_test();
    let res_opt = test_opt.and_then(|t| app.results.get(&t.id));

    let (line, border_color) = if let (Some(test), Some(res)) = (test_opt, res_opt) {
        if app.waiting_for_human.as_deref() == Some(&test.id) {
            let what = test
                .human
                .as_ref()
                .map(|h| h.watch.as_str())
                .unwrap_or("Window placed accurately");
            (
                Line::from(vec![
                    Span::styled(
                        "? What to look for: ",
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(what, Style::default().fg(Color::White)),
                ]),
                Color::Yellow,
            )
        } else if res.final_status == FinalStatus::Fail || res.auto_status == AutoStatus::AutoFail {
            let reason = res.message.as_deref().unwrap_or("Verification failed");
            (
                Line::from(vec![
                    Span::styled(
                        "✖ FAIL: ",
                        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(reason, Style::default().fg(Color::LightRed)),
                ]),
                Color::Red,
            )
        } else if res.final_status == FinalStatus::Pass || res.auto_status == AutoStatus::AutoPass {
            (
                Line::from(vec![
                    Span::styled(
                        "✔ PASS: ",
                        Style::default()
                            .fg(Color::Green)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        format!("Assertions passed ({} ms)", res.duration_ms),
                        Style::default().fg(Color::White),
                    ),
                ]),
                Color::Green,
            )
        } else if res.auto_status == AutoStatus::Running {
            (
                Line::from(vec![
                    Span::styled("◐ Running: ", Style::default().fg(Color::Cyan)),
                    Span::styled("Executing assertions...", Style::default().fg(Color::Cyan)),
                ]),
                Color::Cyan,
            )
        } else {
            (
                Line::from(vec![
                    Span::styled("● Not run yet. ", Style::default().fg(Color::DarkGray)),
                    Span::styled(
                        "Press 'r' to execute or 'a' to run all.",
                        Style::default().fg(Color::DarkGray),
                    ),
                ]),
                Color::DarkGray,
            )
        }
    } else {
        (
            Line::from(vec![
                Span::styled("● Not run yet. ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    "Press 'r' to execute or 'a' to run all.",
                    Style::default().fg(Color::DarkGray),
                ),
            ]),
            Color::DarkGray,
        )
    };

    let p = Paragraph::new(line).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Result ")
            .border_style(Style::default().fg(border_color))
            .padding(Padding::horizontal(1)),
    );
    f.render_widget(p, area);
}

fn draw_detail_drawer(f: &mut Frame, app: &TuiApp, area: Rect, is_expanded: bool) {
    let test_opt = app.selected_test();

    let mut lines = Vec::new();

    if let Some(test) = test_opt {
        let res_opt = app.results.get(&test.id);

        // Header info in drawer
        lines.push(Line::styled(
            &test.title,
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ));
        lines.push(Line::from(vec![
            Span::styled("ID: ", Style::default().fg(Color::DarkGray)),
            Span::styled(&test.id, Style::default().fg(Color::White)),
            Span::styled(" · Kind: ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{:?}", test.kind), Style::default().fg(Color::Gray)),
        ]));
        lines.push(Line::raw(""));

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
                    let actual_color = if res.auto_status == AutoStatus::AutoFail {
                        Color::LightRed
                    } else {
                        Color::Green
                    };
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
                            Style::default().fg(actual_color),
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

        // 4. CAPTURED STDOUT / STDERR (No red on passing tests! Item A.1)
        lines.push(Line::styled(
            "--- 3. Captured Output (Stdout & Stderr) ---",
            Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
        ));
        if let Some(res) = res_opt {
            if !res.stderr.is_empty() {
                let stderr_color = if res.final_status == FinalStatus::Fail
                    || res.auto_status == AutoStatus::AutoFail
                {
                    Color::LightRed
                } else {
                    Color::DarkGray
                };
                lines.push(Line::styled(
                    "Stderr Output:",
                    Style::default().fg(stderr_color),
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

            let (v_str, v_color) = res_opt
                .map(|r| match r.human_verdict.verdict {
                    HumanVerdictKind::Pass => ("PASS", Color::Green),
                    HumanVerdictKind::Fail => ("FAIL", Color::Red),
                    HumanVerdictKind::Skip => ("SKIP", Color::DarkGray),
                    HumanVerdictKind::Flaky => ("FLAKY", Color::Magenta),
                    HumanVerdictKind::Unset => ("PENDING", Color::Yellow),
                })
                .unwrap_or(("PENDING", Color::Yellow));

            lines.push(Line::from(vec![
                Span::styled("Human Verdict: ", Style::default().fg(Color::Yellow)),
                Span::styled(
                    v_str,
                    Style::default().fg(v_color).add_modifier(Modifier::BOLD),
                ),
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
    if app.waiting_for_human.is_some() {
        // While waiting for a human verdict, replace the bar with a highlighted BOX (accent border) that says:
        // "Your verdict:  p Pass   x Fail   s Skip   k Flaky   r Re-run   Esc Cancel".
        // This is the only thing the bottom shows in that state.
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )
            .padding(Padding::horizontal(1));

        let lines = if area.width >= 75 {
            vec![Line::from(vec![
                Span::styled(
                    "Your verdict:  ",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    "p",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(" Pass   "),
                Span::styled(
                    "x",
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                ),
                Span::raw(" Fail   "),
                Span::styled(
                    "s",
                    Style::default()
                        .fg(Color::DarkGray)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(" Skip   "),
                Span::styled(
                    "k",
                    Style::default()
                        .fg(Color::Magenta)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(" Flaky   "),
                Span::styled(
                    "r",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(" Re-run   "),
                Span::styled(
                    "Esc",
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(" Cancel"),
            ])]
        } else {
            // Wrapped at item boundaries, never cut mid-word!
            vec![
                Line::from(vec![
                    Span::styled(
                        "Your verdict:  ",
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        "p",
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(" Pass   "),
                    Span::styled(
                        "x",
                        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(" Fail   "),
                    Span::styled(
                        "s",
                        Style::default()
                            .fg(Color::DarkGray)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(" Skip"),
                ]),
                Line::from(vec![
                    Span::styled(
                        "k",
                        Style::default()
                            .fg(Color::Magenta)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(" Flaky   "),
                    Span::styled(
                        "r",
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(" Re-run   "),
                    Span::styled(
                        "Esc",
                        Style::default()
                            .fg(Color::White)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(" Cancel"),
                ]),
            ]
        };

        let p = Paragraph::new(lines).block(block);
        f.render_widget(p, area);
    } else {
        // Normal keys bar
        let lines = if area.width < 75 {
            // Narrow: essential keys plus "? Help"
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
                    "Tab",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(" Panel  "),
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
            ])]
        } else if area.width >= 105 {
            // Wide single line wrapped at group boundaries
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
                    "R",
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
            // Medium 2 lines wrapped at group boundaries, never cut mid-word
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
                        "R",
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
                    Span::raw(" Fold"),
                ]),
                Line::from(vec![
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
                ]),
            ]
        };

        let p = Paragraph::new(lines);
        f.render_widget(p, area);
    }
}

fn draw_help_modal(f: &mut Frame, _app: &TuiApp, area: Rect) {
    let popup_area = centered_rect(68, 85, area);
    f.render_widget(Clear, popup_area);

    let lines = vec![
        Line::styled(
            "KEYS & COMMANDS",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Line::from(vec![
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
                "R",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Run category"),
        ]),
        Line::from(vec![
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
            Span::raw(" Fold/unfold"),
        ]),
        Line::from(vec![
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
                "C",
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
        ]),
        Line::from(vec![
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
        ]),
        Line::from(vec![
            Span::styled("  Quit:      ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                "q",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Quit · "),
            Span::styled(
                "Ctrl+C",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Emergency quit & kill test windows"),
        ]),
        Line::raw(""),
        Line::styled(
            "STATUS SYMBOLS",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Line::from(vec![
            Span::styled("  ● ", Style::default().fg(Color::DarkGray)),
            Span::raw("Not run   "),
            Span::styled("◐ ", Style::default().fg(Color::Cyan)),
            Span::raw("Running   "),
            Span::styled("✔ ", Style::default().fg(Color::Green)),
            Span::raw("Pass   "),
            Span::styled("✖ ", Style::default().fg(Color::Red)),
            Span::raw("Fail"),
        ]),
        Line::from(vec![
            Span::styled("  ! ", Style::default().fg(Color::Yellow)),
            Span::raw("Warning   "),
            Span::styled("? ", Style::default().fg(Color::Yellow)),
            Span::raw("Waiting for you   "),
            Span::styled("⊘ ", Style::default().fg(Color::Magenta)),
            Span::raw("Skipped / Blocked"),
        ]),
        Line::raw(""),
        Line::styled(
            "KINDS LEGEND",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Line::from(vec![
            Span::styled(
                "  auto: ",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("Verified automatically by math & trace"),
        ]),
        Line::from(vec![
            Span::styled(
                "  human:",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Visual review only"),
        ]),
        Line::from(vec![
            Span::styled(
                "  both: ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("Automated assertions + visual review prompt"),
        ]),
        Line::raw(""),
        Line::styled(
            "Press Esc or ? or q to close",
            Style::default().fg(Color::DarkGray),
        ),
    ];

    let p = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Help · Keys & Legend ")
            .border_style(Style::default().fg(Color::Cyan))
            .padding(Padding::horizontal(1)),
    );
    f.render_widget(p, popup_area);
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
                " [X] Fail ",
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
