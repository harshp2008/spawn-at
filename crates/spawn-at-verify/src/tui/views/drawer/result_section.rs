//! Drawer RESULT section rendering (Verdict, Cloak duration, provisional timing budget).

use crate::runner::{AutoStatus, FinalStatus, HumanVerdictKind, TestResult};
use crate::schema::{TestDefinition, TestKind};
use crate::trace::TraceAnalyser;
use crate::tui::views::builder::DrawerBuilder;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;

pub fn render_result_section(
    builder: &mut DrawerBuilder,
    test: &TestDefinition,
    res_opt: Option<&TestResult>,
) {
    // 1. Section: RESULT
    builder.push_heading("RESULT", None);
    let (v_spans, v_plain): (Vec<Span<'static>>, String) = if let Some(r) = res_opt {
        match r.human_verdict.verdict {
            HumanVerdictKind::Pass => (
                vec![
                    Span::styled(
                        "✔ ",
                        Style::default()
                            .fg(Color::Green)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        "Pass (marked by you)",
                        Style::default().fg(Color::Rgb(200, 200, 200)),
                    ),
                ],
                "✔ Pass (marked by you)".into(),
            ),
            HumanVerdictKind::Fail => (
                vec![
                    Span::styled(
                        "✖ ",
                        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        "Fail (marked by you)",
                        Style::default().fg(Color::Rgb(200, 200, 200)),
                    ),
                ],
                "✖ Fail (marked by you)".into(),
            ),
            HumanVerdictKind::Flaky => (
                vec![
                    Span::styled(
                        "! ",
                        Style::default()
                            .fg(Color::Magenta)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        "Flaky (marked by you)",
                        Style::default().fg(Color::Rgb(200, 200, 200)),
                    ),
                ],
                "! Flaky (marked by you)".into(),
            ),
            HumanVerdictKind::Skip => (
                vec![
                    Span::styled(
                        "- ",
                        Style::default()
                            .fg(Color::DarkGray)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        "Skip (marked by you)",
                        Style::default().fg(Color::Rgb(200, 200, 200)),
                    ),
                ],
                "- Skip (marked by you)".into(),
            ),
            HumanVerdictKind::Unset => {
                if test.kind == TestKind::Auto {
                    match r.final_status {
                        FinalStatus::Pass => (
                            vec![
                                Span::styled(
                                    "✔ ",
                                    Style::default()
                                        .fg(Color::Green)
                                        .add_modifier(Modifier::BOLD),
                                ),
                                Span::styled(
                                    "Pass",
                                    Style::default().fg(Color::Rgb(200, 200, 200)),
                                ),
                            ],
                            "✔ Pass".into(),
                        ),
                        FinalStatus::Fail => (
                            vec![
                                Span::styled(
                                    "✖ ",
                                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                                ),
                                Span::styled(
                                    "Fail",
                                    Style::default().fg(Color::Rgb(200, 200, 200)),
                                ),
                            ],
                            "✖ Fail".into(),
                        ),
                        _ => (
                            vec![
                                Span::styled(
                                    "◐ ",
                                    Style::default()
                                        .fg(Color::Yellow)
                                        .add_modifier(Modifier::BOLD),
                                ),
                                Span::styled(
                                    "Pending",
                                    Style::default().fg(Color::Rgb(200, 200, 200)),
                                ),
                            ],
                            "◐ Pending".into(),
                        ),
                    }
                } else {
                    (
                        vec![
                            Span::styled(
                                "◐ ",
                                Style::default()
                                    .fg(Color::Yellow)
                                    .add_modifier(Modifier::BOLD),
                            ),
                            Span::styled("Pending", Style::default().fg(Color::Rgb(200, 200, 200))),
                        ],
                        "◐ Pending".into(),
                    )
                }
            }
        }
    } else {
        (
            vec![
                Span::styled(
                    "◐ ",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    "Pending",
                    Style::default()
                        .fg(Color::DarkGray)
                        .add_modifier(Modifier::ITALIC),
                ),
            ],
            "◐ Pending".into(),
        )
    };
    builder.push_kv("Verdict", v_spans, &v_plain);

    let trace_report = res_opt
        .and_then(|r| r.log_slice.as_ref())
        .map(|s| TraceAnalyser::new().analyse(s));

    let cloak_ms = trace_report
        .as_ref()
        .and_then(|rep| rep.cloak_duration_ms)
        .or_else(|| {
            res_opt.and_then(|r| {
                if r.duration_ms > 0 {
                    Some(r.duration_ms as f64)
                } else {
                    None
                }
            })
        });

    let (c_spans, c_plain): (Vec<Span<'static>>, String) = if let Some(dur) = cloak_ms {
        (
            vec![Span::styled(
                format!("{:.1} ms", dur),
                Style::default().fg(Color::Rgb(200, 200, 200)),
            )],
            format!("{:.1} ms", dur),
        )
    } else {
        (
            vec![Span::styled(
                "not measured",
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::ITALIC),
            )],
            "not measured".into(),
        )
    };
    builder.push_kv("Cloak", c_spans, &c_plain);

    // Budget: 100 ms (provisional)   ✔ within budget | ! over provisional budget
    let (b_spans, b_plain): (Vec<Span<'static>>, String) = if let Some(dur) = cloak_ms {
        if dur <= 100.0 {
            (
                vec![
                    Span::styled(
                        "100 ms (provisional)   ",
                        Style::default().fg(Color::Rgb(200, 200, 200)),
                    ),
                    Span::styled(
                        "✔ ",
                        Style::default()
                            .fg(Color::Green)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled("within budget", Style::default().fg(Color::Green)),
                ],
                "100 ms (provisional)   ✔ within budget".into(),
            )
        } else {
            (
                vec![
                    Span::styled(
                        "100 ms (provisional)   ",
                        Style::default().fg(Color::Rgb(200, 200, 200)),
                    ),
                    Span::styled(
                        "! ",
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        "over provisional budget",
                        Style::default().fg(Color::Yellow),
                    ),
                ],
                "100 ms (provisional)   ! over provisional budget".into(),
            )
        }
    } else {
        (
            vec![
                Span::styled(
                    "100 ms (provisional)   ",
                    Style::default().fg(Color::Rgb(200, 200, 200)),
                ),
                Span::styled(
                    "not measured",
                    Style::default()
                        .fg(Color::DarkGray)
                        .add_modifier(Modifier::ITALIC),
                ),
            ],
            "100 ms (provisional)   not measured".into(),
        )
    };
    builder.push_kv("Budget", b_spans, &b_plain);

    let (t_spans, t_plain): (Vec<Span<'static>>, String) = if let Some(ref rep) = trace_report {
        if rep.is_pass() {
            (
                vec![
                    Span::styled(
                        "✔ ",
                        Style::default()
                            .fg(Color::Green)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        "clean, no violations",
                        Style::default().fg(Color::Rgb(200, 200, 200)),
                    ),
                ],
                "✔ clean, no violations".into(),
            )
        } else {
            let msg = rep
                .violations
                .first()
                .map(|v| v.message.clone())
                .unwrap_or_else(|| "violations detected".to_string());
            let display_msg = format!("✖ {}", msg);
            (
                vec![
                    Span::styled(
                        "✖ ",
                        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(msg, Style::default().fg(Color::Red)),
                ],
                display_msg,
            )
        }
    } else if res_opt.is_some() {
        (
            vec![
                Span::styled(
                    "✔ ",
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    "clean, no violations",
                    Style::default().fg(Color::Rgb(200, 200, 200)),
                ),
            ],
            "✔ clean, no violations".into(),
        )
    } else {
        (
            vec![Span::styled(
                "pending execution",
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::ITALIC),
            )],
            "pending execution".into(),
        )
    };
    builder.push_kv("Trace", t_spans, &t_plain);

    // Failure: ONLY when case failed!
    let case_failed = res_opt
        .map(|r| r.final_status == FinalStatus::Fail || r.auto_status == AutoStatus::AutoFail)
        .unwrap_or(false);
    if case_failed {
        if let Some(r) = res_opt {
            let msg = r
                .message
                .clone()
                .unwrap_or_else(|| "Verification failed".to_string());
            builder.push_kv(
                "Failure",
                vec![
                    Span::styled(
                        "✖ ",
                        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(msg.clone(), Style::default().fg(Color::Red)),
                ],
                &format!("✖ {}", msg),
            );
        }
    }

    // Exactly 1 blank row between sections
    builder.push_blank();
}
