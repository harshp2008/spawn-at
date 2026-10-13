//! Test result status display box and height calculation.

use super::description::count_wrapped_lines;
use crate::runner::{AutoStatus, FinalStatus};
use crate::tui::app::TuiApp;
use crate::tui::views::theme::{FLASH, FLASH_SYMBOL, TRACE_UNCHECKED};
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Padding, Paragraph, Wrap},
    Frame,
};

/// Extracts formatted status lines, timings, and diagnostic details for the result box.
pub fn get_result_content(app: &TuiApp) -> (String, Line<'static>, Color) {
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
        } else if res.is_flash_pass() {
            let msg = format!("{} ({} ms)", res.flash_info.summary(), res.duration_ms);
            let plain = format!("{} PASS (flash): {}", FLASH_SYMBOL, msg);
            let line = Line::from(vec![
                Span::styled(
                    format!("{} PASS (flash): ", FLASH_SYMBOL),
                    Style::default().fg(FLASH).add_modifier(Modifier::BOLD),
                ),
                Span::styled(msg, Style::default().fg(Color::White)),
            ]);
            (plain, line, FLASH)
        } else if res.final_status == FinalStatus::PassUnchecked
            || res.auto_status == AutoStatus::AutoPassUnchecked
        {
            let msg = format!(
                "geometry was not verified: no workarea ({} ms)",
                res.duration_ms
            );
            let plain = format!("⚠ PASS (geometry unchecked): {}", msg);
            let line = Line::from(vec![
                Span::styled(
                    "⚠ PASS (geometry unchecked): ",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(msg, Style::default().fg(Color::White)),
            ]);
            (plain, line, Color::Yellow)
        } else if res.final_status == FinalStatus::PassTraceUnchecked
            || res.auto_status == AutoStatus::AutoPassTraceUnchecked
        {
            let msg = res
                .message
                .clone()
                .unwrap_or_else(|| format!("the trace was not captured ({} ms)", res.duration_ms));
            let plain = format!("⚠ PASS (trace unchecked): {}", msg);
            let line = Line::from(vec![
                Span::styled(
                    "⚠ PASS (trace unchecked): ",
                    Style::default()
                        .fg(TRACE_UNCHECKED)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(msg, Style::default().fg(Color::White)),
            ]);
            (plain, line, TRACE_UNCHECKED)
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

/// Calculates dynamic height allocation for the bottom result box.
pub fn compute_result_box_height(app: &TuiApp, body_h: u16, box_w: u16) -> u16 {
    let (plain, _, _) = get_result_content(app);
    let inner_w = (box_w.saturating_sub(4) as usize).max(1);
    let wrapped_rows = count_wrapped_lines(&plain, inner_w);
    let needed_box_h = (wrapped_rows as u16).saturating_add(2);
    let max_box_h = ((body_h as u32 * 40) / 100).max(3) as u16;
    needed_box_h.clamp(3, max_box_h)
}

/// Renders the bottom result box panel displaying outcome status and messages.
pub fn draw_result_box(f: &mut Frame, app: &TuiApp, area: Rect) {
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
