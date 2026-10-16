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

/// The text of the end marker the drawer shows below its content (display only: copy, save and
/// the reports are built from the content without it).
pub const END_MARKER: &str = "end of details";

/// The marker line for an inner width of `width` columns: ` end of details ` centered with `─`
/// on both sides so the line is exactly `width` columns (the extra `─` of an odd width goes on the
/// right). Narrower than the text it is cut to `width` columns, so it never wraps.
pub fn end_marker_text(width: usize) -> String {
    use unicode_width::UnicodeWidthStr;
    let text = format!(" {END_MARKER} ");
    let text_w = text.width();
    if width < text_w {
        return text.chars().take(width).collect();
    }
    let fill = width - text_w;
    let left = fill / 2;
    format!("{}{}{}", "─".repeat(left), text, "─".repeat(fill - left))
}

/// Appends a blank row and the dim end marker (as wide as the inner width the content wraps to),
/// so the last row of the content is not flush against the border. It is part of the height the
/// scroll maximum is computed from.
fn push_end_marker(lines: &mut Vec<Line<'static>>, inner_width: usize) {
    lines.push(Line::raw(""));
    lines.push(Line::from(Span::styled(
        end_marker_text(inner_width),
        Style::default().fg(Color::DarkGray),
    )));
}

/// The rows the drawer's content takes when drawn in an area `width` columns wide (the number the
/// scroll maximum is computed from).
pub fn drawer_content_rows(app: &TuiApp, test: &TestDefinition, width: usize) -> usize {
    let (mut lines, _) = build_drawer_content(app, test, width, !app.drawer_raw_view);
    push_end_marker(&mut lines, width.saturating_sub(2));
    super::wrapping::wrapped_row_count(&lines, width.saturating_sub(2))
}

/// Renders the detail drawer popup overlay and scroll indicators.
pub fn draw_detail_drawer(f: &mut Frame, app: &TuiApp, area: Rect, _is_expanded: bool) {
    let test_opt = app.selected_test();
    if let Some(test) = test_opt {
        let (mut lines, _plain) =
            build_drawer_content(app, test, area.width as usize, !app.drawer_raw_view);
        push_end_marker(&mut lines, area.width.saturating_sub(2) as usize);
        let inner_height = area.height.saturating_sub(2) as usize;
        // The rows the drawn text really takes (wrapped to the inner width) decide how far it can
        // scroll. The offset is clamped here, where it is read, as well as where keys write it:
        // a ratatui paragraph adds the offset to a row index and overflows on a huge one.
        let total_lines =
            super::wrapping::wrapped_row_count(&lines, area.width.saturating_sub(2) as usize);
        let max = super::wrapping::max_scroll(total_lines, inner_height);
        app.scroll_max.set(max);
        let scroll = app.detail_scroll.min(max);
        let title = app.get_drawer_title(&test.id);

        let more_above = scroll > 0 && total_lines > inner_height;
        let more_below =
            total_lines > inner_height && (scroll as usize + inner_height) < total_lines;
        let mut title_spans = vec![Span::styled(
            format!(" {} ", title),
            Style::default().fg(Color::Cyan),
        )];
        if more_above {
            title_spans.push(Span::styled(
                "  ▲ more ",
                Style::default().fg(Color::DarkGray),
            ));
        }
        if more_below {
            title_spans.push(Span::styled(
                "  ▼ more ",
                Style::default().fg(Color::DarkGray),
            ));
        }
        let title_line = Line::from(title_spans);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan))
            .title(title_line);

        let widget = Paragraph::new(lines)
            .block(block)
            .scroll((scroll, 0))
            .wrap(Wrap { trim: false });

        f.render_widget(widget, area);
    } else {
        let p = Paragraph::new(Line::raw("No test selected."))
            .block(Block::default().borders(Borders::ALL));
        f.render_widget(p, area);
    }
}
