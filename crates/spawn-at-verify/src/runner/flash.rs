//! Flash check of one run: where the fixture writes its events, how long the run watches after the
//! reveal, and how the extension's log, the fixture's events and the clock anchor become a
//! [`FlashInfo`]. The comparison itself is [`crate::trace::flash::detect_flash`].

use super::engine::TestRunner;
use super::models::AutoStatus;
use crate::ext_logging::LoggingControl;
use crate::schema::TestDefinition;
use crate::trace::flash::{
    detect_flash, echo_times_ms, monotonic_ns, parse_client_events, ClockAnchor, FlashInfo,
    FlashInputs, FlashParams, MonoBracket, ANCHOR_SAMPLES, POST_REVEAL_WATCH_MS,
};
use crate::trace::wire::parse_wire_paints;
use crate::trace::{TraceAnalyser, TraceEvent};
use std::os::unix::fs::DirBuilderExt;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, SystemTime};

/// Environment variable naming the directory the fixture writes `fixture-<pid>.events` into.
pub const EVENT_DIR_ENV: &str = "SPAWN_AT_FIXTURE_EVENT_DIR";

/// Environment variable that makes the fixture write `fixture-<pid>.log` with the Wayland protocol
/// dump (`WAYLAND_DEBUG=client`) into the named directory; set unless `run --no-wire-paint`.
pub const TRACE_DIR_ENV: &str = "SPAWN_AT_FIXTURE_TRACE_DIR";

/// Longest the run waits for the reveal to show up in the log before it gives up and judges
/// without a watch window (the log is written from an idle callback, a little after the event).
const REVEAL_POLL_LIMIT: usize = 10;

/// Pause between two looks for the reveal.
const REVEAL_POLL_INTERVAL: Duration = Duration::from_millis(100);

/// Margin added to the watch window so an event 1500 ms after the reveal is read, not raced.
const WATCH_MARGIN: Duration = Duration::from_millis(100);

/// Counter that makes each run's event directory name unique within the process.
static DIR_SEQ: AtomicUsize = AtomicUsize::new(0);

/// What one run needs to compare the fixture's clock with the extension's: a private directory
/// the fixture writes its events into, and the bracketed `SetLogging` calls made before the action.
/// The directory is removed when this is dropped.
pub struct FlashCapture {
    /// Directory named by [`EVENT_DIR_ENV`] (and [`TRACE_DIR_ENV`] with `wire`) for this run.
    dir: PathBuf,
    /// Monotonic brackets of the anchor calls, oldest first.
    pub brackets: Vec<MonoBracket>,
    /// Whether the fixture also writes the protocol dump into `dir`.
    wire: bool,
}

impl FlashCapture {
    /// Makes the event directory and sends [`ANCHOR_SAMPLES`] bracketed `SetLogging(true)` calls.
    /// `None` when the logging control cannot bracket calls (no GNOME extension) or the directory
    /// cannot be created: the run is then judged on the extension's log alone. `wire` asks the
    /// fixture for the protocol dump too (on unless `run --no-wire-paint`).
    pub fn start(logging: &dyn LoggingControl, wire: bool) -> Option<FlashCapture> {
        let brackets = logging.anchor_brackets(ANCHOR_SAMPLES);
        if brackets.is_empty() {
            return None;
        }
        let dir = std::env::temp_dir().join(format!(
            "spawn-at-verify-flash-{}-{}",
            std::process::id(),
            DIR_SEQ.fetch_add(1, Ordering::SeqCst)
        ));
        std::fs::DirBuilder::new().mode(0o700).create(&dir).ok()?;
        Some(FlashCapture {
            dir,
            brackets,
            wire,
        })
    }

    /// Makes the spawned command (and the fixture it starts) write its events into this run's directory.
    pub fn apply(&self, cmd: &mut Command) {
        cmd.env(EVENT_DIR_ENV, &self.dir);
        if self.wire {
            cmd.env(TRACE_DIR_ENV, &self.dir);
        }
    }

    /// The text of the fixture's trace file `fixture-<pid>.log` (its own lines and the protocol
    /// dump) when the dump was asked for and the file exists, else nothing. Unlike the event
    /// file there is no fallback to "the only file": a run without a known pid is not guessed.
    pub fn read_wire(&self, pid: Option<u64>) -> String {
        match (self.wire, pid) {
            (true, Some(pid)) => {
                std::fs::read_to_string(self.dir.join(format!("fixture-{}.log", pid)))
                    .unwrap_or_default()
            }
            _ => String::new(),
        }
    }

    /// The text of the fixture's event file: `fixture-<pid>.events` for `pid` when it exists,
    /// else the only event file in the directory, else nothing.
    pub fn read_events(&self, pid: Option<u64>) -> String {
        if let Some(pid) = pid {
            if let Ok(text) =
                std::fs::read_to_string(self.dir.join(format!("fixture-{}.events", pid)))
            {
                return text;
            }
        }
        let files: Vec<PathBuf> = std::fs::read_dir(&self.dir)
            .map(|rd| rd.filter_map(|e| e.ok().map(|e| e.path())).collect())
            .unwrap_or_default();
        match files.as_slice() {
            [only] => std::fs::read_to_string(only).unwrap_or_default(),
            _ => String::new(),
        }
    }
}

impl Drop for FlashCapture {
    /// Removes the event directory with the files in it.
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// Whether `test` spawns the fixture and expects it to start: the runs for which the fixture's
/// paint events exist and the flash check is worth its wait.
pub fn wants_flash_capture(test: &TestDefinition) -> bool {
    test.action.command == "spawn"
        && test.expect.exit_code == 0
        && test.subject.as_ref().map_or(false, |s| s.app == "fixture")
}

/// What a flash does to a run's automatic status and message.
///
/// A flash on a run that passed (plain, geometry unchecked or trace unchecked) keeps the pass
/// and appends `(flash: ...)` to the message; with `strict` the run becomes `AutoFail` with
/// `Flash (--strict-flash): ...` as its message. A run that did not flash, and a run that did not
/// pass, are returned unchanged.
pub fn apply_flash_policy(
    auto_status: AutoStatus,
    message: String,
    flash: &FlashInfo,
    strict: bool,
) -> (AutoStatus, String) {
    if !flash.flash {
        return (auto_status, message);
    }
    let passed = matches!(
        auto_status,
        AutoStatus::AutoPass | AutoStatus::AutoPassUnchecked | AutoStatus::AutoPassTraceUnchecked
    );
    if !passed {
        return (auto_status, message);
    }
    let what = flash.summary();
    if strict {
        (
            AutoStatus::AutoFail,
            format!("Flash (--strict-flash): {}", what),
        )
    } else {
        (auto_status, format!("{} (flash: {})", message, what))
    }
}

/// The extension's events for `window` in `slice`, in log order.
fn window_events(slice: &str, window: u64) -> Vec<TraceEvent> {
    TraceAnalyser::new()
        .with_target_window_id(window)
        .parse_events(slice)
}

/// Process id the extension logged for `window`, which is the fixture's pid.
pub fn window_pid(slice: &str, window: u64) -> Option<u64> {
    window_events(slice, window)
        .iter()
        .find_map(|e| e.meta.as_ref().and_then(|m| m.pid))
}

/// Where the reveal of a window is, as far as the log and the clock anchor tell.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Reveal {
    /// The slice has no `UNCLOAK` for the window (yet).
    NotLogged,
    /// There is an `UNCLOAK` but the brackets and echoes give no clock anchor.
    Unanchored,
    /// Middle of the interval the anchor allows for the first `UNCLOAK`, in `CLOCK_MONOTONIC`
    /// nanoseconds.
    At(f64),
}

/// Locates the first `UNCLOAK` of `window` in `slice` on the monotonic clock.
pub fn reveal_mono_ns(slice: &str, window: u64, brackets: &[MonoBracket]) -> Reveal {
    let events = window_events(slice, window);
    let Some(unc) = events.iter().find(|e| e.tag == "UNCLOAK") else {
        return Reveal::NotLogged;
    };
    match ClockAnchor::from_samples(brackets, &echo_times_ms(slice)) {
        Ok(a) => Reveal::At((a.lo_ns + a.hi_ns) / 2.0 + unc.elapsed_ms * 1e6),
        Err(_) => Reveal::Unanchored,
    }
}

/// Combines the extension's log slice, the fixture's event text, the fixture's protocol-dump text
/// (`None` when the run did not ask for one) and the anchor brackets into the run's [`FlashInfo`].
/// Without a `window` nothing can be attributed and the check is not run. A dump that was asked
/// for but yields no commit leaves the verdict on the fixture's stamps and says so in
/// `flash_notes`.
pub fn evaluate(
    slice: Option<&str>,
    window: Option<u64>,
    client_text: &str,
    wire_text: Option<&str>,
    brackets: &[MonoBracket],
    params: FlashParams,
) -> FlashInfo {
    let (Some(slice), Some(window)) = (slice, window) else {
        return FlashInfo::default();
    };
    let events = window_events(slice, window);
    let client = parse_client_events(client_text);
    let wire = wire_text.map(parse_wire_paints).unwrap_or_default();
    let mut notes = Vec::new();
    if wire_text.is_some() && wire.is_empty() {
        notes.push(
            "no wire paint times (the protocol dump has no commit or no TRACE_START): \
             paint times are the fixture's stamps"
                .to_string(),
        );
    }
    let anchor = if brackets.is_empty() {
        None
    } else {
        match ClockAnchor::from_samples(brackets, &echo_times_ms(slice)) {
            Ok(a) => Some(a),
            Err(e) => {
                notes.push(e);
                None
            }
        }
    };
    let mut info = detect_flash(&FlashInputs {
        trace_events: &events,
        client_events: &client,
        wire_paints: &wire,
        anchor: anchor.as_ref(),
        params,
    });
    notes.append(&mut info.flash_notes);
    info.flash_notes = notes;
    info
}

/// Seconds since the epoch, for slice bounds.
fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

impl TestRunner {
    /// Judges the run for flash. With a `capture` it first waits until [`POST_REVEAL_WATCH_MS`]
    /// have passed since the reveal (polling up to 1 s for the reveal to reach the log), then
    /// takes the log slice again so a correction made during the wait is in it. Returns the
    /// result and the slice it was judged on.
    pub(super) fn run_flash_check(
        &self,
        capture: Option<&FlashCapture>,
        window: Option<u64>,
        start_secs: u64,
        log_slice: Option<String>,
    ) -> (FlashInfo, Option<String>) {
        let Some(window) = window else {
            return (FlashInfo::default(), log_slice);
        };
        let mut slice = log_slice;
        if let Some(cap) = capture {
            let mut polls = 0;
            loop {
                let reveal = slice.as_deref().map_or(Reveal::NotLogged, |s| {
                    reveal_mono_ns(s, window, &cap.brackets)
                });
                match reveal {
                    Reveal::At(reveal_ns) => {
                        let until =
                            reveal_ns + POST_REVEAL_WATCH_MS * 1e6 + WATCH_MARGIN.as_nanos() as f64;
                        let now = monotonic_ns() as f64;
                        if until > now {
                            std::thread::sleep(Duration::from_nanos((until - now) as u64));
                        }
                        break;
                    }
                    // Waiting longer cannot repair a missing anchor.
                    Reveal::Unanchored => break,
                    Reveal::NotLogged => {}
                }
                if polls >= REVEAL_POLL_LIMIT {
                    break;
                }
                polls += 1;
                std::thread::sleep(REVEAL_POLL_INTERVAL);
                slice = self
                    .log_source
                    .capture_slice(start_secs, now_secs())
                    .ok()
                    .or(slice);
            }
            slice = self
                .log_source
                .capture_slice(start_secs, now_secs())
                .ok()
                .or(slice);
        }
        let client_text = match (capture, slice.as_deref()) {
            (Some(cap), Some(s)) => cap.read_events(window_pid(s, window)),
            _ => String::new(),
        };
        let wire_text = match (capture, slice.as_deref()) {
            (Some(cap), Some(s)) if cap.wire => Some(cap.read_wire(window_pid(s, window))),
            _ => None,
        };
        let brackets: &[MonoBracket] = capture.map(|c| c.brackets.as_slice()).unwrap_or(&[]);
        let refresh = crate::display::refresh();
        let params = FlashParams {
            refresh_hz: refresh.hz,
            threshold_ms: self.options.flash_threshold_ms,
        };
        let mut info = evaluate(
            slice.as_deref(),
            Some(window),
            &client_text,
            wire_text.as_deref(),
            brackets,
            params,
        );
        if refresh.assumed && info.flash_basis.is_some() {
            info.flash_notes
                .push(format!("refresh rate not read: {}", refresh.source));
        }
        (info, slice)
    }
}
