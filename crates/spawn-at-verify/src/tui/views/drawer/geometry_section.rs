//! Drawer GEOMETRY section rendering: expected (oracle) vs actual placement with a verdict.

use super::info_sections::spec_flag_value;
use crate::oracle::{check_geometry, GeometryCheck, Rect, WorkareaProbe};
use crate::runner::TestResult;
use crate::schema::TestDefinition;
use crate::tui::views::builder::DrawerBuilder;
use crate::tui::views::constants::{DRAWER_INDENT, DRAWER_RIGHT_PAD};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

/// Foreground used for ordinary values.
const VALUE: Color = Color::Rgb(200, 200, 200);

/// Renders the GEOMETRY section.
///
/// With a rectangle expectation the table shows Expected (from the oracle and the spec, never
/// from the observed result), Actual (from stdout, or "not measured") and a Result mark that
/// comes from [`GeometryCheck`], the same comparison the runner's verdict uses.
/// `env_workarea` is the probed workarea, used only when the run's own log records none.
pub fn render_geometry_section(
    builder: &mut DrawerBuilder,
    test: &TestDefinition,
    res_opt: Option<&TestResult>,
    env_workarea: WorkareaProbe,
) {
    builder.push_heading("GEOMETRY", None);

    if let Some(rect) = &test.expect.rect {
        let check = match res_opt {
            Some(r) => r.geometry_check(test, env_workarea),
            None => None,
        }
        .unwrap_or_else(|| check_geometry(rect, "", None, env_workarea));

        render_table(builder, &check);
        builder.push_blank();

        // Anchor and margin appear in SPEC as `--anchor` / `--margin` when the action passes
        // them; they are shown here only when SPEC does not already state the same value.
        let anchor_row = merged_expectation(test, "--anchor", "Anchor", rect.anchor.clone());
        let margin_row = merged_expectation(
            test,
            "--margin",
            "Margin",
            rect.margin.map(|m| m.to_string()),
        );
        push_expectation_row(builder, anchor_row.as_ref());
        push_expectation_row(builder, margin_row.as_ref());
        push_plain_row(builder, "Tolerance", &format!("±{} px", check.tolerance_px));

        if let Some((dx, dy, dw, dh)) = check.deltas() {
            if check.status() == crate::oracle::GeometryStatus::Fail {
                let off = match check.spec_size {
                    Some(_) => format!("dx={:+} dy={:+} dw={:+} dh={:+}", dx, dy, dw, dh),
                    None => format!("dx={:+} dy={:+}", dx, dy),
                };
                builder.push_kv(
                    "Off by",
                    vec![Span::styled(
                        off.clone(),
                        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                    )],
                    &off,
                );
            }
        }
        if let Some(why) = &check.expected_error {
            let note = format!("expected rectangle unavailable: {}", why);
            builder.push_kv(
                "Note",
                vec![Span::styled(
                    note.clone(),
                    Style::default()
                        .fg(Color::DarkGray)
                        .add_modifier(Modifier::ITALIC),
                )],
                &note,
            );
        }
    } else if let Some(actual) =
        res_opt.and_then(|r| crate::oracle::check::actual_rect_from_stdout(&r.stdout))
    {
        let src = res_opt
            .and_then(|r| serde_json::from_str::<serde_json::Value>(&r.stdout).ok())
            .and_then(|v| v.get("source").and_then(|s| s.as_str()).map(String::from))
            .unwrap_or_else(|| "claim".to_string());
        push_plain_row(builder, "Position", &format!("{}, {}", actual.x, actual.y));
        push_plain_row(builder, "Size", &format!("{}×{}", actual.w, actual.h));
        push_plain_row(builder, "Source", &src);
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

/// Draws the Field / Expected / Actual / Result table for `check`.
fn render_table(builder: &mut DrawerBuilder, check: &GeometryCheck) {
    // Expected/Actual columns shrink (12..=16) when a wide shared label column leaves less room.
    let col_w = (builder
        .inner_width
        .saturating_sub(DRAWER_INDENT + builder.label_w + 8 + DRAWER_RIGHT_PAD)
        / 2)
    .clamp(12, 16);
    let grey = Style::default().fg(Color::DarkGray);

    let indent_str = " ".repeat(DRAWER_INDENT);
    let hdr = |s: &str, w: usize| format!("{:<w$}", s, w = w);
    let (f, e, a, r) = (
        builder.format_label("Field"),
        hdr("Expected", col_w),
        hdr("Actual", col_w),
        "Result".to_string(),
    );
    let plain = format!("{}{}{}{}{}", indent_str, f, e, a, r);
    builder.push(
        Line::from(vec![
            Span::raw(indent_str.clone()),
            Span::styled(f, grey),
            Span::styled(e, grey),
            Span::styled(a, grey),
            Span::styled(r, grey),
        ]),
        plain,
    );

    let fmt_rect_pos = |r: Rect| format!("{}, {}", r.x, r.y);
    let fmt_rect_size = |w: u32, h: u32| format!("{}×{}", w, h);

    table_row(
        builder,
        col_w,
        "Position",
        check
            .expected
            .map(fmt_rect_pos)
            .unwrap_or_else(|| "unknown".into()),
        check
            .actual
            .map(fmt_rect_pos)
            .unwrap_or_else(|| "not measured".into()),
        check.position_ok(),
    );
    table_row(
        builder,
        col_w,
        "Size",
        check
            .spec_size
            .map(|(w, h)| fmt_rect_size(w, h))
            .unwrap_or_else(|| "—".into()),
        check
            .actual
            .map(|a| fmt_rect_size(a.w, a.h))
            .unwrap_or_else(|| "not measured".into()),
        check.size_ok(),
    );
}

/// Pushes one table row. `mark` is `Some(true)` for ✔, `Some(false)` for ✘ (the Actual cell
/// also turns red) and `None` for no mark.
fn table_row(
    builder: &mut DrawerBuilder,
    col_w: usize,
    label: &str,
    expected: String,
    actual: String,
    mark: Option<bool>,
) {
    let indent_str = " ".repeat(DRAWER_INDENT);
    let label_fmt = builder.format_label(label);
    let exp_fmt = format!("{:<w$}", expected, w = col_w);
    let act_fmt = format!("{:<w$}", actual, w = col_w);
    let act_style = match mark {
        Some(false) => Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        _ if actual == "not measured" => Style::default()
            .fg(Color::DarkGray)
            .add_modifier(Modifier::ITALIC),
        _ => Style::default().fg(VALUE),
    };
    let (mark_ch, mark_style) = match mark {
        Some(true) => (
            "✔",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        Some(false) => (
            "✘",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        ),
        None => (" ", Style::default()),
    };
    // The mark sits 3 columns after the Actual cell, under the "Result" header. Nothing follows
    // it, and a row without a mark ends at the Actual text, so no row has trailing spaces.
    let mut spans = vec![
        Span::raw(indent_str.clone()),
        Span::styled(label_fmt.clone(), Style::default().fg(Color::DarkGray)),
        Span::styled(exp_fmt.clone(), Style::default().fg(VALUE)),
    ];
    let plain;
    if mark.is_some() {
        plain = format!(
            "{}{}{}{}   {}",
            indent_str, label_fmt, exp_fmt, act_fmt, mark_ch
        );
        spans.push(Span::styled(act_fmt, act_style));
        spans.push(Span::raw("   "));
        spans.push(Span::styled(mark_ch, mark_style));
    } else {
        let act_trim = act_fmt.trim_end().to_string();
        plain = format!("{}{}{}{}", indent_str, label_fmt, exp_fmt, act_trim);
        spans.push(Span::styled(act_trim, act_style));
    }
    builder.push(Line::from(spans), plain);
}

/// Pushes a label/value row in the ordinary value colour.
fn push_plain_row(builder: &mut DrawerBuilder, label: &str, value: &str) {
    builder.push_kv(
        label,
        vec![Span::styled(value.to_string(), Style::default().fg(VALUE))],
        value,
    );
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
