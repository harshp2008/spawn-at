//! Developer-only event trace of the fixture, enabled by `SPAWN_AT_FIXTURE_TRACE_DIR`.
//!
//! With the variable set to a directory, the fixture writes `fixture-<pid>.log` there. Each of its
//! own lines carries `CLOCK_MONOTONIC` nanoseconds, the clock the GNOME extension's `+N ms` log
//! counts from. The file also receives the Wayland client protocol dump (`WAYLAND_DEBUG=client`,
//! stderr is redirected into it), so configure, ack_configure, frame callbacks and commits are
//! timestamped by libwayland itself. Without the variable nothing is opened and `log` is a no-op.

use std::fs::OpenOptions;
use std::io::Write;
use std::os::unix::io::AsRawFd;
use std::sync::Mutex;

/// Open trace file, `None` while tracing is off.
static SINK: Mutex<Option<std::fs::File>> = Mutex::new(None);

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

/// Starts tracing when `SPAWN_AT_FIXTURE_TRACE_DIR` is set; call before any Wayland connection.
///
/// Redirects stderr (fd 2) into the trace file and sets `WAYLAND_DEBUG=client` so libwayland
/// prints every protocol message it sends or receives, with its own timestamp.
pub(crate) fn init() {
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

#[cfg(test)]
mod tests {
    use super::*;

    /// With no `SPAWN_AT_FIXTURE_TRACE_DIR` set (the case for every normal fixture run), `log`
    /// finds no open file and returns without writing or panicking.
    #[test]
    fn test_log_is_a_no_op_while_tracing_is_off() {
        assert!(SINK.lock().unwrap().is_none());
        log("ANY_TAG", "detail");
        assert!(SINK.lock().unwrap().is_none());
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
