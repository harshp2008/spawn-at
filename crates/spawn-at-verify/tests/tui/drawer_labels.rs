//! Drawer-wide shared label column: width rule, cap, truncation and wrapped continuation.

use super::create_app_with_real_fixture;
use spawn_at_verify::tui::views::build_drawer_content;
use unicode_width::UnicodeWidthStr;

/// Returns the column at which the value text starts on the row beginning with `prefix`.
fn value_col(plain: &str, prefix: &str) -> usize {
    let row = plain
        .lines()
        .find(|l| l.starts_with(prefix))
        .unwrap_or_else(|| panic!("row starting with {:?} not found in:\n{}", prefix, plain));
    let start = prefix.len();
    row[start..]
        .find(|c| c != ' ')
        .map(|i| i + start)
        .unwrap_or_else(|| panic!("row {:?} has no value", row))
}

/// `--paint-delay-ms` is 16 columns, so the shared label column becomes 16 + 2 = 18 and
/// every section (header, RESULT, geometry table and inputs, SPEC) starts values at
/// 2 + 18 = 20. The fixture's own longest label (15, "needs re-anchor") would give 19, so a
/// result of 20 proves the flag label drove the width.
#[test]
fn test_long_flag_label_widens_shared_value_column_for_every_section() {
    let (app, mut test) = create_app_with_real_fixture();
    test.action.args.push("--paint-delay-ms".to_string());
    test.action.args.push("50".to_string());

    for &w in &[80usize, 140] {
        let (_lines, plain) = build_drawer_content(&app, &test, w, true);
        for prefix in [
            "  Title",
            "  ID",
            "  Mode",
            "  Verdict",
            "  Tolerance",
            "  Action",
        ] {
            assert_eq!(
                value_col(&plain, prefix),
                20,
                "{:?} value must start at column 20 at width {}",
                prefix,
                w
            );
        }
        assert_eq!(
            value_col(&plain, "  --paint-delay-ms"),
            20,
            "flag value must start at column 20 at width {}",
            w
        );
        assert!(
            plain.contains("  --paint-delay-ms  50"),
            "flag label must fit untruncated followed by two spaces at width {}",
            w
        );
        let hdr = plain
            .lines()
            .find(|l| l.contains("Field") && l.contains("Expected"))
            .unwrap();
        assert_eq!(
            hdr.find("Expected"),
            Some(20),
            "geometry table must share column 20 at width {}",
            w
        );
    }
}

/// A 36-column flag exceeds the 24-column cap: labels truncate to 22 columns ending in `…`,
/// leaving two spaces before the value, and values align at 2 + 24 = 26.
#[test]
fn test_label_column_is_capped_at_24_and_long_labels_truncate() {
    let (app, mut test) = create_app_with_real_fixture();
    test.action
        .args
        .push("--an-extremely-long-flag-name-here".to_string());
    test.action.args.push("7".to_string());

    let (_lines, plain) = build_drawer_content(&app, &test, 100, true);
    assert_eq!(
        value_col(&plain, "  Title"),
        26,
        "value column capped at 2 + 24"
    );
    let row = plain
        .lines()
        .find(|l| l.starts_with("  --an-extremely-long"))
        .expect("truncated flag row present");
    let chars: Vec<char> = row.chars().collect();
    let label: String = chars[2..24].iter().collect();
    let gap: String = chars[24..26].iter().collect();
    assert_eq!(
        label, "--an-extremely-long-f…",
        "label truncated to 22 columns"
    );
    assert_eq!(gap, "  ", "two spaces separate label and value");
    assert!(row.ends_with('7'), "value kept: {:?}", row);
}

/// Wrapped text continues in its own value column (value_x spaces) and never spills past the
/// drawer's inner width, even at the narrowest width with a wide label column.
#[test]
fn test_wrapped_values_continue_at_shared_value_column_and_fit_width() {
    let (app, mut test) = create_app_with_real_fixture();
    test.action.args.push("--paint-delay-ms".to_string());
    test.action.args.push("50".to_string());
    test.description = "Spawns a test window anchored to the bottom-right corner of the work \
        area with a sixteen pixel boundary margin so it never touches the screen edge."
        .to_string();

    for &w in &[60usize, 80, 140] {
        let (_lines, plain) = build_drawer_content(&app, &test, w, true);
        let rows: Vec<&str> = plain.lines().collect();
        let d = rows
            .iter()
            .position(|l| l.starts_with("  Description"))
            .unwrap();
        assert_eq!(value_col(&plain, "  Description"), 20);
        let cont = rows[d + 1];
        assert!(
            cont.starts_with(&" ".repeat(20)) && !cont[20..].starts_with(' '),
            "wrapped description must continue at column 20 at width {}: {:?}",
            w,
            cont
        );
        for r in &rows {
            assert!(
                r.width() <= w - 2,
                "row wider than inner width {} at width {}: {:?}",
                w - 2,
                w,
                r
            );
        }
    }
}

/// Builds the plain drawer text for the real delay-500 recording, whose spec passes the 16-column
/// flag `--paint-delay-ms`, at `width`.
fn real_delay500_drawer(width: usize) -> String {
    let case = spawn_at_verify::tui::render::find_case("delay-500").unwrap();
    let mut app = super::create_test_app();
    app.results.insert(case.id.clone(), case.result.clone());
    let test = app
        .registry
        .tests
        .iter()
        .find(|t| t.id == case.id)
        .expect("delay-500 test is in the registry")
        .clone();
    build_drawer_content(&app, &test, width, true).1
}

/// Column where the first value starts on a key/value row: the first non-space character after
/// the label and the run of two or more spaces that follows it. `start` is the label's column.
fn row_value_col(line: &str, start: usize) -> Option<usize> {
    let rest = &line[start..];
    let gap = rest.find("  ")?;
    let after = &rest[gap..];
    let skip = after.find(|c| c != ' ')?;
    Some(start + gap + skip)
}

/// With the real 16-column flag `--paint-delay-ms` in SPEC, the label is shown whole (followed
/// by two spaces), and every section - header, RESULT, WHAT TO LOOK FOR, GEOMETRY, SPEC, OUTPUT -
/// starts its labels at column 2 and its values at column 20 (2 + (16 + 2)). Timeline detail rows
/// are nested 15 columns but use the same 18-column label width.
#[test]
fn test_real_paint_delay_flag_label_and_value_columns_match_across_all_sections() {
    const LABEL_COL: usize = 2;
    const VALUE_COL: usize = 20;
    const LABEL_W: usize = VALUE_COL - LABEL_COL;
    const TIMELINE_DETAIL_INDENT: usize = 15;

    for &w in &[60usize, 80, 140] {
        let plain = real_delay500_drawer(w);
        assert!(
            plain.lines().any(|l| l == "  --paint-delay-ms  500"),
            "label must not be truncated and must be followed by 2 spaces at width {}:\n{}",
            w,
            plain
        );

        let mut section = "header".to_string();
        let mut seen_sections = Vec::new();
        let mut flat_rows = 0;
        let mut detail_rows = 0;
        for line in plain.lines() {
            if let Some(rest) = line.trim_start().strip_prefix("── ") {
                section = rest.split(" ──").next().unwrap().trim_end().to_string();
                seen_sections.push(section.clone());
                continue;
            }
            if line.trim().is_empty() {
                continue;
            }
            let indent = line.len() - line.trim_start().len();
            if section == "TIMELINE" {
                if indent == TIMELINE_DETAIL_INDENT {
                    let vc = row_value_col(line, indent)
                        .unwrap_or_else(|| panic!("detail row without value: {:?}", line));
                    assert_eq!(
                        vc - indent,
                        LABEL_W,
                        "timeline detail label width must be {} at width {}: {:?}",
                        LABEL_W,
                        w,
                        line
                    );
                    detail_rows += 1;
                }
                continue;
            }
            // Wrapped continuation rows start exactly at the value column.
            if indent == VALUE_COL {
                continue;
            }
            assert_eq!(
                indent, LABEL_COL,
                "label must start at column {} in {} at width {}: {:?}",
                LABEL_COL, section, w, line
            );
            let vc = row_value_col(line, indent)
                .unwrap_or_else(|| panic!("row without value in {}: {:?}", section, line));
            assert_eq!(
                vc, VALUE_COL,
                "value must start at column {} in {} at width {}: {:?}",
                VALUE_COL, section, w, line
            );
            flat_rows += 1;
        }
        assert_eq!(
            seen_sections,
            vec![
                "RESULT",
                "WHAT TO LOOK FOR",
                "GEOMETRY",
                "SPEC",
                "OUTPUT",
                "TIMELINE"
            ]
        );
        assert!(
            flat_rows >= 25,
            "expected many key/value rows, saw {}",
            flat_rows
        );
        assert!(
            detail_rows >= 10,
            "expected timeline detail rows, saw {}",
            detail_rows
        );
    }
}
