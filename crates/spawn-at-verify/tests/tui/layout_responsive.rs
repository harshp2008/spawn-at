//! Responsive layout and breakpoint rendering integration tests.

use super::{create_test_app, dump_buffer_to_string, validate_render_spec};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{backend::TestBackend, Terminal};
use spawn_at_verify::runner::{AutoStatus, FinalStatus, HumanVerdict, HumanVerdictKind};
use spawn_at_verify::schema::TestKind;
use spawn_at_verify::tui::views::draw_ui;
use std::path::PathBuf;

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
        trace_note: None,
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
        trace_note: None,
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
        trace_note: None,
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
            trace_note: None,
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
            trace_note: None,
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
