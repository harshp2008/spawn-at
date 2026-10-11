//! Test category tree and list widget panel.

use crate::runner::{AutoStatus, FinalStatus};
use crate::schema::{TestDefinition, TestKind};
use crate::tui::app::{ActivePane, TuiApp};
use crate::tui::views::theme::{get_test_category, status_symbol_and_color};
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState},
    Frame,
};

/// Computes total visible row count in the test category tree factoring folded groups.
pub fn compute_list_items_count(app: &TuiApp) -> usize {
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

/// Renders the categorized test list navigation panel with status indicators and folding.
pub fn draw_test_list(f: &mut Frame, app: &TuiApp, area: Rect) {
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
