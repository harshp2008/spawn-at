//! Drawer informational sections (WHAT TO LOOK FOR, SPEC, OUTPUT).

use crate::runner::{HumanVerdictKind, TestResult};
use crate::schema::{TestDefinition, TestKind};
use crate::tui::views::builder::DrawerBuilder;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

/// Formats launch arguments specification into a compact string.
fn format_args_spec(args: &[String]) -> Vec<(String, String)> {
    let mut pairs = Vec::new();
    let mut i = 0;
    while i < args.len() {
        let arg = &args[i];
        if arg.starts_with("--") {
            let key = arg.clone();
            let mut vals = Vec::new();
            i += 1;
            while i < args.len() && !args[i].starts_with("--") {
                vals.push(args[i].as_str());
                i += 1;
            }
            pairs.push((key, vals.join(" ")));
        } else {
            pairs.push((arg.clone(), String::new()));
            i += 1;
        }
    }
    pairs
}

/// Renders the SUBJECT, ACTION, and EXPECTATIONS sections of the drawer.
pub fn render_info_sections(
    builder: &mut DrawerBuilder,
    test: &TestDefinition,
    res_opt: Option<&TestResult>,
    compact: bool,
) {
    // 4. Section: WHAT TO LOOK FOR
    builder.push_heading("WHAT TO LOOK FOR", None);
    let question = if let Some(h) = &test.human {
        &h.watch
    } else if !test.description.is_empty() {
        &test.description
    } else {
        "(none)"
    };
    builder.push_wrapped_kv(
        "Watch",
        question,
        Style::default().fg(Color::Rgb(200, 200, 200)),
    );

    let (v_label, v_col) = match res_opt.map(|r| r.human_verdict.verdict) {
        Some(HumanVerdictKind::Pass) => ("✔ Pass", Color::Green),
        Some(HumanVerdictKind::Fail) => ("✖ Fail", Color::Red),
        Some(HumanVerdictKind::Skip) => ("Skip", Color::DarkGray),
        Some(HumanVerdictKind::Flaky) => ("! Flaky", Color::Magenta),
        _ => {
            if test.kind == TestKind::Auto {
                ("auto test", Color::Rgb(200, 200, 200))
            } else {
                ("◐ Pending", Color::Yellow)
            }
        }
    };
    builder.push_kv(
        "Your verdict",
        vec![Span::styled(
            v_label,
            Style::default().fg(v_col).add_modifier(Modifier::BOLD),
        )],
        v_label,
    );
    builder.push_blank();

    // 5. Section: SPEC
    builder.push_heading("SPEC", None);
    builder.push_kv(
        "Action",
        vec![Span::styled(
            test.action.command.clone(),
            Style::default().fg(Color::Rgb(200, 200, 200)),
        )],
        &test.action.command,
    );
    for (flag, val) in format_args_spec(&test.action.args) {
        builder.push_kv(
            &flag,
            vec![Span::styled(
                val.clone(),
                Style::default().fg(Color::Rgb(200, 200, 200)),
            )],
            &val,
        );
    }

    if test.subject.is_some() {
        builder.push_blank();
        if let Some(sub) = &test.subject {
            builder.push_kv(
                "Subject",
                vec![Span::styled(
                    sub.app.clone(),
                    Style::default().fg(Color::Rgb(200, 200, 200)),
                )],
                &sub.app,
            );
            for (flag, val) in format_args_spec(&sub.args) {
                builder.push_kv(
                    &flag,
                    vec![Span::styled(
                        val.clone(),
                        Style::default().fg(Color::Rgb(200, 200, 200)),
                    )],
                    &val,
                );
            }
        }
    }
    builder.push_blank();

    // 6. Section: OUTPUT
    builder.push_heading("OUTPUT", None);
    let stderr_val = if let Some(r) = res_opt {
        if !r.stderr.is_empty() {
            r.stderr.lines().next().unwrap_or("").to_string()
        } else {
            "(none)".to_string()
        }
    } else {
        "(none)".to_string()
    };
    builder.push_kv(
        "stderr",
        vec![Span::styled(
            stderr_val.clone(),
            Style::default().fg(Color::Rgb(200, 200, 200)),
        )],
        &stderr_val,
    );

    builder.push_blank();

    if let Some(r) = res_opt {
        if compact {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&r.stdout) {
                if let Some(obj) = val.as_object() {
                    let keys = [
                        "window_id",
                        "x",
                        "y",
                        "w",
                        "h",
                        "rel_x",
                        "rel_y",
                        "area",
                        "source",
                    ];
                    let mut rendered_any = false;
                    for k in keys {
                        if let Some(v) = obj.get(k) {
                            let val_str = match v {
                                serde_json::Value::String(s) => s.clone(),
                                serde_json::Value::Number(n) => n.to_string(),
                                serde_json::Value::Bool(b) => b.to_string(),
                                _ => v.to_string(),
                            };
                            builder.push_kv(
                                k,
                                vec![Span::styled(
                                    val_str.clone(),
                                    Style::default().fg(Color::Rgb(200, 200, 200)),
                                )],
                                &val_str,
                            );
                            rendered_any = true;
                        }
                    }
                    if !rendered_any {
                        for (k, v) in obj {
                            let val_str = v.to_string();
                            builder.push_kv(
                                k,
                                vec![Span::styled(
                                    val_str.clone(),
                                    Style::default().fg(Color::Rgb(200, 200, 200)),
                                )],
                                &val_str,
                            );
                        }
                    }
                } else {
                    builder.push_kv(
                        "stdout",
                        vec![Span::styled(
                            r.stdout.clone(),
                            Style::default().fg(Color::Rgb(200, 200, 200)),
                        )],
                        &r.stdout,
                    );
                }
            } else if !r.stdout.is_empty() {
                let first = r.stdout.lines().next().unwrap_or("").to_string();
                builder.push_kv(
                    "stdout",
                    vec![Span::styled(
                        first.clone(),
                        Style::default().fg(Color::Rgb(200, 200, 200)),
                    )],
                    &first,
                );
            } else {
                builder.push_kv(
                    "stdout",
                    vec![Span::styled(
                        "(none)",
                        Style::default()
                            .fg(Color::DarkGray)
                            .add_modifier(Modifier::ITALIC),
                    )],
                    "(none)",
                );
            }
        } else {
            if !r.stdout.is_empty() {
                for line in r.stdout.lines() {
                    let clean = line.trim();
                    if !clean.is_empty() {
                        builder.push(
                            Line::from(vec![
                                Span::raw("  "),
                                Span::styled(
                                    clean.to_string(),
                                    Style::default().fg(Color::Rgb(200, 200, 200)),
                                ),
                            ]),
                            format!("  {}", clean),
                        );
                    }
                }
            } else {
                builder.push_kv(
                    "stdout",
                    vec![Span::styled(
                        "(none)",
                        Style::default()
                            .fg(Color::DarkGray)
                            .add_modifier(Modifier::ITALIC),
                    )],
                    "(none)",
                );
            }
        }
    } else {
        builder.push_kv(
            "stdout",
            vec![Span::styled(
                "not executed yet",
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::ITALIC),
            )],
            "not executed yet",
        );
    }
}
