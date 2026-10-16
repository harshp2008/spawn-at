//! Common test harness helpers and submodule declarations for TUI test suite.

pub mod action_trace_display;
pub mod app_flow;
pub mod category_counts;
pub mod drawer_content;
pub mod drawer_geometry;
pub mod drawer_keys_clipboard;
pub mod drawer_labels;
pub mod drawer_order;
pub mod drawer_trace;
pub mod drawer_workarea;
pub mod flash;
pub mod footer_widths;
pub mod golden_invariants;
pub mod layout_responsive;
pub mod polish;
pub mod v2_flow;
pub mod verdict_feedback;

use ratatui::{backend::TestBackend, Terminal};
use spawn_at_verify::runner::RunOptions;
use spawn_at_verify::tui::app::TuiApp;
use spawn_at_verify::tui::find_verify_dir;
use std::path::PathBuf;

/// Creates a mock test application harness initialized with registry specs.
pub(crate) fn create_test_app() -> TuiApp {
    let verify_dir = find_verify_dir().expect("Failed to locate verify/ directory");
    let options = RunOptions {
        bin_path: PathBuf::from("target/debug/spawn-at"),
        ..Default::default()
    };
    TuiApp::new(&verify_dir, options).expect("Failed to initialize TuiApp against verify/")
}

/// Converts a terminal backend buffer into a plain text representation.
pub(crate) fn dump_buffer_to_string(
    terminal: &Terminal<TestBackend>,
    width: u16,
    height: u16,
) -> String {
    let buf = terminal.backend().buffer();
    let mut out = String::new();
    for y in 0..height {
        let mut line = String::new();
        for x in 0..width {
            line.push_str(buf[(x, y)].symbol());
        }
        out.push_str(&line);
        out.push('\n');
    }
    out
}

/// Validates that a rendered terminal buffer satisfies layout constraints.
pub(crate) fn validate_render_spec(rendered: &str, width: u16, height: u16, is_waiting: bool) {
    let lines: Vec<&str> = rendered.lines().collect();
    assert_eq!(
        lines.len(),
        height as usize,
        "Rendered buffer line count must match height"
    );

    // Spacing system:
    // 1 blank row above the header (row 0)
    assert!(
        lines[0].trim().is_empty(),
        "Row 0 above header must be blank at {}x{}",
        width,
        height
    );
    // 1 blank row between the header and the first box (row 2)
    assert!(
        lines[2].trim().is_empty(),
        "Row 2 after header must be blank at {}x{}",
        width,
        height
    );
    // 1 blank row at the BOTTOM below keys bar
    assert!(
        lines.last().unwrap().trim().is_empty(),
        "Row below keys bar at bottom must be blank at {}x{}",
        width,
        height
    );

    // Description pane border title is "Description"
    assert!(
        rendered.contains(" Description "),
        "Details pane border title must be 'Description' at {}x{}",
        width,
        height
    );

    // Result box is present as a boxed panel
    assert!(
        rendered.contains(" Result "),
        "Result box must be present as a boxed panel at {}x{}",
        width,
        height
    );

    let status_symbols = ["●", "◐", "✔", "✖", "!", "?", "⊘"];
    let mut test_rows_found = 0;
    let mut cat_headings_found = 0;
    let mut prev_row_was_blank = false;

    for (y, line) in lines.iter().enumerate() {
        assert!(
            line.chars().count() <= width as usize,
            "Line {} exceeds terminal width {}: {}",
            y,
            width,
            line
        );

        if line.contains('│') {
            // Check for category heading line
            let is_cat_heading = (line.contains("PREFLIGHT")
                || line.contains("PLACEMENT")
                || line.contains("CLOAK & DELAYED PAINT")
                || line.contains("TRANSFORMS")
                || line.contains("FOCUS")
                || line.contains("ROBUSTNESS"))
                && (line.contains("/1")
                    || line.contains("/4")
                    || line.contains("/6")
                    || line.contains("/10")
                    || line.contains("/30")
                    || line.contains("hidden)"));

            if is_cat_heading {
                cat_headings_found += 1;
                // Principle: NO triangle symbol in category heading
                assert!(
                    !line.contains('▶') && !line.contains('▼'),
                    "Category heading row must have NO triangle symbol (line {}): {}",
                    y,
                    line
                );
                // Principle: Category headings are never truncated
                assert!(
                    !line.contains('…'),
                    "Category headings must never be truncated: {}",
                    line
                );
                // Principle: A blank row precedes each category heading (after the first)
                if cat_headings_found > 1 {
                    assert!(
                        prev_row_was_blank,
                        "A blank row must precede each category heading at line {}: current='{}'",
                        y, line
                    );
                }
            }

            // Check if this row inside the list block is blank
            let list_cell = line.split('│').nth(1).unwrap_or("");
            prev_row_was_blank = list_cell.trim().is_empty();

            // Check if this line contains a test row in the list block
            let is_test_row = (list_cell.contains("auto")
                || list_cell.contains("both")
                || list_cell.contains("human"))
                && (list_cell.contains("binary-head")
                    || list_cell.contains("bottom-right")
                    || list_cell.contains("delay-0")
                    || list_cell.contains("delay-50")
                    || list_cell.contains("delay-100"));

            if is_test_row {
                test_rows_found += 1;

                // 1. Status symbol visible in every test row
                let has_symbol = status_symbols.iter().any(|sym| list_cell.contains(sym));
                assert!(
                    has_symbol,
                    "Status symbol must be visible in test row (line {}): '{}'",
                    y, line
                );

                // 2. At least 2 spaces between name and kind in every row
                let has_2_space_gap = list_cell.contains("  auto")
                    || list_cell.contains("  both")
                    || list_cell.contains("  human");
                assert!(
                    has_2_space_gap,
                    "Row must have at least 2 spaces gap between name and kind: '{}'",
                    line
                );

                // 3. Right padding so "auto"/"both" never touch the border
                assert!(
                    !line.contains("auto│") && !line.contains("both│") && !line.contains("human│"),
                    "Right padding in list rows required so kind never touches border: '{}'",
                    line
                );

                // 4. No row text is clipped without "…"
                if list_cell.contains("…") {
                    assert!(
                        list_cell.contains("… "),
                        "Truncated text must end cleanly with … before kind gap: '{}'",
                        line
                    );
                }
            }
        }
    }

    // With 36 tests the 4-row list at 60x28 is scrolled to the selected test (`bottom-right`),
    // and its category heading is one of the rows above ("▲ n more" in the Tests border title).
    let list_scrolled = rendered
        .lines()
        .any(|l| l.contains(" Tests ") && l.contains('▲'));
    assert!(
        cat_headings_found > 0 || list_scrolled || (height <= 20 && is_waiting),
        "Must have found category headings at {}x{} in:\n{}",
        width,
        height,
        rendered
    );
    assert!(
        test_rows_found > 0,
        "Must have found test rows at {}x{}",
        width,
        height
    );

    // 4. Keys bar and verdict box are not clipped mid-word
    if is_waiting {
        assert!(
            rendered.contains("Your verdict:"),
            "Verdict box must be visible"
        );
        assert!(rendered.contains("Pass"), "Verdict box must contain 'Pass'");
        assert!(rendered.contains("Fail"), "Verdict box must contain 'Fail'");
        assert!(rendered.contains("Skip"), "Verdict box must contain 'Skip'");
        assert!(
            rendered.contains("Flaky"),
            "Verdict box must contain 'Flaky'"
        );
        assert!(
            rendered.contains("Re-run"),
            "Verdict box must contain 'Re-run'"
        );
        assert!(
            rendered.contains("Cancel"),
            "Verdict box must contain 'Cancel'"
        );
        // Ensure no mid-word cuts
        assert!(
            !rendered.contains("Pas ") && !rendered.contains("Fai ") && !rendered.contains("Ski "),
            "Verdict box must not be cut mid-word"
        );
        // And the question being asked ("What to look for") must be visible right above it
        assert!(
            rendered.contains("What to look for"),
            "What to look for must be visible right above verdict box"
        );
    } else {
        assert!(rendered.contains("Run"), "Keys bar must contain 'Run'");
        assert!(rendered.contains("All"), "Keys bar must contain 'All'");
        assert!(rendered.contains("Help"), "Keys bar must contain 'Help'");
        assert!(rendered.contains("Quit"), "Keys bar must contain 'Quit'");
        // Ensure no mid-word cuts
        assert!(
            !rendered.contains("Ru ") && !rendered.contains("Hel ") && !rendered.contains("Qui "),
            "Keys bar must not be cut mid-word"
        );
    }

    // 5. Description and "What to look for" are visible at narrow width (<75 cols)
    if width < 75 {
        assert!(
            rendered.contains("ID"),
            "ID must be visible at narrow width ({} cols)",
            width
        );
        if height >= 22 {
            assert!(
                rendered.contains("Mode"),
                "Mode must be visible at narrow width ({} cols, height {})",
                width,
                height
            );
        }
        // The description and "What to look for" are visible at narrow width (scrollable with a hint if long)
        if is_waiting {
            assert!(
                rendered.contains("What to look for"),
                "What to look for must be visible at narrow width ({} cols, height {})",
                width,
                height
            );
        } else {
            // When description is long, shows "▼ more" hint
            assert!(
                rendered.contains("What to look for") || rendered.contains("▼ more"),
                "Either 'What to look for' or scrollable clipping hint '▼ more' must be visible at {}x{}",
                width,
                height
            );
        }
    }
}

/// Constructs a test application pre-populated with real fixture execution data.
pub(crate) fn create_app_with_real_fixture() -> (TuiApp, spawn_at_verify::schema::TestDefinition) {
    let mut app = create_test_app();
    // Pin the probe to the workarea the recording itself logged (0,40 1920x1040) instead of
    // asking the live desktop, so these tests give the same result on every machine.
    app.env_info = Some(spawn_at_verify::probe::EnvironmentInfo {
        session_type: "wayland".into(),
        desktop: "ubuntu:gnome".into(),
        gnome_version: Some(46),
        monitors_count: 1,
        primary_workarea: Some(spawn_at_verify::oracle::Rect::new(0, 40, 1920, 1040)),
        independent_workarea: Some(spawn_at_verify::oracle::Rect::new(0, 40, 1920, 1040)),
    });
    let fixture_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/real_placement_run.json");
    let content = std::fs::read_to_string(&fixture_path)
        .expect("Failed to read tests/fixtures/real_placement_run.json");
    let report: spawn_at_verify::report::SessionReport =
        serde_json::from_str(&content).expect("Failed to parse fixture real_placement_run.json");
    for res in report.results {
        app.results.insert(res.id.clone(), res);
    }
    let test_idx = app
        .registry
        .tests
        .iter()
        .position(|t| t.id == "placement/bottom-right")
        .expect("placement/bottom-right test definition must exist in registry");
    app.selected_index = test_idx;
    let test = app.registry.tests[test_idx].clone();
    (app, test)
}
