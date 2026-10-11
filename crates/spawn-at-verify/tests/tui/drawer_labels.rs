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
