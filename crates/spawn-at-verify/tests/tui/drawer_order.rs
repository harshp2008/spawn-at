//! Drawer section order, anchor/margin de-duplication and the auto-test verdict row.

use super::create_app_with_real_fixture;
use spawn_at_verify::schema::TestKind;
use spawn_at_verify::tui::views::build_drawer_content;

/// Strips the `--anchor <v>` / `--margin <v>` pairs from a spec's action arguments.
fn without_flag(args: &[String], flag: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut skip_value = false;
    for a in args {
        if a == flag {
            skip_value = true;
        } else if skip_value && !a.starts_with("--") {
            skip_value = false;
        } else {
            skip_value = false;
            out.push(a.clone());
        }
    }
    out
}

/// Section headings appear in the order RESULT, WHAT TO LOOK FOR, GEOMETRY, SPEC, OUTPUT, TIMELINE,
/// and TIMELINE is the last section: no heading follows it.
#[test]
fn test_sections_are_ordered_with_timeline_last() {
    let (app, test) = create_app_with_real_fixture();
    for &w in &[60usize, 80, 140] {
        let (_l, plain) = build_drawer_content(&app, &test, w, true);
        let headings: Vec<&str> = plain
            .lines()
            .filter_map(|l| l.trim_start().strip_prefix("── "))
            .map(|rest| rest.split(" ──").next().unwrap().trim_end())
            .collect();
        assert_eq!(
            headings,
            vec![
                "RESULT",
                "WHAT TO LOOK FOR",
                "GEOMETRY",
                "SPEC",
                "OUTPUT",
                "TIMELINE"
            ],
            "section order at width {}",
            w
        );
        assert!(
            !plain.ends_with("\n\n"),
            "drawer must not end with a trailing blank row at width {}",
            w
        );
    }
}

/// The fixture spec passes `--anchor bottom-right` and `--margin 16`, equal to the expectation,
/// so GEOMETRY must not repeat them while SPEC shows them once.
#[test]
fn test_anchor_and_margin_appear_once_in_spec_not_in_geometry() {
    let (app, test) = create_app_with_real_fixture();
    let (_l, plain) = build_drawer_content(&app, &test, 80, true);
    assert!(plain.contains("  --anchor"), "SPEC shows --anchor");
    assert!(plain.contains("  --margin"), "SPEC shows --margin");
    assert!(!plain.lines().any(|l| l.starts_with("  Anchor")));
    assert!(!plain.lines().any(|l| l.starts_with("  Margin")));
    assert!(plain.lines().any(|l| l.starts_with("  Tolerance")));
}

/// When SPEC does not pass the flags, the expectation is the only place the fact appears, so
/// GEOMETRY must show it.
#[test]
fn test_geometry_shows_anchor_and_margin_when_spec_lacks_them() {
    let (app, mut test) = create_app_with_real_fixture();
    test.action.args = without_flag(&test.action.args, "--anchor");
    test.action.args = without_flag(&test.action.args, "--margin");
    let (_l, plain) = build_drawer_content(&app, &test, 80, true);
    assert!(!plain.contains("  --anchor"));
    assert!(plain
        .lines()
        .any(|l| l.starts_with("  Anchor") && l.ends_with("bottom-right")));
    assert!(plain
        .lines()
        .any(|l| l.starts_with("  Margin") && l.ends_with("16")));
}

/// If SPEC passes a different value than the expectation, both facts are shown and the
/// GEOMETRY row is marked as the expectation.
#[test]
fn test_geometry_marks_expected_anchor_when_spec_differs() {
    let (app, mut test) = create_app_with_real_fixture();
    let i = test
        .action
        .args
        .iter()
        .position(|a| a == "--anchor")
        .unwrap();
    test.action.args[i + 1] = "center".to_string();
    let (_l, plain) = build_drawer_content(&app, &test, 80, true);
    assert!(plain
        .lines()
        .any(|l| l.starts_with("  --anchor") && l.ends_with("center")));
    assert!(plain
        .lines()
        .any(|l| l.starts_with("  Expected anchor") && l.ends_with("bottom-right")));
}

/// An automatic test has no human verdict, so the row says `n/a (automatic)`, not `auto test`.
#[test]
fn test_auto_test_verdict_row_says_not_applicable() {
    let (app, mut test) = create_app_with_real_fixture();
    test.kind = TestKind::Auto;
    let (_l, plain) = build_drawer_content(&app, &test, 80, true);
    let row = plain
        .lines()
        .find(|l| l.starts_with("  Your verdict"))
        .expect("Your verdict row");
    assert!(row.ends_with("n/a (automatic)"), "row was {:?}", row);
    assert!(!plain.contains("auto test"));
}
