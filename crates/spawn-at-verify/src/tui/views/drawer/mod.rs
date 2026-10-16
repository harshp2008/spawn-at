//! Drawer widget rendering and content assembly.

pub mod action_trace_rows;
pub mod geometry_section;
pub mod header_section;
pub mod info_sections;
pub mod result_section;
pub mod timeline_blocks;
pub mod timeline_format;
pub mod timeline_section;

pub use geometry_section::render_geometry_section;
pub use header_section::render_header_section;
pub use info_sections::{render_look_for_section, render_output_section, render_spec_section};
pub use result_section::render_result_section;
pub use timeline_format::{parse_compact_timeline, parse_timeline_lines, ParsedTimelineEvent};
pub use timeline_section::render_timeline_section;

use super::builder::DrawerBuilder;
use super::constants::{DRAWER_LABEL_W, DRAWER_LABEL_W_MAX};
use crate::schema::TestDefinition;
use crate::tui::app::TuiApp;
use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

/// Renders every drawer section into `builder` in display order.
fn render_sections(
    builder: &mut DrawerBuilder,
    app: &TuiApp,
    test: &TestDefinition,
    compact: bool,
) {
    let res_opt = app.results.get(&test.id);
    let env_workarea = app
        .env_info
        .as_ref()
        .map(|e| e.workarea_probe())
        .unwrap_or_default();
    render_header_section(builder, test);
    render_result_section(builder, test, res_opt, env_workarea);
    // A busy machine inflates timings; say so in the RESULT section (the last row of it).
    if let Some(warning) = app
        .run_load
        .and_then(|l| crate::loadavg::busy_warning(l.start, l.end, l.cpus))
    {
        builder.push_kv(
            "Load",
            vec![ratatui::text::Span::styled(
                format!("⚠ {}", warning),
                ratatui::style::Style::default().fg(ratatui::style::Color::Yellow),
            )],
            &format!("⚠ {}", warning),
        );
    }
    render_look_for_section(builder, test, res_opt);
    render_geometry_section(builder, test, res_opt, env_workarea);
    render_spec_section(builder, test);
    render_output_section(builder, res_opt, compact);
    // TIMELINE is the longest section and reference material, so it goes last.
    render_timeline_section(builder, test, res_opt, compact);
}

/// Assembles all sections of the detail drawer using the shared builder grid.
///
/// Order: header, RESULT, WHAT TO LOOK FOR, GEOMETRY, SPEC, OUTPUT, TIMELINE.
///
/// A probe pass at the minimum label width records the longest label any section
/// pushes; the real pass then uses `clamp(longest + 2, 14, 24)` as the label
/// column for every section so all values start at the same column.
pub fn build_drawer_content(
    app: &TuiApp,
    test: &TestDefinition,
    width: usize,
    compact: bool,
) -> (Vec<Line<'static>>, String) {
    let mut probe = DrawerBuilder::new(width);
    render_sections(&mut probe, app, test, compact);
    let label_w = (probe.max_label_seen + 2).clamp(DRAWER_LABEL_W, DRAWER_LABEL_W_MAX);

    let mut builder = DrawerBuilder::with_label_w(width, label_w);
    render_sections(&mut builder, app, test, compact);

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
