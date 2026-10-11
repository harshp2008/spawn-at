//! App workflow integration tests (initialization, navigation, filtering, verdicts).

use super::{create_test_app, dump_buffer_to_string};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{backend::TestBackend, Terminal};
use spawn_at_verify::runner::{AutoStatus, FinalStatus, HumanVerdict, HumanVerdictKind};
use spawn_at_verify::schema::TestKind;
use spawn_at_verify::tui::app::{FilterMode, ModalState};
use spawn_at_verify::tui::logging::parse_logging_state_from_content;
use spawn_at_verify::tui::views::draw_ui;

#[test]
fn test_tui_app_initialization() {
    let app = create_test_app();
    assert!(
        !app.registry.tests.is_empty(),
        "Expected verify library to have test definitions"
    );
    assert_eq!(app.selected_index, 0);
    assert_eq!(app.filter_mode, FilterMode::All);
    assert_eq!(app.modal, ModalState::None);
    assert!(!app.should_quit);
}

#[test]
fn test_tui_navigation_and_keybindings() {
    let mut app = create_test_app();
    let total = app.filtered_tests().len();
    assert!(total > 2);

    // Initial index
    assert_eq!(app.selected_index, 0);

    // Down navigation
    app.handle_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE));
    assert_eq!(app.selected_index, 1);

    app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    assert_eq!(app.selected_index, 2);

    // Up navigation
    app.handle_key(KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE));
    assert_eq!(app.selected_index, 1);

    app.handle_key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE));
    assert_eq!(app.selected_index, 0);

    // Up boundary clamp
    app.handle_key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE));
    assert_eq!(app.selected_index, 0);
}

#[test]
fn test_tui_filter_cycling() {
    let mut app = create_test_app();

    // All
    assert_eq!(app.filter_mode, FilterMode::All);
    let all_count = app.filtered_tests().len();

    // Cycle to HumanOnly
    app.handle_key(KeyEvent::new(KeyCode::Char('f'), KeyModifiers::NONE));
    assert_eq!(app.filter_mode, FilterMode::HumanOnly);
    for t in app.filtered_tests() {
        assert!(t.kind == TestKind::Human || t.kind == TestKind::Both);
    }

    // Cycle to AutoOnly
    app.handle_key(KeyEvent::new(KeyCode::Char('f'), KeyModifiers::NONE));
    assert_eq!(app.filter_mode, FilterMode::AutoOnly);
    for t in app.filtered_tests() {
        assert_eq!(t.kind, TestKind::Auto);
    }

    // Cycle to FailedOnly
    app.handle_key(KeyEvent::new(KeyCode::Char('f'), KeyModifiers::NONE));
    assert_eq!(app.filter_mode, FilterMode::FailedOnly);
    assert_eq!(
        app.filtered_tests().len(),
        0,
        "No tests should be failed initially"
    );

    // Cycle back to All
    app.handle_key(KeyEvent::new(KeyCode::Char('f'), KeyModifiers::NONE));
    assert_eq!(app.filter_mode, FilterMode::All);
    assert_eq!(app.filtered_tests().len(), all_count);
}

#[test]
fn test_tui_human_verdict_entry_and_disagreement() {
    let mut app = create_test_app();

    // Select a human or both test
    let human_idx = app
        .filtered_tests()
        .iter()
        .position(|t| t.kind == TestKind::Human || t.kind == TestKind::Both)
        .expect("Verify library must contain at least one visual review test");

    app.selected_index = human_idx;
    let target_test = app.selected_test().unwrap().clone();

    // Press 'v' to open verdict modal
    app.handle_key(KeyEvent::new(KeyCode::Char('v'), KeyModifiers::NONE));
    assert_eq!(
        app.modal,
        ModalState::HumanVerdict {
            test_id: target_test.id.clone()
        }
    );

    // Record 'Pass'
    app.handle_key(KeyEvent::new(KeyCode::Char('p'), KeyModifiers::NONE));
    assert_eq!(app.modal, ModalState::None);

    let res = app
        .results
        .get(&target_test.id)
        .expect("Result must be recorded");
    assert_eq!(res.human_verdict.verdict, HumanVerdictKind::Pass);

    // Check disagreement logic: if kind is Both and auto failed while human passed
    if target_test.kind == TestKind::Both {
        let mut conflicting_res = res.clone();
        conflicting_res.auto_status = AutoStatus::AutoFail;
        conflicting_res.human_verdict = HumanVerdict {
            verdict: HumanVerdictKind::Pass,
            note: None,
        };
        let (final_status, disagreement) = spawn_at_verify::runner::resolve_final_status(
            target_test.kind,
            conflicting_res.auto_status,
            &conflicting_res.human_verdict,
        );
        assert_eq!(final_status, FinalStatus::Fail);
        assert!(
            disagreement,
            "Contradiction between auto-fail and human-pass must flag disagreement"
        );
    }
}

#[test]
fn test_tui_raw_mode_ctrl_c_backstop() {
    let mut app = create_test_app();
    assert!(!app.should_quit);

    // Ctrl+C key event
    app.handle_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
    assert!(
        app.should_quit,
        "Ctrl+C key event in raw mode must immediately set should_quit"
    );
}

#[test]
fn test_logging_state_parser() {
    let sample = r#"
[2026-10-10T20:39:42.259Z] +0.00ms | SET_LOGGING enabled=true
[2026-10-10T20:39:42.275Z] +15.47ms | ARM_SPAWN target="org.gnome.Calculator"
[2026-10-10T20:40:01.314Z] +19031.89ms | SET_LOGGING enabled=false
"#;
    assert_eq!(parse_logging_state_from_content(sample), Some(false));

    let sample_on = r#"
[2026-10-10T20:39:42.259Z] +0.00ms | SET_LOGGING enabled=false
[2026-10-10T20:40:01.314Z] +19031.89ms | SET_LOGGING enabled=true
"#;
    assert_eq!(parse_logging_state_from_content(sample_on), Some(true));

    assert_eq!(parse_logging_state_from_content("no logging line"), None);
}

#[test]
fn test_queue_pauses_on_human_case_and_resumes_only_after_verdict() {
    let mut app = create_test_app();

    // Verify placement/bottom-right is present and kind Both
    let bottom_right = app
        .registry
        .get_test("placement/bottom-right")
        .expect("placement/bottom-right must exist")
        .clone();
    assert_eq!(bottom_right.kind, TestKind::Both);

    // Enqueue an auto test, then the human/both test, then another test
    app.queue.clear();
    app.queue.push_back("preflight/binary-head".into());
    app.queue.push_back("placement/bottom-right".into());
    app.queue.push_back("cloak/delayed-paint/delay-0".into());

    assert_eq!(app.queue.len(), 3);
    assert!(app.waiting_for_human.is_none());

    // Start processing queue
    app.process_queue_step();

    // The queue should have executed preflight/binary-head (Auto),
    // then executed placement/bottom-right, and PAUSED because bottom-right is TestKind::Both!
    assert_eq!(
        app.waiting_for_human,
        Some("placement/bottom-right".to_string()),
        "Queue must pause on human/both test"
    );
    assert_eq!(
        app.queue.len(),
        1,
        "Remaining auto test must stay in queue while waiting for human verdict"
    );
    assert_eq!(app.queue.front().unwrap(), "cloak/delayed-paint/delay-0");

    // Stepping the queue while waiting for human verdict must be a no-op (remains paused)
    app.process_queue_step();
    assert_eq!(
        app.waiting_for_human,
        Some("placement/bottom-right".to_string())
    );
    assert_eq!(app.queue.len(), 1);

    // User dispatches verdict key 'p' (Pass)
    app.handle_key(KeyEvent::new(KeyCode::Char('p'), KeyModifiers::NONE));

    // Waiting flag must be cleared and queue should resume and finish
    assert!(
        app.waiting_for_human.is_none(),
        "Verdict must clear waiting_for_human"
    );
    assert!(
        app.queue.is_empty(),
        "Queue should resume and process remaining tests to completion"
    );

    let br_res = app
        .results
        .get("placement/bottom-right")
        .expect("Result must exist for bottom-right");
    assert_eq!(br_res.human_verdict.verdict, HumanVerdictKind::Pass);
}

#[test]
fn test_done_count_excludes_waiting_human_case() {
    let mut app = create_test_app();
    let t0 = app.registry.tests[0].clone();
    let pass_res = spawn_at_verify::runner::TestResult {
        id: t0.id.clone(),
        title: t0.title.clone(),
        kind: t0.kind,
        required: t0.required,
        auto_status: AutoStatus::AutoPass,
        human_verdict: HumanVerdict {
            verdict: HumanVerdictKind::Pass,
            note: None,
        },
        final_status: FinalStatus::Pass,
        disagreement: false,
        duration_ms: 10,
        exit_code: Some(0),
        stdout: String::new(),
        stderr: String::new(),
        message: None,
        log_slice: None,
    };
    app.results.insert(t0.id.clone(), pass_res);

    if let Some(t1) = app
        .registry
        .tests
        .iter()
        .find(|t| t.kind != TestKind::Auto)
        .cloned()
    {
        app.waiting_for_human = Some(t1.id.clone());
        let wait_res = spawn_at_verify::runner::TestResult {
            id: t1.id.clone(),
            title: t1.title.clone(),
            kind: t1.kind,
            required: t1.required,
            auto_status: AutoStatus::AutoPass,
            human_verdict: HumanVerdict::default(),
            final_status: FinalStatus::Pending,
            disagreement: false,
            duration_ms: 10,
            exit_code: Some(0),
            stdout: String::new(),
            stderr: String::new(),
            message: None,
            log_slice: None,
        };
        app.results.insert(t1.id.clone(), wait_res);

        let backend = TestBackend::new(80, 28);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| draw_ui(f, &app)).unwrap();
        let rendered = dump_buffer_to_string(&terminal, 80, 28);

        let total = app.registry.tests.len();
        assert!(
            rendered.contains(&format!("1/{} done", total)),
            "Done count must exclude waiting human case"
        );
        assert!(rendered.contains("1 waiting"), "Header must show 1 waiting");
    }
}
