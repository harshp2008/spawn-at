//! "PASS (flash)" in the TUI: its own symbol and colour in the list, its own verdict and rows in
//! the drawer, a flash count on the category heading.

use super::create_test_app;
use ratatui::style::Color;
use spawn_at_verify::runner::{AutoStatus, FinalStatus, HumanVerdict, TestResult};
use spawn_at_verify::trace::flash::{FlashBasis, FlashEvent, FlashInfo, FlashKind, FlashSource};
use spawn_at_verify::tui::views::build_drawer_content;
use spawn_at_verify::tui::views::panels::tests_list::{category_counts_text, tally_results};
use spawn_at_verify::tui::views::theme::{
    status_symbol_and_color, FLASH, FLASH_SYMBOL, TRACE_UNCHECKED,
};

/// A passed result for the selected test whose flash check found `events` (certain ones make it
/// a flash).
fn flash_result(
    app: &spawn_at_verify::tui::app::TuiApp,
    events: Vec<FlashEvent>,
) -> (spawn_at_verify::schema::TestDefinition, TestResult) {
    let test = app.selected_test().unwrap().clone();
    let flash = events.iter().any(|e| e.certain);
    let res = TestResult {
        id: test.id.clone(),
        title: test.title.clone(),
        kind: test.kind,
        required: test.required,
        auto_status: AutoStatus::AutoPass,
        human_verdict: HumanVerdict::default(),
        final_status: FinalStatus::Pass,
        disagreement: false,
        duration_ms: 160,
        exit_code: Some(0),
        stdout: String::new(),
        stderr: String::new(),
        message: Some("Assertions passed".into()),
        log_slice: None,
        trace_note: None,
        final_rect: None,
        flash_info: FlashInfo {
            flash,
            flash_events: events,
            flash_basis: Some(FlashBasis::Client),
            flash_notes: vec![],
            flash_clock_uncertainty_ms: Some(1.2),
        },
    };
    (test, res)
}

/// One flash event.
fn event(certain: bool, detail: &str) -> FlashEvent {
    FlashEvent {
        kind: FlashKind::PaintAfterReveal,
        source: FlashSource::Client,
        at_ms_after_reveal: 25.0,
        certain,
        detail: detail.into(),
    }
}

/// The list symbol of a flashing pass is `✱` in pink: neither the green check of a plain pass,
/// the yellow `!` of geometry unchecked nor the orange `!` of trace unchecked. A pass whose only
/// event is uncertain stays a plain green check.
#[test]
fn test_flash_symbol_and_colour_are_distinct() {
    let app = create_test_app();
    let (test, flashed) = flash_result(
        &app,
        vec![event(true, "first paint came 25.0 ms after the reveal")],
    );
    let (sym, colour) = status_symbol_and_color(&test, Some(&flashed), false);
    assert_eq!((sym, colour), (FLASH_SYMBOL, FLASH));
    assert_ne!(colour, Color::Green);
    assert_ne!(colour, Color::Yellow);
    assert_ne!(colour, TRACE_UNCHECKED);
    assert_ne!(colour, Color::Red);

    let (test, near_miss) = flash_result(&app, vec![event(false, "paint 3 ms after the reveal")]);
    assert_eq!(
        status_symbol_and_color(&test, Some(&near_miss), false),
        ("✔", Color::Green)
    );
}

/// A failed run that also flashed is a failure: red cross, never the flash symbol.
#[test]
fn test_failed_run_that_flashed_is_still_a_failure() {
    let app = create_test_app();
    let (test, mut res) = flash_result(&app, vec![event(true, "late paint")]);
    res.auto_status = AutoStatus::AutoFail;
    res.final_status = FinalStatus::Fail;
    assert_eq!(
        status_symbol_and_color(&test, Some(&res), false).1,
        Color::Red
    );
}

/// The drawer verdict reads `✱ Pass (flash)` and a Flash row says what was compared; one row per
/// event follows, and an uncertain event is marked `?` and "not counted".
#[test]
fn test_drawer_shows_pass_flash_verdict_and_flash_rows() {
    let mut app = create_test_app();
    let (test, res) = flash_result(
        &app,
        vec![
            event(true, "first paint at 800x600 came 25.0 ms after the reveal"),
            event(false, "size 400x300 -> 800x600 3.0 ms after the reveal"),
        ],
    );
    app.results.insert(test.id.clone(), res);
    let plain = build_drawer_content(&app, &test, 120, true).1;
    let row = |label: &str| {
        plain
            .lines()
            .find(|l| l.trim_start().starts_with(label))
            .unwrap_or_else(|| panic!("no {} row in:\n{}", label, plain))
            .to_string()
    };
    assert!(row("Verdict").contains("✱ Pass (flash)"), "{}", plain);
    assert!(!plain.contains("✔ Pass"), "{}", plain);
    assert!(
        row("Flash").contains("flash, fixture paint times against UNCLOAK (clock +-1.2 ms)"),
        "{}",
        plain
    );
    assert!(
        plain.contains("✱ first paint at 800x600 came 25.0 ms after the reveal"),
        "{}",
        plain
    );
    assert!(
        plain.contains("? size 400x300 -> 800x600 3.0 ms after the reveal (inside the clock uncertainty: not counted)"),
        "{}",
        plain
    );
}

/// A run checked on the client's paint times that found nothing says "none seen", with its basis.
#[test]
fn test_drawer_says_none_seen_for_a_clean_checked_run() {
    let mut app = create_test_app();
    let (test, res) = flash_result(&app, vec![]);
    app.results.insert(test.id.clone(), res);
    let plain = build_drawer_content(&app, &test, 120, true).1;
    assert!(
        plain.contains("none seen, fixture paint times against UNCLOAK"),
        "{}",
        plain
    );
    assert!(plain.contains("✔ Pass"), "{}", plain);
}

/// The category heading counts flashes among the passes: `1/5 · 1 flash`, and only when non-zero.
#[test]
fn test_category_heading_counts_flashes() {
    let app = create_test_app();
    let (_, flashed) = flash_result(&app, vec![event(true, "late paint")]);
    let (_, plain_pass) = flash_result(&app, vec![]);
    let tally = tally_results([Some(&flashed), Some(&plain_pass)].into_iter());
    assert_eq!((tally.pass, tally.flash), (2, 1));
    assert_eq!(category_counts_text(&tally, 5), "2/5 · 1 flash");
    let none = tally_results([Some(&plain_pass)].into_iter());
    assert_eq!(category_counts_text(&none, 5), "1/5");
}
