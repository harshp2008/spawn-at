//! Input handling for the event loop: read everything already pending, apply it in order with
//! scroll bursts merged and accelerated, then draw once.
//!
//! Why: the loop used to read one event and draw once per event. A terminal turns a mouse wheel or
//! a touchpad flick into Up/Down key presses (the TUI does not capture the mouse), dozens per
//! second; every one cost a full draw, so the view fell behind and kept scrolling after the
//! fingers stopped. Now a frame consumes the whole backlog (capped, see [`MAX_EVENTS_PER_FRAME`]
//! and [`FRAME_BUDGET`]), so when input stops the view stops in the same frame.
//!
//! All the tuning constants are in this file.

use super::app::TuiApp;
use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// Most events read in one frame; the rest are read in the next one.
pub const MAX_EVENTS_PER_FRAME: usize = 512;

/// Longest a frame spends reading events before it draws.
pub const FRAME_BUDGET: Duration = Duration::from_millis(8);

/// Window over which the rate of scroll events is measured.
pub const ACCEL_WINDOW: Duration = Duration::from_millis(100);

/// Scroll events in [`ACCEL_WINDOW`] that still move one row each (a held key repeats about 3
/// times per window; one wheel notch is about 3 events). Above this the step grows.
pub const ACCEL_FREE_EVENTS: usize = 6;

/// One extra row per event for this many events above [`ACCEL_FREE_EVENTS`] in the window.
pub const ACCEL_EVENTS_PER_ROW: usize = 3;

/// Largest step, in rows per event.
pub const ACCEL_MAX_STEP: usize = 6;

/// Rows one scroll event moves when `rate` scroll events fall inside the window: 1 up to
/// [`ACCEL_FREE_EVENTS`], then growing, capped at [`ACCEL_MAX_STEP`].
pub fn accel_step(rate: usize) -> usize {
    (1 + rate.saturating_sub(ACCEL_FREE_EVENTS) / ACCEL_EVENTS_PER_ROW).min(ACCEL_MAX_STEP)
}

/// Where events come from (the terminal in the app, a script in tests).
pub trait EventSource {
    /// True when an event is available within `timeout`.
    fn poll(&mut self, timeout: Duration) -> Result<bool, String>;
    /// The next event; only called after `poll` returned true.
    fn read(&mut self) -> Result<Event, String>;
}

/// The terminal.
pub struct TerminalEvents;

impl EventSource for TerminalEvents {
    fn poll(&mut self, timeout: Duration) -> Result<bool, String> {
        crossterm::event::poll(timeout).map_err(|e| e.to_string())
    }
    fn read(&mut self) -> Result<Event, String> {
        crossterm::event::read().map_err(|e| e.to_string())
    }
}

/// Waits up to `wait` for the first event, then takes every event that is already pending (a zero
/// timeout poll) up to the per-frame caps. Order is the order of arrival.
pub fn read_frame(source: &mut dyn EventSource, wait: Duration) -> Result<Vec<Event>, String> {
    let mut out = Vec::new();
    if !source.poll(wait)? {
        return Ok(out);
    }
    let started = Instant::now();
    loop {
        out.push(source.read()?);
        if out.len() >= MAX_EVENTS_PER_FRAME
            || started.elapsed() >= FRAME_BUDGET
            || !source.poll(Duration::ZERO)?
        {
            break;
        }
    }
    Ok(out)
}

/// Recent scroll events, for the acceleration.
#[derive(Debug, Default)]
pub struct Accel {
    recent: VecDeque<Instant>,
}

impl Accel {
    /// Notes a scroll event at `now` and returns how many rows it moves.
    pub fn next_step(&mut self, now: Instant) -> usize {
        self.recent.push_back(now);
        while let Some(&t) = self.recent.front() {
            if now.saturating_duration_since(t) > ACCEL_WINDOW {
                self.recent.pop_front();
            } else {
                break;
            }
        }
        accel_step(self.recent.len())
    }
}

/// The direction of a plain Up/Down key press (the form a wheel notch arrives in), if it is one.
fn scroll_dir(ev: &Event) -> Option<KeyCode> {
    match ev {
        Event::Key(KeyEvent {
            code: code @ (KeyCode::Up | KeyCode::Down),
            modifiers,
            kind: KeyEventKind::Press | KeyEventKind::Repeat,
            ..
        }) if *modifiers == KeyModifiers::NONE => Some(*code),
        _ => None,
    }
}

/// Applies `events` in order. A run of the same scroll direction is one scroll: each event moves
/// [`accel_step`] rows for the rate at that moment, and the run is applied as that many key
/// presses of the direction (so every view clamps and behaves exactly as for one key). Anything
/// else (another key, a resize) ends the run and is applied where it stands.
pub fn apply_events(app: &mut TuiApp, events: Vec<Event>, accel: &mut Accel, now: Instant) {
    for ev in events {
        match scroll_dir(&ev) {
            Some(code) => {
                let step = accel.next_step(now);
                for _ in 0..step {
                    app.handle_key(KeyEvent::new(code, KeyModifiers::NONE));
                }
            }
            None => {
                if let Event::Key(key) = ev {
                    app.handle_key(key);
                }
            }
        }
    }
}
