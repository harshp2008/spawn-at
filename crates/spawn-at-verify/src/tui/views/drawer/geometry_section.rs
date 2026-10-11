//! Drawer GEOMETRY section rendering (Expected vs Actual table).

use super::info_sections::spec_flag_value;
use crate::runner::TestResult;
use crate::schema::TestDefinition;
use crate::tui::views::builder::DrawerBuilder;
use crate::tui::views::constants::{DRAWER_INDENT, DRAWER_RIGHT_PAD};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

/// Renders the GEOMETRY section comparing expected, actual, and result placement rectangles.
pub fn render_geometry_section(
    builder: &mut DrawerBuilder,
    test: &TestDefinition,
    res_opt: Option<&TestResult>,
) {
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
        // Anchor and margin appear in SPEC as `--anchor` / `--margin` when the action passes
        // them; they are shown here only when SPEC does not already state the same value.
        let anchor_row = merged_expectation(test, "--anchor", "Anchor", rect.anchor.clone());
        let margin_row = merged_expectation(
            test,
            "--margin",
            "Margin",
            rect.margin.map(|m| m.to_string()),
        );

        if let Some((ax, ay, aw, ah, _src)) = actual_geom {
            // Table Header: Field, Expected, Actual, Result
            // Expected/Actual columns shrink (12..=16) when a wide shared label column leaves less room
            let col_w = (builder
                .inner_width
                .saturating_sub(DRAWER_INDENT + builder.label_w + 8 + DRAWER_RIGHT_PAD)
                / 2)
            .clamp(12, 16);
            let indent_str = " ".repeat(DRAWER_INDENT);
            let f_hdr = builder.format_label("Field");
            let exp_hdr = format!("{:<w$}", "Expected", w = col_w);
            let act_hdr = format!("{:<w$}", "Actual", w = col_w);
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
            let f_pos = builder.format_label("Position");
            let exp_pos_fmt = format!("{:<w$}", exp_pos, w = col_w);
            let act_pos_fmt = format!("{:<w$}", act_pos, w = col_w);
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
            let f_sz = builder.format_label("Size");
            let exp_sz_fmt = format!("{:<w$}", exp_sz, w = col_w);
            let act_sz_fmt = format!("{:<w$}", act_sz, w = col_w);
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

            push_expectation_row(builder, anchor_row.as_ref());
            push_expectation_row(builder, margin_row.as_ref());
            builder.push_kv(
                "Tolerance",
                vec![Span::styled(
                    tol_str.clone(),
                    Style::default().fg(Color::Rgb(200, 200, 200)),
                )],
                &tol_str,
            );
        } else {
            push_expectation_row(builder, anchor_row.as_ref());
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
            push_expectation_row(builder, margin_row.as_ref());
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
}

/// Decides whether an expectation (anchor / margin) needs its own GEOMETRY row.
///
/// Returns `None` when the expectation is unset or SPEC already shows the same value for `flag`;
/// `Some(label, value)` otherwise, with the label marked "Expected ..." when SPEC shows a
/// different value for the same flag.
fn merged_expectation(
    test: &TestDefinition,
    flag: &str,
    label: &str,
    expected: Option<String>,
) -> Option<(String, String)> {
    let expected = expected?;
    match spec_flag_value(test, flag) {
        Some(v) if v == expected => None,
        Some(_) => Some((format!("Expected {}", label.to_lowercase()), expected)),
        None => Some((label.to_string(), expected)),
    }
}

/// Pushes a plain label/value expectation row if there is one.
fn push_expectation_row(builder: &mut DrawerBuilder, row: Option<&(String, String)>) {
    if let Some((label, value)) = row {
        builder.push_kv(
            label,
            vec![Span::styled(
                value.clone(),
                Style::default().fg(Color::Rgb(200, 200, 200)),
            )],
            value,
        );
    }
}
