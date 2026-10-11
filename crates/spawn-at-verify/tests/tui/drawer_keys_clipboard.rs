//! Drawer keybindings, shortcuts help modal, and clipboard copy/save tests.

use super::{create_test_app, dump_buffer_to_string};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{backend::TestBackend, Terminal};
use spawn_at_verify::tui::app::ModalState;
use spawn_at_verify::tui::views::draw_ui;

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
    // One filename format: the test id with '/' replaced by '-', plus ".log.txt", directly
    // under report_dir. (Written out literally here so a change to the format fails this test.)
    let expected_name = format!("{}.log.txt", test.id.replace('/', "-"));
    assert_eq!(
        path.file_name().unwrap().to_string_lossy(),
        expected_name,
        "Saved file must be named <id with / -> ->.log.txt"
    );
    assert_eq!(
        path.parent().unwrap(),
        temp_dir,
        "Saved file must live directly under report_dir"
    );
    let title = app.get_drawer_title(&test.id);
    assert!(
        title.contains(&path.display().to_string()),
        "Drawer title must show the saved path, got: {}",
        title
    );
    assert!(
        title.contains(&expected_name),
        "Drawer title must show the filename {}, got: {}",
        expected_name,
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

/// The help overlay must advertise the same `.log.txt` extension that `w` actually writes.
#[test]
fn test_help_overlay_names_the_save_file_extension() {
    let mut app = create_test_app();
    app.fixture_exists = true;
    app.detail_drawer_open = true;
    app.handle_key(KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE));

    let backend = TestBackend::new(100, 50);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| draw_ui(f, &app)).unwrap();
    let rendered = dump_buffer_to_string(&terminal, 100, 50);
    assert!(
        rendered.contains("Save text to file (.log.txt)"),
        "Help overlay must name the .log.txt save format:\n{}",
        rendered
    );
}
