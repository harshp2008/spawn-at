//! Scrolling work, counted (draws, content rebuilds, rows moved), never timed: a burst of queued
//! wheel events (they reach the TUI as Up/Down key presses) is read as one frame, applied in order
//! with the acceleration, and drawn once; the drawer's content is rebuilt only when something it
//! shows changed.
//!
//! Before this change the loop handled one event and drew once per event: 100 queued events were
//! 100 draws and 100 full content rebuilds at both 200x50 and 80x24 (measured with the loop as it
//! was, on the real transform timeline).

use super::action_trace_render::{result_with, trace_of, TRANSFORM};
use super::create_test_app;
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::{backend::TestBackend, Terminal};
use spawn_at_verify::tui::app::TuiApp;
use spawn_at_verify::tui::events::{
    accel_step, apply_events, read_frame, Accel, EventSource, ACCEL_MAX_STEP,
};
use spawn_at_verify::tui::views::draw_ui;
use spawn_at_verify::tui::views::drawer::content_builds;
use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// Events handed out as if they were already pending in the terminal.
struct Fake(VecDeque<Event>);

impl EventSource for Fake {
    fn poll(&mut self, _t: Duration) -> Result<bool, String> {
        Ok(!self.0.is_empty())
    }
    fn read(&mut self) -> Result<Event, String> {
        Ok(self.0.pop_front().unwrap())
    }
}

fn k(code: KeyCode) -> Event {
    Event::Key(KeyEvent::new(code, KeyModifiers::NONE))
}

fn rig(w: u16, h: u16) -> (TuiApp, Terminal<TestBackend>) {
    let mut app = create_test_app();
    app.fixture_exists = true;
    let res = result_with(
        &app,
        vec![trace_of(TRANSFORM, "transform", "w", 3210662173, 0)],
    );
    let test = app.selected_test().unwrap().clone();
    app.results.insert(test.id.clone(), res);
    app.detail_drawer_open = true;
    let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
    t.draw(|f| draw_ui(f, &app)).unwrap();
    (app, t)
}

fn builds() -> usize {
    content_builds()
}

/// What the offset ends at when every event of `n` at one instant moves `accel_step(rate)` rows:
/// the rate is the count of events so far, since none has left the window.
fn expected_offset(n: usize, max: u16) -> u16 {
    let mut off = 0usize;
    for i in 1..=n {
        off = (off + accel_step(i)).min(max as usize);
    }
    off as u16
}

/// 1000 queued wheel events: a handful of frames at most (the per-frame cap and time budget split
/// them), one content build in total, and the final offset is the clamped sum of the accelerated
/// steps. The view does not move after the last event is consumed.
#[test]
fn test_a_thousand_queued_events_are_a_few_draws_and_stop_when_consumed() {
    for (w, h) in [(200u16, 50u16), (80, 24)] {
        let (mut app, mut t) = rig(w, h);
        let max = app.detail_scroll_max();
        assert!(
            max > 0,
            "{w}x{h}: the timeline must be longer than the viewport"
        );
        let b0 = builds();
        let mut src = Fake((0..1000).map(|_| k(KeyCode::Down)).collect());
        let mut accel = Accel::default();
        let now = Instant::now();
        let mut draws = 0;
        loop {
            let frame = read_frame(&mut src, Duration::from_millis(1)).unwrap();
            if frame.is_empty() {
                break;
            }
            apply_events(&mut app, frame, &mut accel, now);
            t.draw(|f| draw_ui(f, &app)).unwrap();
            draws += 1;
        }
        println!(
            "{w}x{h}: {draws} draws and {} content builds for 1000 queued events",
            builds() - b0
        );
        assert!(
            (1..=8).contains(&draws),
            "{w}x{h}: {draws} draws for 1000 events"
        );
        assert_eq!(
            builds() - b0,
            0,
            "{w}x{h}: scrolling alone rebuilt the content"
        );
        assert_eq!(app.detail_scroll, expected_offset(1000, max), "{w}x{h}");
        // Input has stopped: more frames change nothing.
        let at = app.detail_scroll;
        for _ in 0..3 {
            assert!(read_frame(&mut src, Duration::from_millis(1))
                .unwrap()
                .is_empty());
            t.draw(|f| draw_ui(f, &app)).unwrap();
        }
        assert_eq!(app.detail_scroll, at, "no inertia");
    }
}

/// A single slow notch moves exactly what it does today: one key, one row; three events of one
/// notch, three rows; events spaced beyond the window never accelerate.
#[test]
fn test_one_slow_notch_moves_the_base_number_of_rows() {
    let (mut app, _t) = rig(120, 30);
    let mut accel = Accel::default();
    let now = Instant::now();
    apply_events(&mut app, vec![k(KeyCode::Down)], &mut accel, now);
    assert_eq!(app.detail_scroll, 1);
    apply_events(
        &mut app,
        vec![k(KeyCode::Down), k(KeyCode::Down), k(KeyCode::Down)],
        &mut accel,
        now + Duration::from_millis(500),
    );
    assert_eq!(app.detail_scroll, 4);
    let mut slow = Accel::default();
    for i in 0..5u64 {
        let before = app.detail_scroll;
        apply_events(
            &mut app,
            vec![k(KeyCode::Down)],
            &mut slow,
            now + Duration::from_millis(1000 * (i + 1)),
        );
        assert_eq!(app.detail_scroll, before + 1);
    }
    assert_eq!(accel_step(0), 1);
    assert_eq!(accel_step(6), 1);
    assert!(accel_step(60) == ACCEL_MAX_STEP && accel_step(10_000) == ACCEL_MAX_STEP);
}

/// A key between two bursts keeps its place: Home in the middle resets the offset, so only the
/// last burst remains; and a resize inside a burst changes nothing about the order.
#[test]
fn test_mixed_events_keep_their_order() {
    let (mut app, _t) = rig(120, 30);
    let mut accel = Accel::default();
    let now = Instant::now();
    let mut ev: Vec<Event> = (0..5).map(|_| k(KeyCode::Down)).collect();
    ev.push(k(KeyCode::Home));
    ev.extend((0..3).map(|_| k(KeyCode::Down)));
    apply_events(&mut app, ev, &mut accel, now);
    assert_eq!(
        app.detail_scroll, 3,
        "Home in the middle was applied in the middle"
    );

    let (mut app, _t) = rig(120, 30);
    let mut accel = Accel::default();
    let mut ev: Vec<Event> = (0..4).map(|_| k(KeyCode::Down)).collect();
    ev.push(Event::Resize(100, 20));
    ev.extend((0..4).map(|_| k(KeyCode::Down)));
    apply_events(&mut app, ev, &mut accel, now);
    assert_eq!(
        app.detail_scroll, 8,
        "a resize inside the burst does not reorder or drop"
    );
}

/// Far past the end and far past the top: the stored offset stays in range.
#[test]
fn test_wheel_past_the_ends_is_clamped() {
    let (mut app, mut t) = rig(120, 30);
    let max = app.detail_scroll_max();
    let mut accel = Accel::default();
    let now = Instant::now();
    apply_events(
        &mut app,
        (0..3000).map(|_| k(KeyCode::Down)).collect(),
        &mut accel,
        now,
    );
    t.draw(|f| draw_ui(f, &app)).unwrap();
    assert_eq!(app.detail_scroll, max);
    apply_events(
        &mut app,
        (0..3000).map(|_| k(KeyCode::Up)).collect(),
        &mut accel,
        now,
    );
    assert_eq!(app.detail_scroll, 0);
}

/// Escape still closes the drawer when it follows a burst in the same frame.
#[test]
fn test_escape_after_a_burst_closes_the_drawer() {
    let (mut app, _t) = rig(120, 30);
    let mut accel = Accel::default();
    let mut ev: Vec<Event> = (0..200).map(|_| k(KeyCode::Down)).collect();
    ev.push(k(KeyCode::Char('G')));
    ev.push(k(KeyCode::Esc));
    apply_events(&mut app, ev, &mut accel, Instant::now());
    assert!(!app.detail_drawer_open);
}

/// The content is not rebuilt for a scroll-only change, and is rebuilt on a resize, on the
/// raw/compact toggle, and for a new result.
#[test]
fn test_content_cache_rebuilds_only_when_the_content_changed() {
    let (mut app, mut t) = rig(120, 40);
    let b = builds();
    for _ in 0..20 {
        app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        t.draw(|f| draw_ui(f, &app)).unwrap();
    }
    assert_eq!(builds(), b, "scrolling rebuilt the content");

    t.backend_mut().resize(100, 40);
    t.draw(|f| draw_ui(f, &app)).unwrap();
    assert_eq!(builds(), b + 1, "a new width rebuilds once");
    t.draw(|f| draw_ui(f, &app)).unwrap();
    assert_eq!(builds(), b + 1);

    app.drawer_raw_view = true;
    t.draw(|f| draw_ui(f, &app)).unwrap();
    assert_eq!(builds(), b + 2, "raw view rebuilds");
    app.drawer_raw_view = false;
    t.draw(|f| draw_ui(f, &app)).unwrap();
    assert_eq!(builds(), b + 3, "compact view rebuilds");

    let test = app.selected_test().unwrap().clone();
    let mut res = result_with(
        &app,
        vec![trace_of(TRANSFORM, "transform", "w", 3210662173, 0)],
    );
    res.duration_ms += 1;
    app.results.insert(test.id.clone(), res);
    t.draw(|f| draw_ui(f, &app)).unwrap();
    assert_eq!(builds(), b + 4, "a new result rebuilds");

    app.invalidate_drawer_cache();
    t.draw(|f| draw_ui(f, &app)).unwrap();
    assert_eq!(builds(), b + 5, "explicit invalidation rebuilds");
}
