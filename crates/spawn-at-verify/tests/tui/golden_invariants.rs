//! Responsive golden file assertions and drawer grid invariant tests.

use super::create_app_with_real_fixture;

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

#[test]
fn test_real_placement_drawer_grid_invariants() {
    let (app, test) = create_app_with_real_fixture();
    let widths = [60, 70, 80, 100, 140];

    for &w in &widths {
        let (_lines, plain) =
            spawn_at_verify::tui::views::build_drawer_content(&app, &test, w, true);

        // Shared value column: the real fixture's longest label is the 15-char timeline
        // detail "needs re-anchor", so the label column is 15 + 2 = 17 wide and every
        // section's values start at INDENT (2) + 17 = 19.
        let title_row = plain
            .lines()
            .find(|l| l.starts_with("  Title"))
            .expect("Title row exists");
        let vx = title_row[7..]
            .find(|c| c != ' ')
            .map(|i| i + 7)
            .expect("Title has a value");
        assert_eq!(vx, 19, "Shared VALUE_X must be 19 at width {}", w);
        // Expected/Actual column width: 16, shrinking to >= 12 only when the table would
        // overflow the drawer (width 60 with a 17-wide label column gives 14).
        let hdr_row = plain
            .lines()
            .find(|l| l.contains("Field") && l.contains("Expected") && l.contains("Actual"))
            .expect("Geometry header exists");
        let col = hdr_row.find("Actual").unwrap() - hdr_row.find("Expected").unwrap();
        assert!(
            (12..=16).contains(&col),
            "Geometry column width {} out of range at width {}",
            col,
            w
        );
        if w >= 70 {
            assert_eq!(col, 16, "Geometry columns must be 16 wide at width {}", w);
        }
        let inner_width = w.saturating_sub(2);

        // 1. Grid Invariant: Geometry table alignment
        // Header Expected starts at x=VALUE_X; Expected values start at x=VALUE_X; Tolerance starts at x=VALUE_X
        // Actual starts at x=VALUE_X+col; Result starts at x=VALUE_X+2col; ✔ is centered under Result (offset 3)
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
                    Some(vx),
                    "Geometry header 'Expected' must start at x=VALUE_X at width {}",
                    w
                );
                assert_eq!(
                    line.find("Actual"),
                    Some(vx + col),
                    "Geometry header 'Actual' must start at x=VALUE_X+col at width {}",
                    w
                );
                assert_eq!(
                    line.find("Result"),
                    Some(vx + 2 * col),
                    "Geometry header 'Result' must start at x=VALUE_X+2col at width {}",
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
                    line.chars().nth(vx).unwrap_or(' '),
                    ' ',
                    "Position Expected value must start at x=VALUE_X at width {}",
                    w
                );
                assert_ne!(
                    line.chars().nth(vx + col).unwrap_or(' '),
                    ' ',
                    "Position Actual value must start at x=VALUE_X+col at width {}",
                    w
                );
                // Centered under Result: 48 + 3 = 51
                assert_eq!(
                    line.chars().position(|c| c == '✔'),
                    Some(vx + 2 * col + 3),
                    "Position ✔ must be centered at x=VALUE_X+2col+3 under Result at width {}",
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
                    line.chars().nth(vx).unwrap_or(' '),
                    ' ',
                    "Size Expected value must start at x=VALUE_X at width {}",
                    w
                );
                assert_ne!(
                    line.chars().nth(vx + col).unwrap_or(' '),
                    ' ',
                    "Size Actual value must start at x=VALUE_X+col at width {}",
                    w
                );
                assert_eq!(
                    line.chars().position(|c| c == '✔'),
                    Some(vx + 2 * col + 3),
                    "Size ✔ must be centered at x=VALUE_X+2col+3 under Result at width {}",
                    w
                );
            }
            // Anchor and margin live in SPEC (--anchor / --margin); only Tolerance is a GEOMETRY input
            assert!(
                !line.starts_with("  Anchor") && !line.starts_with("  Margin"),
                "Anchor/Margin must not be repeated in GEOMETRY at width {}: {:?}",
                w,
                line
            );
            if line.starts_with("  Tolerance") {
                assert_ne!(
                    line.chars().nth(vx).unwrap_or(' '),
                    ' ',
                    "Geometry input value must start at x=VALUE_X at width {}: '{}'",
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

        // 2. Grid Invariant: every key/value row in RESULT, GEOMETRY inputs, header, WHAT TO LOOK FOR, SPEC and OUTPUT has value at x == VALUE_X
        for line in plain.lines() {
            let prefixes = [
                "  Title",
                "  ID",
                "  Mode",
                "  Verdict",
                "  Cloak",
                "  Budget",
                "  Trace",
                "  --anchor",
                "  --margin",
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
                    let ch = line.chars().nth(vx).unwrap_or(' ');
                    assert_ne!(
                        ch, ' ',
                        "Row '{}' value must start at VALUE_X column at width {}",
                        line, w
                    );
                }
            }
        }

        // 3. Grid Invariant: every timeline detail value across the whole trace starts at one single x; gap >= 2
        let timeline_idx = plain.find("── TIMELINE").expect("TIMELINE section exists");
        // TIMELINE is the last section, so it runs to the end of the drawer text
        let timeline_part = &plain[timeline_idx..];

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
                line_w < inner_width,
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
        // Mode may wrap onto continuation rows at narrow widths, so anchor on the rows
        // directly above Description: one blank row, preceded by non-blank Mode text.
        let above_desc = plain.lines().nth(desc_idx - 1).unwrap();
        let above_blank = plain.lines().nth(desc_idx - 2).unwrap();
        assert!(
            above_desc.is_empty(),
            "Row directly above Description must be blank at width {}",
            w
        );
        assert!(
            !above_blank.is_empty(),
            "Exactly one blank row between Mode and Description at width {}",
            w
        );
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
