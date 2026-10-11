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
    // In 80x24, right panel content overflows and MUST show the clipping hint
    assert!(
        text.contains("▼ more"),
        "Buffer at 80x24 must show clipping hint when content overflows"
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
    assert!(idle_snapshot.contains("r Run  a All"));

    assert!(waiting_snapshot.contains("1 waiting"));
    assert!(waiting_snapshot.contains("Your verdict:"));
    assert!(waiting_snapshot.contains("p Pass"));

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

                // 3. No row text is clipped without "…"
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
        cat_headings_found > 0,
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
        assert!(
            rendered.contains("Kind"),
            "Kind must be visible at narrow width ({} cols)",
            width
        );
        // The description and "What to look for" are visible at narrow width (scrollable with a hint if long)
        if is_waiting || height >= 24 {
            assert!(
                rendered.contains("What to look for"),
                "What to look for must be visible at narrow width ({} cols, height {})",
                width,
                height
            );
        } else {
            // When height is very small (e.g. 60x20) and description is long, shows "▼ more" hint
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
    scroll_app.handle_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE));
    scroll_app.handle_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE));
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
    assert!(help_rendered.contains("KINDS LEGEND"));
    assert!(help_rendered.contains("auto:"));
    assert!(help_rendered.contains("human:"));
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
    let resolutions = [(60, 20), (60, 28), (70, 28), (80, 28), (100, 28), (140, 28)];

    let mut out = String::new();

    for &(w, h) in &resolutions {
        // Idle
        let mut app = create_test_app();
        app.fixture_exists = true;
        let backend = TestBackend::new(w, h);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| draw_ui(f, &app)).unwrap();
        let idle_rendered = dump_buffer_to_string(&terminal, w, h);
        out.push_str(&format!(
            "=== SNAPSHOT {}x{} - IDLE ===\n{}\n\n",
            w, h, idle_rendered
        ));

        // Waiting for verdict
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
        out.push_str(&format!(
            "=== SNAPSHOT {}x{} - WAITING FOR VERDICT ===\n{}\n\n",
            w, h, wait_rendered
        ));
    }

    let scratch_path = "/home/harsh/.gemini/antigravity-ide/brain/c19dfab5-8922-4446-88cf-50dca990ff83/scratch/snapshots.txt";
    let _ = std::fs::write(scratch_path, out);
}
