//! Drawer header section rendering (Title, ID, Mode, Description).

use crate::schema::{TestDefinition, TestKind};
use crate::tui::views::builder::DrawerBuilder;
use crate::tui::views::wrapping::normalise_dimensions;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;

/// Renders the drawer top header section with test ID, title, mode, and description.
pub fn render_header_section(builder: &mut DrawerBuilder, test: &TestDefinition) {
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

    // 1 blank row between Mode and Description
    builder.push_blank();

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
}
