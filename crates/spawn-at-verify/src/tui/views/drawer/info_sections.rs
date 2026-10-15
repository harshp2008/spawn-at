//! Drawer informational sections: WHAT TO LOOK FOR, SPEC and OUTPUT (each a separate function so the drawer can order them).

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

/// Renders the WHAT TO LOOK FOR section: the watch prompt and the "Your verdict" row.
pub fn render_look_for_section(
    builder: &mut DrawerBuilder,
    test: &TestDefinition,
    res_opt: Option<&TestResult>,
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

    // 1 blank row between the Watch prose and the verdict row
    builder.push_blank();

    let (v_label, v_col) = match res_opt.map(|r| r.human_verdict.verdict) {
        Some(HumanVerdictKind::Pass) => ("✔ Pass", Color::Green),
        Some(HumanVerdictKind::Fail) => ("✖ Fail", Color::Red),
        Some(HumanVerdictKind::Skip) => ("Skip", Color::DarkGray),
        Some(HumanVerdictKind::Flaky) => ("! Flaky", Color::Magenta),
        _ => {
            if test.kind == TestKind::Auto {
                ("n/a (automatic)", Color::Rgb(200, 200, 200))
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
}

/// Renders the SPEC section: the action command and subject app with their launch arguments.
pub fn render_spec_section(builder: &mut DrawerBuilder, test: &TestDefinition) {
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

    // Version 2: the windows the runner launches, the actions after the first, and the checks.
    if test.v2.schema_version >= crate::schema::SCHEMA_VERSION_EXISTING_WINDOW {
        let grey = |s: String| {
            vec![Span::styled(
                s.clone(),
                Style::default().fg(Color::Rgb(200, 200, 200)),
            )]
        };
        builder.push_blank();
        for s in &test.v2.setup {
            let text = if s.args.is_empty() {
                "fixture window".to_string()
            } else {
                format!("fixture window {}", s.args.join(" "))
            };
            builder.push_kv(&format!("Setup {}", s.name), grey(text.clone()), &text);
        }
        for (i, a) in test.v2.then.iter().enumerate() {
            let text = format!("{} {}", a.command, a.args.join(" "));
            builder.push_kv(&format!("Then {}", i + 1), grey(text.clone()), &text);
        }
        for c in &test.v2.check {
            let what = match (&c.state, &c.same_rect_as, c.process_exited) {
                (Some(s), _, _) => format!("{s:?}").to_lowercase(),
                (_, Some(r), _) => format!("same rectangle as {r}"),
                (_, _, true) => "fixture process exited".to_string(),
                _ => "nothing".to_string(),
            };
            let when = match c.after {
                Some(n) => format!("after action {n}"),
                None => "after the last action".to_string(),
            };
            let text = format!("{what} ({when})");
            builder.push_kv(&format!("Check {}", c.window), grey(text.clone()), &text);
        }
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
}

/// Renders the OUTPUT section: first stderr line and the captured stdout fields.
pub fn render_output_section(
    builder: &mut DrawerBuilder,
    res_opt: Option<&TestResult>,
    compact: bool,
) {
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
    builder.push_blank();
}

/// Returns the value passed for `flag` (e.g. `--anchor`) in the action's or subject's launch
/// arguments, if either passes it.
pub(super) fn spec_flag_value(test: &TestDefinition, flag: &str) -> Option<String> {
    let mut lists: Vec<&[String]> = vec![&test.action.args];
    if let Some(sub) = &test.subject {
        lists.push(&sub.args);
    }
    lists.into_iter().find_map(|args| {
        format_args_spec(args)
            .into_iter()
            .find(|(k, _)| k == flag)
            .map(|(_, v)| v)
    })
}
