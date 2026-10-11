//! Headless Integration Tests for spawn-at-verify TUI.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{backend::TestBackend, style::Color, Terminal};
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

    // Verify detail drawer ordering: RESULT, GEOMETRY, TIMELINE visible on screen
    assert!(drawer_text.contains("RESULT"));
    assert!(drawer_text.contains("GEOMETRY"));
    assert!(drawer_text.contains("TIMELINE"));

    // Verify detail drawer ordering across all sections in full drawer content
    let (_lines, plain) = spawn_at_verify::tui::views::build_drawer_content(&app, &test, 140, true);
    assert!(plain.contains("RESULT"));
    assert!(plain.contains("GEOMETRY"));
    assert!(plain.contains("TIMELINE"));
    assert!(plain.contains("WHAT TO LOOK FOR"));
    assert!(plain.contains("SPEC"));
    assert!(plain.contains("OUTPUT"));
    let pos_res = plain.find("RESULT").unwrap();
    let pos_geo = plain.find("GEOMETRY").unwrap();
    let pos_time = plain.find("TIMELINE").unwrap();
    let pos_what = plain.find("WHAT TO LOOK FOR").unwrap();
    let pos_spec = plain.find("SPEC").unwrap();
    let pos_out = plain.find("OUTPUT").unwrap();
    assert!(pos_res < pos_geo);
    assert!(pos_geo < pos_time);
    assert!(pos_time < pos_what);
    assert!(pos_what < pos_spec);
    assert!(pos_spec < pos_out);
    assert!(!plain.contains("---"));
    assert!(!plain.contains("1. "));
    // In raw view, raw trace lines are included
    let (_raw_lines, raw_plain) =
        spawn_at_verify::tui::views::build_drawer_content(&app, &test, 140, false);
    assert!(raw_plain.contains("visible flash"));
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
    // In 80x24, right panel content overflows and MUST show the clipping hint with "d to expand"
    assert!(
        text.contains("▼ more · d to expand"),
        "Buffer at 80x24 must show clipping hint '▼ more · d to expand' when content overflows"
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
    assert!(drawer_text.contains("Details"));

    // Drawer bar contains "Copy", "Save", "Scroll", "Close" and does NOT contain "Filter", "Fold", "Run"
    let mut bar_text = String::new();
    for y in 19..24 {
        for x in 0..80 {
            bar_text.push_str(drawer_buffer[(x, y)].symbol());
        }
    }
    assert!(bar_text.contains("Copy"));
    assert!(bar_text.contains("Save"));
    assert!(bar_text.contains("Scroll"));
    assert!(bar_text.contains("Close"));
    assert!(!bar_text.contains("Filter"));
    assert!(!bar_text.contains("Fold"));
    assert!(!bar_text.contains("Run"));
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
    assert!(idle_snapshot.contains("r Run  a All"));

    assert!(waiting_snapshot.contains("1 waiting"));
    assert!(waiting_snapshot.contains("Your verdict:"));
    assert!(waiting_snapshot.contains("Pass"));
    assert!(waiting_snapshot.contains("Fail"));

    assert!(fail_snapshot.contains("FAIL"));
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

fn dump_buffer_to_string(terminal: &Terminal<TestBackend>, width: u16, height: u16) -> String {
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

fn validate_render_spec(rendered: &str, width: u16, height: u16, is_waiting: bool) {
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

    assert!(
        cat_headings_found > 0 || (height <= 20 && is_waiting),
        "Must have found category headings at {}x{}",
        width,
        height
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

#[test]
fn test_render_responsive_breakpoints_60_70_80_100_140_and_small_height_60x20() {
    let resolutions = [(60, 20), (60, 28), (70, 28), (80, 28), (100, 28), (140, 28)];

    for &(w, h) in &resolutions {
        // 1. Idle state render
        let mut app = create_test_app();
        app.fixture_exists = true;
        let backend = TestBackend::new(w, h);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| draw_ui(f, &app)).unwrap();
        let idle_rendered = dump_buffer_to_string(&terminal, w, h);

        validate_render_spec(&idle_rendered, w, h, false);

        // 2. Waiting-for-verdict state render
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
        wait_app.status_message =
            Some("Waiting for your verdict on 'placement/bottom-right'".into());
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

        let wait_backend = TestBackend::new(w, h);
        let mut wait_term = Terminal::new(wait_backend).unwrap();
        wait_term.draw(|f| draw_ui(f, &wait_app)).unwrap();
        let wait_rendered = dump_buffer_to_string(&wait_term, w, h);

        validate_render_spec(&wait_rendered, w, h, true);
    }

    // Verify scrolling works at 60x20 narrow
    let mut scroll_app = create_test_app();
    scroll_app.fixture_exists = true;
    scroll_app.active_pane = spawn_at_verify::tui::app::ActivePane::Details;
    for _ in 0..8 {
        scroll_app.handle_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE));
    }
    let scroll_backend = TestBackend::new(60, 20);
    let mut scroll_term = Terminal::new(scroll_backend).unwrap();
    scroll_term.draw(|f| draw_ui(f, &scroll_app)).unwrap();
    let scroll_rendered = dump_buffer_to_string(&scroll_term, 60, 20);
    assert!(
        scroll_rendered.contains("What to look for"),
        "Scrolling at 60x20 reveals What to look for"
    );
}

#[test]
fn test_help_modal_and_tab_panel_switching() {
    let mut app = create_test_app();
    app.fixture_exists = true;

    // 1. Tab switches focus between List and Details
    assert_eq!(app.active_pane, spawn_at_verify::tui::app::ActivePane::List);
    app.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    assert_eq!(
        app.active_pane,
        spawn_at_verify::tui::app::ActivePane::Details
    );

    // In Details focus, j/k scrolls details pane
    assert_eq!(app.detail_scroll, 0);
    app.handle_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE));
    assert_eq!(app.detail_scroll, 1);
    app.handle_key(KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE));
    assert_eq!(app.detail_scroll, 0);

    // Tab back to List
    app.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    assert_eq!(app.active_pane, spawn_at_verify::tui::app::ActivePane::List);

    // 2. '?' opens help overlay modal
    app.handle_key(KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE));
    assert_eq!(app.modal, ModalState::Help);

    let backend = TestBackend::new(80, 25);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| draw_ui(f, &app)).unwrap();
    let help_rendered = dump_buffer_to_string(&terminal, 80, 25);

    assert!(help_rendered.contains("KEYS & COMMANDS"));
    assert!(help_rendered.contains("STATUS SYMBOLS"));
    assert!(help_rendered.contains("MODES LEGEND"));
    assert!(help_rendered.contains("auto:"));
    assert!(help_rendered.contains("you:"));
    assert!(help_rendered.contains("both:"));

    // Esc closes help modal
    app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert_eq!(app.modal, ModalState::None);
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

#[test]
fn test_dump_snapshots_to_scratch() {
    let resolutions = [(60, 28), (80, 28), (140, 28)];
    let mut out = String::new();

    for &(w, h) in &resolutions {
        // 1. Idle
        let (mut app, _) = create_app_with_real_fixture();
        app.results.clear(); // no results yet
        app.fixture_exists = true;
        let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
        term.draw(|f| draw_ui(f, &app)).unwrap();
        out.push_str(&format!(
            "### SNAPSHOT {}x{} - IDLE\n```\n{}\n```\n\n",
            w,
            h,
            dump_buffer_to_string(&term, w, h)
        ));

        // 2. Waiting for verdict
        let (mut wait_app, _) = create_app_with_real_fixture();
        wait_app.fixture_exists = true;
        wait_app.waiting_for_human = Some("placement/bottom-right".to_string());
        wait_app.status_message =
            Some("Waiting for your verdict on 'placement/bottom-right'".into());
        let mut wait_term = Terminal::new(TestBackend::new(w, h)).unwrap();
        wait_term.draw(|f| draw_ui(f, &wait_app)).unwrap();
        out.push_str(&format!(
            "### SNAPSHOT {}x{} - WAITING FOR A HUMAN CASE\n```\n{}\n```\n\n",
            w,
            h,
            dump_buffer_to_string(&wait_term, w, h)
        ));

        // 3. Passed
        let (mut pass_app, _) = create_app_with_real_fixture();
        pass_app.fixture_exists = true;
        if let Some(r) = pass_app.results.get_mut("placement/bottom-right") {
            r.final_status = FinalStatus::Pass;
            r.human_verdict.verdict = HumanVerdictKind::Pass;
        }
        let mut pass_term = Terminal::new(TestBackend::new(w, h)).unwrap();
        pass_term.draw(|f| draw_ui(f, &pass_app)).unwrap();
        out.push_str(&format!(
            "### SNAPSHOT {}x{} - PASSED\n```\n{}\n```\n\n",
            w,
            h,
            dump_buffer_to_string(&pass_term, w, h)
        ));

        // 4. Failed
        let (mut fail_app, _) = create_app_with_real_fixture();
        fail_app.fixture_exists = true;
        if let Some(r) = fail_app.results.get_mut("placement/bottom-right") {
            r.final_status = FinalStatus::Fail;
            r.human_verdict.verdict = HumanVerdictKind::Fail;
            r.message = Some("Window mapped at (1200, 600) but expected (1504, 764)".into());
        }
        let mut fail_term = Terminal::new(TestBackend::new(w, h)).unwrap();
        fail_term.draw(|f| draw_ui(f, &fail_app)).unwrap();
        out.push_str(&format!(
            "### SNAPSHOT {}x{} - FAILED\n```\n{}\n```\n\n",
            w,
            h,
            dump_buffer_to_string(&fail_term, w, h)
        ));

        // 5. Help modal
        let (mut help_app, _) = create_app_with_real_fixture();
        help_app.fixture_exists = true;
        help_app.modal = ModalState::Help;
        let mut help_term = Terminal::new(TestBackend::new(w, h)).unwrap();
        help_term.draw(|f| draw_ui(f, &help_app)).unwrap();
        out.push_str(&format!(
            "### SNAPSHOT {}x{} - HELP MODAL\n```\n{}\n```\n\n",
            w,
            h,
            dump_buffer_to_string(&help_term, w, h)
        ));

        // 6. Detail drawer: Top
        let (mut d_top_app, _) = create_app_with_real_fixture();
        d_top_app.fixture_exists = true;
        d_top_app.detail_drawer_open = true;
        d_top_app.detail_scroll = 0;
        let mut d_top_term = Terminal::new(TestBackend::new(w, h)).unwrap();
        d_top_term.draw(|f| draw_ui(f, &d_top_app)).unwrap();
        out.push_str(&format!(
            "### SNAPSHOT {}x{} - DETAIL DRAWER (TOP, SCROLL 0)\n```\n{}\n```\n\n",
            w,
            h,
            dump_buffer_to_string(&d_top_term, w, h)
        ));

        // 7. Detail drawer: Middle (Timeline)
        let (mut d_mid_app, _) = create_app_with_real_fixture();
        d_mid_app.fixture_exists = true;
        d_mid_app.detail_drawer_open = true;
        d_mid_app.detail_scroll = 16;
        let mut d_mid_term = Terminal::new(TestBackend::new(w, h)).unwrap();
        d_mid_term.draw(|f| draw_ui(f, &d_mid_app)).unwrap();
        out.push_str(&format!(
            "### SNAPSHOT {}x{} - DETAIL DRAWER (TIMELINE, SCROLL 16)\n```\n{}\n```\n\n",
            w,
            h,
            dump_buffer_to_string(&d_mid_term, w, h)
        ));

        // 8. Detail drawer: Bottom
        let (mut d_bot_app, _) = create_app_with_real_fixture();
        d_bot_app.fixture_exists = true;
        d_bot_app.detail_drawer_open = true;
        d_bot_app.detail_scroll = 40;
        let mut d_bot_term = Terminal::new(TestBackend::new(w, h)).unwrap();
        d_bot_term.draw(|f| draw_ui(f, &d_bot_app)).unwrap();
        out.push_str(&format!(
            "### SNAPSHOT {}x{} - DETAIL DRAWER (BOTTOM, SCROLL 40)\n```\n{}\n```\n\n",
            w,
            h,
            dump_buffer_to_string(&d_bot_term, w, h)
        ));
    }

    let scratch_path =
        "/home/harsh/.gemini/antigravity-ide/brain/c19dfab5-8922-4446-88cf-50dca990ff83/scratch/snapshots.txt";
    let _ = std::fs::write(scratch_path, out);
}

#[test]
fn test_ui_polish_spacing_padding_outcome_symbols_and_result_box() {
    let mut app = create_test_app();
    app.fixture_exists = true;

    // Assert: blank row below keys bar, blank row after header, right padding in list rows, Result box present at 60, 80 and 140 columns
    for &w in &[60, 80, 140] {
        let backend = TestBackend::new(w, 28);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| draw_ui(f, &app)).unwrap();
        let rendered = dump_buffer_to_string(&terminal, w, 28);

        // Result box present at 60, 80 and 140 columns
        assert!(
            rendered.contains(" Result "),
            "Result box must be present as boxed panel at {} columns",
            w
        );

        let lines: Vec<&str> = rendered.lines().collect();

        // Blank row below keys bar
        assert!(
            lines.last().unwrap().trim().is_empty(),
            "Blank row below keys bar at bottom must be present at {} cols",
            w
        );

        // Blank row above header
        assert!(
            lines[0].trim().is_empty(),
            "Blank row above header must be present at {} cols",
            w
        );

        // Blank row after header
        assert!(
            lines[2].trim().is_empty(),
            "Blank row after header must be present at {} cols",
            w
        );

        // Right padding in list rows so "auto"/"both" never touch the border
        for line in &lines {
            if line.contains('│') {
                assert!(
                    !line.contains("auto│") && !line.contains("both│") && !line.contains("human│"),
                    "Right padding in list rows required so kind never touches border: '{}'",
                    line
                );
            }
        }
    }

    // Assert: aligned outcome symbols with fixed 2-char symbol slot
    let mut outcome_app = create_test_app();
    outcome_app.fixture_exists = true;
    if let Some(pos) = outcome_app
        .filtered_tests()
        .iter()
        .position(|t| t.id == "placement/bottom-right")
    {
        outcome_app.selected_index = pos;
    }
    let res = spawn_at_verify::runner::TestResult {
        id: "placement/bottom-right".into(),
        title: "Window placed in bottom-right corner".into(),
        kind: TestKind::Both,
        required: true,
        auto_status: AutoStatus::AutoPass,
        human_verdict: HumanVerdict::default(), // Unset -> "? You: pending review"
        final_status: FinalStatus::Pending,
        disagreement: false,
        duration_ms: 121,
        exit_code: Some(0),
        stdout: r#"{"window_id":870905,"x":1504,"y":764,"w":400,"h":300}"#.into(),
        stderr: String::new(),
        message: Some("Assertions passed".into()),
        log_slice: Some(
            "+0.00ms | CLOAK win=[id=1002 pid=2002 class=\"test\" app=\"test\" type=NORMAL proto=Wayland] opacity=0 frame=(50,50,400x300)\n+32.50ms | UNCLOAK win=[id=1002 pid=2002 class=\"test\" app=\"test\" type=NORMAL proto=Wayland] opacity=255 frame=(50,50,400x300)".into(),
        ),
    };
    outcome_app
        .results
        .insert("placement/bottom-right".into(), res);

    let backend = TestBackend::new(100, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| draw_ui(f, &outcome_app)).unwrap();
    let rendered = dump_buffer_to_string(&terminal, 100, 30);

    // Verify fixed 2-char symbol slot outcome lines are rendered and aligned
    assert!(
        rendered.contains("✔ Auto: PASS (121 ms)"),
        "Rendered must contain aligned '✔ Auto: PASS (121 ms)'"
    );
    assert!(
        rendered.contains("  Cloak: 32.5 ms · Trace: 0 violations"),
        "Rendered must contain aligned '  Cloak: 32.5 ms · Trace: 0 violations'"
    );
    assert!(
        rendered.contains("? You: pending review"),
        "Rendered must contain aligned '? You: pending review'"
    );
}

#[test]
fn test_result_and_spacing_polish_spec() {
    let resolutions = [(60, 28), (80, 28), (140, 28)];

    for &(w, h) in &resolutions {
        // --- 1. Idle state checks ---
        let mut app = create_test_app();
        app.fixture_exists = true;
        let backend = TestBackend::new(w, h);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| draw_ui(f, &app)).unwrap();
        let idle_rendered = dump_buffer_to_string(&terminal, w, h);
        let idle_lines: Vec<&str> = idle_rendered.lines().collect();

        // 1 blank row above header
        assert!(
            idle_lines[0].trim().is_empty(),
            "1 blank row above header required at {}x{}",
            w,
            h
        );
        // 1 blank row between header and first box
        assert!(
            idle_lines[2].trim().is_empty(),
            "1 blank row between header and first box required at {}x{}",
            w,
            h
        );
        // 1 blank row at the bottom below keys bar
        assert!(
            idle_lines[h as usize - 1].trim().is_empty(),
            "1 blank row at bottom required at {}x{}",
            w,
            h
        );

        // Find Result box borders in idle state
        let idle_res_top = idle_lines
            .iter()
            .position(|l| l.contains("┌ Result "))
            .expect("Result top border must exist");
        let idle_res_bottom = idle_lines[idle_res_top..]
            .iter()
            .position(|l| l.contains('└'))
            .map(|idx| idle_res_top + idx)
            .expect("Result bottom border must exist");

        // Exactly 1 blank row between Result box and keys bar
        assert!(
            idle_lines[idle_res_bottom + 1].trim().is_empty(),
            "Exactly 1 blank row required between Result box and keys bar at {}x{} (idle)",
            w,
            h
        );

        // --- 2. Waiting-for-verdict state checks (with long result text) ---
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
            log_slice: Some("[spawn-at-time] +0.00ms | CLOAK".into()),
        };
        wait_app
            .results
            .insert("placement/bottom-right".into(), wait_res);

        let wait_backend = TestBackend::new(w, h);
        let mut wait_term = Terminal::new(wait_backend).unwrap();
        wait_term.draw(|f| draw_ui(f, &wait_app)).unwrap();
        let wait_rendered = dump_buffer_to_string(&wait_term, w, h);
        let wait_lines: Vec<&str> = wait_rendered.lines().collect();

        // Check stacked box spacing:
        if w < 75 {
            // Narrow layout (60x28): Tests / GAP / Description / GAP / Result / GAP / keys bar / 1 blank bottom row
            let tests_top = wait_lines
                .iter()
                .position(|l| l.contains("┌ Tests"))
                .expect("Tests top border");
            let tests_bottom = wait_lines[tests_top..]
                .iter()
                .position(|l| l.contains('└'))
                .map(|i| tests_top + i)
                .expect("Tests bottom border");

            // Exactly 1 blank row between Tests and Description
            assert!(
                wait_lines[tests_bottom + 1].trim().is_empty(),
                "Exactly 1 blank row required between Tests and Description at {}x{}",
                w,
                h
            );
            assert!(
                wait_lines[tests_bottom + 2].contains("┌ Description"),
                "Description top border must follow 1 blank row after Tests at {}x{}",
                w,
                h
            );

            let desc_top = tests_bottom + 2;
            let desc_bottom = wait_lines[desc_top..]
                .iter()
                .position(|l| l.contains('└'))
                .map(|i| desc_top + i)
                .expect("Description bottom border");

            // Exactly 1 blank row between Description and Result
            assert!(
                wait_lines[desc_bottom + 1].trim().is_empty(),
                "Exactly 1 blank row required between Description and Result at {}x{}",
                w,
                h
            );
            assert!(
                wait_lines[desc_bottom + 2].contains("┌ Result"),
                "Result top border must follow 1 blank row after Description at {}x{}",
                w,
                h
            );

            let res_top = desc_bottom + 2;
            let res_bottom = wait_lines[res_top..]
                .iter()
                .position(|l| l.contains('└'))
                .map(|i| res_top + i)
                .expect("Result bottom border");

            // Exactly 1 blank row between Result and verdict keys bar
            assert!(
                wait_lines[res_bottom + 1].trim().is_empty(),
                "Exactly 1 blank row required between Result and keys bar at {}x{}",
                w,
                h
            );
            assert!(
                wait_lines[res_bottom + 2].contains('┌')
                    || wait_lines[res_bottom + 2].contains("verdict"),
                "Verdict keys bar must follow 1 blank row after Result at {}x{}",
                w,
                h
            );
        } else {
            // Wide layout (80x28, 140x28): left column = Tests box / 1 blank row GAP / Result box pinned to bottom
            let tests_top = wait_lines
                .iter()
                .position(|l| l.contains("┌ Tests"))
                .expect("Tests top border");
            let tests_bottom = wait_lines[tests_top..]
                .iter()
                .position(|l| l.contains('└'))
                .map(|i| tests_top + i)
                .expect("Tests bottom border");

            // Row directly following Tests bottom border in left column must be blank (GAP)
            let left_w = (w as usize * if w >= 110 { 52 } else { 50 }) / 100;
            let gap_line_left: String = wait_lines[tests_bottom + 1].chars().take(left_w).collect();
            assert!(
                gap_line_left.trim().is_empty(),
                "Left column must have exactly 1 blank row gap between Tests and Result at {}x{}",
                w,
                h
            );

            // Row tests_bottom + 2 has Result top border
            assert!(
                wait_lines[tests_bottom + 2].contains("┌ Result"),
                "Result top border must directly follow 1 blank row after Tests at {}x{}",
                w,
                h
            );

            let res_top = tests_bottom + 2;
            let res_bottom = wait_lines[res_top..]
                .iter()
                .position(|l| {
                    let left_part: String = l.chars().take(left_w).collect();
                    left_part.contains('└')
                })
                .map(|i| res_top + i)
                .expect("Result bottom border");

            // Exactly 1 blank row between Result and verdict keys bar across the screen
            assert!(
                wait_lines[res_bottom + 1].trim().is_empty(),
                "Exactly 1 blank row required between Result and keys bar at {}x{}",
                w,
                h
            );
        }

        // --- 3. Result text must WRAP, never clip ---
        // The watch text is: "Did the window appear once in the bottom-right corner with a small gap from screen edges, without flickering or jumping?"
        // Must contain "corner" (no mid-word truncation to "corn")
        assert!(
            wait_rendered.contains("corner"),
            "Result text must be fully visible and wrap without clipping 'corner' at {}x{}",
            w,
            h
        );
        assert!(
            wait_rendered.contains("flickering"),
            "Result text must wrap without clipping 'flickering' at {}x{}",
            w,
            h
        );
        assert!(
            wait_rendered.contains("jumping?"),
            "Result text must wrap without clipping 'jumping?' at {}x{}",
            w,
            h
        );
        assert!(
            !wait_rendered.contains("bottom-right corn "),
            "Result text must not be cut mid-word 'corn' at {}x{}",
            w,
            h
        );

        // --- 4. Tests box shrinks (with a more-hint) when Result is made taller ---
        let idle_tests_top = idle_lines
            .iter()
            .position(|l| l.contains("┌ Tests"))
            .unwrap();
        let idle_tests_bottom = idle_lines[idle_tests_top..]
            .iter()
            .position(|l| l.contains('└'))
            .unwrap()
            + idle_tests_top;
        let idle_tests_h = idle_tests_bottom - idle_tests_top + 1;

        let wait_tests_top = wait_lines
            .iter()
            .position(|l| l.contains("┌ Tests"))
            .unwrap();
        let wait_tests_bottom = wait_lines[wait_tests_top..]
            .iter()
            .position(|l| l.contains('└'))
            .unwrap()
            + wait_tests_top;
        let wait_tests_h = wait_tests_bottom - wait_tests_top + 1;

        assert!(
            wait_tests_h < idle_tests_h,
            "Tests box height must shrink when Result is taller at {}x{}: wait_h={} vs idle_h={}",
            w,
            h,
            wait_tests_h,
            idle_tests_h
        );

        // At 60x28 and 80x28, when Tests box shrinks, more-hint must appear
        if w <= 80 {
            assert!(
                wait_rendered.contains("more"),
                "Tests box must show more-hint when it shrinks at {}x{}",
                w,
                h
            );
        }
    }

    // At 140x28: verify that if Result grows taller (e.g. multi-line failure message), Tests box shrinks and shows more-hint
    let mut tall_app = create_test_app();
    tall_app.fixture_exists = true;
    let long_fail_msg = "Detailed assertion failure:\nline 1: unexpected coordinates\nline 2: opacity out of bounds\nline 3: frame violation\nline 4: bounding box mismatch\nline 5: cloak window error\nline 6: trace violation detected";
    let tall_res = spawn_at_verify::runner::TestResult {
        id: "placement/bottom-right".into(),
        title: "Window placed in bottom-right corner".into(),
        kind: TestKind::Both,
        required: true,
        auto_status: AutoStatus::AutoFail,
        human_verdict: HumanVerdict::default(),
        final_status: FinalStatus::Fail,
        disagreement: false,
        duration_ms: 210,
        exit_code: Some(1),
        stdout: String::new(),
        stderr: String::new(),
        message: Some(long_fail_msg.into()),
        log_slice: None,
    };
    tall_app
        .results
        .insert("placement/bottom-right".into(), tall_res);
    if let Some(pos) = tall_app
        .filtered_tests()
        .iter()
        .position(|t| t.id == "placement/bottom-right")
    {
        tall_app.selected_index = pos;
    }
    let tall_backend = TestBackend::new(140, 28);
    let mut tall_term = Terminal::new(tall_backend).unwrap();
    tall_term.draw(|f| draw_ui(f, &tall_app)).unwrap();
    let tall_rendered = dump_buffer_to_string(&tall_term, 140, 28);
    assert!(
        tall_rendered.contains("more"),
        "Tests box must shrink and show more-hint when Result is made taller at 140x28"
    );
}

#[test]
fn test_render_60x20_long_description_and_waiting_drawer_flow() {
    let mut app = create_test_app();
    app.fixture_exists = true;

    if let Some(t) = app.registry.tests.first_mut() {
        t.description = "Line 1 of long test description\nLine 2 with extensive details about placement\nLine 3 describing boundaries and margins\nLine 4 detailing expectations and behaviors\nLine 5 verifying behavior across resolutions\nLine 6 more text to ensure clipping at small terminal heights".into();
    }

    let backend = TestBackend::new(60, 20);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| draw_ui(f, &app)).unwrap();
    let rendered = dump_buffer_to_string(&terminal, 60, 20);

    // 1. Clip hint must name the key: "d to expand"
    assert!(
        rendered.contains("d to expand"),
        "Clip hint at 60x20 must contain 'd to expand', got:\n{}",
        rendered
    );

    // 2. Set waiting for human verdict
    let test_id = app.selected_test().unwrap().id.clone();
    app.waiting_for_human = Some(test_id.clone());

    // Press 'd' while waiting for verdict
    app.handle_key(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE));
    assert!(
        app.detail_drawer_open,
        "Pressing 'd' while waiting for verdict must open drawer"
    );
    assert_eq!(
        app.waiting_for_human,
        Some(test_id.clone()),
        "Waiting state must be preserved"
    );

    // Esc closes drawer and returns to verdict bar WITHOUT cancelling the test
    app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert!(!app.detail_drawer_open, "Esc must close detail drawer");
    assert_eq!(
        app.waiting_for_human,
        Some(test_id),
        "Case must still be waiting after closing drawer"
    );

    // Re-render and verify verdict bar is showing
    terminal.draw(|f| draw_ui(f, &app)).unwrap();
    let after_esc = dump_buffer_to_string(&terminal, 60, 20);
    assert!(
        after_esc.contains("Your verdict:"),
        "Verdict bar must be showing after Esc closes drawer"
    );
}

#[test]
fn test_drawer_bar_chips_and_exclusion_of_main_keys() {
    let mut app = create_test_app();
    app.fixture_exists = true;
    app.detail_drawer_open = true;

    let backend = TestBackend::new(80, 28);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| draw_ui(f, &app)).unwrap();
    let buf = terminal.backend().buffer().clone();

    // Drawer footer bar is rows 22..28 in 80x28
    let mut bar_text = String::new();
    for y in 22..28 {
        for x in 0..80 {
            bar_text.push_str(buf[(x, y)].symbol());
        }
    }

    // Must contain drawer keys
    assert!(bar_text.contains("Copy"), "Drawer bar must contain Copy");
    assert!(bar_text.contains("Save"), "Drawer bar must contain Save");
    assert!(
        bar_text.contains("Scroll"),
        "Drawer bar must contain Scroll"
    );
    assert!(bar_text.contains("Close"), "Drawer bar must contain Close");
    assert!(
        bar_text.contains("Raw/Compact"),
        "Drawer bar must contain Raw/Compact"
    );
    assert!(bar_text.contains("Help"), "Drawer bar must contain Help");

    // Must NEVER show main-screen keys while drawer is open
    assert!(
        !bar_text.contains("Filter"),
        "Drawer bar must NOT contain Filter"
    );
    assert!(
        !bar_text.contains("Fold"),
        "Drawer bar must NOT contain Fold"
    );
    assert!(!bar_text.contains("Run"), "Drawer bar must NOT contain Run");
    assert!(!bar_text.contains("All"), "Drawer bar must NOT contain All");
    assert!(
        !bar_text.contains("Panel"),
        "Drawer bar must NOT contain Panel"
    );
}

#[test]
fn test_drawer_sections_order_and_formatting() {
    let app = create_test_app();
    let test = app.selected_test().unwrap().clone();

    let (_lines, plain) = spawn_at_verify::tui::views::build_drawer_content(&app, &test, 80, true);

    // Sections must appear in exact order: RESULT, GEOMETRY, TIMELINE, WHAT TO LOOK FOR, SPEC, OUTPUT
    let pos_result = plain.find("RESULT").expect("RESULT section missing");
    let pos_geometry = plain.find("GEOMETRY").expect("GEOMETRY section missing");
    let pos_timeline = plain.find("TIMELINE").expect("TIMELINE section missing");
    let pos_what = plain
        .find("WHAT TO LOOK FOR")
        .expect("WHAT TO LOOK FOR section missing");
    let pos_spec = plain.find("SPEC").expect("SPEC section missing");
    let pos_output = plain.find("OUTPUT").expect("OUTPUT section missing");

    assert!(pos_result < pos_geometry, "RESULT must precede GEOMETRY");
    assert!(
        pos_geometry < pos_timeline,
        "GEOMETRY must precede TIMELINE"
    );
    assert!(
        pos_timeline < pos_what,
        "TIMELINE must precede WHAT TO LOOK FOR"
    );
    assert!(pos_what < pos_spec, "WHAT TO LOOK FOR must precede SPEC");
    assert!(pos_spec < pos_output, "SPEC must precede OUTPUT");

    // No "---" and no numbering like "1. Name"
    assert!(
        !plain.contains("---"),
        "Drawer must not contain '---' headings"
    );
    assert!(
        !plain.contains("1. "),
        "Drawer must not contain numbered headings '1. '"
    );
    assert!(
        !plain.contains("2. "),
        "Drawer must not contain numbered headings '2. '"
    );
    assert!(
        !plain.contains("3. "),
        "Drawer must not contain numbered headings '3. '"
    );
}

#[test]
fn test_stdout_window_id_appears_exactly_once_in_compact_drawer() {
    let mut app = create_test_app();
    let test = app.selected_test().unwrap().clone();

    let unique_win_id = 9876543210u64;
    let res = spawn_at_verify::runner::TestResult {
        id: test.id.clone(),
        title: test.title.clone(),
        kind: test.kind,
        required: test.required,
        auto_status: AutoStatus::AutoPass,
        human_verdict: HumanVerdict::default(),
        final_status: FinalStatus::Pass,
        disagreement: false,
        duration_ms: 35,
        exit_code: Some(0),
        stdout: format!(
            r#"{{"window_id":{},"x":1504,"y":764,"w":400,"h":300,"rel_x":1504,"rel_y":724,"area":"workarea","source":"claim"}}"#,
            unique_win_id
        ),
        stderr: "[spawn-at] INFO: Window mapped at final size (400x300)\n".into(),
        message: None,
        log_slice: None,
    };
    app.results.insert(test.id.clone(), res);

    let (_lines, plain) = spawn_at_verify::tui::views::build_drawer_content(&app, &test, 80, true);

    let id_str = unique_win_id.to_string();
    let count = plain.matches(&id_str).count();
    assert_eq!(
        count, 1,
        "The stdout window_id value '{}' must appear exactly once in the compact drawer, found {}:\n{}",
        id_str, count, plain
    );
}

#[test]
fn test_compact_timeline_contains_neither_prefix_nor_proto() {
    let mut app = create_test_app();
    let test = app.selected_test().unwrap().clone();

    let trace_log = "\
[spawn-at-time] +0.0ms | ARM_SPAWN win=[id=870906018 pid=1083136 class=\"verify.placement.bottomright\" app=\"verify.placement.bottomright\" type=normal proto=wayland] target=verify.placement.bottomright
[spawn-at-time] +10.0ms | WINDOW_CREATED win=[id=870906018 pid=1083136 class=\"verify.placement.bottomright\" app=\"verify.placement.bottomright\" type=normal proto=wayland] actor found
[spawn-at-time] +56.1ms | ACTOR_MAP win=[id=870906018 pid=1083136 class=\"verify.placement.bottomright\" app=\"verify.placement.bottomright\" type=normal proto=wayland]
[spawn-at-time] +56.2ms | CLOAK win=[id=870906018 pid=1083136 class=\"verify.placement.bottomright\" app=\"verify.placement.bottomright\" type=normal proto=wayland] opacity=0 frame=(50,90,400x300)
[spawn-at-time] +56.3ms | SET_SIZE_REQUEST win=[id=870906018 pid=1083136 class=\"verify.placement.bottomright\" app=\"verify.placement.bottomright\" type=normal proto=wayland] requested=400x300 clamped=400x300
[spawn-at-time] +56.4ms | COMMIT_RESOLVED win=[id=870906018 pid=1083136 class=\"verify.placement.bottomright\" app=\"verify.placement.bottomright\" type=normal proto=wayland] 400x300
";

    let res = spawn_at_verify::runner::TestResult {
        id: test.id.clone(),
        title: test.title.clone(),
        kind: test.kind,
        required: test.required,
        auto_status: AutoStatus::AutoPass,
        human_verdict: HumanVerdict::default(),
        final_status: FinalStatus::Pass,
        disagreement: false,
        duration_ms: 60,
        exit_code: Some(0),
        stdout: r#"{"window_id":870906018,"pid":1083136}"#.into(),
        stderr: String::new(),
        message: None,
        log_slice: Some(trace_log.into()),
    };
    app.results.insert(test.id.clone(), res);

    let (_lines, plain) = spawn_at_verify::tui::views::build_drawer_content(&app, &test, 80, true);

    // Find the TIMELINE section portion of plain text
    let timeline_idx = plain.find("TIMELINE").expect("TIMELINE section exists");
    let next_section_idx = plain
        .find("WHAT TO LOOK FOR")
        .expect("WHAT TO LOOK FOR section exists");
    let timeline_section = &plain[timeline_idx..next_section_idx];

    assert!(
        !timeline_section.contains("[spawn-at-time]"),
        "Compact timeline must not contain '[spawn-at-time]', found in:\n{}",
        timeline_section
    );
    assert!(
        !timeline_section.contains("proto="),
        "Compact timeline must not contain 'proto=', found in:\n{}",
        timeline_section
    );
}

#[test]
fn test_label_values_alignment_across_section() {
    let mut app = create_test_app();
    let test = app.selected_test().unwrap().clone();

    let res = spawn_at_verify::runner::TestResult {
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
        duration_ms: 36,
        exit_code: Some(0),
        stdout: r#"{"x":1504,"y":764,"w":400,"h":300,"source":"claim"}"#.into(),
        stderr: String::new(),
        message: None,
        log_slice: None,
    };
    app.results.insert(test.id.clone(), res);

    let (_lines, plain) = spawn_at_verify::tui::views::build_drawer_content(&app, &test, 80, true);

    // In RESULT section:
    // "  Verdict       ✔ Pass"
    // "  Cloak         36 ms..."
    // "  Trace         ✔ clean..."
    // Inner padding 2, label column 14 wide -> values start at index 16 (0-based)
    let mut verified = 0;
    for line in plain.lines() {
        if line.starts_with("  Verdict ")
            || line.starts_with("  Cloak ")
            || line.starts_with("  Trace ")
            || line.starts_with("  Budget ")
        {
            assert!(line.starts_with("  "), "KV line must start with 2 spaces");
            let val_char = line.chars().nth(16).unwrap_or(' ');
            assert_ne!(
                val_char, ' ',
                "Value in line '{}' must start at column 16",
                line
            );
            verified += 1;
        }
    }
    assert!(
        verified >= 3,
        "Must have verified alignment on at least 3 rows"
    );
}

#[test]
fn test_drawer_w_writes_full_drawer_and_y_copies_to_clipboard() {
    let mut app = create_test_app();
    let test = app.selected_test().unwrap().clone();

    let temp_dir =
        std::env::temp_dir().join(format!("spawn-at-verify-test-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&temp_dir);
    app.report_dir = temp_dir.clone();
    app.detail_drawer_open = true;

    let (_lines, expected_plain) =
        spawn_at_verify::tui::views::build_drawer_content(&app, &test, 80, true);

    // 1. Test 'w': writes file whose content equals full drawer text
    let res = app.save_drawer_text();
    assert!(res.is_ok(), "save_drawer_text must succeed: {:?}", res);
    let path = res.unwrap();
    assert!(
        path.exists(),
        "Saved log file must exist at {}",
        path.display()
    );
    let saved_content = std::fs::read_to_string(&path).expect("read saved file");
    assert_eq!(
        saved_content, expected_plain,
        "Saved file content must equal full drawer text"
    );
    let title = app.get_drawer_title(&test.id);
    assert!(
        title.contains(&path.display().to_string()),
        "Drawer title must show path, got: {}",
        title
    );

    // 2. Test 'y': hands the same text to clipboard helper via injected fake clipboard
    use std::sync::{Arc, Mutex};
    let clipboard_capture = Arc::new(Mutex::new(String::new()));
    let capture_clone = Arc::clone(&clipboard_capture);
    app.clipboard_fn = Some(Arc::new(move |text: &str| {
        *capture_clone.lock().unwrap() = text.to_string();
        Ok(())
    }));

    app.copy_drawer_text();
    let captured = clipboard_capture.lock().unwrap().clone();
    assert_eq!(
        captured, expected_plain,
        "Clipboard helper must receive exact full drawer text"
    );
    let title_copied = app.get_drawer_title(&test.id);
    assert!(
        title_copied.contains("Copied"),
        "Drawer title must indicate copy success, got: {}",
        title_copied
    );

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_question_mark_from_drawer_opens_help_and_esc_returns() {
    let mut app = create_test_app();
    app.fixture_exists = true;
    app.detail_drawer_open = true;

    // Press '?' from drawer
    app.handle_key(KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE));
    assert_eq!(
        app.modal,
        ModalState::Help,
        "'?' from drawer must open help modal"
    );

    let backend = TestBackend::new(80, 25);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| draw_ui(f, &app)).unwrap();
    let rendered = dump_buffer_to_string(&terminal, 80, 25);

    // Drawer keys first, then main keys
    assert!(rendered.contains("DRAWER KEYS & COMMANDS"));
    assert!(rendered.contains("MAIN KEYS & COMMANDS"));
    let pos_drawer = rendered.find("DRAWER KEYS & COMMANDS").unwrap();
    let pos_main = rendered.find("MAIN KEYS & COMMANDS").unwrap();
    assert!(
        pos_drawer < pos_main,
        "Help modal from drawer must show DRAWER KEYS before MAIN KEYS"
    );

    // Esc closes help and returns to drawer
    app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert_eq!(app.modal, ModalState::None, "Esc must close help modal");
    assert!(
        app.detail_drawer_open,
        "Esc from help must return to drawer (drawer remains open)"
    );

    // Second Esc closes drawer
    app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert!(!app.detail_drawer_open, "Second Esc must close drawer");
}

#[test]
fn test_no_bare_uppercase_key_bindings_in_bars_or_help() {
    let mut app = create_test_app();
    app.fixture_exists = true;

    let backend = TestBackend::new(80, 28);
    let mut terminal = Terminal::new(backend).unwrap();

    // 1. Normal main screen
    terminal.draw(|f| draw_ui(f, &app)).unwrap();
    let main_rendered = dump_buffer_to_string(&terminal, 80, 28);

    // 2. Verdict mode
    app.waiting_for_human = Some("placement/bottom-right".into());
    terminal.draw(|f| draw_ui(f, &app)).unwrap();
    let verdict_rendered = dump_buffer_to_string(&terminal, 80, 28);
    app.waiting_for_human = None;

    // 3. Drawer mode
    app.detail_drawer_open = true;
    terminal.draw(|f| draw_ui(f, &app)).unwrap();
    let drawer_rendered = dump_buffer_to_string(&terminal, 80, 28);

    // 4. Help overlay (from drawer)
    app.modal = ModalState::Help;
    terminal.draw(|f| draw_ui(f, &app)).unwrap();
    let help_drawer_rendered = dump_buffer_to_string(&terminal, 80, 28);

    // 5. Help overlay (from main)
    app.detail_drawer_open = false;
    terminal.draw(|f| draw_ui(f, &app)).unwrap();
    let help_main_rendered = dump_buffer_to_string(&terminal, 80, 28);

    let all_screens = [
        ("main", main_rendered),
        ("verdict", verdict_rendered),
        ("drawer", drawer_rendered),
        ("help_drawer", help_drawer_rendered),
        ("help_main", help_main_rendered),
    ];

    // Regex checking for bare uppercase bindings inside brackets like [R], [A], [C], [P], [X], [S], [K]
    // The ONLY allowed uppercase binding is "Shift+G"
    let re_bracket_bare = regex::Regex::new(r#"\[([A-FH-Z])\]"#).unwrap();
    for (name, screen) in &all_screens {
        if let Some(cap) = re_bracket_bare.captures_iter(screen).next() {
            panic!(
                "Screen '{}' contains forbidden bare uppercase binding [{}]:\n{}",
                name, &cap[1], screen
            );
        }
        assert!(
            !screen.contains("Shift+C"),
            "Screen '{}' should not have Shift+C (use lowercase c)",
            name
        );
    }
}

#[test]
fn test_render_drawer_at_60x28_80x28_140x28_no_clipped_headings() {
    for &w in &[60, 80, 140] {
        let mut app = create_test_app();
        app.fixture_exists = true;
        app.detail_drawer_open = true;

        let backend = TestBackend::new(w, 28);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| draw_ui(f, &app)).unwrap();
        let rendered = dump_buffer_to_string(&terminal, w, 28);

        // Heading lines: RESULT, GEOMETRY, TIMELINE must appear without corruption
        assert!(
            rendered.contains("RESULT"),
            "Heading RESULT must not be clipped at width {}",
            w
        );
        assert!(
            rendered.contains("GEOMETRY"),
            "Heading GEOMETRY must not be clipped at width {}",
            w
        );
        if w > 80 {
            assert!(
                rendered.contains("TIMELINE"),
                "Heading TIMELINE must not be clipped at width {}",
                w
            );
        } else {
            app.detail_scroll = 5;
            terminal.draw(|f| draw_ui(f, &app)).unwrap();
            let scrolled_rendered = dump_buffer_to_string(&terminal, w, 28);
            assert!(
                scrolled_rendered.contains("TIMELINE"),
                "Heading TIMELINE must not be clipped at width {}",
                w
            );
        }
    }
}

#[test]
fn test_multisize_drawer_rendering_and_token_wrapping() {
    let trace_log_12 = "\
[spawn-at-time] +0.0ms | ARM_SPAWN win=[id=870906018 pid=1083136 class=\"verify.placement.bottomright\" app=\"verify.placement.bottomright\" type=normal proto=wayland] target=verify.placement.bottomright
[spawn-at-time] +10.0ms | WINDOW_CREATED win=[id=870906018 pid=1083136 class=\"verify.placement.bottomright\" app=\"verify.placement.bottomright\" type=normal proto=wayland] actor found
[spawn-at-time] +56.1ms | ACTOR_MAP win=[id=870906018 pid=1083136 class=\"verify.placement.bottomright\" app=\"verify.placement.bottomright\" type=normal proto=wayland]
[spawn-at-time] +56.2ms | CLOAK win=[id=870906018 pid=1083136 class=\"verify.placement.bottomright\" app=\"verify.placement.bottomright\" type=normal proto=wayland] opacity=0 frame=(50,90,400x300)
[spawn-at-time] +56.3ms | SET_SIZE_REQUEST win=[id=870906018 pid=1083136 class=\"verify.placement.bottomright\" app=\"verify.placement.bottomright\" type=normal proto=wayland] requested=400x300 clamped=400x300
[spawn-at-time] +56.4ms | COMMIT_RESOLVED win=[id=870906018 pid=1083136 class=\"verify.placement.bottomright\" app=\"verify.placement.bottomright\" type=normal proto=wayland] 400x300
[spawn-at-time] +60.0ms | BUFFER_ATTACH win=[id=870906018 pid=1083136 class=\"verify.placement.bottomright\" app=\"verify.placement.bottomright\" type=normal proto=wayland]
[spawn-at-time] +65.0ms | FIRST_FRAME win=[id=870906018 pid=1083136 class=\"verify.placement.bottomright\" app=\"verify.placement.bottomright\" type=normal proto=wayland]
[spawn-at-time] +70.0ms | UNCLOAK win=[id=870906018 pid=1083136 class=\"verify.placement.bottomright\" app=\"verify.placement.bottomright\" type=normal proto=wayland] opacity=255
[spawn-at-time] +75.0ms | AT_TARGET win=[id=870906018 pid=1083136 class=\"verify.placement.bottomright\" app=\"verify.placement.bottomright\" type=normal proto=wayland]
[spawn-at-time] +80.0ms | FOCUS_IN win=[id=870906018 pid=1083136 class=\"verify.placement.bottomright\" app=\"verify.placement.bottomright\" type=normal proto=wayland]
[spawn-at-time] +85.0ms | RESTORE win=[id=870906018 pid=1083136 class=\"verify.placement.bottomright\" app=\"verify.placement.bottomright\" type=normal proto=wayland]";

    for &w in &[50, 60, 70, 80, 100, 140] {
        for &_h in &[20, 28] {
            let mut app = create_test_app();
            let mut test = app.selected_test().unwrap().clone();
            test.description = "Verifies that the window is placed strictly in the bottom-right corner of the primary display workarea with zero visual displacement or clipping artifact on initial map.".into();

            let res = spawn_at_verify::runner::TestResult {
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
                duration_ms: 85,
                exit_code: Some(0),
                stdout: r#"{"window_id":870906018,"x":1504,"y":764,"w":400,"h":300,"rel_x":1504,"rel_y":724,"area":"workarea","source":"claim"}"#.into(),
                stderr: "[spawn-at] INFO: Window mapped at final size (400x300)\n".into(),
                message: None,
                log_slice: Some(trace_log_12.into()),
            };
            app.results.insert(test.id.clone(), res);

            let (_lines, plain) =
                spawn_at_verify::tui::views::build_drawer_content(&app, &test, w as usize, true);

            // Assert tokens are intact on lines where they fit
            if w >= 70 {
                assert!(
                    plain.contains("verify.placement.bottomright"),
                    "ARM_SPAWN target token intact at width {}",
                    w
                );
            } else {
                assert!(
                    plain.contains("verify.placement."),
                    "ARM_SPAWN target token broken cleanly at width {}",
                    w
                );
            }
            if w >= 60 {
                assert!(
                    plain.contains("AT_TARGET") || plain.contains("at target"),
                    "AT_TARGET intact at width {}",
                    w
                );
                assert!(
                    plain.contains("50,90 400×300"),
                    "Frame rect token intact at width {}",
                    w
                );
            }
            assert!(
                plain.contains("400×300"),
                "Geometry 400×300 token intact at width {}",
                w
            );

            // Check that header continuation rows start with 16 spaces (hanging indent)
            let timeline_pos = plain.find("── TIMELINE").unwrap_or(plain.len());
            let header_part = &plain[..timeline_pos];
            for line in header_part.lines() {
                if line.starts_with("                ") {
                    let first_char = line.chars().nth(16).unwrap_or(' ');
                    assert_ne!(
                        first_char, ' ',
                        "Continuation line must start content at column 16 at width {}: '{}'",
                        w, line
                    );
                }
            }
        }
    }
}

#[test]
fn test_synthetic_token_hard_wrap_alignment_at_width_50() {
    let app = create_test_app();
    let mut test = app.selected_test().unwrap().clone();
    test.description = "A".repeat(60); // 60-char synthetic unbroken token

    let (_lines, plain) = spawn_at_verify::tui::views::build_drawer_content(&app, &test, 50, true);
    let desc_lines: Vec<&str> = plain.lines().filter(|l| l.contains("AAAA")).collect();
    assert!(
        desc_lines.len() >= 2,
        "60-char token must wrap into at least 2 lines at width 50"
    );
    // Continuation line must have exactly 16 spaces of indent
    let second_line = desc_lines[1];
    assert!(
        second_line.starts_with("                "),
        "Continuation row must have 16 leading spaces at width 50"
    );
    let c = second_line.chars().nth(16).unwrap_or(' ');
    assert_ne!(c, ' ', "Continuation row must start content at column 16");
}

#[test]
fn test_506_9_ms_cloak_yellow_warning_does_not_fail_pass() {
    let mut app = create_test_app();
    let test = app.selected_test().unwrap().clone();

    let res = spawn_at_verify::runner::TestResult {
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
        duration_ms: 507,
        exit_code: Some(0),
        stdout: r#"{"window_id":870906018,"x":1504,"y":764,"w":400,"h":300}"#.into(),
        stderr: String::new(),
        message: Some("Assertions passed".into()),
        log_slice: Some(
            "[spawn-at-time] +0.0ms | CLOAK\n[spawn-at-time] +506.9ms | UNCLOAK".into(),
        ),
    };
    app.results.insert(test.id.clone(), res);

    let (lines, plain) = spawn_at_verify::tui::views::build_drawer_content(&app, &test, 80, true);
    assert!(plain.contains("506.9 ms"));
    assert!(plain.contains("over provisional budget"));
    assert!(!plain.contains("Failure:"));

    // Check styling: yellow '!', no red cell in RESULT section
    let mut found_yellow_warning = false;
    let mut found_red_in_result = false;
    let mut in_result = false;
    for line in &lines {
        let line_text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        if line_text.contains("RESULT") {
            in_result = true;
        } else if line_text.contains("GEOMETRY") {
            in_result = false;
        }
        if in_result {
            for span in &line.spans {
                if span.content.contains("!") && span.style.fg == Some(Color::Yellow) {
                    found_yellow_warning = true;
                }
                if span.style.fg == Some(Color::Red) {
                    found_red_in_result = true;
                }
            }
        }
    }
    assert!(
        found_yellow_warning,
        "Must render yellow '!' for 506.9 ms cloak budget warning"
    );
    assert!(
        !found_red_in_result,
        "Passing case with 506.9 ms cloak must have zero red cells in RESULT"
    );
}

#[test]
fn test_failing_case_renders_failure_row_with_real_reason_in_red() {
    let mut app = create_test_app();
    let test = app.selected_test().unwrap().clone();

    let res = spawn_at_verify::runner::TestResult {
        id: test.id.clone(),
        title: test.title.clone(),
        kind: test.kind,
        required: test.required,
        auto_status: AutoStatus::AutoFail,
        human_verdict: HumanVerdict::default(),
        final_status: FinalStatus::Fail,
        disagreement: false,
        duration_ms: 45,
        exit_code: Some(1),
        stdout: r#"{"window_id":870906018,"x":1200,"y":600,"w":400,"h":300}"#.into(),
        stderr: "[spawn-at] ERROR: Geometric target mismatch".into(),
        message: Some("Window mapped at (1200, 600) but expected (1504, 764)".into()),
        log_slice: None,
    };
    app.results.insert(test.id.clone(), res);

    let (lines, plain) = spawn_at_verify::tui::views::build_drawer_content(&app, &test, 80, true);
    assert!(plain.contains("Failure"));
    assert!(plain.contains("Window mapped at (1200, 600) but expected (1504, 764)"));

    let mut found_red_failure = false;
    for line in &lines {
        for span in &line.spans {
            if span.content.contains("Window mapped at") && span.style.fg == Some(Color::Red) {
                found_red_failure = true;
            }
        }
    }
    assert!(
        found_red_failure,
        "Failure row message must be styled in Red"
    );
}

#[test]
fn test_drawer_bar_chips_at_140_and_80() {
    use spawn_at_verify::tui::views::{render_key_grid, DRAWER_KEYS};

    // State 1 at width 140 (inner width 138)
    let lines_140 = render_key_grid(DRAWER_KEYS, 138);
    assert_eq!(
        lines_140.len(),
        1,
        "Drawer bar at 140 wide must be exactly 1 line"
    );
    let text_140: String = lines_140[0]
        .spans
        .iter()
        .map(|s| s.content.as_ref())
        .collect();
    assert_eq!(
        text_140,
        " [y] Copy   [w] Save   [t] Raw/Compact   [j/k] Scroll   [?] Help   [Esc] Close"
    );

    // State 2 at width 80 (inner width 78)
    let lines_80 = render_key_grid(DRAWER_KEYS, 78);
    assert_eq!(
        lines_80.len(),
        2,
        "Drawer bar at 80 wide must be 3 columns x 2 rows"
    );
    let row0: String = lines_80[0]
        .spans
        .iter()
        .map(|s| s.content.as_ref())
        .collect();
    let row1: String = lines_80[1]
        .spans
        .iter()
        .map(|s| s.content.as_ref())
        .collect();

    assert!(row0.contains("[ y ]"));
    assert!(row0.contains("[ w ]"));
    assert!(row0.contains("[ t ]"));
    assert!(row1.contains("[j/k]"));
    assert!(row1.contains("[ ? ]"));
    assert!(row1.contains("[Esc]"));
}

#[test]
fn test_drawer_scrolling_keys_in_waiting_state() {
    let mut app = create_test_app();
    let test_id = app.selected_test().unwrap().id.clone();
    app.waiting_for_human = Some(test_id);
    app.detail_drawer_open = true;
    app.detail_scroll = 0;

    // j scrolls down
    app.handle_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE));
    assert_eq!(app.detail_scroll, 1);

    // Down arrow scrolls down
    app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    assert_eq!(app.detail_scroll, 2);

    // k scrolls up
    app.handle_key(KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE));
    assert_eq!(app.detail_scroll, 1);

    // Up arrow scrolls up
    app.handle_key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE));
    assert_eq!(app.detail_scroll, 0);

    // PgDn scrolls down 8
    app.handle_key(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE));
    assert_eq!(app.detail_scroll, 8);

    // PgUp scrolls up 8
    app.handle_key(KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE));
    assert_eq!(app.detail_scroll, 0);

    // Shift+G scrolls to bottom (u16::MAX)
    app.handle_key(KeyEvent::new(KeyCode::Char('G'), KeyModifiers::SHIFT));
    assert_eq!(app.detail_scroll, u16::MAX);

    // g scrolls to top (0)
    app.handle_key(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE));
    assert_eq!(app.detail_scroll, 0);
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

fn create_app_with_real_fixture() -> (TuiApp, spawn_at_verify::schema::TestDefinition) {
    let mut app = create_test_app();
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
        let inner_width = w.saturating_sub(2);

        // 1. Grid Invariant: Geometry table alignment
        // Header Expected starts at x=16; Expected values start at x=16; Anchor/Margin/Tolerance start at x=16
        // Actual starts at x=32; Result starts at x=48; ✔ is centered under Result (offset 3)
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
                    Some(16),
                    "Geometry header 'Expected' must start at x=16 at width {}",
                    w
                );
                assert_eq!(
                    line.find("Actual"),
                    Some(32),
                    "Geometry header 'Actual' must start at x=32 at width {}",
                    w
                );
                assert_eq!(
                    line.find("Result"),
                    Some(48),
                    "Geometry header 'Result' must start at x=48 at width {}",
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
                    line.chars().nth(16).unwrap_or(' '),
                    ' ',
                    "Position Expected value must start at x=16 at width {}",
                    w
                );
                assert_ne!(
                    line.chars().nth(32).unwrap_or(' '),
                    ' ',
                    "Position Actual value must start at x=32 at width {}",
                    w
                );
                // Centered under Result: 48 + 3 = 51
                assert_eq!(
                    line.chars().position(|c| c == '✔'),
                    Some(51),
                    "Position ✔ must be centered at x=51 under Result at width {}",
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
                    line.chars().nth(16).unwrap_or(' '),
                    ' ',
                    "Size Expected value must start at x=16 at width {}",
                    w
                );
                assert_ne!(
                    line.chars().nth(32).unwrap_or(' '),
                    ' ',
                    "Size Actual value must start at x=32 at width {}",
                    w
                );
                assert_eq!(
                    line.chars().position(|c| c == '✔'),
                    Some(51),
                    "Size ✔ must be centered at x=51 under Result at width {}",
                    w
                );
            }
            if line.starts_with("  Anchor")
                || line.starts_with("  Margin")
                || line.starts_with("  Tolerance")
            {
                assert_ne!(
                    line.chars().nth(16).unwrap_or(' '),
                    ' ',
                    "Geometry input value must start at x=16 at width {}: '{}'",
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

        // 2. Grid Invariant: every key/value row in RESULT, GEOMETRY inputs, header, WHAT TO LOOK FOR, SPEC and OUTPUT has value at x == 16
        for line in plain.lines() {
            let prefixes = [
                "  Title",
                "  ID",
                "  Mode",
                "  Verdict",
                "  Cloak",
                "  Budget",
                "  Trace",
                "  Anchor",
                "  Margin",
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
                    let ch = line.chars().nth(16).unwrap_or(' ');
                    assert_ne!(
                        ch, ' ',
                        "Row '{}' value must start at column 16 at width {}",
                        line, w
                    );
                }
            }
        }

        // 3. Grid Invariant: every timeline detail value across the whole trace starts at one single x; gap >= 2
        let timeline_idx = plain.find("── TIMELINE").expect("TIMELINE section exists");
        let next_section_idx = plain
            .find("── WHAT TO LOOK FOR")
            .expect("WHAT TO LOOK FOR section exists");
        let timeline_part = &plain[timeline_idx..next_section_idx];

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
                line_w <= inner_width - 1,
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
        for l in plain
            .lines()
            .skip(mode_idx + 1)
            .take(desc_idx - (mode_idx + 1))
        {
            assert!(
                !l.is_empty(),
                "No blank rows between Mode and Description at width {}",
                w
            );
        }

        // 8. Grid Invariant: no raw key=value, no raw camelCase, no unclosed parens
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
