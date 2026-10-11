//! Drawer GEOMETRY section rendering (Expected vs Actual table).

use crate::runner::TestResult;
use crate::schema::TestDefinition;
use crate::tui::views::builder::DrawerBuilder;
use crate::tui::views::constants::{DRAWER_INDENT, DRAWER_LABEL_W};
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
}
