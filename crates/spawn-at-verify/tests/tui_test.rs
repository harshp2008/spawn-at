//! Headless Integration Tests for spawn-at-verify TUI.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{backend::TestBackend, Terminal};
use spawn_at_verify::runner::{
    AutoStatus, FinalStatus, HumanVerdict, HumanVerdictKind, RunOptions,
};
use spawn_at_verify::schema::TestKind;
use spawn_at_verify::tui::app::{FilterMode, ModalState, TuiApp};
use spawn_at_verify::tui::find_verify_dir;
use spawn_at_verify::tui::logging::parse_logging_state_from_content;
use spawn_at_verify::tui::views::draw_ui;
use std::path::PathBuf;

fn create_test_app() -> TuiApp {
    let verify_dir = find_verify_dir().expect("Failed to locate verify/ directory");
    let options = RunOptions {
        bin_path: PathBuf::from("target/debug/spawn-at"),
        ..Default::default()
    };
    TuiApp::new(&verify_dir, options).expect("Failed to initialize TuiApp against verify/")
}

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
        .registry
        .tests
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
fn test_tui_headless_render_and_missing_fixture_banner() {
    let mut app = create_test_app();
    // Simulate missing fixture binary
    app.fixture_exists = false;
    app.fixture_path = PathBuf::from("target/debug/fixture");

    let backend = TestBackend::new(120, 30);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal
        .draw(|f| {
            draw_ui(f, &app);
        })
        .unwrap();

    let buffer = terminal.backend().buffer().clone();
    let text: String = buffer.content().iter().map(|cell| cell.symbol()).collect();

    // Verify header title
    assert!(text.contains("spawn-at-verify TUI"));
    // Verify clear missing fixture warning banner
    assert!(
        text.contains("FIXTURE MISSING")
            || text.contains("cargo build -p spawn-at-verify --bin fixture --features fixture")
    );
}

#[test]
fn test_tui_detail_drawer_toggle_and_scrolling() {
    let mut app = create_test_app();
    assert!(!app.detail_drawer_open);
    assert_eq!(app.detail_scroll, 0);

    // Press Enter to open detail drawer full view
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(app.detail_drawer_open);

    // Scroll down in drawer
    app.handle_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE));
    assert_eq!(app.detail_scroll, 1);

    app.handle_key(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE));
    assert_eq!(app.detail_scroll, 9);

    // Scroll up in drawer
    app.handle_key(KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE));
    assert_eq!(app.detail_scroll, 1);

    // Home resets scroll
    app.handle_key(KeyEvent::new(KeyCode::Home, KeyModifiers::NONE));
    assert_eq!(app.detail_scroll, 0);

    // Press Esc to exit detail drawer
    app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert!(!app.detail_drawer_open);

    // Press 'd' to toggle open
    app.handle_key(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE));
    assert!(app.detail_drawer_open);

    // Press 'd' to toggle close
    app.handle_key(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE));
    assert!(!app.detail_drawer_open);
}

#[test]
fn test_tui_detail_drawer_content_ordering_and_failure_box() {
    let mut app = create_test_app();
    let test = app.selected_test().unwrap().clone();

    // Insert a failed result with diagnostic message
    let failed_result = spawn_at_verify::runner::TestResult {
        id: test.id.clone(),
        title: test.title.clone(),
        kind: test.kind,
        required: test.required,
        auto_status: AutoStatus::AutoFail,
        human_verdict: HumanVerdict::default(),
        final_status: FinalStatus::Fail,
        disagreement: false,
        duration_ms: 42,
        exit_code: Some(1),
        stdout: r#"{"window_id":123,"x":100,"y":200,"w":400,"h":300,"source":"claim"}"#.into(),
        stderr: "[spawn-at] ERROR: Failed window alignment\n".into(),
        message: Some("Window mapped at (100, 200) but expected (500, 500)".into()),
        log_slice: Some(
            "[spawn-at-time] +0.00ms | CLOAK\n[spawn-at-time] +5.00ms | VIOLATION: visible flash"
                .into(),
        ),
    };
    app.results.insert(test.id.clone(), failed_result);

    let backend = TestBackend::new(140, 35);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal
        .draw(|f| {
            draw_ui(f, &app);
        })
        .unwrap();

    let buffer = terminal.backend().buffer().clone();
    let text: String = buffer.content().iter().map(|cell| cell.symbol()).collect();

    // Verify failure summary box appears on main pane
    assert!(text.contains("Failure Summary"));
    assert!(text.contains("✖ Reason:"));
    assert!(text.contains("Window mapped at (100, 200)"));

    // Verify detail drawer ordering: failure reason first
    assert!(text.contains("✖ FAILURE REASON:"));
    assert!(text.contains("Geometric Oracle Comparison"));
    assert!(text.contains("Actual Geometry: pos=(100, 200)"));
    assert!(text.contains("Compositor Actor Trace Diagnostics"));
    assert!(text.contains("VIOLATION: visible flash"));
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
