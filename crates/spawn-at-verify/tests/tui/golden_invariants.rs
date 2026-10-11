//! Responsive golden file assertions and drawer grid invariant tests.

use super::{create_app_with_real_fixture, dump_buffer_to_string};
use ratatui::{backend::TestBackend, Terminal};
use spawn_at_verify::runner::{FinalStatus, HumanVerdictKind};
use spawn_at_verify::tui::app::ModalState;
use spawn_at_verify::tui::views::draw_ui;

/// Dumps current drawer rendering snapshots to scratch directory for analysis.
#[test]
fn test_dump_snapshots_to_scratch() {
    let resolutions = [(60, 28), (80, 28), (140, 28)];
    let mut out = String::new();

    for &(w, h) in &resolutions {
        // 1. Idle
        let (mut app, _) = create_app_with_real_fixture();
        app.results.clear(); // no results yet
        app.fixture_exists = true;
        let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
        term.draw(|f| draw_ui(f, &app)).unwrap();
        out.push_str(&format!(
            "### SNAPSHOT {}x{} - IDLE\n```\n{}\n```\n\n",
            w,
            h,
            dump_buffer_to_string(&term, w, h)
        ));

        // 2. Waiting for verdict
        let (mut wait_app, _) = create_app_with_real_fixture();
        wait_app.fixture_exists = true;
        wait_app.waiting_for_human = Some("placement/bottom-right".to_string());
        wait_app.status_message =
            Some("Waiting for your verdict on 'placement/bottom-right'".into());
        let mut wait_term = Terminal::new(TestBackend::new(w, h)).unwrap();
        wait_term.draw(|f| draw_ui(f, &wait_app)).unwrap();
        out.push_str(&format!(
            "### SNAPSHOT {}x{} - WAITING FOR A HUMAN CASE\n```\n{}\n```\n\n",
            w,
            h,
            dump_buffer_to_string(&wait_term, w, h)
        ));

        // 3. Passed
        let (mut pass_app, _) = create_app_with_real_fixture();
        pass_app.fixture_exists = true;
        if let Some(r) = pass_app.results.get_mut("placement/bottom-right") {
            r.final_status = FinalStatus::Pass;
            r.human_verdict.verdict = HumanVerdictKind::Pass;
        }
        let mut pass_term = Terminal::new(TestBackend::new(w, h)).unwrap();
        pass_term.draw(|f| draw_ui(f, &pass_app)).unwrap();
        out.push_str(&format!(
            "### SNAPSHOT {}x{} - PASSED\n```\n{}\n```\n\n",
            w,
            h,
            dump_buffer_to_string(&pass_term, w, h)
        ));

        // 4. Failed
        let (mut fail_app, _) = create_app_with_real_fixture();
        fail_app.fixture_exists = true;
        if let Some(r) = fail_app.results.get_mut("placement/bottom-right") {
            r.final_status = FinalStatus::Fail;
            r.human_verdict.verdict = HumanVerdictKind::Fail;
            r.message = Some("Window mapped at (1200, 600) but expected (1504, 764)".into());
        }
        let mut fail_term = Terminal::new(TestBackend::new(w, h)).unwrap();
        fail_term.draw(|f| draw_ui(f, &fail_app)).unwrap();
        out.push_str(&format!(
            "### SNAPSHOT {}x{} - FAILED\n```\n{}\n```\n\n",
            w,
            h,
            dump_buffer_to_string(&fail_term, w, h)
        ));

        // 5. Help modal
        let (mut help_app, _) = create_app_with_real_fixture();
        help_app.fixture_exists = true;
        help_app.modal = ModalState::Help;
        let mut help_term = Terminal::new(TestBackend::new(w, h)).unwrap();
        help_term.draw(|f| draw_ui(f, &help_app)).unwrap();
        out.push_str(&format!(
            "### SNAPSHOT {}x{} - HELP MODAL\n```\n{}\n```\n\n",
            w,
            h,
            dump_buffer_to_string(&help_term, w, h)
        ));

        // 6. Detail drawer: Top
        let (mut d_top_app, _) = create_app_with_real_fixture();
        d_top_app.fixture_exists = true;
        d_top_app.detail_drawer_open = true;
        d_top_app.detail_scroll = 0;
        let mut d_top_term = Terminal::new(TestBackend::new(w, h)).unwrap();
        d_top_term.draw(|f| draw_ui(f, &d_top_app)).unwrap();
        out.push_str(&format!(
            "### SNAPSHOT {}x{} - DETAIL DRAWER (TOP, SCROLL 0)\n```\n{}\n```\n\n",
            w,
            h,
            dump_buffer_to_string(&d_top_term, w, h)
        ));

        // 7. Detail drawer: Middle (Timeline)
        let (mut d_mid_app, _) = create_app_with_real_fixture();
        d_mid_app.fixture_exists = true;
        d_mid_app.detail_drawer_open = true;
        d_mid_app.detail_scroll = 16;
        let mut d_mid_term = Terminal::new(TestBackend::new(w, h)).unwrap();
        d_mid_term.draw(|f| draw_ui(f, &d_mid_app)).unwrap();
        out.push_str(&format!(
            "### SNAPSHOT {}x{} - DETAIL DRAWER (TIMELINE, SCROLL 16)\n```\n{}\n```\n\n",
            w,
            h,
            dump_buffer_to_string(&d_mid_term, w, h)
        ));

        // 8. Detail drawer: Bottom
        let (mut d_bot_app, _) = create_app_with_real_fixture();
        d_bot_app.fixture_exists = true;
        d_bot_app.detail_drawer_open = true;
        d_bot_app.detail_scroll = 40;
        let mut d_bot_term = Terminal::new(TestBackend::new(w, h)).unwrap();
        d_bot_term.draw(|f| draw_ui(f, &d_bot_app)).unwrap();
        out.push_str(&format!(
            "### SNAPSHOT {}x{} - DETAIL DRAWER (BOTTOM, SCROLL 40)\n```\n{}\n```\n\n",
            w,
            h,
            dump_buffer_to_string(&d_bot_term, w, h)
        ));
    }

    let scratch_path =
        "/home/harsh/.gemini/antigravity-ide/brain/c19dfab5-8922-4446-88cf-50dca990ff83/scratch/snapshots.txt";
    let _ = std::fs::write(scratch_path, out);
}

/// Compares rendered detail drawer output against golden snapshot files.
#[test]
fn test_real_placement_drawer_golden_files() {
    let (app, test) = create_app_with_real_fixture();
    let widths = [60, 70, 80, 100, 140];

    for &w in &widths {
        let (_lines, plain) =
            spawn_at_verify::tui::views::build_drawer_content(&app, &test, w, true);
        let golden_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(format!("tests/golden/drawer_{}.txt", w));

        let regen = std::env::var("REGENERATE_GOLDEN").is_ok();
        if regen || !golden_path.exists() {
            std::fs::write(&golden_path, &plain).unwrap_or_else(|e| {
                panic!(
                    "Failed to write golden file {}: {}",
                    golden_path.display(),
                    e
                )
            });
        }

        let expected = std::fs::read_to_string(&golden_path).unwrap_or_else(|e| {
            panic!(
                "Failed to read golden file {}: {}",
                golden_path.display(),
                e
            )
        });

        if plain != expected {
            panic!(
                "Golden file mismatch for width {}!\n--- ACTUAL ---\n{}\n--- EXPECTED ---\n{}",
                w, plain, expected
            );
        }
    }
}

/// Asserts structural alignment invariants across drawer text output.
#[test]
fn test_real_placement_drawer_grid_invariants() {
    let (app, test) = create_app_with_real_fixture();
    let widths = [60, 70, 80, 100, 140];

    for &w in &widths {
        let (_lines, plain) =
            spawn_at_verify::tui::views::build_drawer_content(&app, &test, w, true);
        let inner_width = w.saturating_sub(2);

        // 1. Grid Invariant: Geometry table alignment
        // Header Expected starts at x=16; Expected values start at x=16; Anchor/Margin/Tolerance start at x=16
        // Actual starts at x=32; Result starts at x=48; ✔ is centered under Result (offset 3)
        let mut geom_found = false;
        for line in plain.lines() {
            if line.contains("Field")
                && line.contains("Expected")
                && line.contains("Actual")
                && line.contains("Result")
            {
                geom_found = true;
                assert_eq!(
                    &line[0..2],
                    "  ",
                    "Table header must start with INDENT (2 spaces)"
                );
                assert_eq!(
                    line.find("Expected"),
                    Some(16),
                    "Geometry header 'Expected' must start at x=16 at width {}",
                    w
                );
                assert_eq!(
                    line.find("Actual"),
                    Some(32),
                    "Geometry header 'Actual' must start at x=32 at width {}",
                    w
                );
                assert_eq!(
                    line.find("Result"),
                    Some(48),
                    "Geometry header 'Result' must start at x=48 at width {}",
                    w
                );
            }
            if line.starts_with("  Position") {
                assert_eq!(
                    &line[0..2],
                    "  ",
                    "Position row must start with INDENT (2 spaces)"
                );
                assert_ne!(
                    line.chars().nth(16).unwrap_or(' '),
                    ' ',
                    "Position Expected value must start at x=16 at width {}",
                    w
                );
                assert_ne!(
                    line.chars().nth(32).unwrap_or(' '),
                    ' ',
                    "Position Actual value must start at x=32 at width {}",
                    w
                );
                // Centered under Result: 48 + 3 = 51
                assert_eq!(
                    line.chars().position(|c| c == '✔'),
                    Some(51),
                    "Position ✔ must be centered at x=51 under Result at width {}",
                    w
                );
            }
            if line.starts_with("  Size") {
                assert_eq!(
                    &line[0..2],
                    "  ",
                    "Size row must start with INDENT (2 spaces)"
                );
                assert_ne!(
                    line.chars().nth(16).unwrap_or(' '),
                    ' ',
                    "Size Expected value must start at x=16 at width {}",
                    w
                );
                assert_ne!(
                    line.chars().nth(32).unwrap_or(' '),
                    ' ',
                    "Size Actual value must start at x=32 at width {}",
                    w
                );
                assert_eq!(
                    line.chars().position(|c| c == '✔'),
                    Some(51),
                    "Size ✔ must be centered at x=51 under Result at width {}",
                    w
                );
            }
            if line.starts_with("  Anchor")
                || line.starts_with("  Margin")
                || line.starts_with("  Tolerance")
            {
                assert_ne!(
                    line.chars().nth(16).unwrap_or(' '),
                    ' ',
                    "Geometry input value must start at x=16 at width {}: '{}'",
                    w,
                    line
                );
            }
        }
        assert!(
            geom_found,
            "Geometry table header must be present at width {}",
            w
        );

        // 2. Grid Invariant: every key/value row in RESULT, GEOMETRY inputs, header, WHAT TO LOOK FOR, SPEC and OUTPUT has value at x == 16
        for line in plain.lines() {
            let prefixes = [
                "  Title",
                "  ID",
                "  Mode",
                "  Verdict",
                "  Cloak",
                "  Budget",
                "  Trace",
                "  Anchor",
                "  Margin",
                "  Tolerance",
                "  Watch",
                "  Your verdict",
                "  Action",
                "  Subject",
                "  stderr",
                "  stdout",
                "  x",
                "  y",
                "  w",
                "  h",
                "  area",
                "  source",
            ];
            for p in &prefixes {
                if line.starts_with(p) && !line.contains("Expected") {
                    let ch = line.chars().nth(16).unwrap_or(' ');
                    assert_ne!(
                        ch, ' ',
                        "Row '{}' value must start at column 16 at width {}",
                        line, w
                    );
                }
            }
        }

        // 3. Grid Invariant: every timeline detail value across the whole trace starts at one single x; gap >= 2
        let timeline_idx = plain.find("── TIMELINE").expect("TIMELINE section exists");
        let next_section_idx = plain
            .find("── WHAT TO LOOK FOR")
            .expect("WHAT TO LOOK FOR section exists");
        let timeline_part = &plain[timeline_idx..next_section_idx];

        let mut timeline_val_x: Option<usize> = None;
        for line in timeline_part.lines() {
            if line.starts_with("               ") && !line.trim().is_empty() {
                let non_space_idx = line
                    .char_indices()
                    .skip(15)
                    .find(|(_, c)| *c != ' ')
                    .map(|(i, _)| i);
                if let Some(lbl_start) = non_space_idx {
                    if lbl_start == 15 {
                        // Detail row with label: label followed by at least 2 spaces, then value
                        let after_lbl = &line[lbl_start..];
                        if let Some(gap_idx) = after_lbl.find("  ") {
                            let val_part = &after_lbl[gap_idx..];
                            if let Some(val_start_rel) = val_part
                                .char_indices()
                                .find(|(_, c)| *c != ' ')
                                .map(|(i, _)| i)
                            {
                                let actual_val_x = lbl_start + gap_idx + val_start_rel;
                                assert!(
                                    val_start_rel >= 2,
                                    "Detail label and value gap must be >= 2 spaces in '{}'",
                                    line
                                );
                                if let Some(expected_x) = timeline_val_x {
                                    assert_eq!(actual_val_x, expected_x, "All timeline detail values must start at single x (expected {}, got {}) in '{}'", expected_x, actual_val_x, line);
                                } else {
                                    timeline_val_x = Some(actual_val_x);
                                }
                            }
                        }
                    } else {
                        // Wrapped continuation line starting directly at value column
                        let actual_val_x = lbl_start;
                        if let Some(expected_x) = timeline_val_x {
                            assert_eq!(actual_val_x, expected_x, "Continuation must start at detail value x (expected {}, got {}) in '{}'", expected_x, actual_val_x, line);
                        }
                    }
                }
            }
        }
        assert!(
            timeline_val_x.is_some(),
            "Must have verified timeline detail value x alignment at width {}",
            w
        );

        // 4. Grid Invariant: no row has first non-space cell at x < INDENT (2) unless heading row; no continuation starts anywhere but value x
        for line in plain.lines() {
            if line.is_empty() {
                continue;
            }
            let first_non_space = line
                .char_indices()
                .find(|(_, c)| *c != ' ')
                .map(|(i, _)| i)
                .unwrap_or(0);
            if line.contains("── ") {
                // Heading row starts with 1 space margin
                assert_eq!(
                    first_non_space, 1,
                    "Heading row must start with 1 space margin at width {}: '{}'",
                    w, line
                );
            } else {
                assert!(
                    first_non_space >= 2,
                    "No row can start before INDENT (2 spaces) at width {}: '{}'",
                    w,
                    line
                );
            }
        }

        // 5. Grid Invariant: no non-space cell lies within RIGHT_PAD (2) cells of right border
        for line in plain.lines() {
            use unicode_width::UnicodeWidthStr;
            let line_w = line.width();
            assert!(
                line_w <= inner_width - 1,
                "Line display width ({}) must not touch or exceed inner right boundary ({} - 1) at width {}: '{}'",
                line_w, inner_width, w, line
            );
        }

        // 6. Grid Invariant: heading format
        for line in plain.lines() {
            if line.starts_with(" ── ") {
                assert!(
                    line.contains(" ── "),
                    "Heading row must have leading rule at width {}: '{}'",
                    w,
                    line
                );
                use unicode_width::UnicodeWidthStr;
                assert!(
                    line.width() <= inner_width.saturating_sub(1),
                    "Heading rule must end at least 1 cell before right border at width {}: '{}'",
                    w,
                    line
                );
            }
        }

        // 7. Grid Invariant: Title row is followed by exactly one blank row; ID/Mode/Description follow directly
        let title_idx = plain
            .lines()
            .position(|l| l.starts_with("  Title"))
            .expect("Title exists");
        let line_after_title = plain
            .lines()
            .nth(title_idx + 1)
            .expect("Line after title exists");
        assert!(
            line_after_title.is_empty(),
            "Title row must be followed by exactly one blank row at width {}",
            w
        );
        let id_idx = plain
            .lines()
            .position(|l| l.starts_with("  ID"))
            .expect("ID exists");
        assert_eq!(
            id_idx,
            title_idx + 2,
            "ID must follow blank row after Title at width {}",
            w
        );
        let mode_idx = plain
            .lines()
            .position(|l| l.starts_with("  Mode"))
            .expect("Mode exists");
        assert!(mode_idx > id_idx, "Mode must follow ID at width {}", w);
        for l in plain.lines().skip(id_idx + 1).take(mode_idx - (id_idx + 1)) {
            assert!(
                !l.is_empty(),
                "No blank rows between ID and Mode at width {}",
                w
            );
        }
        let desc_idx = plain
            .lines()
            .position(|l| l.starts_with("  Description"))
            .expect("Description exists");
        assert!(
            desc_idx > mode_idx,
            "Description must follow Mode at width {}",
            w
        );
        for l in plain
            .lines()
            .skip(mode_idx + 1)
            .take(desc_idx - (mode_idx + 1))
        {
            assert!(
                !l.is_empty(),
                "No blank rows between Mode and Description at width {}",
                w
            );
        }

        // 8. Grid Invariant: no raw key=value, no raw camelCase, no unclosed parens
        for line in timeline_part.lines() {
            assert!(
                !line.contains("requestedSize"),
                "No raw requestedSize in timeline at width {}",
                w
            );
            assert!(
                !line.contains("clampedSafeSize"),
                "No raw clampedSafeSize in timeline at width {}",
                w
            );
            assert!(
                !line.contains("bufferRect"),
                "No raw bufferRect in timeline at width {}",
                w
            );
            assert!(
                !line.contains("preFrame"),
                "No raw preFrame in timeline at width {}",
                w
            );
            assert!(
                !line.contains("postFrame"),
                "No raw postFrame in timeline at width {}",
                w
            );
            assert!(
                !line.contains("opacity="),
                "No raw opacity= in compact timeline at width {}",
                w
            );
            assert!(
                !line.contains("frame="),
                "No raw frame= in compact timeline at width {}",
                w
            );
            assert!(
                !line.contains("400x300"),
                "400x300 must be normalised to 400×300 at width {}",
                w
            );
        }

        // 9. Grid Invariant: Keys bar has 1 space left padding
        let keys_lines = spawn_at_verify::tui::views::render_key_grid(
            spawn_at_verify::tui::views::DRAWER_KEYS,
            inner_width,
        );
        for k_line in keys_lines {
            let first_span = &k_line.spans[0];
            assert!(
                first_span.content.starts_with(' '),
                "Keys bar must have 1 space left padding at width {}: '{:?}'",
                w,
                first_span.content
            );
        }

        // 10. Grid Invariant: t raw view still shows original raw lines
        let (_raw_lines, raw_plain) =
            spawn_at_verify::tui::views::build_drawer_content(&app, &test, w, false);
        assert!(
            raw_plain.contains("[spawn-at-time]"),
            "Raw view must retain raw prefix [spawn-at-time] at width {}",
            w
        );
        assert!(
            raw_plain.contains("win=[id="),
            "Raw view must retain raw win=[id=] metadata at width {}",
            w
        );
    }
}
