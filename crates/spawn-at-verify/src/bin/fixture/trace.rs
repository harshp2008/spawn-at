//! Event files of the fixture: a developer trace and the client-side events the harness reads.
//!
//! `SPAWN_AT_FIXTURE_TRACE_DIR=<dir>` (developer only): the fixture writes `fixture-<pid>.log`
//! there. Each of its own lines carries `CLOCK_MONOTONIC` nanoseconds, the clock the GNOME
//! extension's `+N ms` log counts from. The file also receives the Wayland client protocol dump
//! (`WAYLAND_DEBUG=client`, stderr is redirected into it), so configure, ack_configure, frame
//! callbacks and commits are timestamped by libwayland itself.
//!
//! `SPAWN_AT_FIXTURE_EVENT_DIR=<dir>` (set by the harness for the flash check): the fixture writes
//! `fixture-<pid>.events` there with only the lines written by [`event`] (`FIRST_PAINT`,
//! `RESIZED`, `MOVED`), in the same line format. It does not touch stderr or `WAYLAND_DEBUG`, so
//! it does not change the timing of the Wayland session it observes. With both set, [`event`]
//! lines go to both files.
//!
//! Without either variable nothing is opened and `log` and `event` are no-ops.

use std::fs::OpenOptions;
use std::io::Write;
use std::os::unix::io::AsRawFd;
use std::sync::Mutex;

/// Open trace file, `None` while tracing is off.
static SINK: Mutex<Option<std::fs::File>> = Mutex::new(None);

/// Open client-event file read by the harness, `None` while it is off.
static EVENTS: Mutex<Option<std::fs::File>> = Mutex::new(None);

/// Current `CLOCK_MONOTONIC` in nanoseconds.
fn mono_ns() -> u64 {
    let mut ts = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // SAFETY: `ts` is a valid, writable timespec for the duration of the call.
    unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut ts) };
    ts.tv_sec as u64 * 1_000_000_000 + ts.tv_nsec as u64
}

/// Current `CLOCK_REALTIME` in nanoseconds (written once so the protocol dump's own clock can be
/// related to the monotonic one).
fn real_ns() -> u64 {
    let mut ts = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // SAFETY: `ts` is a valid, writable timespec for the duration of the call.
    unsafe { libc::clock_gettime(libc::CLOCK_REALTIME, &mut ts) };
    ts.tv_sec as u64 * 1_000_000_000 + ts.tv_nsec as u64
}

/// Opens the event files named by the environment; call before any Wayland connection.
///
/// `SPAWN_AT_FIXTURE_EVENT_DIR` opens the client-event file only. `SPAWN_AT_FIXTURE_TRACE_DIR`
/// additionally redirects stderr (fd 2) into the trace file and sets `WAYLAND_DEBUG=client` so
/// libwayland prints every protocol message it sends or receives, with its own timestamp.
pub(crate) fn init() {
    if let Ok(dir) = std::env::var("SPAWN_AT_FIXTURE_EVENT_DIR") {
        let path = format!("{}/fixture-{}.events", dir, std::process::id());
        if let Ok(file) = OpenOptions::new().create(true).append(true).open(&path) {
            *EVENTS.lock().unwrap() = Some(file);
        }
    }
    let Ok(dir) = std::env::var("SPAWN_AT_FIXTURE_TRACE_DIR") else {
        return;
    };
    let path = format!("{}/fixture-{}.log", dir, std::process::id());
    let Ok(file) = OpenOptions::new().create(true).append(true).open(&path) else {
        return;
    };
    // SAFETY: both descriptors are valid; dup2 replaces fd 2 with the trace file.
    unsafe { libc::dup2(file.as_raw_fd(), 2) };
    std::env::set_var("WAYLAND_DEBUG", "client");
    *SINK.lock().unwrap() = Some(file);
    log("TRACE_START", &format!("realtime_ns={}", real_ns()));
}

/// Appends `<mono_ns> FIXTURE <tag> <detail>` to the trace file; no-op when tracing is off.
pub(crate) fn log(tag: &str, detail: &str) {
    let t = mono_ns();
    if let Some(f) = SINK.lock().unwrap().as_mut() {
        let _ = writeln!(f, "{} FIXTURE {} {}", t, tag, detail);
    }
}

/// Appends `<mono_ns> FIXTURE <tag> <detail>` to the client-event file and, when it is open, to
/// the trace file; no-op when neither is open. The harness reads these lines for the flash check.
pub(crate) fn event(tag: &str, detail: &str) {
    let t = mono_ns();
    for sink in [&EVENTS, &SINK] {
        if let Some(f) = sink.lock().unwrap().as_mut() {
            let _ = writeln!(f, "{} FIXTURE {} {}", t, tag, detail);
        }
    }
}

/// Records `FIRST_PAINT <w>x<h>` when a buffer of that size has not been painted before, and
/// remembers the size in `last`. A repaint at the size already painted writes nothing.
pub(crate) fn note_paint(last: &mut Option<(u32, u32)>, w: u32, h: u32) {
    if *last != Some((w, h)) {
        *last = Some((w, h));
        event("FIRST_PAINT", &format!("{}x{}", w, h));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// With neither `SPAWN_AT_FIXTURE_TRACE_DIR` nor `SPAWN_AT_FIXTURE_EVENT_DIR` set (the case
    /// for every normal fixture run), `log` and `event` find no open file and return without
    /// writing or panicking.
    #[test]
    fn test_log_is_a_no_op_while_tracing_is_off() {
        assert!(SINK.lock().unwrap().is_none());
        assert!(EVENTS.lock().unwrap().is_none());
        log("ANY_TAG", "detail");
        event("ANY_TAG", "detail");
        assert!(SINK.lock().unwrap().is_none());
        assert!(EVENTS.lock().unwrap().is_none());
    }

    /// `note_paint` updates the remembered size only when it changes: the second paint at 400x300
    /// leaves it alone, the paint at 800x600 replaces it. (With the event file closed the call
    /// still tracks the size; only the write is skipped.)
    #[test]
    fn test_note_paint_tracks_new_sizes_only() {
        let mut last = None;
        note_paint(&mut last, 400, 300);
        assert_eq!(last, Some((400, 300)));
        note_paint(&mut last, 400, 300);
        assert_eq!(last, Some((400, 300)));
        note_paint(&mut last, 800, 600);
        assert_eq!(last, Some((800, 600)));
    }

    /// The monotonic clock never goes backwards between two reads, which the analysis relies on
    /// when it orders trace lines by their nanosecond stamp.
    #[test]
    fn test_mono_ns_is_non_decreasing() {
        let a = mono_ns();
        let b = mono_ns();
        assert!(b >= a);
    }
}
