//! Top header banner and missing fixture warning panel.

use crate::runner::{AutoStatus, FinalStatus, HumanVerdictKind};
use crate::schema::{TestDefinition, TestKind};
use crate::tui::app::TuiApp;
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

pub fn draw_header(f: &mut Frame, app: &TuiApp, area: Rect) {
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
