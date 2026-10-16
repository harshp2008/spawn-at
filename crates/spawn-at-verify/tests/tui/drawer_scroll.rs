//! The details drawer of a long spec (the real transform timeline: 16 event blocks) scrolled far
//! past its end, with the keys the TUI handles (there is no mouse capture): the offset must be
//! clamped to what the content allows, the content must still be drawn, scrolling back must work,
//! Escape must close the drawer, and a resize while scrolled must clamp again.

use super::action_trace_render::{result_with, trace_of, TRANSFORM};
use super::{create_test_app, dump_buffer_to_string};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{backend::TestBackend, Terminal};
use spawn_at_verify::tui::app::TuiApp;
use spawn_at_verify::tui::views::draw_ui;
use spawn_at_verify::tui::views::drawer::drawer_content_rows;

fn app_with_long_drawer() -> TuiApp {
    let mut app = create_test_app();
    app.fixture_exists = true;
    let res = result_with(
        &app,
        vec![trace_of(TRANSFORM, "transform", "w", 3210662173, 0)],
    );
    let test = app.selected_test().unwrap().clone();
    app.results.insert(test.id.clone(), res);
    app.detail_drawer_open = true;
    app
}

fn key(app: &mut TuiApp, code: KeyCode) {
    app.handle_key(KeyEvent::new(code, KeyModifiers::NONE));
}

fn draw(app: &mut TuiApp, w: u16, h: u16) -> String {
    let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
    t.draw(|f| draw_ui(f, app)).unwrap();
    dump_buffer_to_string(&t, w, h)
}

/// The text of the drawer's rows other than blank ones and borders.
fn visible_content(screen: &str) -> usize {
    screen
        .lines()
        .filter(|l| l.chars().any(|c| c.is_alphanumeric()))
        .count()
}

/// `G` (End) and a held PageDown / Down far past the end: the offset stays within the content.
#[test]
fn test_scroll_offset_is_clamped_after_end_and_held_keys() {
    for (w, h) in [(120u16, 30u16), (100, 24), (80, 10), (30, 8)] {
        let mut app = app_with_long_drawer();
        draw(&mut app, w, h);
        key(&mut app, KeyCode::Char('G'));
        for _ in 0..500 {
            key(&mut app, KeyCode::PageDown);
            key(&mut app, KeyCode::Down);
        }
        let screen = draw(&mut app, w, h);
        let max = app.detail_scroll_max();
        assert!(
            app.detail_scroll <= max,
            "{w}x{h}: {} > {max}",
            app.detail_scroll
        );
        assert!(
            visible_content(&screen) > 1,
            "{w}x{h}: the drawer is blank after scrolling past the end:\n{screen}"
        );
    }
}

/// Scrolling back up from past the end moves at once (the offset was not left far beyond the
/// content), Home returns to the top, and Escape closes the drawer with the main view working.
#[test]
fn test_scroll_back_up_escape_and_main_view_work_after_overscroll() {
    let mut app = app_with_long_drawer();
    draw(&mut app, 100, 24);
    key(&mut app, KeyCode::Char('G'));
    for _ in 0..50 {
        key(&mut app, KeyCode::PageDown);
    }
    draw(&mut app, 100, 24);
    let at_end = app.detail_scroll;
    key(&mut app, KeyCode::Up);
    assert!(app.detail_scroll < at_end, "one Up must move: {at_end}");
    key(&mut app, KeyCode::Home);
    assert_eq!(app.detail_scroll, 0);
    key(&mut app, KeyCode::Char('G'));
    key(&mut app, KeyCode::Esc);
    assert!(!app.detail_drawer_open, "Escape closes the drawer");
    let screen = draw(&mut app, 100, 24);
    assert!(screen.contains(" Tests "), "{screen}");
    let before = app.selected_index;
    key(&mut app, KeyCode::Down);
    assert_ne!(app.selected_index, before, "the list answers keys again");
}

/// Escape and q work with the offset at the maximum value of its type, in the Help modal too.
#[test]
fn test_escape_works_from_any_scroll_state() {
    let mut app = app_with_long_drawer();
    app.detail_scroll = u16::MAX;
    key(&mut app, KeyCode::Esc);
    assert!(!app.detail_drawer_open);
    app.detail_drawer_open = true;
    app.detail_scroll = u16::MAX;
    key(&mut app, KeyCode::Char('?'));
    key(&mut app, KeyCode::Char('G'));
    key(&mut app, KeyCode::Esc);
    key(&mut app, KeyCode::Esc);
    assert!(!app.detail_drawer_open);
}

/// A resize while scrolled to the end: the next draw at the new size clamps the offset, and the
/// content is drawn.
#[test]
fn test_resize_while_scrolled_clamps_again() {
    let mut app = app_with_long_drawer();
    draw(&mut app, 100, 16);
    key(&mut app, KeyCode::Char('G'));
    draw(&mut app, 100, 16);
    let small_max = app.detail_scroll_max();
    let screen = draw(&mut app, 140, 60);
    // The stored offset may still be the old one; the draw clamped it, and one Up moves from the
    // clamped position, not from the stale one.
    let new_max = app.detail_scroll_max();
    key(&mut app, KeyCode::Up);
    assert_eq!(app.detail_scroll, new_max.saturating_sub(1));
    assert!(app.detail_scroll_max() <= small_max);
    assert!(visible_content(&screen) > 5, "{screen}");
}

/// The maximum is the content's rows minus the drawer's viewport, with the rows counted the way
/// the drawer draws them (wrapped to its inner width, blank rows between events included): at the
/// maximum the last content row is on screen and the viewport is full, at one less it is too, and
/// the viewport height is read from the drawn frame, not assumed.
#[test]
fn test_content_height_matches_the_rendered_row_count() {
    for (w, h) in [(100u16, 24u16), (140, 40), (70, 30), (60, 16)] {
        let mut app = app_with_long_drawer();
        let screen = draw(&mut app, w, h);
        let rows: Vec<&str> = screen.lines().collect();
        let top = rows
            .iter()
            .position(|l| l.contains("Details"))
            .unwrap_or_else(|| panic!("drawer title in:\n{screen}"));
        let bottom = rows[top + 1..]
            .iter()
            .position(|l| l.contains('└'))
            .map(|i| i + top + 1)
            .expect("drawer bottom border");
        let viewport = bottom - top - 1;
        let test = app.selected_test().unwrap().clone();
        let total = drawer_content_rows(&app, &test, w as usize);
        assert_eq!(
            app.detail_scroll_max() as usize,
            total.saturating_sub(viewport),
            "{w}x{h}: total {total}, viewport {viewport}"
        );
        key(&mut app, KeyCode::Char('G'));
        let end = draw(&mut app, w, h);
        let last_non_blank = end
            .lines()
            .skip(top + 1)
            .take(viewport)
            .filter(|l| l.contains('│') && l.chars().filter(|c| c.is_alphanumeric()).count() > 0)
            .count();
        assert!(
            last_non_blank > 0,
            "{w}x{h}: the end of the content is not drawn:\n{end}"
        );
        assert!(
            end.contains("ANCHOR_RELEASED") || end.contains("SNAPSHOT_DESTROYED"),
            "{w}x{h}:\n{end}"
        );
    }
}

/// The details pane of the main view (not the drawer) shares the offset: End and held PageDown
/// there cannot overflow the draw either.
#[test]
fn test_details_pane_survives_end_and_held_page_down() {
    let mut app = create_test_app();
    app.fixture_exists = true;
    app.active_pane = spawn_at_verify::tui::app::ActivePane::Details;
    draw(&mut app, 100, 24);
    key(&mut app, KeyCode::Char('G'));
    for _ in 0..300 {
        key(&mut app, KeyCode::PageDown);
    }
    let screen = draw(&mut app, 100, 24);
    assert!(app.detail_scroll <= app.detail_scroll_max());
    assert!(screen.contains(" Tests "), "{screen}");
    key(&mut app, KeyCode::Home);
    assert_eq!(app.detail_scroll, 0);
}

/// The draw clamps what it reads, whatever the stored offset is (a stale one after a resize, or
/// the largest value the type holds): it must neither panic nor draw a blank drawer.
#[test]
fn test_draw_alone_clamps_a_huge_stored_offset() {
    for (w, h) in [(100u16, 24u16), (60, 16), (140, 50)] {
        let mut app = app_with_long_drawer();
        app.detail_scroll = u16::MAX;
        let screen = draw(&mut app, w, h);
        assert!(visible_content(&screen) > 3, "{w}x{h}:\n{screen}");
        app.detail_drawer_open = false;
        app.active_pane = spawn_at_verify::tui::app::ActivePane::Details;
        app.detail_scroll = u16::MAX;
        let main = draw(&mut app, w, h);
        assert!(visible_content(&main) > 3, "{w}x{h}:\n{main}");
    }
}

const MARKER: &str = "── end of details ──";

/// The drawer box's inner rows (between its top and bottom borders) of a drawn frame.
fn inner_rows(screen: &str) -> Vec<String> {
    let rows: Vec<&str> = screen.lines().collect();
    let top = rows
        .iter()
        .position(|l| l.contains("Details"))
        .expect("drawer title");
    let bottom = rows[top + 1..]
        .iter()
        .position(|l| l.contains('└'))
        .map(|i| i + top + 1)
        .expect("drawer bottom border");
    rows[top + 1..bottom]
        .iter()
        .map(|l| l.trim_matches(|c| c == '│' || c == ' ').to_string())
        .collect()
}

fn title_row(screen: &str) -> String {
    screen
        .lines()
        .find(|l| l.contains("Details"))
        .unwrap()
        .to_string()
}

/// At the end of the scroll the last rows drawn are a blank row and the marker, the marker is
/// fully visible, and the arrows say there is content above and none below.
#[test]
fn test_end_of_scroll_shows_blank_row_then_marker() {
    let mut app = app_with_long_drawer();
    draw(&mut app, 100, 24);
    key(&mut app, KeyCode::Char('G'));
    let screen = draw(&mut app, 100, 24);
    let rows = inner_rows(&screen);
    assert_eq!(rows.last().unwrap(), MARKER, "{screen}");
    assert_eq!(
        rows[rows.len() - 2],
        "",
        "blank row above the marker:\n{screen}"
    );
    let t = title_row(&screen);
    assert!(t.contains("▲ more") && !t.contains("▼ more"), "{t}");
}

/// Mid-scroll the marker is not visible and `▼ more` shows (with `▲ more`); at the top only
/// `▼ more` shows.
#[test]
fn test_arrows_follow_the_position() {
    let mut app = app_with_long_drawer();
    let top = draw(&mut app, 100, 24);
    let t = title_row(&top);
    assert!(!t.contains("▲ more") && t.contains("▼ more"), "{t}");
    assert!(!top.contains(MARKER));
    key(&mut app, KeyCode::PageDown);
    let mid = draw(&mut app, 100, 24);
    let t = title_row(&mid);
    assert!(t.contains("▲ more") && t.contains("▼ more"), "{t}");
    assert!(!mid.contains(MARKER), "{mid}");
}

/// Content that fits: neither arrow, and the marker is still there.
#[test]
fn test_content_that_fits_shows_the_marker_and_no_arrows() {
    let mut app = app_with_long_drawer();
    let screen = draw(&mut app, 100, 120);
    let t = title_row(&screen);
    assert!(!t.contains("more"), "{t}");
    assert!(screen.contains(MARKER), "{screen}");
}

/// The marker and the arrows are display only: copied text and saved files are the content as
/// built, without them.
#[test]
fn test_copy_and_save_do_not_contain_the_marker_or_arrows() {
    use std::sync::{Arc, Mutex};
    let mut app = app_with_long_drawer();
    let dir = std::env::temp_dir().join(format!("spawn-at-verify-marker-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    app.report_dir = dir.clone();
    draw(&mut app, 100, 24);
    let got = Arc::new(Mutex::new(String::new()));
    let g = Arc::clone(&got);
    app.clipboard_fn = Some(Arc::new(move |t: &str| {
        *g.lock().unwrap() = t.to_string();
        Ok(())
    }));
    app.copy_drawer_text();
    let copied = got.lock().unwrap().clone();
    let saved = std::fs::read_to_string(app.save_drawer_text().unwrap()).unwrap();
    for text in [&copied, &saved] {
        assert!(text.contains("TIMELINE"), "the content is there");
        assert!(!text.contains(MARKER) && !text.contains("more"), "{text}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}
