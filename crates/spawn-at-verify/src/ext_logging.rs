//! Control of the GNOME extension's timing log over session D-Bus.

use crate::log_source::LogSource;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};

/// Invokes `SetLogging` on the GNOME Shell extension via session D-Bus.
pub fn set_extension_logging(enabled: bool) -> Result<(), String> {
    let output = Command::new("gdbus")
        .args([
            "call",
            "--session",
            "--dest",
            "org.gnome.Shell",
            "--object-path",
            "/org/gnome/Shell/Extensions/SpawnAt",
            "--method",
            "org.gnome.Shell.Extensions.SpawnAt.SetLogging",
            if enabled { "true" } else { "false" },
        ])
        .output();

    match output {
        Ok(out) if out.status.success() => Ok(()),
        Ok(out) => Err(format!(
            "gdbus returned non-zero exit code: {:?}, stderr: {}",
            out.status.code(),
            String::from_utf8_lossy(&out.stderr)
        )),
        Err(e) => Err(format!("Failed to execute gdbus: {}", e)),
    }
}

/// Line the extension writes when `SetLogging(true)` takes effect. It is only written while
/// logging is on, so seeing it proves the call reached a live logger.
const ENABLE_ECHO: &str = "SET_LOGGING enabled=true";
/// Longest total wait for [`ENABLE_ECHO`] after `SetLogging(true)`; no new poll starts after it.
const ECHO_TIMEOUT: Duration = Duration::from_millis(1000);
/// Pause between polls for the echo (the first poll is immediate).
const ECHO_POLL_INTERVAL: Duration = Duration::from_millis(20);

/// Polls `log` for [`ENABLE_ECHO`] in the slice starting at `since`, immediately and then every
/// `interval`, and stops at the first match. No poll starts once `timeout` has elapsed, so the
/// wait is at most `timeout` plus one in-flight `capture_slice` call.
pub fn wait_for_echo(
    log: &dyn LogSource,
    since: u64,
    timeout: Duration,
    interval: Duration,
) -> bool {
    let began = Instant::now();
    loop {
        let until = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(since);
        if let Ok(slice) = log.capture_slice(since, until) {
            if slice.contains(ENABLE_ECHO) {
                return true;
            }
        }
        if began.elapsed() >= timeout {
            return false;
        }
        std::thread::sleep(interval);
    }
}

/// What happened when the extension's timing log was asked to be on before a run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoggingOutcome {
    /// The extension's timing log is on (its echo was seen, or the source needs none).
    Enabled,
    /// There is nothing to enable: the log source is not the extension's journal.
    Unneeded,
    /// The timing log is not known to be on; the reason, as one sentence shown to the user.
    Failed(String),
}

impl LoggingOutcome {
    /// The reason to record next to a result's trace, or `None` when logging is fine.
    pub fn note(&self) -> Option<String> {
        match self {
            LoggingOutcome::Failed(why) => Some(why.clone()),
            LoggingOutcome::Enabled | LoggingOutcome::Unneeded => None,
        }
    }
}

/// Turns the extension's timing log on before a run.
///
/// The extension exposes no getter for its logging state, and its logger starts with logging
/// off again whenever the extension is reloaded. So instead of asking, the runner enables it
/// (idempotent) before every run and checks that the extension echoed the change.
pub trait LoggingControl: Send + Sync {
    /// Enables the timing log and reports whether it is known to be on. `log` is the source the
    /// run's slice will be read from; it is where the enable echo is looked for.
    fn ensure_enabled(&self, log: &dyn LogSource) -> LoggingOutcome;
}

/// Real control: `SetLogging(true)` over D-Bus, then a wait for the echo in the journal.
pub struct GdbusLogging;

impl LoggingControl for GdbusLogging {
    /// Calls `SetLogging(true)` and waits up to [`ECHO_TIMEOUT`] for [`ENABLE_ECHO`] (see
    /// [`wait_for_echo`]).
    fn ensure_enabled(&self, log: &dyn LogSource) -> LoggingOutcome {
        let since = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        if let Err(e) = set_extension_logging(true) {
            return LoggingOutcome::Failed(format!(
                "extension timing log could not be enabled: {}",
                e.trim()
            ));
        }
        if wait_for_echo(log, since, ECHO_TIMEOUT, ECHO_POLL_INTERVAL) {
            return LoggingOutcome::Enabled;
        }
        LoggingOutcome::Failed(format!(
            "SetLogging(true) succeeded but no '{}' line reached the journal within {} ms, so the timing log may be off",
            ENABLE_ECHO,
            ECHO_TIMEOUT.as_millis()
        ))
    }
}

/// Control for log sources that are not the extension's journal.
pub struct NoLogging {
    /// Why there is no log to enable (a no-op source), or `None` when the source is complete
    /// by itself (an in-memory fake).
    pub reason: Option<String>,
}

impl LoggingControl for NoLogging {
    /// [`LoggingOutcome::Failed`] with `reason` when there is one, else [`LoggingOutcome::Unneeded`].
    fn ensure_enabled(&self, _log: &dyn LogSource) -> LoggingOutcome {
        match &self.reason {
            Some(why) => LoggingOutcome::Failed(why.clone()),
            None => LoggingOutcome::Unneeded,
        }
    }
}

/// State shared by the handles of one [`FakeExtension`].
#[derive(Debug, Default)]
struct FakeExtensionState {
    /// Whether the fake logger is currently on.
    enabled: bool,
    /// Lines the extension logs for the next window, written only while logging is on.
    script: Vec<String>,
    /// When set, `ensure_enabled` fails with this text and leaves logging as it was.
    enable_error: Option<String>,
    /// How many times `ensure_enabled` was called.
    enable_calls: usize,
}

/// In-memory stand-in for the extension: a log source plus its logging switch.
///
/// While logging is on, a slice holds the scripted lines; while it is off the slice is empty,
/// as with the real logger. [`FakeExtension::reload`] switches logging off, as an extension
/// reload does. Clones share one state, so one handle can be the runner's log source and
/// another its logging control.
#[derive(Debug, Clone, Default)]
pub struct FakeExtension {
    state: Arc<Mutex<FakeExtensionState>>,
}

impl FakeExtension {
    /// A fake that logs `script` while on; `enabled` is its state before any call.
    pub fn new(enabled: bool, script: &[&str]) -> Self {
        let ext = Self::default();
        {
            let mut s = ext.state.lock().unwrap();
            s.enabled = enabled;
            s.script = script.iter().map(|l| l.to_string()).collect();
        }
        ext
    }

    /// Switches logging off, as reloading the extension does.
    pub fn reload(&self) {
        self.state.lock().unwrap().enabled = false;
    }

    /// Makes later `ensure_enabled` calls fail with `error` (or succeed again with `None`).
    pub fn set_enable_error(&self, error: Option<&str>) {
        self.state.lock().unwrap().enable_error = error.map(String::from);
    }

    /// Whether logging is currently on.
    pub fn is_enabled(&self) -> bool {
        self.state.lock().unwrap().enabled
    }

    /// How many times logging was requested.
    pub fn enable_calls(&self) -> usize {
        self.state.lock().unwrap().enable_calls
    }
}

impl LogSource for FakeExtension {
    /// Returns the provider name "fake-extension".
    fn name(&self) -> &'static str {
        "fake-extension"
    }

    /// The scripted lines while logging is on, else an empty slice; the time range is ignored.
    fn capture_slice(&self, _since_secs: u64, _until_secs: u64) -> Result<String, String> {
        let s = self.state.lock().unwrap();
        Ok(if s.enabled {
            s.script.join("\n")
        } else {
            String::new()
        })
    }
}

impl LoggingControl for FakeExtension {
    /// Turns the fake logger on unless an enable error was set.
    fn ensure_enabled(&self, _log: &dyn LogSource) -> LoggingOutcome {
        let mut s = self.state.lock().unwrap();
        s.enable_calls += 1;
        if let Some(e) = &s.enable_error {
            return LoggingOutcome::Failed(format!(
                "extension timing log could not be enabled: {}",
                e
            ));
        }
        s.enabled = true;
        LoggingOutcome::Enabled
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// Journal that shows the enable echo from its `echo_on_call`-th read on (1-based), or never.
    struct FakeJournal {
        echo_on_call: Option<usize>,
        calls: AtomicUsize,
    }

    impl FakeJournal {
        /// A journal whose echo appears on read number `echo_on_call`, or never for `None`.
        fn new(echo_on_call: Option<usize>) -> Self {
            Self {
                echo_on_call,
                calls: AtomicUsize::new(0),
            }
        }

        /// How many times the journal was read.
        fn calls(&self) -> usize {
            self.calls.load(Ordering::SeqCst)
        }
    }

    impl LogSource for FakeJournal {
        /// Returns the provider name "fake-journal".
        fn name(&self) -> &'static str {
            "fake-journal"
        }

        /// A line unrelated to logging until the echo's read number, then the echo line.
        fn capture_slice(&self, _since: u64, _until: u64) -> Result<String, String> {
            let n = self.calls.fetch_add(1, Ordering::SeqCst) + 1;
            Ok(match self.echo_on_call {
                Some(k) if n >= k => format!("other line\n+1.00ms | {}", ENABLE_ECHO),
                _ => "other line".to_string(),
            })
        }
    }

    /// Echo already there: one read, no sleep, true.
    #[test]
    fn test_wait_for_echo_immediate() {
        let j = FakeJournal::new(Some(1));
        let began = Instant::now();
        assert!(wait_for_echo(
            &j,
            0,
            Duration::from_secs(5),
            Duration::from_millis(200)
        ));
        assert_eq!(j.calls(), 1);
        assert!(began.elapsed() < Duration::from_millis(150), "slept first");
    }

    /// Echo on the 4th read: stops there (no 5th read) after waiting about 3 intervals.
    #[test]
    fn test_wait_for_echo_late_stops_at_first_match() {
        let j = FakeJournal::new(Some(4));
        let began = Instant::now();
        assert!(wait_for_echo(
            &j,
            0,
            Duration::from_secs(5),
            Duration::from_millis(20)
        ));
        assert_eq!(j.calls(), 4);
        let took = began.elapsed();
        assert!(took >= Duration::from_millis(60), "took {:?}", took);
        assert!(took < Duration::from_millis(1000), "took {:?}", took);
    }

    /// No echo: gives up after the timeout, having polled repeatedly, and not much later.
    #[test]
    fn test_wait_for_echo_never_gives_up_at_the_timeout() {
        let j = FakeJournal::new(None);
        let began = Instant::now();
        assert!(!wait_for_echo(
            &j,
            0,
            Duration::from_millis(100),
            Duration::from_millis(20)
        ));
        let took = began.elapsed();
        assert!(took >= Duration::from_millis(100), "took {:?}", took);
        assert!(took < Duration::from_millis(400), "took {:?}", took);
        assert!(j.calls() >= 3, "polled {} times", j.calls());
    }
}
