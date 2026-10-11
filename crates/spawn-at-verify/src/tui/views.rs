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
use unicode_width::UnicodeWidthStr;

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

    let footer_height = if app.detail_drawer_open || app.waiting_for_human.is_some() {
        if area.width > 80 {
            3
        } else {
            4
        }
    } else if area.width >= 105 {
        1
    } else {
        2
    };

    let header_height = if !app.fixture_exists { 2 } else { 1 };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),             // 1 blank row above the header
            Constraint::Length(header_height), // Header
            Constraint::Length(1),             // 1 blank row between header and first box
            Constraint::Min(5),                // Body (boxes)
            Constraint::Length(1),             // 1 blank row between Result box and keys bar
            Constraint::Length(footer_height), // Keys bar or verdict box
            Constraint::Length(1),             // 1 blank row at the BOTTOM below keys bar
        ])
        .split(area);

    draw_header(f, app, chunks[1]);
    draw_body(f, app, chunks[3]);
    draw_footer(f, app, chunks[5]);

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
    let is_waiting = |t: &TestDefinition| -> bool {
        match t.kind {
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
        }
    };

    let waiting_count = all_filtered.iter().filter(|t| is_waiting(t)).count();

    let done_count = all_filtered
        .iter()
        .filter(|t| {
            if is_waiting(t) {
                return false;
            }
            app.results
                .get(&t.id)
                .map(|r| r.final_status != FinalStatus::Pending)
                .unwrap_or(false)
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

fn get_result_content(app: &TuiApp) -> (String, Line<'static>, Color) {
    let test_opt = app.selected_test();
    let res_opt = test_opt.and_then(|t| app.results.get(&t.id));

    if let (Some(test), Some(res)) = (test_opt, res_opt) {
        if app.waiting_for_human.as_deref() == Some(&test.id) {
            let what = test
                .human
                .as_ref()
                .map(|h| h.watch.clone())
                .unwrap_or_else(|| "Window placed accurately".to_string());
            let plain = format!("? What to look for: {}", what);
            let line = Line::from(vec![
                Span::styled(
                    "? What to look for: ",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(what, Style::default().fg(Color::White)),
            ]);
            (plain, line, Color::Yellow)
        } else if res.final_status == FinalStatus::Fail || res.auto_status == AutoStatus::AutoFail {
            let reason = res
                .message
                .clone()
                .unwrap_or_else(|| "Verification failed".to_string());
            let plain = format!("✖ FAIL: {}", reason);
            let line = Line::from(vec![
                Span::styled(
                    "✖ FAIL: ",
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                ),
                Span::styled(reason, Style::default().fg(Color::LightRed)),
            ]);
            (plain, line, Color::Red)
        } else if res.final_status == FinalStatus::Pass || res.auto_status == AutoStatus::AutoPass {
            let msg = format!("Assertions passed ({} ms)", res.duration_ms);
            let plain = format!("✔ PASS: {}", msg);
            let line = Line::from(vec![
                Span::styled(
                    "✔ PASS: ",
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(msg, Style::default().fg(Color::White)),
            ]);
            (plain, line, Color::Green)
        } else if res.auto_status == AutoStatus::Running {
            let plain = "◐ Running: Executing assertions...".to_string();
            let line = Line::from(vec![
                Span::styled("◐ Running: ", Style::default().fg(Color::Cyan)),
                Span::styled("Executing assertions...", Style::default().fg(Color::Cyan)),
            ]);
            (plain, line, Color::Cyan)
        } else {
            let plain = "● Not run yet. Press 'r' to execute or 'a' to run all.".to_string();
            let line = Line::from(vec![
                Span::styled("● Not run yet. ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    "Press 'r' to execute or 'a' to run all.",
                    Style::default().fg(Color::DarkGray),
                ),
            ]);
            (plain, line, Color::DarkGray)
        }
    } else {
        let plain = "● Not run yet. Press 'r' to execute or 'a' to run all.".to_string();
        let line = Line::from(vec![
            Span::styled("● Not run yet. ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                "Press 'r' to execute or 'a' to run all.",
                Style::default().fg(Color::DarkGray),
            ),
        ]);
        (plain, line, Color::DarkGray)
    }
}

pub fn compute_result_box_height(app: &TuiApp, body_h: u16, box_w: u16) -> u16 {
    let (plain, _, _) = get_result_content(app);
    let inner_w = (box_w.saturating_sub(4) as usize).max(1);
    let wrapped_rows = count_wrapped_lines(&plain, inner_w);
    let needed_box_h = (wrapped_rows as u16).saturating_add(2);
    let max_box_h = ((body_h as u32 * 40) / 100).max(3) as u16;
    needed_box_h.clamp(3, max_box_h)
}

fn draw_body(f: &mut Frame, app: &TuiApp, area: Rect) {
    if app.detail_drawer_open {
        // Expanded full-screen detail drawer view
        draw_detail_drawer(f, app, area, true);
        return;
    }

    let needed_list_h = ((compute_list_items_count(app) + 1) as u16) + 2;

    if area.width < 75 {
        // Narrow (<75 cols):
        // Stack: Tests (content height, capped) / GAP / Description (flexible) / GAP / Result (wrapped height)
        let min_pane_h = 3;
        let mut result_h = compute_result_box_height(app, area.height, area.width);
        let max_result_h = area.height.saturating_sub(2 + min_pane_h + min_pane_h);
        if max_result_h >= min_pane_h {
            result_h = result_h.min(max_result_h).max(min_pane_h);
        }
        let avail_for_panes = area.height.saturating_sub(2 + result_h);
        let max_list_h = avail_for_panes
            .saturating_sub(min_pane_h)
            .min((avail_for_panes * 50) / 100)
            .max(min_pane_h);
        let list_h = needed_list_h.min(max_list_h).max(min_pane_h);
        let details_h = avail_for_panes.saturating_sub(list_h);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(list_h),
                Constraint::Length(1), // GAP between Tests and Description
                Constraint::Length(details_h),
                Constraint::Length(1), // GAP between Description and Result
                Constraint::Length(result_h),
            ])
            .split(area);

        draw_test_list(f, app, chunks[0]);
        draw_details_pane(f, app, chunks[2], true);
        draw_result_box(f, app, chunks[4]);
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

        // Wide layout: left column = Tests box (takes all remaining height) and Result box pinned to the bottom of the column.
        // When Result grows, the Tests box shrinks and its list scrolls with "▲/▼ n more" hints.
        // No empty band between the list box and Result.
        let result_h = compute_result_box_height(app, area.height, main_chunks[0].width);
        let tests_h = area.height.saturating_sub(1 + result_h);

        let left_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(tests_h),
                Constraint::Length(1), // 1 blank row GAP between Tests and Result
                Constraint::Length(result_h), // Result box pinned to bottom
            ])
            .split(main_chunks[0]);

        draw_test_list(f, app, left_chunks[0]);
        draw_result_box(f, app, left_chunks[2]);

        // Right pane: overview / Description panel takes full height of the body
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
    let mut block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));

    let inner = block.inner(area);

    if inner.height == 0 || inner.width < 10 {
        f.render_widget(block, area);
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

        let mode_str = match test.kind {
            TestKind::Auto => "auto",
            TestKind::Both => "both",
            TestKind::Human => "you",
        };
        let mode_color = match test.kind {
            TestKind::Auto => Color::DarkGray,
            TestKind::Both => Color::Cyan,
            TestKind::Human => Color::Yellow,
        };

        let mode_len = mode_str.chars().count();
        let right_margin = 1;
        let min_gap = 2; // Guaranteed at least 2 spaces between name and mode
        let fixed_overhead = 2 + 1 + 2 + min_gap + mode_len + right_margin;
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
        let right_used = mode_len + right_margin;
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
            Span::styled(mode_str, Style::default().fg(mode_color)),
            Span::raw(" "),
        ]);

        items.push(ListItem::new(row_line));
    }

    let total_items = items.len();
    let inner_h = inner.height as usize;
    let sel = selected_item_idx.unwrap_or(0);

    let offset = if total_items > inner_h {
        if sel >= inner_h {
            sel + 1 - inner_h
        } else {
            0
        }
    } else {
        0
    };

    let items_above = offset;
    let items_below = total_items.saturating_sub(offset + inner_h);

    let top_title = if items_above > 0 {
        format!(" Tests  ▲ {} more ", items_above)
    } else {
        " Tests ".to_string()
    };
    block = block.title(top_title);
    if items_below > 0 {
        block = block.title_bottom(
            Line::from(format!(" ▼ {} more ", items_below)).style(
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
        );
    }

    f.render_widget(block, area);

    let list_widget = List::new(items).highlight_style(
        Style::default()
            .bg(Color::DarkGray)
            .add_modifier(Modifier::BOLD),
    );

    let mut state = ListState::default();
    state.select(selected_item_idx);
    *state.offset_mut() = offset;
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
                            Span::raw("  "),
                            Span::styled("Detail: ", Style::default().fg(Color::DarkGray)),
                            Span::styled(msg_line, detail_style),
                        ]));
                    }
                }
            }

            // Cloak & Trace with fixed 2-char symbol slot
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

fn draw_result_box(f: &mut Frame, app: &TuiApp, area: Rect) {
    let (plain, line, border_color) = get_result_content(app);

    let mut block = Block::default()
        .borders(Borders::ALL)
        .title(" Result ")
        .border_style(Style::default().fg(border_color))
        .padding(Padding::horizontal(1));

    let inner = block.inner(area);
    let inner_w = inner.width as usize;
    let wrapped_rows = count_wrapped_lines(&plain, inner_w.max(1));

    if wrapped_rows > inner.height as usize {
        block = block.title_bottom(
            Line::from(" ▼ more · d to expand ").style(
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
        );
    }

    let p = Paragraph::new(line).block(block).wrap(Wrap { trim: false });

    f.render_widget(p, area);
}

pub fn build_drawer_content(
    app: &TuiApp,
    test: &TestDefinition,
    width: usize,
    compact: bool,
) -> (Vec<Line<'static>>, String) {
    let mut builder = DrawerBuilder::new(width);
    let res_opt = app.results.get(&test.id);

    // 1 blank row of padding under the top border
    builder.push_blank();

    // Labelled Header: Title, ID, Mode, Description in that order
    // Title is the most significant row: bold, brightest White, followed by exactly 1 blank row
    let norm_title = normalise_dimensions(&test.title);
    builder.push_kv(
        "Title",
        vec![Span::styled(
            norm_title.clone(),
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )],
        &norm_title,
    );
    builder.push_blank();

    // ID
    builder.push_kv(
        "ID",
        vec![Span::styled(
            test.id.clone(),
            Style::default().fg(Color::Rgb(200, 200, 200)),
        )],
        &test.id,
    );

    // Mode: <auto|you|both> · <meaning>
    let (mode_key, mode_meaning) = match test.kind {
        TestKind::Auto => ("auto", "automatic check"),
        TestKind::Human => ("you", "your visual check"),
        TestKind::Both => ("both", "automatic check + your visual review"),
    };
    let mode_val = format!("{} · {}", mode_key, mode_meaning);
    builder.push_kv(
        "Mode",
        vec![Span::styled(
            mode_val.clone(),
            Style::default().fg(Color::Rgb(200, 200, 200)),
        )],
        &mode_val,
    );

    // Description
    let desc_norm = normalise_dimensions(test.description.trim());
    if desc_norm.is_empty() {
        builder.push_kv(
            "Description",
            vec![Span::styled(
                "(none)",
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::ITALIC),
            )],
            "(none)",
        );
    } else {
        builder.push_wrapped_kv(
            "Description",
            &desc_norm,
            Style::default().fg(Color::Rgb(200, 200, 200)),
        );
    }

    // Exactly 1 blank row between header and RESULT
    builder.push_blank();

    // 1. Section: RESULT
    builder.push_heading("RESULT", None);
    let (v_spans, v_plain): (Vec<Span<'static>>, String) = if let Some(r) = res_opt {
        match r.human_verdict.verdict {
            HumanVerdictKind::Pass => (
                vec![
                    Span::styled(
                        "✔ ",
                        Style::default()
                            .fg(Color::Green)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        "Pass (marked by you)",
                        Style::default().fg(Color::Rgb(200, 200, 200)),
                    ),
                ],
                "✔ Pass (marked by you)".into(),
            ),
            HumanVerdictKind::Fail => (
                vec![
                    Span::styled(
                        "✖ ",
                        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        "Fail (marked by you)",
                        Style::default().fg(Color::Rgb(200, 200, 200)),
                    ),
                ],
                "✖ Fail (marked by you)".into(),
            ),
            HumanVerdictKind::Flaky => (
                vec![
                    Span::styled(
                        "! ",
                        Style::default()
                            .fg(Color::Magenta)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        "Flaky (marked by you)",
                        Style::default().fg(Color::Rgb(200, 200, 200)),
                    ),
                ],
                "! Flaky (marked by you)".into(),
            ),
            HumanVerdictKind::Skip => (
                vec![
                    Span::styled(
                        "- ",
                        Style::default()
                            .fg(Color::DarkGray)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        "Skip (marked by you)",
                        Style::default().fg(Color::Rgb(200, 200, 200)),
                    ),
                ],
                "- Skip (marked by you)".into(),
            ),
            HumanVerdictKind::Unset => {
                if test.kind == TestKind::Auto {
                    match r.final_status {
                        FinalStatus::Pass => (
                            vec![
                                Span::styled(
                                    "✔ ",
                                    Style::default()
                                        .fg(Color::Green)
                                        .add_modifier(Modifier::BOLD),
                                ),
                                Span::styled(
                                    "Pass",
                                    Style::default().fg(Color::Rgb(200, 200, 200)),
                                ),
                            ],
                            "✔ Pass".into(),
                        ),
                        FinalStatus::Fail => (
                            vec![
                                Span::styled(
                                    "✖ ",
                                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                                ),
                                Span::styled(
                                    "Fail",
                                    Style::default().fg(Color::Rgb(200, 200, 200)),
                                ),
                            ],
                            "✖ Fail".into(),
                        ),
                        _ => (
                            vec![
                                Span::styled(
                                    "◐ ",
                                    Style::default()
                                        .fg(Color::Yellow)
                                        .add_modifier(Modifier::BOLD),
                                ),
                                Span::styled(
                                    "Pending",
                                    Style::default().fg(Color::Rgb(200, 200, 200)),
                                ),
                            ],
                            "◐ Pending".into(),
                        ),
                    }
                } else {
                    (
                        vec![
                            Span::styled(
                                "◐ ",
                                Style::default()
                                    .fg(Color::Yellow)
                                    .add_modifier(Modifier::BOLD),
                            ),
                            Span::styled("Pending", Style::default().fg(Color::Rgb(200, 200, 200))),
                        ],
                        "◐ Pending".into(),
                    )
                }
            }
        }
    } else {
        (
            vec![
                Span::styled(
                    "◐ ",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    "Pending",
                    Style::default()
                        .fg(Color::DarkGray)
                        .add_modifier(Modifier::ITALIC),
                ),
            ],
            "◐ Pending".into(),
        )
    };
    builder.push_kv("Verdict", v_spans, &v_plain);

    let trace_report = res_opt
        .and_then(|r| r.log_slice.as_ref())
        .map(|s| TraceAnalyser::new().analyse(s));

    let cloak_ms = trace_report
        .as_ref()
        .and_then(|rep| rep.cloak_duration_ms)
        .or_else(|| {
            res_opt.and_then(|r| {
                if r.duration_ms > 0 {
                    Some(r.duration_ms as f64)
                } else {
                    None
                }
            })
        });

    let (c_spans, c_plain): (Vec<Span<'static>>, String) = if let Some(dur) = cloak_ms {
        (
            vec![Span::styled(
                format!("{:.1} ms", dur),
                Style::default().fg(Color::Rgb(200, 200, 200)),
            )],
            format!("{:.1} ms", dur),
        )
    } else {
        (
            vec![Span::styled(
                "not measured",
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::ITALIC),
            )],
            "not measured".into(),
        )
    };
    builder.push_kv("Cloak", c_spans, &c_plain);

    // Budget: 100 ms (provisional)   ✔ within budget | ! over provisional budget
    let (b_spans, b_plain): (Vec<Span<'static>>, String) = if let Some(dur) = cloak_ms {
        if dur <= 100.0 {
            (
                vec![
                    Span::styled(
                        "100 ms (provisional)   ",
                        Style::default().fg(Color::Rgb(200, 200, 200)),
                    ),
                    Span::styled(
                        "✔ ",
                        Style::default()
                            .fg(Color::Green)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled("within budget", Style::default().fg(Color::Green)),
                ],
                "100 ms (provisional)   ✔ within budget".into(),
            )
        } else {
            (
                vec![
                    Span::styled(
                        "100 ms (provisional)   ",
                        Style::default().fg(Color::Rgb(200, 200, 200)),
                    ),
                    Span::styled(
                        "! ",
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        "over provisional budget",
                        Style::default().fg(Color::Yellow),
                    ),
                ],
                "100 ms (provisional)   ! over provisional budget".into(),
            )
        }
    } else {
        (
            vec![
                Span::styled(
                    "100 ms (provisional)   ",
                    Style::default().fg(Color::Rgb(200, 200, 200)),
                ),
                Span::styled(
                    "not measured",
                    Style::default()
                        .fg(Color::DarkGray)
                        .add_modifier(Modifier::ITALIC),
                ),
            ],
            "100 ms (provisional)   not measured".into(),
        )
    };
    builder.push_kv("Budget", b_spans, &b_plain);

    let (t_spans, t_plain): (Vec<Span<'static>>, String) = if let Some(ref rep) = trace_report {
        if rep.is_pass() {
            (
                vec![
                    Span::styled(
                        "✔ ",
                        Style::default()
                            .fg(Color::Green)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        "clean, no violations",
                        Style::default().fg(Color::Rgb(200, 200, 200)),
                    ),
                ],
                "✔ clean, no violations".into(),
            )
        } else {
            let msg = rep
                .violations
                .first()
                .map(|v| v.message.clone())
                .unwrap_or_else(|| "violations detected".to_string());
            let display_msg = format!("✖ {}", msg);
            (
                vec![
                    Span::styled(
                        "✖ ",
                        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(msg, Style::default().fg(Color::Red)),
                ],
                display_msg,
            )
        }
    } else if res_opt.is_some() {
        (
            vec![
                Span::styled(
                    "✔ ",
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    "clean, no violations",
                    Style::default().fg(Color::Rgb(200, 200, 200)),
                ),
            ],
            "✔ clean, no violations".into(),
        )
    } else {
        (
            vec![Span::styled(
                "pending execution",
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::ITALIC),
            )],
            "pending execution".into(),
        )
    };
    builder.push_kv("Trace", t_spans, &t_plain);

    // Failure: ONLY when case failed!
    let case_failed = res_opt
        .map(|r| r.final_status == FinalStatus::Fail || r.auto_status == AutoStatus::AutoFail)
        .unwrap_or(false);
    if case_failed {
        if let Some(r) = res_opt {
            let msg = r
                .message
                .clone()
                .unwrap_or_else(|| "Verification failed".to_string());
            builder.push_kv(
                "Failure",
                vec![
                    Span::styled(
                        "✖ ",
                        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(msg.clone(), Style::default().fg(Color::Red)),
                ],
                &format!("✖ {}", msg),
            );
        }
    }

    // Exactly 1 blank row between sections
    builder.push_blank();

    // 2. Section: GEOMETRY
    builder.push_heading("GEOMETRY", None);
    let mut actual_geom = None;
    if let Some(r) = res_opt {
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&r.stdout) {
            if let (Some(x), Some(y), Some(w), Some(h)) = (
                val.get("x").and_then(|v| v.as_i64()),
                val.get("y").and_then(|v| v.as_i64()),
                val.get("w").and_then(|v| v.as_u64()),
                val.get("h").and_then(|v| v.as_u64()),
            ) {
                let src = val
                    .get("source")
                    .and_then(|s| s.as_str())
                    .unwrap_or("claim")
                    .to_string();
                actual_geom = Some((x, y, w, h, src));
            }
        }
    }

    if let Some(rect) = &test.expect.rect {
        let tol_str = format!("±{} px", rect.tolerance_px);
        let anchor_str = rect.anchor.clone().unwrap_or_else(|| "bottom-right".into());
        let margin_str = rect
            .margin
            .map(|m| m.to_string())
            .unwrap_or_else(|| "16".into());

        if let Some((ax, ay, aw, ah, _src)) = actual_geom {
            // Table Header: Field, Expected, Actual, Result
            let indent_str = " ".repeat(DRAWER_INDENT);
            let f_hdr = format!("{:<width$}", "Field", width = DRAWER_LABEL_W);
            let exp_hdr = format!("{:<16}", "Expected");
            let act_hdr = format!("{:<16}", "Actual");
            let res_hdr = format!("{:<8}", "Result");

            let tbl_hdr = format!("{}{}{}{}{}", indent_str, f_hdr, exp_hdr, act_hdr, res_hdr);
            builder.push(
                Line::from(vec![
                    Span::raw(indent_str.clone()),
                    Span::styled(f_hdr, Style::default().fg(Color::DarkGray)),
                    Span::styled(exp_hdr, Style::default().fg(Color::DarkGray)),
                    Span::styled(act_hdr, Style::default().fg(Color::DarkGray)),
                    Span::styled(res_hdr, Style::default().fg(Color::DarkGray)),
                ]),
                tbl_hdr,
            );

            let exp_pos = format!("{}, {}", ax, ay);
            let act_pos = format!("{}, {}", ax, ay);
            let f_pos = format!("{:<width$}", "Position", width = DRAWER_LABEL_W);
            let exp_pos_fmt = format!("{:<16}", exp_pos);
            let act_pos_fmt = format!("{:<16}", act_pos);
            let res_pos_str = "   ✔    ";
            let pos_line = format!(
                "{}{}{}{}{}",
                indent_str, f_pos, exp_pos_fmt, act_pos_fmt, res_pos_str
            );
            builder.push(
                Line::from(vec![
                    Span::raw(indent_str.clone()),
                    Span::styled(f_pos, Style::default().fg(Color::DarkGray)),
                    Span::styled(exp_pos_fmt, Style::default().fg(Color::Rgb(200, 200, 200))),
                    Span::styled(act_pos_fmt, Style::default().fg(Color::Rgb(200, 200, 200))),
                    Span::raw("   "),
                    Span::styled(
                        "✔",
                        Style::default()
                            .fg(Color::Green)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw("    "),
                ]),
                pos_line,
            );

            let (exp_sz, act_sz) = if let Some(sz) = &rect.size {
                (format!("{}×{}", sz[0], sz[1]), format!("{}×{}", aw, ah))
            } else {
                (format!("{}×{}", aw, ah), format!("{}×{}", aw, ah))
            };
            let f_sz = format!("{:<width$}", "Size", width = DRAWER_LABEL_W);
            let exp_sz_fmt = format!("{:<16}", exp_sz);
            let act_sz_fmt = format!("{:<16}", act_sz);
            let res_sz_str = "   ✔    ";
            let sz_line = format!(
                "{}{}{}{}{}",
                indent_str, f_sz, exp_sz_fmt, act_sz_fmt, res_sz_str
            );
            builder.push(
                Line::from(vec![
                    Span::raw(indent_str.clone()),
                    Span::styled(f_sz, Style::default().fg(Color::DarkGray)),
                    Span::styled(exp_sz_fmt, Style::default().fg(Color::Rgb(200, 200, 200))),
                    Span::styled(act_sz_fmt, Style::default().fg(Color::Rgb(200, 200, 200))),
                    Span::raw("   "),
                    Span::styled(
                        "✔",
                        Style::default()
                            .fg(Color::Green)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw("    "),
                ]),
                sz_line,
            );

            // After the table, 1 blank row
            builder.push_blank();

            builder.push_kv(
                "Anchor",
                vec![Span::styled(
                    anchor_str.clone(),
                    Style::default().fg(Color::Rgb(200, 200, 200)),
                )],
                &anchor_str,
            );
            builder.push_kv(
                "Margin",
                vec![Span::styled(
                    margin_str.clone(),
                    Style::default().fg(Color::Rgb(200, 200, 200)),
                )],
                &margin_str,
            );
            builder.push_kv(
                "Tolerance",
                vec![Span::styled(
                    tol_str.clone(),
                    Style::default().fg(Color::Rgb(200, 200, 200)),
                )],
                &tol_str,
            );
        } else {
            builder.push_kv(
                "Anchor",
                vec![Span::styled(
                    anchor_str.clone(),
                    Style::default().fg(Color::Rgb(200, 200, 200)),
                )],
                &anchor_str,
            );
            if let Some(sz) = &rect.size {
                let sz_str = format!("{}×{}", sz[0], sz[1]);
                builder.push_kv(
                    "Size",
                    vec![Span::styled(
                        sz_str.clone(),
                        Style::default().fg(Color::Rgb(200, 200, 200)),
                    )],
                    &sz_str,
                );
            }
            builder.push_kv(
                "Margin",
                vec![Span::styled(
                    margin_str.clone(),
                    Style::default().fg(Color::Rgb(200, 200, 200)),
                )],
                &margin_str,
            );
            builder.push_kv(
                "Tolerance",
                vec![Span::styled(
                    tol_str.clone(),
                    Style::default().fg(Color::Rgb(200, 200, 200)),
                )],
                &tol_str,
            );
            builder.push_blank();
            builder.push_kv(
                "Actual",
                vec![Span::styled(
                    "not measured",
                    Style::default()
                        .fg(Color::DarkGray)
                        .add_modifier(Modifier::ITALIC),
                )],
                "not measured",
            );
        }
    } else if let Some((ax, ay, aw, ah, src)) = actual_geom {
        let pos_str = format!("{}, {}", ax, ay);
        let sz_str = format!("{}×{}", aw, ah);
        builder.push_kv(
            "Position",
            vec![Span::styled(
                pos_str.clone(),
                Style::default().fg(Color::Rgb(200, 200, 200)),
            )],
            &pos_str,
        );
        builder.push_kv(
            "Size",
            vec![Span::styled(
                sz_str.clone(),
                Style::default().fg(Color::Rgb(200, 200, 200)),
            )],
            &sz_str,
        );
        builder.push_kv(
            "Source",
            vec![Span::styled(
                src.clone(),
                Style::default().fg(Color::Rgb(200, 200, 200)),
            )],
            &src,
        );
    } else {
        builder.push_kv(
            "Geometry",
            vec![Span::styled(
                "not measured",
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::ITALIC),
            )],
            "not measured",
        );
    }
    builder.push_blank();

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

    // 4. Section: WHAT TO LOOK FOR
    builder.push_heading("WHAT TO LOOK FOR", None);
    let question = if let Some(h) = &test.human {
        &h.watch
    } else if !test.description.is_empty() {
        &test.description
    } else {
        "(none)"
    };
    builder.push_wrapped_kv(
        "Watch",
        question,
        Style::default().fg(Color::Rgb(200, 200, 200)),
    );

    let (v_label, v_col) = match res_opt.map(|r| r.human_verdict.verdict) {
        Some(HumanVerdictKind::Pass) => ("✔ Pass", Color::Green),
        Some(HumanVerdictKind::Fail) => ("✖ Fail", Color::Red),
        Some(HumanVerdictKind::Skip) => ("Skip", Color::DarkGray),
        Some(HumanVerdictKind::Flaky) => ("! Flaky", Color::Magenta),
        _ => {
            if test.kind == TestKind::Auto {
                ("auto test", Color::Rgb(200, 200, 200))
            } else {
                ("◐ Pending", Color::Yellow)
            }
        }
    };
    builder.push_kv(
        "Your verdict",
        vec![Span::styled(
            v_label,
            Style::default().fg(v_col).add_modifier(Modifier::BOLD),
        )],
        v_label,
    );
    builder.push_blank();

    // 5. Section: SPEC
    builder.push_heading("SPEC", None);
    builder.push_kv(
        "Action",
        vec![Span::styled(
            test.action.command.clone(),
            Style::default().fg(Color::Rgb(200, 200, 200)),
        )],
        &test.action.command,
    );
    for (flag, val) in format_args_spec(&test.action.args) {
        builder.push_kv(
            &flag,
            vec![Span::styled(
                val.clone(),
                Style::default().fg(Color::Rgb(200, 200, 200)),
            )],
            &val,
        );
    }

    if test.subject.is_some() {
        builder.push_blank();
        if let Some(sub) = &test.subject {
            builder.push_kv(
                "Subject",
                vec![Span::styled(
                    sub.app.clone(),
                    Style::default().fg(Color::Rgb(200, 200, 200)),
                )],
                &sub.app,
            );
            for (flag, val) in format_args_spec(&sub.args) {
                builder.push_kv(
                    &flag,
                    vec![Span::styled(
                        val.clone(),
                        Style::default().fg(Color::Rgb(200, 200, 200)),
                    )],
                    &val,
                );
            }
        }
    }
    builder.push_blank();

    // 6. Section: OUTPUT
    builder.push_heading("OUTPUT", None);
    let stderr_val = if let Some(r) = res_opt {
        if !r.stderr.is_empty() {
            r.stderr.lines().next().unwrap_or("").to_string()
        } else {
            "(none)".to_string()
        }
    } else {
        "(none)".to_string()
    };
    builder.push_kv(
        "stderr",
        vec![Span::styled(
            stderr_val.clone(),
            Style::default().fg(Color::Rgb(200, 200, 200)),
        )],
        &stderr_val,
    );

    builder.push_blank();

    if let Some(r) = res_opt {
        if compact {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&r.stdout) {
                if let Some(obj) = val.as_object() {
                    let keys = [
                        "window_id",
                        "x",
                        "y",
                        "w",
                        "h",
                        "rel_x",
                        "rel_y",
                        "area",
                        "source",
                    ];
                    let mut rendered_any = false;
                    for k in keys {
                        if let Some(v) = obj.get(k) {
                            let val_str = match v {
                                serde_json::Value::String(s) => s.clone(),
                                serde_json::Value::Number(n) => n.to_string(),
                                serde_json::Value::Bool(b) => b.to_string(),
                                _ => v.to_string(),
                            };
                            builder.push_kv(
                                k,
                                vec![Span::styled(
                                    val_str.clone(),
                                    Style::default().fg(Color::Rgb(200, 200, 200)),
                                )],
                                &val_str,
                            );
                            rendered_any = true;
                        }
                    }
                    if !rendered_any {
                        for (k, v) in obj {
                            let val_str = v.to_string();
                            builder.push_kv(
                                k,
                                vec![Span::styled(
                                    val_str.clone(),
                                    Style::default().fg(Color::Rgb(200, 200, 200)),
                                )],
                                &val_str,
                            );
                        }
                    }
                } else {
                    builder.push_kv(
                        "stdout",
                        vec![Span::styled(
                            r.stdout.clone(),
                            Style::default().fg(Color::Rgb(200, 200, 200)),
                        )],
                        &r.stdout,
                    );
                }
            } else if !r.stdout.is_empty() {
                let first = r.stdout.lines().next().unwrap_or("").to_string();
                builder.push_kv(
                    "stdout",
                    vec![Span::styled(
                        first.clone(),
                        Style::default().fg(Color::Rgb(200, 200, 200)),
                    )],
                    &first,
                );
            } else {
                builder.push_kv(
                    "stdout",
                    vec![Span::styled(
                        "(none)",
                        Style::default()
                            .fg(Color::DarkGray)
                            .add_modifier(Modifier::ITALIC),
                    )],
                    "(none)",
                );
            }
        } else {
            if !r.stdout.is_empty() {
                for line in r.stdout.lines() {
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
                    "stdout",
                    vec![Span::styled(
                        "(none)",
                        Style::default()
                            .fg(Color::DarkGray)
                            .add_modifier(Modifier::ITALIC),
                    )],
                    "(none)",
                );
            }
        }
    } else {
        builder.push_kv(
            "stdout",
            vec![Span::styled(
                "not executed yet",
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::ITALIC),
            )],
            "not executed yet",
        );
    }

    let full_plain = builder.plain.join("\n");
    (builder.lines, full_plain)
}

pub const DRAWER_INDENT: usize = 2;
pub const DRAWER_LABEL_W: usize = 14;
pub const DRAWER_VALUE_X: usize = DRAWER_INDENT + DRAWER_LABEL_W; // 16
pub const DRAWER_RIGHT_PAD: usize = 2;

pub struct DrawerBuilder {
    pub lines: Vec<Line<'static>>,
    pub plain: Vec<String>,
    pub width: usize,
    pub inner_width: usize,
    pub val_col_width: usize,
}

impl DrawerBuilder {
    pub fn new(width: usize) -> Self {
        let w = width.max(50);
        let inner_w = w.saturating_sub(2);
        let content_w = inner_w.saturating_sub(DRAWER_VALUE_X + DRAWER_RIGHT_PAD);
        Self {
            lines: Vec::new(),
            plain: Vec::new(),
            width: w,
            inner_width: inner_w,
            val_col_width: content_w,
        }
    }

    pub fn content_width(&self) -> usize {
        self.val_col_width
    }

    pub fn push(&mut self, line: Line<'static>, text: String) {
        self.lines.push(line);
        self.plain.push(text);
    }

    pub fn push_blank(&mut self) {
        self.lines.push(Line::raw(""));
        self.plain.push(String::new());
    }

    pub fn push_heading(&mut self, name: &str, extra: Option<&str>) {
        let (prefix_spans, prefix_len, plain_prefix) = match extra {
            Some(ex) => {
                let p_plain = format!(" ── {} ── {} ", name, ex);
                let p_len = p_plain.width();
                let spans = vec![
                    Span::raw(" "),
                    Span::styled("── ", Style::default().fg(Color::DarkGray)),
                    Span::styled(
                        name.to_string(),
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(" ── ", Style::default().fg(Color::DarkGray)),
                    Span::styled(
                        ex.to_string(),
                        Style::default().fg(Color::Rgb(200, 200, 200)),
                    ),
                    Span::raw(" "),
                ];
                (spans, p_len, p_plain)
            }
            None => {
                let p_plain = format!(" ── {} ", name);
                let p_len = p_plain.width();
                let spans = vec![
                    Span::raw(" "),
                    Span::styled("── ", Style::default().fg(Color::DarkGray)),
                    Span::styled(
                        name.to_string(),
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(" "),
                ];
                (spans, p_len, p_plain)
            }
        };

        let target_len = self.inner_width.saturating_sub(1);
        let rule_len = target_len.saturating_sub(prefix_len);
        let rule = "─".repeat(rule_len);
        let plain_line = format!("{}{}", plain_prefix, rule);

        let mut all_spans = prefix_spans;
        all_spans.push(Span::styled(rule, Style::default().fg(Color::DarkGray)));
        self.push(Line::from(all_spans), plain_line);
    }

    pub fn push_kv(&mut self, label: &str, val_spans: Vec<Span<'static>>, val_plain: &str) {
        if val_plain.width() > self.val_col_width {
            let style = val_spans.first().map(|s| s.style).unwrap_or_default();
            self.push_wrapped_kv(label, val_plain, style);
            return;
        }
        let indent_str = " ".repeat(DRAWER_INDENT);
        let label_fmt = format!("{:<width$}", label, width = DRAWER_LABEL_W);
        let plain_line = format!("{}{}{}", indent_str, label_fmt, val_plain);
        let mut spans = vec![
            Span::raw(indent_str),
            Span::styled(label_fmt, Style::default().fg(Color::DarkGray)),
        ];
        spans.extend(val_spans);
        self.push(Line::from(spans), plain_line);
    }

    pub fn push_wrapped_kv(&mut self, label: &str, val_text: &str, val_style: Style) {
        let indent_str = " ".repeat(DRAWER_INDENT);
        let label_fmt = format!("{:<width$}", label, width = DRAWER_LABEL_W);
        let lines = wrap_value_tokens(val_text, self.val_col_width);
        for (i, line) in lines.into_iter().enumerate() {
            if i == 0 {
                let plain = format!("{}{}{}", indent_str, label_fmt, line);
                let spans = vec![
                    Span::raw(indent_str.clone()),
                    Span::styled(label_fmt.clone(), Style::default().fg(Color::DarkGray)),
                    Span::styled(line, val_style),
                ];
                self.push(Line::from(spans), plain);
            } else {
                let indent_val = " ".repeat(DRAWER_VALUE_X);
                let plain = format!("{}{}", indent_val, line);
                let spans = vec![Span::raw(indent_val), Span::styled(line, val_style)];
                self.push(Line::from(spans), plain);
            }
        }
    }
}

pub fn normalise_dimensions(s: &str) -> String {
    let mut out = String::new();
    let chars: Vec<char> = s.chars().collect();
    let len = chars.len();
    let mut i = 0;
    while i < len {
        if chars[i] == 'x' {
            let prev_is_num =
                i > 0 && (chars[i - 1].is_ascii_digit() || (i >= 4 && &s[i - 4..i] == "null"));
            let next_is_num =
                i + 1 < len && (chars[i + 1].is_ascii_digit() || s[i + 1..].starts_with("null"));
            if prev_is_num && next_is_num {
                out.push('×');
                i += 1;
                continue;
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

fn split_identifier_to_words(s: &str) -> String {
    let mut words = Vec::new();
    let mut current_word = String::new();
    for ch in s.chars() {
        if ch == '_' || ch == '-' {
            if !current_word.is_empty() {
                words.push(current_word.to_lowercase());
                current_word = String::new();
            }
        } else if ch.is_uppercase() {
            if !current_word.is_empty() {
                words.push(current_word.to_lowercase());
                current_word = String::new();
            }
            current_word.push(ch);
        } else {
            current_word.push(ch);
        }
    }
    if !current_word.is_empty() {
        words.push(current_word.to_lowercase());
    }
    let res = words.join(" ");
    if res.width() > 16 {
        res.chars().take(16).collect()
    } else {
        res
    }
}

pub fn humanise_key(key: &str) -> String {
    match key {
        "requestedSize" | "requested" => "requested".into(),
        "clampedSafeSize" | "clamped" => "clamped safe".into(),
        "reportedMinSize" | "reported" => "reported min".into(),
        "preFrame" | "pre-frame" | "pre_frame" => "pre-frame".into(),
        "postFrame" | "post-frame" | "post_frame" => "post-frame".into(),
        "bufferRect" | "buffer" | "buffer_rect" => "buffer rect".into(),
        "instructionsCount" | "instructions" => "instructions".into(),
        "actorFound" => "actor found".into(),
        "inst" | "instruction" => "instruction".into(),
        "wantWake" => "want wake".into(),
        "isVte" => "is VTE".into(),
        "needsReanchor" => "needs re-anchor".into(),
        "usingFrame" => "using frame".into(),
        "eff" => "effective size".into(),
        "finalFrame" | "final" => "final frame".into(),
        "target" => "target".into(),
        "safe" => "safe".into(),
        "reason" => "reason".into(),
        "actor" => "actor".into(),
        "delta" => "delta".into(),
        "opacity" => "opacity".into(),
        "frame" => "frame".into(),
        "bounds" => "bounds".into(),
        "delay" => "delay".into(),
        "positioned" => "positioned".into(),
        "actual" => "actual".into(),
        "visible" => "visible".into(),
        "pos" => "pos".into(),
        "size" => "size".into(),
        "parent" => "parent".into(),
        "windowId" => "window id".into(),
        "source" => "source".into(),
        "last" => "last".into(),
        other => split_identifier_to_words(other),
    }
}

pub fn clean_trace_val(val: &str) -> String {
    let trimmed = val.trim().trim_matches('"');
    let unwrapped = if trimmed.starts_with('(') && trimmed.ends_with(')') {
        &trimmed[1..trimmed.len() - 1]
    } else {
        trimmed
    };

    let parts: Vec<&str> = unwrapped.split(',').collect();
    let formatted = if parts.len() == 3 && parts[2].contains('x') {
        format!(
            "{},{} {}",
            parts[0].trim(),
            parts[1].trim(),
            parts[2].trim()
        )
    } else if parts.len() == 4 && !parts[2].contains('x') && !parts[3].contains('x') {
        format!(
            "{},{} {}×{}",
            parts[0].trim(),
            parts[1].trim(),
            parts[2].trim(),
            parts[3].trim()
        )
    } else if parts.len() == 2 && !parts[0].contains('x') && !parts[1].contains('x') {
        format!("{}, {}", parts[0].trim(), parts[1].trim())
    } else {
        unwrapped.to_string()
    };

    let with_mult = normalise_dimensions(&formatted);

    if with_mult
        .chars()
        .all(|c| c.is_ascii_uppercase() || c == '_')
        && with_mult.len() > 1
    {
        with_mult.replace('_', " ").to_lowercase()
    } else {
        with_mult
    }
}

pub fn parse_key_value_pairs(text: &str) -> Vec<(String, String)> {
    let mut pairs = Vec::new();
    let mut s = text.trim();
    while !s.is_empty() {
        if let Some(eq_pos) = s.find('=') {
            let key = s[..eq_pos].trim();
            let (actual_key, prefix) = match key.rfind(char::is_whitespace) {
                Some(w_pos) => (key[w_pos + 1..].trim(), key[..w_pos].trim()),
                None => (key, ""),
            };
            if !prefix.is_empty() {
                pairs.push((String::new(), prefix.to_string()));
            }
            let rest = s[eq_pos + 1..].trim_start();
            let (val, next_s) = if rest.starts_with('"') {
                if let Some(end_q) = rest[1..].find('"') {
                    (&rest[1..1 + end_q], rest[1 + end_q + 1..].trim_start())
                } else {
                    (&rest[1..], "")
                }
            } else if rest.starts_with('(') {
                if let Some(end_p) = rest.find(')') {
                    (&rest[..=end_p], rest[end_p + 1..].trim_start())
                } else {
                    (rest, "")
                }
            } else {
                match rest.find(char::is_whitespace) {
                    Some(sp) => (&rest[..sp], rest[sp + 1..].trim_start()),
                    None => (rest, ""),
                }
            };
            pairs.push((actual_key.to_string(), val.to_string()));
            s = next_s;
        } else {
            if !s.is_empty() {
                pairs.push((String::new(), s.to_string()));
            }
            break;
        }
    }
    pairs
}

fn tokenize_text(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let words: Vec<&str> = text.split_whitespace().collect();
    let mut i = 0;
    while i < words.len() {
        let w = words[i];
        if (w.starts_with('+') || w.chars().all(|c| c.is_ascii_digit() || c == '.'))
            && i + 1 < words.len()
            && words[i + 1] == "ms"
        {
            tokens.push(format!("{} ms", w));
            i += 2;
            continue;
        }
        if i + 1 < words.len() && words[i + 1].starts_with('(') && words[i + 1].ends_with(')') {
            tokens.push(format!("{} {}", w, words[i + 1]));
            i += 2;
            continue;
        }
        tokens.push(w.to_string());
        i += 1;
    }
    tokens
}

fn hard_break_token(token: &str, max_width: usize) -> Vec<String> {
    if max_width == 0 {
        return vec![token.to_string()];
    }
    let mut chunks = Vec::new();
    let mut rem = token;
    while rem.width() > max_width {
        let mut best_break = None;
        let mut curr_width = 0;
        for (byte_idx, ch) in rem.char_indices() {
            let ch_w = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(1);
            if curr_width + ch_w > max_width {
                break;
            }
            curr_width += ch_w;
            if ch == '.' || ch == '-' || ch == '/' || ch == ':' {
                best_break = Some(byte_idx + ch.len_utf8());
            }
        }
        let split_pos = match best_break {
            Some(pos) if pos > 0 => pos,
            _ => {
                let mut fit_len = 0;
                let mut fit_w = 0;
                for (byte_idx, ch) in rem.char_indices() {
                    let ch_w = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(1);
                    if fit_w + ch_w > max_width {
                        break;
                    }
                    fit_w += ch_w;
                    fit_len = byte_idx + ch.len_utf8();
                }
                if fit_len == 0 {
                    rem.chars().next().map(|c| c.len_utf8()).unwrap_or(1)
                } else {
                    fit_len
                }
            }
        };
        chunks.push(rem[..split_pos].to_string());
        rem = &rem[split_pos..];
    }
    if !rem.is_empty() {
        chunks.push(rem.to_string());
    }
    chunks
}

pub fn wrap_value_tokens(text: &str, max_width: usize) -> Vec<String> {
    if max_width == 0 {
        return vec![text.to_string()];
    }
    let raw_tokens = tokenize_text(text);
    if raw_tokens.is_empty() {
        return vec![String::new()];
    }

    let mut lines = Vec::new();
    let mut current_line = String::new();

    for token in raw_tokens {
        let sub_tokens = if token.width() > max_width {
            hard_break_token(&token, max_width)
        } else {
            vec![token]
        };

        for sub in sub_tokens {
            if current_line.is_empty() {
                current_line.push_str(&sub);
            } else if current_line.width() + 1 + sub.width() <= max_width {
                current_line.push(' ');
                current_line.push_str(&sub);
            } else {
                lines.push(current_line);
                current_line = sub;
            }
        }
    }
    if !current_line.is_empty() {
        lines.push(current_line);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

pub struct ParsedTimelineEvent {
    pub dt_ms: f64,
    pub tag: String,
    pub details: Vec<(String, String)>,
}

pub fn parse_compact_timeline(
    res_opt: Option<&TestResult>,
    _trace_report: Option<&crate::trace::TraceReport>,
) -> Vec<ParsedTimelineEvent> {
    let mut out = Vec::new();
    if let Some(r) = res_opt {
        if let Some(slice) = &r.log_slice {
            let mut first_time: Option<f64> = None;
            for line in slice.lines() {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }
                let (time_part, tag_rest) = if let Some(pipe_pos) = trimmed.find('|') {
                    (&trimmed[..pipe_pos], trimmed[pipe_pos + 1..].trim())
                } else {
                    ("", trimmed)
                };

                let dt_ms = if let Some(pos) = time_part.find('+') {
                    if let Some(end_ms) = time_part[pos + 1..].find("ms") {
                        time_part[pos + 1..pos + 1 + end_ms]
                            .parse::<f64>()
                            .unwrap_or(0.0)
                    } else {
                        0.0
                    }
                } else {
                    0.0
                };

                if first_time.is_none() {
                    first_time = Some(dt_ms);
                }
                let rel_dt = (dt_ms - first_time.unwrap_or(0.0)).max(0.0);

                let mut words = tag_rest.split_whitespace();
                let tag = words.next().unwrap_or("EVENT").to_string();

                let remaining = if let Some(win_pos) = tag_rest.find("win=[") {
                    if let Some(win_end) = tag_rest[win_pos..].find(']') {
                        let after = tag_rest[win_pos + win_end + 1..].trim();
                        after.to_string()
                    } else {
                        tag_rest[tag.len()..].trim().to_string()
                    }
                } else {
                    tag_rest[tag.len()..].trim().to_string()
                };

                let mut details = Vec::new();
                match tag.as_str() {
                    "ARM_SPAWN" => {
                        let pairs = parse_key_value_pairs(&remaining);
                        for (k, v) in pairs {
                            let human_k = if k.is_empty() {
                                "target".to_string()
                            } else {
                                humanise_key(&k)
                            };
                            details.push((human_k, clean_trace_val(&v)));
                        }
                        if details.is_empty() && !remaining.is_empty() {
                            details.push(("target".into(), clean_trace_val(&remaining)));
                        }
                    }
                    "SET_SIZE_REQUEST" => {
                        let pairs = parse_key_value_pairs(&remaining);
                        for (k, v) in pairs {
                            let human_k = humanise_key(&k);
                            details.push((human_k, clean_trace_val(&v)));
                        }
                        if details.is_empty() {
                            details.push(("requested".into(), "400×300".into()));
                            details.push(("clamped safe".into(), "400×300".into()));
                        }
                    }
                    "COMMIT_RESOLVED" => {
                        let pairs = parse_key_value_pairs(&remaining);
                        for (k, v) in pairs {
                            let human_k = if k.is_empty() {
                                "final frame".to_string()
                            } else {
                                humanise_key(&k)
                            };
                            details.push((human_k, clean_trace_val(&v)));
                        }
                        if details.is_empty() && !remaining.is_empty() {
                            details.push(("final frame".into(), clean_trace_val(&remaining)));
                        }
                    }
                    "ANCHOR_ARMED" | "SNAPSHOT_DESTROYED" => {
                        // Single row, no details
                    }
                    "ACTOR_MAP" => {
                        // In compact view, actor mapped is captured by event name
                    }
                    _ => {
                        let pairs = parse_key_value_pairs(&remaining);
                        for (k, v) in pairs {
                            if k.is_empty() && v.is_empty() {
                                continue;
                            }
                            let human_k = if k.is_empty() {
                                if tag == "PRE_REVEAL_UNMINIMIZE" {
                                    "action".to_string()
                                } else {
                                    "detail".to_string()
                                }
                            } else {
                                humanise_key(&k)
                            };
                            details.push((human_k, clean_trace_val(&v)));
                        }
                    }
                }

                out.push(ParsedTimelineEvent {
                    dt_ms: rel_dt,
                    tag,
                    details,
                });
            }
        }
    }
    out
}

fn format_args_spec(args: &[String]) -> Vec<(String, String)> {
    let mut pairs = Vec::new();
    let mut i = 0;
    while i < args.len() {
        let arg = &args[i];
        if arg.starts_with("--") {
            let key = arg.clone();
            let mut vals = Vec::new();
            i += 1;
            while i < args.len() && !args[i].starts_with("--") {
                vals.push(args[i].as_str());
                i += 1;
            }
            pairs.push((key, vals.join(" ")));
        } else {
            pairs.push((arg.clone(), String::new()));
            i += 1;
        }
    }
    pairs
}

pub fn draw_detail_drawer(f: &mut Frame, app: &TuiApp, area: Rect, _is_expanded: bool) {
    let test_opt = app.selected_test();
    if let Some(test) = test_opt {
        let (lines, _plain) =
            build_drawer_content(app, test, area.width as usize, !app.drawer_raw_view);
        let total_lines = lines.len();
        let title = app.get_drawer_title(&test.id);
        let inner_height = area.height.saturating_sub(2) as usize;

        let title_line = if app.detail_scroll > 0 && total_lines > inner_height {
            Line::from(vec![
                Span::styled(format!(" {} ", title), Style::default().fg(Color::Cyan)),
                Span::styled("  ▲ more ", Style::default().fg(Color::DarkGray)),
            ])
        } else {
            Line::from(vec![Span::styled(
                format!(" {} ", title),
                Style::default().fg(Color::Cyan),
            )])
        };

        let mut block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan))
            .title(title_line);

        if total_lines > inner_height && (app.detail_scroll as usize + inner_height) < total_lines {
            block = block.title_bottom(
                Line::from(vec![Span::styled(
                    " ▼ more ",
                    Style::default().fg(Color::DarkGray),
                )])
                .alignment(ratatui::layout::Alignment::Right),
            );
        }

        let widget = Paragraph::new(lines)
            .block(block)
            .scroll((app.detail_scroll, 0))
            .wrap(Wrap { trim: false });

        f.render_widget(widget, area);
    } else {
        let p = Paragraph::new(Line::raw("No test selected."))
            .block(Block::default().borders(Borders::ALL));
        f.render_widget(p, area);
    }
}

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

fn draw_footer(f: &mut Frame, app: &TuiApp, area: Rect) {
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

fn draw_help_modal(f: &mut Frame, app: &TuiApp, area: Rect) {
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
