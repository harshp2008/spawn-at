//! Drawer content ordering, formatting, geometry table, and timeline tests.

use super::{create_test_app, dump_buffer_to_string};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{backend::TestBackend, style::Color, Terminal};
use spawn_at_verify::runner::{AutoStatus, FinalStatus, HumanVerdict, HumanVerdictKind};
use spawn_at_verify::tui::views::draw_ui;

#[test]
fn test_tui_detail_drawer_content_ordering_and_failure_box() {
    let mut app = create_test_app();
    let test = app.selected_test().unwrap().clone();

    // Insert a failed result with diagnostic message
    let failed_result = spawn_at_verify::runner::TestResult {
        id: test.id.clone(),
        title: test.title.clone(),
        kind: test.kind,
        required: test.required,
        auto_status: AutoStatus::AutoFail,
        human_verdict: HumanVerdict::default(),
        final_status: FinalStatus::Fail,
        disagreement: false,
        duration_ms: 42,
        exit_code: Some(1),
        stdout: r#"{"window_id":123,"x":100,"y":200,"w":400,"h":300,"source":"claim"}"#.into(),
        stderr: "[spawn-at] ERROR: Failed window alignment\n".into(),
        message: Some("Window mapped at (100, 200) but expected (500, 500)".into()),
        log_slice: Some(
            "[spawn-at-time] +0.00ms | CLOAK\n[spawn-at-time] +5.00ms | VIOLATION: visible flash"
                .into(),
        ),
    };
    app.results.insert(test.id.clone(), failed_result);

    let backend = TestBackend::new(140, 35);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal
        .draw(|f| {
            draw_ui(f, &app);
        })
        .unwrap();

    let buffer = terminal.backend().buffer().clone();
    let text: String = buffer.content().iter().map(|cell| cell.symbol()).collect();

    // Verify Result box appears on main pane
    assert!(text.contains("Result"));
    assert!(text.contains("✖ Auto: FAIL"));
    assert!(text.contains("Window mapped at (100, 200)"));

    // Open detail drawer to verify drawer content ordering
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(app.detail_drawer_open);

    terminal
        .draw(|f| {
            draw_ui(f, &app);
        })
        .unwrap();

    let drawer_buf = terminal.backend().buffer().clone();
    let drawer_text: String = drawer_buf
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect();

    // Verify detail drawer ordering: RESULT, GEOMETRY, TIMELINE visible on screen
    assert!(drawer_text.contains("RESULT"));
    assert!(drawer_text.contains("GEOMETRY"));
    assert!(drawer_text.contains("TIMELINE"));

    // Verify detail drawer ordering across all sections in full drawer content
    let (_lines, plain) = spawn_at_verify::tui::views::build_drawer_content(&app, &test, 140, true);
    assert!(plain.contains("RESULT"));
    assert!(plain.contains("GEOMETRY"));
    assert!(plain.contains("TIMELINE"));
    assert!(plain.contains("WHAT TO LOOK FOR"));
    assert!(plain.contains("SPEC"));
    assert!(plain.contains("OUTPUT"));
    let pos_res = plain.find("RESULT").unwrap();
    let pos_geo = plain.find("GEOMETRY").unwrap();
    let pos_time = plain.find("TIMELINE").unwrap();
    let pos_what = plain.find("WHAT TO LOOK FOR").unwrap();
    let pos_spec = plain.find("SPEC").unwrap();
    let pos_out = plain.find("OUTPUT").unwrap();
    assert!(pos_res < pos_geo);
    assert!(pos_geo < pos_time);
    assert!(pos_time < pos_what);
    assert!(pos_what < pos_spec);
    assert!(pos_spec < pos_out);
    assert!(!plain.contains("---"));
    assert!(!plain.contains("1. "));
    // In raw view, raw trace lines are included
    let (_raw_lines, raw_plain) =
        spawn_at_verify::tui::views::build_drawer_content(&app, &test, 140, false);
    assert!(raw_plain.contains("visible flash"));
}

#[test]
fn test_drawer_sections_order_and_formatting() {
    let app = create_test_app();
    let test = app.selected_test().unwrap().clone();

    let (_lines, plain) = spawn_at_verify::tui::views::build_drawer_content(&app, &test, 80, true);

    // Sections must appear in exact order: RESULT, GEOMETRY, TIMELINE, WHAT TO LOOK FOR, SPEC, OUTPUT
    let pos_result = plain.find("RESULT").expect("RESULT section missing");
    let pos_geometry = plain.find("GEOMETRY").expect("GEOMETRY section missing");
    let pos_timeline = plain.find("TIMELINE").expect("TIMELINE section missing");
    let pos_what = plain
        .find("WHAT TO LOOK FOR")
        .expect("WHAT TO LOOK FOR section missing");
    let pos_spec = plain.find("SPEC").expect("SPEC section missing");
    let pos_output = plain.find("OUTPUT").expect("OUTPUT section missing");

    assert!(pos_result < pos_geometry, "RESULT must precede GEOMETRY");
    assert!(
        pos_geometry < pos_timeline,
        "GEOMETRY must precede TIMELINE"
    );
    assert!(
        pos_timeline < pos_what,
        "TIMELINE must precede WHAT TO LOOK FOR"
    );
    assert!(pos_what < pos_spec, "WHAT TO LOOK FOR must precede SPEC");
    assert!(pos_spec < pos_output, "SPEC must precede OUTPUT");

    // No "---" and no numbering like "1. Name"
    assert!(
        !plain.contains("---"),
        "Drawer must not contain '---' headings"
    );
    assert!(
        !plain.contains("1. "),
        "Drawer must not contain numbered headings '1. '"
    );
    assert!(
        !plain.contains("2. "),
        "Drawer must not contain numbered headings '2. '"
    );
    assert!(
        !plain.contains("3. "),
        "Drawer must not contain numbered headings '3. '"
    );
}

/// Injects unique window ID `9876543210` into stdout JSON and renders compact drawer at width 80.
/// Asserts the window ID string appears exactly once in the rendered plain text output.
/// Fails if the compact drawer duplicates window ID in both facts and raw stdout blocks.
#[test]
fn test_stdout_window_id_appears_exactly_once_in_compact_drawer() {
    let mut app = create_test_app();
    let test = app.selected_test().unwrap().clone();

    let unique_win_id = 9876543210u64;
    let res = spawn_at_verify::runner::TestResult {
        id: test.id.clone(),
        title: test.title.clone(),
        kind: test.kind,
        required: test.required,
        auto_status: AutoStatus::AutoPass,
        human_verdict: HumanVerdict::default(),
        final_status: FinalStatus::Pass,
        disagreement: false,
        duration_ms: 35,
        exit_code: Some(0),
        stdout: format!(
            r#"{{"window_id":{},"x":1504,"y":764,"w":400,"h":300,"rel_x":1504,"rel_y":724,"area":"workarea","source":"claim"}}"#,
            unique_win_id
        ),
        stderr: "[spawn-at] INFO: Window mapped at final size (400x300)\n".into(),
        message: None,
        log_slice: None,
    };
    app.results.insert(test.id.clone(), res);

    let (_lines, plain) = spawn_at_verify::tui::views::build_drawer_content(&app, &test, 80, true);

    let id_str = unique_win_id.to_string();
    let count = plain.matches(&id_str).count();
    assert_eq!(
        count, 1,
        "The stdout window_id value '{}' must appear exactly once in the compact drawer, found {}:\n{}",
        id_str, count, plain
    );
}

/// Renders compact drawer at width 80 with raw log lines containing `[spawn-at-time]` and `proto=wayland`.
/// Asserts the rendered TIMELINE section contains neither substring, keeping output concise.
/// Fails if log cleanup regexes fail to strip logger prefix tags or protocol metadata.
#[test]
fn test_compact_timeline_contains_neither_prefix_nor_proto() {
    let mut app = create_test_app();
    let test = app.selected_test().unwrap().clone();

    let trace_log = "\
[spawn-at-time] +0.0ms | ARM_SPAWN win=[id=870906018 pid=1083136 class=\"verify.placement.bottomright\" app=\"verify.placement.bottomright\" type=normal proto=wayland] target=verify.placement.bottomright
[spawn-at-time] +10.0ms | WINDOW_CREATED win=[id=870906018 pid=1083136 class=\"verify.placement.bottomright\" app=\"verify.placement.bottomright\" type=normal proto=wayland] actor found
[spawn-at-time] +56.1ms | ACTOR_MAP win=[id=870906018 pid=1083136 class=\"verify.placement.bottomright\" app=\"verify.placement.bottomright\" type=normal proto=wayland]
[spawn-at-time] +56.2ms | CLOAK win=[id=870906018 pid=1083136 class=\"verify.placement.bottomright\" app=\"verify.placement.bottomright\" type=normal proto=wayland] opacity=0 frame=(50,90,400x300)
[spawn-at-time] +56.3ms | SET_SIZE_REQUEST win=[id=870906018 pid=1083136 class=\"verify.placement.bottomright\" app=\"verify.placement.bottomright\" type=normal proto=wayland] requested=400x300 clamped=400x300
[spawn-at-time] +56.4ms | COMMIT_RESOLVED win=[id=870906018 pid=1083136 class=\"verify.placement.bottomright\" app=\"verify.placement.bottomright\" type=normal proto=wayland] 400x300
";

    let res = spawn_at_verify::runner::TestResult {
        id: test.id.clone(),
        title: test.title.clone(),
        kind: test.kind,
        required: test.required,
        auto_status: AutoStatus::AutoPass,
        human_verdict: HumanVerdict::default(),
        final_status: FinalStatus::Pass,
        disagreement: false,
        duration_ms: 60,
        exit_code: Some(0),
        stdout: r#"{"window_id":870906018,"pid":1083136}"#.into(),
        stderr: String::new(),
        message: None,
        log_slice: Some(trace_log.into()),
    };
    app.results.insert(test.id.clone(), res);

    let (_lines, plain) = spawn_at_verify::tui::views::build_drawer_content(&app, &test, 80, true);

    // Find the TIMELINE section portion of plain text
    let timeline_idx = plain.find("TIMELINE").expect("TIMELINE section exists");
    let next_section_idx = plain
        .find("WHAT TO LOOK FOR")
        .expect("WHAT TO LOOK FOR section exists");
    let timeline_section = &plain[timeline_idx..next_section_idx];

    assert!(
        !timeline_section.contains("[spawn-at-time]"),
        "Compact timeline must not contain '[spawn-at-time]', found in:\n{}",
        timeline_section
    );
    assert!(
        !timeline_section.contains("proto="),
        "Compact timeline must not contain 'proto=', found in:\n{}",
        timeline_section
    );
}

#[test]
fn test_label_values_alignment_across_section() {
    let mut app = create_test_app();
    let test = app.selected_test().unwrap().clone();

    let res = spawn_at_verify::runner::TestResult {
        id: test.id.clone(),
        title: test.title.clone(),
        kind: test.kind,
        required: test.required,
        auto_status: AutoStatus::AutoPass,
        human_verdict: HumanVerdict {
            verdict: HumanVerdictKind::Pass,
            note: None,
        },
        final_status: FinalStatus::Pass,
        disagreement: false,
        duration_ms: 36,
        exit_code: Some(0),
        stdout: r#"{"x":1504,"y":764,"w":400,"h":300,"source":"claim"}"#.into(),
        stderr: String::new(),
        message: None,
        log_slice: None,
    };
    app.results.insert(test.id.clone(), res);

    let (_lines, plain) = spawn_at_verify::tui::views::build_drawer_content(&app, &test, 80, true);

    // In RESULT section:
    // "  Verdict       ✔ Pass"
    // "  Cloak         36 ms..."
    // "  Trace         ✔ clean..."
    // Inner padding 2, label column 14 wide -> values start at index 16 (0-based)
    let mut verified = 0;
    for line in plain.lines() {
        if line.starts_with("  Verdict ")
            || line.starts_with("  Cloak ")
            || line.starts_with("  Trace ")
            || line.starts_with("  Budget ")
        {
            assert!(line.starts_with("  "), "KV line must start with 2 spaces");
            let val_char = line.chars().nth(16).unwrap_or(' ');
            assert_ne!(
                val_char, ' ',
                "Value in line '{}' must start at column 16",
                line
            );
            verified += 1;
        }
    }
    assert!(
        verified >= 3,
        "Must have verified alignment on at least 3 rows"
    );
}

#[test]
fn test_render_drawer_at_60x28_80x28_140x28_no_clipped_headings() {
    for &w in &[60, 80, 140] {
        let mut app = create_test_app();
        app.fixture_exists = true;
        app.detail_drawer_open = true;

        let backend = TestBackend::new(w, 28);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| draw_ui(f, &app)).unwrap();
        let rendered = dump_buffer_to_string(&terminal, w, 28);

        // Heading lines: RESULT, GEOMETRY, TIMELINE must appear without corruption
        assert!(
            rendered.contains("RESULT"),
            "Heading RESULT must not be clipped at width {}",
            w
        );
        assert!(
            rendered.contains("GEOMETRY"),
            "Heading GEOMETRY must not be clipped at width {}",
            w
        );
        if w > 80 {
            assert!(
                rendered.contains("TIMELINE"),
                "Heading TIMELINE must not be clipped at width {}",
                w
            );
        } else {
            app.detail_scroll = 5;
            terminal.draw(|f| draw_ui(f, &app)).unwrap();
            let scrolled_rendered = dump_buffer_to_string(&terminal, w, 28);
            assert!(
                scrolled_rendered.contains("TIMELINE"),
                "Heading TIMELINE must not be clipped at width {}",
                w
            );
        }
    }
}

#[test]
fn test_multisize_drawer_rendering_and_token_wrapping() {
    let trace_log_12 = "\
[spawn-at-time] +0.0ms | ARM_SPAWN win=[id=870906018 pid=1083136 class=\"verify.placement.bottomright\" app=\"verify.placement.bottomright\" type=normal proto=wayland] target=verify.placement.bottomright
[spawn-at-time] +10.0ms | WINDOW_CREATED win=[id=870906018 pid=1083136 class=\"verify.placement.bottomright\" app=\"verify.placement.bottomright\" type=normal proto=wayland] actor found
[spawn-at-time] +56.1ms | ACTOR_MAP win=[id=870906018 pid=1083136 class=\"verify.placement.bottomright\" app=\"verify.placement.bottomright\" type=normal proto=wayland]
[spawn-at-time] +56.2ms | CLOAK win=[id=870906018 pid=1083136 class=\"verify.placement.bottomright\" app=\"verify.placement.bottomright\" type=normal proto=wayland] opacity=0 frame=(50,90,400x300)
[spawn-at-time] +56.3ms | SET_SIZE_REQUEST win=[id=870906018 pid=1083136 class=\"verify.placement.bottomright\" app=\"verify.placement.bottomright\" type=normal proto=wayland] requested=400x300 clamped=400x300
[spawn-at-time] +56.4ms | COMMIT_RESOLVED win=[id=870906018 pid=1083136 class=\"verify.placement.bottomright\" app=\"verify.placement.bottomright\" type=normal proto=wayland] 400x300
[spawn-at-time] +60.0ms | BUFFER_ATTACH win=[id=870906018 pid=1083136 class=\"verify.placement.bottomright\" app=\"verify.placement.bottomright\" type=normal proto=wayland]
[spawn-at-time] +65.0ms | FIRST_FRAME win=[id=870906018 pid=1083136 class=\"verify.placement.bottomright\" app=\"verify.placement.bottomright\" type=normal proto=wayland]
[spawn-at-time] +70.0ms | UNCLOAK win=[id=870906018 pid=1083136 class=\"verify.placement.bottomright\" app=\"verify.placement.bottomright\" type=normal proto=wayland] opacity=255
[spawn-at-time] +75.0ms | AT_TARGET win=[id=870906018 pid=1083136 class=\"verify.placement.bottomright\" app=\"verify.placement.bottomright\" type=normal proto=wayland]
[spawn-at-time] +80.0ms | FOCUS_IN win=[id=870906018 pid=1083136 class=\"verify.placement.bottomright\" app=\"verify.placement.bottomright\" type=normal proto=wayland]
[spawn-at-time] +85.0ms | RESTORE win=[id=870906018 pid=1083136 class=\"verify.placement.bottomright\" app=\"verify.placement.bottomright\" type=normal proto=wayland]";

    for &w in &[50, 60, 70, 80, 100, 140] {
        for &_h in &[20, 28] {
            let mut app = create_test_app();
            let mut test = app.selected_test().unwrap().clone();
            test.description = "Verifies that the window is placed strictly in the bottom-right corner of the primary display workarea with zero visual displacement or clipping artifact on initial map.".into();

            let res = spawn_at_verify::runner::TestResult {
                id: test.id.clone(),
                title: test.title.clone(),
                kind: test.kind,
                required: test.required,
                auto_status: AutoStatus::AutoPass,
                human_verdict: HumanVerdict {
                    verdict: HumanVerdictKind::Pass,
                    note: None,
                },
                final_status: FinalStatus::Pass,
                disagreement: false,
                duration_ms: 85,
                exit_code: Some(0),
                stdout: r#"{"window_id":870906018,"x":1504,"y":764,"w":400,"h":300,"rel_x":1504,"rel_y":724,"area":"workarea","source":"claim"}"#.into(),
                stderr: "[spawn-at] INFO: Window mapped at final size (400x300)\n".into(),
                message: None,
                log_slice: Some(trace_log_12.into()),
            };
            app.results.insert(test.id.clone(), res);

            let (_lines, plain) =
                spawn_at_verify::tui::views::build_drawer_content(&app, &test, w as usize, true);

            // Assert tokens are intact on lines where they fit
            if w >= 70 {
                assert!(
                    plain.contains("verify.placement.bottomright"),
                    "ARM_SPAWN target token intact at width {}",
                    w
                );
            } else {
                assert!(
                    plain.contains("verify.placement."),
                    "ARM_SPAWN target token broken cleanly at width {}",
                    w
                );
            }
            if w >= 60 {
                assert!(
                    plain.contains("AT_TARGET") || plain.contains("at target"),
                    "AT_TARGET intact at width {}",
                    w
                );
                assert!(
                    plain.contains("50,90 400×300"),
                    "Frame rect token intact at width {}",
                    w
                );
            }
            assert!(
                plain.contains("400×300"),
                "Geometry 400×300 token intact at width {}",
                w
            );

            // Check that header continuation rows start with 16 spaces (hanging indent)
            let timeline_pos = plain.find("── TIMELINE").unwrap_or(plain.len());
            let header_part = &plain[..timeline_pos];
            for line in header_part.lines() {
                if line.starts_with("                ") {
                    let first_char = line.chars().nth(16).unwrap_or(' ');
                    assert_ne!(
                        first_char, ' ',
                        "Continuation line must start content at column 16 at width {}: '{}'",
                        w, line
                    );
                }
            }
        }
    }
}

#[test]
fn test_synthetic_token_hard_wrap_alignment_at_width_50() {
    let app = create_test_app();
    let mut test = app.selected_test().unwrap().clone();
    test.description = "A".repeat(60); // 60-char synthetic unbroken token

    let (_lines, plain) = spawn_at_verify::tui::views::build_drawer_content(&app, &test, 50, true);
    let desc_lines: Vec<&str> = plain.lines().filter(|l| l.contains("AAAA")).collect();
    assert!(
        desc_lines.len() >= 2,
        "60-char token must wrap into at least 2 lines at width 50"
    );
    // Continuation line must have exactly 16 spaces of indent
    let second_line = desc_lines[1];
    assert!(
        second_line.starts_with("                "),
        "Continuation row must have 16 leading spaces at width 50"
    );
    let c = second_line.chars().nth(16).unwrap_or(' ');
    assert_ne!(c, ' ', "Continuation row must start content at column 16");
}

#[test]
fn test_506_9_ms_cloak_yellow_warning_does_not_fail_pass() {
    let mut app = create_test_app();
    let test = app.selected_test().unwrap().clone();

    let res = spawn_at_verify::runner::TestResult {
        id: test.id.clone(),
        title: test.title.clone(),
        kind: test.kind,
        required: test.required,
        auto_status: AutoStatus::AutoPass,
        human_verdict: HumanVerdict {
            verdict: HumanVerdictKind::Pass,
            note: None,
        },
        final_status: FinalStatus::Pass,
        disagreement: false,
        duration_ms: 507,
        exit_code: Some(0),
        stdout: r#"{"window_id":870906018,"x":1504,"y":764,"w":400,"h":300}"#.into(),
        stderr: String::new(),
        message: Some("Assertions passed".into()),
        log_slice: Some(
            "[spawn-at-time] +0.0ms | CLOAK\n[spawn-at-time] +506.9ms | UNCLOAK".into(),
        ),
    };
    app.results.insert(test.id.clone(), res);

    let (lines, plain) = spawn_at_verify::tui::views::build_drawer_content(&app, &test, 80, true);
    assert!(plain.contains("506.9 ms"));
    assert!(plain.contains("over provisional budget"));
    assert!(!plain.contains("Failure:"));

    // Check styling: yellow '!', no red cell in RESULT section
    let mut found_yellow_warning = false;
    let mut found_red_in_result = false;
    let mut in_result = false;
    for line in &lines {
        let line_text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        if line_text.contains("RESULT") {
            in_result = true;
        } else if line_text.contains("GEOMETRY") {
            in_result = false;
        }
        if in_result {
            for span in &line.spans {
                if span.content.contains("!") && span.style.fg == Some(Color::Yellow) {
                    found_yellow_warning = true;
                }
                if span.style.fg == Some(Color::Red) {
                    found_red_in_result = true;
                }
            }
        }
    }
    assert!(
        found_yellow_warning,
        "Must render yellow '!' for 506.9 ms cloak budget warning"
    );
    assert!(
        !found_red_in_result,
        "Passing case with 506.9 ms cloak must have zero red cells in RESULT"
    );
}

#[test]
fn test_failing_case_renders_failure_row_with_real_reason_in_red() {
    let mut app = create_test_app();
    let test = app.selected_test().unwrap().clone();

    let res = spawn_at_verify::runner::TestResult {
        id: test.id.clone(),
        title: test.title.clone(),
        kind: test.kind,
        required: test.required,
        auto_status: AutoStatus::AutoFail,
        human_verdict: HumanVerdict::default(),
        final_status: FinalStatus::Fail,
        disagreement: false,
        duration_ms: 45,
        exit_code: Some(1),
        stdout: r#"{"window_id":870906018,"x":1200,"y":600,"w":400,"h":300}"#.into(),
        stderr: "[spawn-at] ERROR: Geometric target mismatch".into(),
        message: Some("Window mapped at (1200, 600) but expected (1504, 764)".into()),
        log_slice: None,
    };
    app.results.insert(test.id.clone(), res);

    let (lines, plain) = spawn_at_verify::tui::views::build_drawer_content(&app, &test, 80, true);
    assert!(plain.contains("Failure"));
    assert!(plain.contains("Window mapped at (1200, 600) but expected (1504, 764)"));

    let mut found_red_failure = false;
    for line in &lines {
        for span in &line.spans {
            if span.content.contains("Window mapped at") && span.style.fg == Some(Color::Red) {
                found_red_failure = true;
            }
        }
    }
    assert!(
        found_red_failure,
        "Failure row message must be styled in Red"
    );
}
