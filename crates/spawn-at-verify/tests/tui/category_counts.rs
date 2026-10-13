//! Category heading counts in the tests list: only a final Pass counts as passed; tests that
//! wait for a human verdict and failed tests are shown separately.

use super::{create_test_app, dump_buffer_to_string};
use ratatui::{backend::TestBackend, Terminal};
use spawn_at_verify::runner::{AutoStatus, FinalStatus, HumanVerdict, TestResult};
use spawn_at_verify::schema::TestDefinition;
use spawn_at_verify::tui::views::draw_ui;
use spawn_at_verify::tui::views::panels::tests_list::{category_counts_text, tally_results, Tally};

/// A result for `test` with the given statuses.
fn result(test: &TestDefinition, auto: AutoStatus, fin: FinalStatus) -> TestResult {
    TestResult {
        id: test.id.clone(),
        title: test.title.clone(),
        kind: test.kind,
        required: test.required,
        auto_status: auto,
        human_verdict: HumanVerdict::default(),
        final_status: fin,
        disagreement: false,
        duration_ms: 10,
        exit_code: Some(0),
        stdout: String::new(),
        stderr: String::new(),
        message: None,
        log_slice: None,
        trace_note: None,
        final_rect: None,
        flash_info: Default::default(),
    }
}

/// Table of statuses against the bucket they land in. A `both` test with an automatic pass and
/// no human verdict (Pending) is "pending", never "pass"; an automatic failure is "fail" even
/// if the final status were not yet Fail; a test that has not run counts in none.
#[test]
fn test_tally_buckets() {
    let app = create_test_app();
    let t = app.registry.tests[0].clone();
    let cases: &[(AutoStatus, FinalStatus, Tally)] = &[
        (
            AutoStatus::AutoPass,
            FinalStatus::Pass,
            Tally {
                pass: 1,
                pending: 0,
                fail: 0,
            },
        ),
        (
            AutoStatus::AutoPass,
            FinalStatus::Pending,
            Tally {
                pass: 0,
                pending: 1,
                fail: 0,
            },
        ),
        (
            AutoStatus::AutoFail,
            FinalStatus::Fail,
            Tally {
                pass: 0,
                pending: 0,
                fail: 1,
            },
        ),
        (
            AutoStatus::AutoFail,
            FinalStatus::Pending,
            Tally {
                pass: 0,
                pending: 0,
                fail: 1,
            },
        ),
        (AutoStatus::Skipped, FinalStatus::Skipped, Tally::default()),
    ];
    for (auto, fin, want) in cases {
        let r = result(&t, *auto, *fin);
        assert_eq!(
            tally_results([Some(&r)].into_iter()),
            *want,
            "{:?}/{:?}",
            auto,
            fin
        );
    }
    assert_eq!(tally_results([None].into_iter()), Tally::default());
}

/// Heading text: plain `p/t` when nothing is pending or failed, extra parts only when non-zero.
#[test]
fn test_counts_text() {
    let none = Tally::default();
    assert_eq!(category_counts_text(&none, 30), "0/30");
    let some = Tally {
        pass: 1,
        pending: 2,
        fail: 3,
    };
    assert_eq!(category_counts_text(&some, 30), "1/30 · 2 pending · 3 fail");
}

/// In the rendered list (100 columns) the PLACEMENT heading reads `1/30 · 1 pending · 1 fail`
/// for one final Pass (`placement/top`), one `both` test with an automatic pass awaiting a
/// human (`placement/top-left`) and one failed `both` test (`placement/center-200x150`).
/// Before the change the automatic pass of top-left was counted as passed (`2/30`).
#[test]
fn test_heading_counts_pending_and_fail_separately() {
    let mut app = create_test_app();
    let by_id = |id: &str| {
        app.registry
            .tests
            .iter()
            .find(|t| t.id == id)
            .unwrap()
            .clone()
    };
    let top = by_id("placement/top");
    let top_left = by_id("placement/top-left");
    let c200 = by_id("placement/center-200x150");
    app.results.insert(
        top.id.clone(),
        result(&top, AutoStatus::AutoPass, FinalStatus::Pass),
    );
    app.results.insert(
        top_left.id.clone(),
        result(&top_left, AutoStatus::AutoPass, FinalStatus::Pending),
    );
    app.results.insert(
        c200.id.clone(),
        result(&c200, AutoStatus::AutoFail, FinalStatus::Fail),
    );
    let mut terminal = Terminal::new(TestBackend::new(100, 40)).unwrap();
    terminal.draw(|f| draw_ui(f, &app)).unwrap();
    let text = dump_buffer_to_string(&terminal, 100, 40);
    let heading = text
        .lines()
        .find(|l| l.contains("PLACEMENT"))
        .unwrap_or_else(|| panic!("no PLACEMENT heading in:\n{}", text));
    assert!(heading.contains("1/30 · 1 pending · 1 fail"), "{}", heading);
}
