//! UI polish, padding, spacing, and characterization specification tests.

use super::{create_test_app, dump_buffer_to_string};
use ratatui::{backend::TestBackend, Terminal};
use spawn_at_verify::runner::{AutoStatus, FinalStatus, HumanVerdict};
use spawn_at_verify::schema::TestKind;
use spawn_at_verify::tui::views::draw_ui;

/// Asserts UI layout polish across 60, 80, and 140 columns at height 28:
/// 1. Row 0 (above header), row 2 (below header), and the bottom row are blank.
/// 2. List rows have padding before the right border so "auto│", "both│", or "human│" never touch it.
/// 3. At 100x30, Result box lines use a 2-char symbol slot ("✔ Auto: PASS (121 ms)", "? You: pending review").
/// Fails if panel borders shift into padding rows or symbol glyph alignment drifts.
#[test]
fn test_ui_polish_spacing_padding_outcome_symbols_and_result_box() {
    let mut app = create_test_app();
    app.fixture_exists = true;

    // Assert: blank row below keys bar, blank row after header, right padding in list rows, Result box present at 60, 80 and 140 columns
    for &w in &[60, 80, 140] {
        let backend = TestBackend::new(w, 28);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| draw_ui(f, &app)).unwrap();
        let rendered = dump_buffer_to_string(&terminal, w, 28);

        // Result box present at 60, 80 and 140 columns
        assert!(
            rendered.contains(" Result "),
            "Result box must be present as boxed panel at {} columns",
            w
        );

        let lines: Vec<&str> = rendered.lines().collect();

        // Blank row below keys bar
        assert!(
            lines.last().unwrap().trim().is_empty(),
            "Blank row below keys bar at bottom must be present at {} cols",
            w
        );

        // Blank row above header
        assert!(
            lines[0].trim().is_empty(),
            "Blank row above header must be present at {} cols",
            w
        );

        // Blank row after header
        assert!(
            lines[2].trim().is_empty(),
            "Blank row after header must be present at {} cols",
            w
        );

        // Right padding in list rows so "auto"/"both" never touch the border
        for line in &lines {
            if line.contains('│') {
                assert!(
                    !line.contains("auto│") && !line.contains("both│") && !line.contains("human│"),
                    "Right padding in list rows required so kind never touches border: '{}'",
                    line
                );
            }
        }
    }

    // Assert: aligned outcome symbols with fixed 2-char symbol slot
    let mut outcome_app = create_test_app();
    outcome_app.fixture_exists = true;
    if let Some(pos) = outcome_app
        .filtered_tests()
        .iter()
        .position(|t| t.id == "placement/bottom-right")
    {
        outcome_app.selected_index = pos;
    }
    let res = spawn_at_verify::runner::TestResult {
        id: "placement/bottom-right".into(),
        title: "Window placed in bottom-right corner".into(),
        kind: TestKind::Both,
        required: true,
        auto_status: AutoStatus::AutoPass,
        human_verdict: HumanVerdict::default(), // Unset -> "? You: pending review"
        final_status: FinalStatus::Pending,
        disagreement: false,
        duration_ms: 121,
        exit_code: Some(0),
        stdout: r#"{"window_id":1002,"x":1504,"y":764,"w":400,"h":300}"#.into(),
        stderr: String::new(),
        message: Some("Assertions passed".into()),
        log_slice: Some(
            "+0.00ms | CLOAK win=[id=1002 pid=2002 class=\"test\" app=\"test\" type=NORMAL proto=Wayland] opacity=0 frame=(50,50,400x300)\n+32.50ms | UNCLOAK win=[id=1002 pid=2002 class=\"test\" app=\"test\" type=NORMAL proto=Wayland] opacity=255 frame=(50,50,400x300)".into(),
        ),
        trace_note: None, final_rect: None, flash_info: Default::default(),
    };
    outcome_app
        .results
        .insert("placement/bottom-right".into(), res);

    let backend = TestBackend::new(100, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| draw_ui(f, &outcome_app)).unwrap();
    let rendered = dump_buffer_to_string(&terminal, 100, 30);

    // Verify fixed 2-char symbol slot outcome lines are rendered and aligned
    assert!(
        rendered.contains("✔ Auto: PASS (121 ms)"),
        "Rendered must contain aligned '✔ Auto: PASS (121 ms)'"
    );
    assert!(
        rendered.contains("  Cloak: 32.5 ms · Trace: 0 violations"),
        "Rendered must contain aligned '  Cloak: 32.5 ms · Trace: 0 violations'"
    );
    assert!(
        rendered.contains("? You: pending review"),
        "Rendered must contain aligned '? You: pending review'"
    );
}

/// Verifies blank-row vertical spacing rules across resolutions 60x28, 80x28, and 140x28:
/// 1. In idle state, row 0 (above header), row 2 (between header and first box), and row 27 (below keys bar) are blank.
/// 2. Exactly 1 blank row separates the bottom border of the Result box from the keys bar in both idle and waiting states.
/// 3. In stacked 60x28 mode, exactly 1 blank row sits between the Tests box bottom border and Description box top border.
/// Fails if vertical layout constraints add or drop a row, colliding widgets or breaking terminal height limits.
#[test]
fn test_result_and_spacing_polish_spec() {
    let resolutions = [(60, 28), (80, 28), (140, 28)];

    for &(w, h) in &resolutions {
        // --- 1. Idle state checks ---
        let mut app = create_test_app();
        app.fixture_exists = true;
        let backend = TestBackend::new(w, h);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| draw_ui(f, &app)).unwrap();
        let idle_rendered = dump_buffer_to_string(&terminal, w, h);
        let idle_lines: Vec<&str> = idle_rendered.lines().collect();

        // 1 blank row above header
        assert!(
            idle_lines[0].trim().is_empty(),
            "1 blank row above header required at {}x{}",
            w,
            h
        );
        // 1 blank row between header and first box
        assert!(
            idle_lines[2].trim().is_empty(),
            "1 blank row between header and first box required at {}x{}",
            w,
            h
        );
        // 1 blank row at the bottom below keys bar
        assert!(
            idle_lines[h as usize - 1].trim().is_empty(),
            "1 blank row at bottom required at {}x{}",
            w,
            h
        );

        // Find Result box borders in idle state
        let idle_res_top = idle_lines
            .iter()
            .position(|l| l.contains("┌ Result "))
            .expect("Result top border must exist");
        let idle_res_bottom = idle_lines[idle_res_top..]
            .iter()
            .position(|l| l.contains('└'))
            .map(|idx| idle_res_top + idx)
            .expect("Result bottom border must exist");

        // Exactly 1 blank row between Result box and keys bar
        assert!(
            idle_lines[idle_res_bottom + 1].trim().is_empty(),
            "Exactly 1 blank row required between Result box and keys bar at {}x{} (idle)",
            w,
            h
        );

        // --- 2. Waiting-for-verdict state checks (with long result text) ---
        let mut wait_app = create_test_app();
        wait_app.fixture_exists = true;
        if let Some(pos) = wait_app
            .filtered_tests()
            .iter()
            .position(|t| t.id == "placement/bottom-right")
        {
            wait_app.selected_index = pos;
        }
        wait_app.waiting_for_human = Some("placement/bottom-right".to_string());
        let wait_res = spawn_at_verify::runner::TestResult {
            id: "placement/bottom-right".into(),
            title: "Window placed in bottom-right corner".into(),
            kind: TestKind::Both,
            required: true,
            auto_status: AutoStatus::AutoPass,
            human_verdict: HumanVerdict::default(),
            final_status: FinalStatus::Pending,
            disagreement: false,
            duration_ms: 185,
            exit_code: Some(0),
            stdout: r#"{"window_id":1002,"x":1504,"y":764,"w":400,"h":300}"#.into(),
            stderr: String::new(),
            message: Some("Assertions passed".into()),
            log_slice: Some("[spawn-at-time] +0.00ms | CLOAK".into()),
            trace_note: None,
            final_rect: None,
            flash_info: Default::default(),
        };
        wait_app
            .results
            .insert("placement/bottom-right".into(), wait_res);

        let wait_backend = TestBackend::new(w, h);
        let mut wait_term = Terminal::new(wait_backend).unwrap();
        wait_term.draw(|f| draw_ui(f, &wait_app)).unwrap();
        let wait_rendered = dump_buffer_to_string(&wait_term, w, h);
        let wait_lines: Vec<&str> = wait_rendered.lines().collect();

        // Check stacked box spacing:
        if w < 75 {
            // Narrow layout (60x28): Tests / GAP / Description / GAP / Result / GAP / keys bar / 1 blank bottom row
            let tests_top = wait_lines
                .iter()
                .position(|l| l.contains("┌ Tests"))
                .expect("Tests top border");
            let tests_bottom = wait_lines[tests_top..]
                .iter()
                .position(|l| l.contains('└'))
                .map(|i| tests_top + i)
                .expect("Tests bottom border");

            // Exactly 1 blank row between Tests and Description
            assert!(
                wait_lines[tests_bottom + 1].trim().is_empty(),
                "Exactly 1 blank row required between Tests and Description at {}x{}",
                w,
                h
            );
            assert!(
                wait_lines[tests_bottom + 2].contains("┌ Description"),
                "Description top border must follow 1 blank row after Tests at {}x{}",
                w,
                h
            );

            let desc_top = tests_bottom + 2;
            let desc_bottom = wait_lines[desc_top..]
                .iter()
                .position(|l| l.contains('└'))
                .map(|i| desc_top + i)
                .expect("Description bottom border");

            // Exactly 1 blank row between Description and Result
            assert!(
                wait_lines[desc_bottom + 1].trim().is_empty(),
                "Exactly 1 blank row required between Description and Result at {}x{}",
                w,
                h
            );
            assert!(
                wait_lines[desc_bottom + 2].contains("┌ Result"),
                "Result top border must follow 1 blank row after Description at {}x{}",
                w,
                h
            );

            let res_top = desc_bottom + 2;
            let res_bottom = wait_lines[res_top..]
                .iter()
                .position(|l| l.contains('└'))
                .map(|i| res_top + i)
                .expect("Result bottom border");

            // Exactly 1 blank row between Result and verdict keys bar
            assert!(
                wait_lines[res_bottom + 1].trim().is_empty(),
                "Exactly 1 blank row required between Result and keys bar at {}x{}",
                w,
                h
            );
            assert!(
                wait_lines[res_bottom + 2].contains('┌')
                    || wait_lines[res_bottom + 2].contains("verdict"),
                "Verdict keys bar must follow 1 blank row after Result at {}x{}",
                w,
                h
            );
        } else {
            // Wide layout (80x28, 140x28): left column = Tests box / 1 blank row GAP / Result box pinned to bottom
            let tests_top = wait_lines
                .iter()
                .position(|l| l.contains("┌ Tests"))
                .expect("Tests top border");
            let tests_bottom = wait_lines[tests_top..]
                .iter()
                .position(|l| l.contains('└'))
                .map(|i| tests_top + i)
                .expect("Tests bottom border");

            // Row directly following Tests bottom border in left column must be blank (GAP)
            let left_w = (w as usize * if w >= 110 { 52 } else { 50 }) / 100;
            let gap_line_left: String = wait_lines[tests_bottom + 1].chars().take(left_w).collect();
            assert!(
                gap_line_left.trim().is_empty(),
                "Left column must have exactly 1 blank row gap between Tests and Result at {}x{}",
                w,
                h
            );

            // Row tests_bottom + 2 has Result top border
            assert!(
                wait_lines[tests_bottom + 2].contains("┌ Result"),
                "Result top border must directly follow 1 blank row after Tests at {}x{}",
                w,
                h
            );

            let res_top = tests_bottom + 2;
            let res_bottom = wait_lines[res_top..]
                .iter()
                .position(|l| {
                    let left_part: String = l.chars().take(left_w).collect();
                    left_part.contains('└')
                })
                .map(|i| res_top + i)
                .expect("Result bottom border");

            // Exactly 1 blank row between Result and verdict keys bar across the screen
            assert!(
                wait_lines[res_bottom + 1].trim().is_empty(),
                "Exactly 1 blank row required between Result and keys bar at {}x{}",
                w,
                h
            );
        }

        // --- 3. Result text must WRAP, never clip ---
        // The watch text is: "Did the window appear once in the bottom-right corner with a small gap from screen edges, without flickering or jumping?"
        // Must contain "corner" (no mid-word truncation to "corn")
        assert!(
            wait_rendered.contains("corner"),
            "Result text must be fully visible and wrap without clipping 'corner' at {}x{}",
            w,
            h
        );
        assert!(
            wait_rendered.contains("flickering"),
            "Result text must wrap without clipping 'flickering' at {}x{}",
            w,
            h
        );
        assert!(
            wait_rendered.contains("jumping?"),
            "Result text must wrap without clipping 'jumping?' at {}x{}",
            w,
            h
        );
        assert!(
            !wait_rendered.contains("bottom-right corn "),
            "Result text must not be cut mid-word 'corn' at {}x{}",
            w,
            h
        );

        // --- 4. Tests box shrinks (with a more-hint) when Result is made taller ---
        let idle_tests_top = idle_lines
            .iter()
            .position(|l| l.contains("┌ Tests"))
            .unwrap();
        let idle_tests_bottom = idle_lines[idle_tests_top..]
            .iter()
            .position(|l| l.contains('└'))
            .unwrap()
            + idle_tests_top;
        let idle_tests_h = idle_tests_bottom - idle_tests_top + 1;

        let wait_tests_top = wait_lines
            .iter()
            .position(|l| l.contains("┌ Tests"))
            .unwrap();
        let wait_tests_bottom = wait_lines[wait_tests_top..]
            .iter()
            .position(|l| l.contains('└'))
            .unwrap()
            + wait_tests_top;
        let wait_tests_h = wait_tests_bottom - wait_tests_top + 1;

        assert!(
            wait_tests_h < idle_tests_h,
            "Tests box height must shrink when Result is taller at {}x{}: wait_h={} vs idle_h={}",
            w,
            h,
            wait_tests_h,
            idle_tests_h
        );

        // At 60x28 and 80x28, when Tests box shrinks, more-hint must appear
        if w <= 80 {
            assert!(
                wait_rendered.contains("more"),
                "Tests box must show more-hint when it shrinks at {}x{}",
                w,
                h
            );
        }
    }

    // At 140x28: verify that if Result grows taller (e.g. multi-line failure message), Tests box shrinks and shows more-hint
    let mut tall_app = create_test_app();
    tall_app.fixture_exists = true;
    let long_fail_msg = "Detailed assertion failure:\nline 1: unexpected coordinates\nline 2: opacity out of bounds\nline 3: frame violation\nline 4: bounding box mismatch\nline 5: cloak window error\nline 6: trace violation detected";
    let tall_res = spawn_at_verify::runner::TestResult {
        id: "placement/bottom-right".into(),
        title: "Window placed in bottom-right corner".into(),
        kind: TestKind::Both,
        required: true,
        auto_status: AutoStatus::AutoFail,
        human_verdict: HumanVerdict::default(),
        final_status: FinalStatus::Fail,
        disagreement: false,
        duration_ms: 210,
        exit_code: Some(1),
        stdout: String::new(),
        stderr: String::new(),
        message: Some(long_fail_msg.into()),
        log_slice: None,
        trace_note: None,
        final_rect: None,
        flash_info: Default::default(),
    };
    tall_app
        .results
        .insert("placement/bottom-right".into(), tall_res);
    if let Some(pos) = tall_app
        .filtered_tests()
        .iter()
        .position(|t| t.id == "placement/bottom-right")
    {
        tall_app.selected_index = pos;
    }
    let tall_backend = TestBackend::new(140, 28);
    let mut tall_term = Terminal::new(tall_backend).unwrap();
    tall_term.draw(|f| draw_ui(f, &tall_app)).unwrap();
    let tall_rendered = dump_buffer_to_string(&tall_term, 140, 28);
    assert!(
        tall_rendered.contains("more"),
        "Tests box must shrink and show more-hint when Result is made taller at 140x28"
    );
}
