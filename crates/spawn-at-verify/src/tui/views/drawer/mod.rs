//! Drawer widget rendering and content assembly.

pub mod geometry_section;
pub mod header_section;
pub mod info_sections;
pub mod result_section;
pub mod timeline_section;

pub use geometry_section::render_geometry_section;
pub use header_section::render_header_section;
pub use info_sections::render_info_sections;
pub use result_section::render_result_section;
pub use timeline_section::render_timeline_section;

use super::builder::DrawerBuilder;
use crate::schema::TestDefinition;
use crate::tui::app::TuiApp;
use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

/// Assembles all sections of the detail drawer using the shared builder grid.
pub fn build_drawer_content(
    app: &TuiApp,
    test: &TestDefinition,
    width: usize,
    compact: bool,
) -> (Vec<Line<'static>>, String) {
    let mut builder = DrawerBuilder::new(width);
    let res_opt = app.results.get(&test.id);

    render_header_section(&mut builder, test);
    render_result_section(&mut builder, test, res_opt);
    render_geometry_section(&mut builder, test, res_opt);
    render_timeline_section(&mut builder, test, res_opt, compact);
    render_info_sections(&mut builder, test, res_opt, compact);

    let full_plain = builder.plain.join("\n");
    (builder.lines, full_plain)
}

/// Renders the detail drawer popup overlay and scroll indicators.
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
