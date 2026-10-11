//! Drawer RESULT section rendering (Verdict, Cloak duration, provisional timing budget).

use crate::oracle::GeometryStatus;
use crate::runner::{AutoStatus, FinalStatus, HumanVerdictKind, TestResult};
use crate::schema::{TestDefinition, TestKind};
use crate::tui::views::builder::DrawerBuilder;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;

/// Renders the RESULT section with verdict banners, timings, and failure breakdowns.
pub fn render_result_section(
    builder: &mut DrawerBuilder,
    test: &TestDefinition,
    res_opt: Option<&TestResult>,
    env_workarea: crate::oracle::WorkareaProbe,
) {
    // One geometry comparison for the whole section (Verdict, Geometry row).
    let geometry_check = test.expect.rect.as_ref().map(|rect| {
        res_opt
            .and_then(|r| r.geometry_check(test, env_workarea))
            .unwrap_or_else(|| crate::oracle::check_geometry(rect, "", None, env_workarea))
    });
    // A passed run whose geometry could not be verified is not shown as a plain Pass.
    let geometry_unchecked = res_opt.is_some()
        && geometry_check
            .as_ref()
            .is_some_and(|c| c.status() == GeometryStatus::Unchecked);
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
                        FinalStatus::PassUnchecked => unchecked_verdict(),
                        FinalStatus::Pass if geometry_unchecked => unchecked_verdict(),
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

    let trace_report = res_opt.and_then(|r| r.trace_report());

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

    // Geometry: the same comparison as the GEOMETRY table and the runner's verdict.
    if let Some(check) = &geometry_check {
        let tol = check.tolerance_px;
        let grey = Color::Rgb(200, 200, 200);
        let warn = Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD);
        let (g_spans, g_plain): (Vec<Span<'static>>, String) = match check.status() {
            GeometryStatus::Pass if check.workarea_source.is_extension_derived() => {
                // The extension supplied its own workarea: judged, but not independent evidence.
                let msg = format!(
                    "within ±{} px (workarea from the extension, not independently probed)",
                    tol
                );
                (
                    vec![
                        Span::styled("✔ ", warn),
                        Span::styled(msg.clone(), Style::default().fg(Color::Yellow)),
                    ],
                    format!("✔ {}", msg),
                )
            }
            GeometryStatus::Pass => (
                vec![
                    Span::styled(
                        "✔ ",
                        Style::default()
                            .fg(Color::Green)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        format!("within ±{} px of expected", tol),
                        Style::default().fg(grey),
                    ),
                ],
                format!("✔ within ±{} px of expected", tol),
            ),
            GeometryStatus::Fail => {
                let (dx, dy, dw, dh) = check.deltas().unwrap_or((0, 0, 0, 0));
                let msg = format!(
                    "off expected by dx={:+} dy={:+} dw={:+} dh={:+} (tolerance ±{} px)",
                    dx, dy, dw, dh, tol
                );
                (
                    vec![
                        Span::styled(
                            "✘ ",
                            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(msg.clone(), Style::default().fg(Color::Red)),
                    ],
                    format!("✘ {}", msg),
                )
            }
            GeometryStatus::Unchecked => {
                let msg =
                    "unchecked: no workarea available from the window system or the extension";
                (
                    vec![
                        Span::styled("⚠ ", warn),
                        Span::styled(msg, Style::default().fg(Color::Yellow)),
                    ],
                    format!("⚠ {}", msg),
                )
            }
            GeometryStatus::NotMeasured => (
                vec![Span::styled(
                    "not measured",
                    Style::default()
                        .fg(Color::DarkGray)
                        .add_modifier(Modifier::ITALIC),
                )],
                "not measured".into(),
            ),
        };
        builder.push_kv("Geometry", g_spans, &g_plain);
        if let Some(w) = check.warning() {
            builder.push_kv(
                "Warning",
                vec![
                    Span::styled("⚠ ", warn),
                    Span::styled(w.clone(), Style::default().fg(Color::Yellow)),
                ],
                &format!("⚠ {}", w),
            );
        }
    }

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

/// Verdict cell for an automatic test that passed without its geometry being verified: a yellow
/// warning marker and "Pass (geometry unchecked)", distinct from the green "✔ Pass".
fn unchecked_verdict() -> (Vec<Span<'static>>, String) {
    (
        vec![
            Span::styled(
                "⚠ ",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "Pass (geometry unchecked)",
                Style::default().fg(Color::Yellow),
            ),
        ],
        "⚠ Pass (geometry unchecked)".into(),
    )
}
