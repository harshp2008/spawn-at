//! TUI views module organizing layout, widgets, drawer, and modals.

pub mod builder;
pub mod constants;
pub mod drawer;
pub mod footer;
pub mod modals;
pub mod panels;
pub mod theme;
pub mod wrapping;

pub use builder::DrawerBuilder;
pub use constants::*;
pub use drawer::{build_drawer_content, draw_detail_drawer};
pub use footer::{draw_footer, render_key_grid, DRAWER_KEYS, VERDICT_KEYS};
pub use modals::{draw_help_modal, draw_verdict_modal};
pub use panels::{
    compute_list_items_count, compute_result_box_height, count_wrapped_lines, draw_details_pane,
    draw_header, draw_result_box, draw_test_list,
};
pub use theme::*;
pub use wrapping::*;

use super::app::{ModalState, TuiApp};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

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
