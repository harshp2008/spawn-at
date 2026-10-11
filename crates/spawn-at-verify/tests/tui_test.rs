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
    assert!(text.contains("spawn-at verify"));
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

    // Verify Result box appears on main pane
    assert!(text.contains("Result"));
    assert!(text.contains("✖ Auto: FAIL"));
    assert!(text.contains("Window mapped at (100, 200)"));

    // Open detail drawer to verify drawer content ordering
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(app.detail_drawer_open);

    terminal
        .draw(|f| {
            draw_ui(f, &app);
        })
        .unwrap();

    let drawer_buf = terminal.backend().buffer().clone();
    let drawer_text: String = drawer_buf
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect();

    // Verify detail drawer ordering: failure reason first
    assert!(drawer_text.contains("✖ FAILURE REASON:"));
    assert!(drawer_text.contains("Geometric Oracle Comparison"));
    assert!(drawer_text.contains("Actual Geometry: pos=(100, 200)"));
    assert!(drawer_text.contains("Compositor Actor Trace Diagnostics"));
    assert!(drawer_text.contains("VIOLATION: visible flash"));
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
fn test_headless_render_80x24_shows_clipping_hint_when_overflowed() {
    let mut app = create_test_app();
    let test = app.selected_test().unwrap().clone();

    // Set multi-line description on selected test to guarantee overflow in 80x24 terminal
    let mut long_desc = String::new();
    for i in 1..=25 {
        long_desc.push_str(&format!("Detailed behavioral description line {}\n", i));
    }
    let target_id = test.id.clone();
    if let Some(t) = app.registry.tests.iter_mut().find(|t| t.id == target_id) {
        t.description = long_desc;
    }

    let overflow_result = spawn_at_verify::runner::TestResult {
        id: test.id.clone(),
        title: test.title.clone(),
        kind: test.kind,
        required: test.required,
        auto_status: AutoStatus::AutoFail,
        human_verdict: HumanVerdict::default(),
        final_status: FinalStatus::Fail,
        disagreement: false,
        duration_ms: 120,
        exit_code: Some(1),
        stdout: "Standard output captured from run\n".into(),
        stderr: "Standard error output with diagnostic notes\n".into(),
        message: Some("Assertions failed: geometry mismatch on placement test".into()),
        log_slice: Some(
            "[spawn-at-time] +0.00ms | CLOAK\n[spawn-at-time] +8.50ms | UNCLOAK\n[spawn-at-time] +9.00ms | VIOLATION: visible flicker\n[spawn-at-time] +10.00ms | WATCHDOG: frame timeout"
                .into(),
        ),
    };
    app.results.insert(test.id.clone(), overflow_result);

    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal
        .draw(|f| {
            draw_ui(f, &app);
        })
        .unwrap();

    let buffer = terminal.backend().buffer().clone();
    let text: String = buffer.content().iter().map(|cell| cell.symbol()).collect();
    // In 80x24, right panel content overflows and MUST show the clipping hint (Item 14 & 16)
    assert!(
        text.contains("more: press Enter or d"),
        "Buffer at 80x24 must show clipping hint when content overflows"
    );
    assert!(
        text.contains("details") || text.contains("expand"),
        "Buffer at 80x24 must show expand details action"
    );

    // Open detail drawer with Enter key
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(app.detail_drawer_open);

    terminal
        .draw(|f| {
            draw_ui(f, &app);
        })
        .unwrap();

    let drawer_buffer = terminal.backend().buffer().clone();
    let drawer_text: String = drawer_buffer
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect();

    // Drawer must show scroll hints and key controls
    assert!(
        drawer_text.contains("j/k scroll, Esc/Enter close"),
        "Drawer must show scroll hints and close keys"
    );
}

#[test]
fn test_render_snapshots() {
    fn dump_buffer(terminal: &Terminal<TestBackend>, width: u16, height: u16) -> String {
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

    let width = 100;
    let height = 26;

    // 1. Idle State
    let mut app = create_test_app();
    app.fixture_exists = true;
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| draw_ui(f, &app)).unwrap();
    let idle_snapshot = dump_buffer(&terminal, width, height);

    // 2. Waiting-for-verdict State
    let mut wait_app = create_test_app();
    wait_app.fixture_exists = true;
    if let Some(pos) = wait_app
        .filtered_tests()
        .iter()
        .position(|t| t.id == "placement/bottom-right")
    {
        wait_app.selected_index = pos;
    }
    wait_app.waiting_for_human = Some("placement/bottom-right".to_string());
    wait_app.status_message = Some("Waiting for your verdict on 'placement/bottom-right'".into());
    let wait_res = spawn_at_verify::runner::TestResult {
        id: "placement/bottom-right".into(),
        title: "Window placed in bottom-right corner".into(),
        kind: TestKind::Both,
        required: true,
        auto_status: AutoStatus::AutoPass,
        human_verdict: HumanVerdict::default(),
        final_status: FinalStatus::Pending,
        disagreement: false,
        duration_ms: 185,
        exit_code: Some(0),
        stdout: r#"{"window_id":870905,"x":1504,"y":764,"w":400,"h":300}"#.into(),
        stderr: String::new(),
        message: Some("Assertions passed".into()),
        log_slice: Some(
            "[spawn-at-time] +0.00ms | CLOAK\n[spawn-at-time] +4.80ms | UNCLOAK".into(),
        ),
    };
    wait_app
        .results
        .insert("placement/bottom-right".into(), wait_res);
    let mut wait_term = Terminal::new(TestBackend::new(width, height)).unwrap();
    wait_term.draw(|f| draw_ui(f, &wait_app)).unwrap();
    let waiting_snapshot = dump_buffer(&wait_term, width, height);

    // 3. Failure State
    let mut fail_app = create_test_app();
    fail_app.fixture_exists = true;
    let fail_test = fail_app.selected_test().unwrap().clone();
    let fail_res = spawn_at_verify::runner::TestResult {
        id: fail_test.id.clone(),
        title: fail_test.title.clone(),
        kind: fail_test.kind,
        required: fail_test.required,
        auto_status: AutoStatus::AutoFail,
        human_verdict: HumanVerdict::default(),
        final_status: FinalStatus::Fail,
        disagreement: false,
        duration_ms: 94,
        exit_code: Some(1),
        stdout: r#"{"window_id":870905,"x":1200,"y":600,"w":400,"h":300}"#.into(),
        stderr: "[spawn-at] ERROR: Geometric target mismatch".into(),
        message: Some("Window mapped at (1200, 600) but expected (1504, 764)".into()),
        log_slice: Some("[spawn-at-time] +0.00ms | CLOAK\n[spawn-at-time] +8.20ms | VIOLATION: visible flicker\n[spawn-at-time] +9.50ms | UNCLOAK".into()),
    };
    fail_app.results.insert(fail_test.id.clone(), fail_res);
    let mut fail_term = Terminal::new(TestBackend::new(width, height)).unwrap();
    fail_term.draw(|f| draw_ui(f, &fail_app)).unwrap();
    let fail_snapshot = dump_buffer(&fail_term, width, height);
    assert!(idle_snapshot.contains("spawn-at verify"));
    assert!(idle_snapshot.contains("r: Run · a: Run all"));

    assert!(waiting_snapshot.contains("1 waiting for you"));
    assert!(waiting_snapshot.contains("p: Pass · x: Fail"));

    assert!(fail_snapshot.contains("✖ Auto: FAIL"));
}

#[test]
fn test_render_passing_case_contains_no_red_styled_cells() {
    let mut app = create_test_app();
    app.fixture_exists = true;

    // Simulate passing results for all tests
    for test in &app.registry.tests {
        let pass_res = spawn_at_verify::runner::TestResult {
            id: test.id.clone(),
            title: test.title.clone(),
            kind: test.kind,
            required: test.required,
            auto_status: AutoStatus::AutoPass,
            human_verdict: HumanVerdict {
                verdict: HumanVerdictKind::Pass,
                note: None,
            },
            final_status: FinalStatus::Pass,
            disagreement: false,
            duration_ms: 35,
            exit_code: Some(0),
            stdout: r#"{"window_id":123,"x":100,"y":100,"w":400,"h":300}"#.into(),
            stderr: String::new(),
            message: Some("Assertions passed".into()),
            log_slice: Some(
                "[spawn-at-time] +0.00ms | CLOAK\n[spawn-at-time] +5.00ms | UNCLOAK".into(),
            ),
        };
        app.results.insert(test.id.clone(), pass_res);
    }

    let width = 120;
    let height = 30;
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal.draw(|f| draw_ui(f, &app)).unwrap();

    let buf = terminal.backend().buffer();
    for y in 0..height {
        for x in 0..width {
            let cell = &buf[(x, y)];
            assert_ne!(
                cell.fg,
                ratatui::style::Color::Red,
                "Found Red fg cell at ({}, {}): '{}'",
                x,
                y,
                cell.symbol()
            );
            assert_ne!(
                cell.fg,
                ratatui::style::Color::LightRed,
                "Found LightRed fg cell at ({}, {}): '{}'",
                x,
                y,
                cell.symbol()
            );
            assert_ne!(
                cell.bg,
                ratatui::style::Color::Red,
                "Found Red bg cell at ({}, {}): '{}'",
                x,
                y,
                cell.symbol()
            );
            assert_ne!(
                cell.bg,
                ratatui::style::Color::LightRed,
                "Found LightRed bg cell at ({}, {}): '{}'",
                x,
                y,
                cell.symbol()
            );
        }
    }
}

#[test]
fn test_render_columns_alignment_between_header_and_rows() {
    let mut app = create_test_app();
    app.fixture_exists = true;

    let width = 140;
    let height = 30;
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal.draw(|f| draw_ui(f, &app)).unwrap();

    let buf = terminal.backend().buffer();

    // Check actual grid cell x-coordinates for columns
    let mut header_y = None;
    let mut h_status_x = None;
    let mut h_id_x = None;
    let mut h_kind_x = None;
    let mut h_title_x = None;

    for y in 0..height {
        let mut line = String::new();
        for x in 0..width {
            line.push_str(buf[(x, y)].symbol());
        }
        if line.contains("Status") && line.contains("Test ID") {
            header_y = Some(y);
            // Search cell x
            for x in 0..(width - 6) {
                if (0..6usize).all(|i| buf[(x + i as u16, y)].symbol() == &"Status"[i..i + 1]) {
                    h_status_x = Some(x);
                }
                if (0..7usize).all(|i| buf[(x + i as u16, y)].symbol() == &"Test ID"[i..i + 1]) {
                    h_id_x = Some(x);
                }
                if (0..4usize).all(|i| buf[(x + i as u16, y)].symbol() == &"Kind"[i..i + 1]) {
                    h_kind_x = Some(x);
                }
                if (0..5usize).all(|i| buf[(x + i as u16, y)].symbol() == &"Title"[i..i + 1]) {
                    h_title_x = Some(x);
                }
            }
            break;
        }
    }

    assert!(header_y.is_some(), "Header row must be rendered");
    let h_status = h_status_x.expect("Status must be present in header");
    let h_id = h_id_x.expect("Test ID must be present in header");
    let h_kind = h_kind_x.expect("Kind must be present in header");
    let h_title = h_title_x.expect("Title must be present in header");

    // Find data row for preflight/binary-head
    let mut found_matching_row = false;
    for y in (header_y.unwrap() + 1)..height {
        let mut line = String::new();
        for x in 0..width {
            line.push_str(buf[(x, y)].symbol());
        }
        if line.contains("preflight/binary-head") {
            let mut row_status_x = None;
            let mut row_id_x = None;
            let mut row_kind_x = None;
            let mut row_title_x = None;

            for x in 0..(width - 20) {
                if row_status_x.is_none() && buf[(x, y)].symbol() == "●" {
                    row_status_x = Some(x);
                }
                if row_id_x.is_none()
                    && (0..21usize).all(|i| {
                        buf[(x + i as u16, y)].symbol() == &"preflight/binary-head"[i..i + 1]
                    })
                {
                    row_id_x = Some(x);
                }
                if row_kind_x.is_none()
                    && (0..4usize).all(|i| buf[(x + i as u16, y)].symbol() == &"auto"[i..i + 1])
                {
                    row_kind_x = Some(x);
                }
                if row_title_x.is_none()
                    && (0..6usize).all(|i| buf[(x + i as u16, y)].symbol() == &"Binary"[i..i + 1])
                {
                    row_title_x = Some(x);
                }
            }

            assert_eq!(
                row_status_x.unwrap(),
                h_status,
                "Status in data row must start at identical x position as Status in header"
            );
            assert_eq!(
                row_id_x.unwrap(),
                h_id,
                "Test ID in data row must start at identical x position as Test ID in header"
            );
            assert_eq!(
                row_kind_x.unwrap(),
                h_kind,
                "Kind in data row must start at identical x position as Kind in header"
            );
            assert_eq!(
                row_title_x.unwrap(),
                h_title,
                "Title in data row must start at identical x position as Title in header"
            );
            found_matching_row = true;
            break;
        }
    }

    assert!(
        found_matching_row,
        "Must find and verify at least one test data row"
    );
}

#[test]
fn test_render_responsive_breakpoints_60_80_100_140() {
    let mut app = create_test_app();
    app.fixture_exists = true;

    fn get_text(app: &TuiApp, width: u16, height: u16) -> String {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| draw_ui(f, app)).unwrap();
        let buf = terminal.backend().buffer();
        let mut out = String::new();
        for y in 0..height {
            for x in 0..width {
                out.push_str(buf[(x, y)].symbol());
            }
            out.push('\n');
        }
        out
    }

    // 1. Wide breakpoint (140 cols >= 110):
    // List | Overview side by side, all columns (Cursor, Status, full ID, Kind, Title)
    let wide_text = get_text(&app, 140, 28);
    assert!(
        wide_text.contains("Test Suite"),
        "Wide must render Test Suite"
    );
    assert!(
        wide_text.contains("Test Overview"),
        "Wide must render Test Overview side by side"
    );
    assert!(
        wide_text.contains("Status"),
        "Wide must render Status column"
    );
    assert!(
        wide_text.contains("Test ID"),
        "Wide must render full Test ID header"
    );
    assert!(wide_text.contains("Kind"), "Wide must render Kind column");
    assert!(wide_text.contains("Title"), "Wide must render Title column");
    assert!(
        wide_text.contains("preflight/binary-head"),
        "Wide must render full test ID"
    );

    // 2. Medium breakpoint (100 cols in 75-109):
    // Side by side; list shows Status + short name + Kind; drops Title column; category heading carries context
    let med100_text = get_text(&app, 100, 28);
    assert!(
        med100_text.contains("Test Suite"),
        "Medium 100 must render Test Suite"
    );
    assert!(
        med100_text.contains("Test Overview"),
        "Medium 100 must render Test Overview side by side"
    );
    assert!(
        med100_text.contains("Status"),
        "Medium 100 must render Status column"
    );
    assert!(
        med100_text.contains("Name"),
        "Medium 100 must render Name header"
    );
    assert!(
        med100_text.contains("Kind"),
        "Medium 100 must render Kind column"
    );
    assert!(
        !med100_text
            .lines()
            .any(|l| l.contains("Status") && l.contains("Title")),
        "Medium 100 must drop Title column from header"
    );
    assert!(
        med100_text.contains("binary-head"),
        "Medium 100 must show short name"
    );
    assert!(
        med100_text.contains("Cloak & Delayed Paint"),
        "Medium 100 category carries context"
    );

    // 3. Medium breakpoint (80 cols in 75-109):
    let med80_text = get_text(&app, 80, 28);
    assert!(
        med80_text.contains("Test Suite"),
        "Medium 80 must render Test Suite"
    );
    assert!(
        med80_text.contains("Test Overview"),
        "Medium 80 must render Test Overview side by side"
    );
    assert!(
        med80_text.contains("Status"),
        "Medium 80 must render Status column"
    );
    assert!(
        med80_text.contains("Name"),
        "Medium 80 must render Name header"
    );
    assert!(
        med80_text.contains("Kind"),
        "Medium 80 must render Kind column"
    );
    assert!(
        med80_text.contains("binary-head"),
        "Medium 80 must show short name"
    );

    // 4. Narrow breakpoint (60 cols < 75):
    // Single pane; list full width; status NEVER dropped
    let narrow_text = get_text(&app, 60, 28);
    assert!(
        narrow_text.contains("Test Suite"),
        "Narrow must render Test Suite"
    );
    assert!(
        !narrow_text.contains("Test Overview"),
        "Narrow must NOT render side-by-side Test Overview"
    );
    assert!(
        narrow_text.contains("Status"),
        "Narrow must always show Status column"
    );
    assert!(
        narrow_text.contains("● not run"),
        "Narrow must show status indicators"
    );
}

#[test]
fn test_render_terminal_below_minimum_size_shows_warning() {
    let mut app = create_test_app();
    app.fixture_exists = true;

    // Below 60x15: width 55, height 12
    let backend = TestBackend::new(55, 12);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| draw_ui(f, &app)).unwrap();

    let buf = terminal.backend().buffer();
    let mut text = String::new();
    for y in 0..12 {
        for x in 0..55 {
            text.push_str(buf[(x, y)].symbol());
        }
        text.push('\n');
    }

    assert!(
        text.contains("Terminal too small — need at least 60x15"),
        "Must display clear size warning below 60x15"
    );
}
