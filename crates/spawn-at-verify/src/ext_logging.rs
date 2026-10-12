//! Control of the GNOME extension's timing log over session D-Bus.

use crate::log_source::LogSource;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
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
/// Longest echo wait once an earlier wait has timed out; stops at the first match.
const REMEMBERED_ECHO_TIMEOUT: Duration = Duration::from_millis(150);

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

    /// Called when a run's slice held a trace for its window, which proves the timing log is on.
    /// The default does nothing.
    fn trace_captured(&self) {}
}

/// Whether the last echo wait of this session timed out. It lives for the whole process, so
/// `run`, `calibrate` and the TUI (which builds a runner per test) share it.
#[derive(Debug, Default)]
pub struct EchoMemory {
    timed_out: AtomicBool,
}

impl EchoMemory {
    /// The one memory shared by every real [`GdbusLogging`] in this process.
    pub fn session() -> Arc<EchoMemory> {
        static SESSION: OnceLock<Arc<EchoMemory>> = OnceLock::new();
        SESSION.get_or_init(Arc::default).clone()
    }
}

/// The reason recorded when no enable echo was seen. It holds for both a full wait and the
/// single check made after an earlier timeout, so every affected result carries the same text.
pub fn echo_timeout_note() -> String {
    format!(
        "SetLogging(true) succeeded but no '{}' line reached the journal (the first wait lasts {} ms; once one times out, later tests wait only {} ms), so the timing log may be off",
        ENABLE_ECHO,
        ECHO_TIMEOUT.as_millis(),
        REMEMBERED_ECHO_TIMEOUT.as_millis()
    )
}

/// Real control: `SetLogging(true)` over D-Bus, then a wait for the echo in the journal.
pub struct GdbusLogging {
    /// How `SetLogging(true)` is sent; [`set_extension_logging`] unless a test replaces it.
    set_logging: fn(bool) -> Result<(), String>,
    /// Set when an echo wait timed out; later checks then poll once instead of up to the timeout.
    memory: Arc<EchoMemory>,
    /// Longest echo wait while no timeout is remembered.
    timeout: Duration,
}

impl GdbusLogging {
    /// A control that sends `SetLogging` through the real `gdbus`.
    pub fn new() -> Self {
        Self {
            set_logging: set_extension_logging,
            memory: EchoMemory::session(),
            timeout: ECHO_TIMEOUT,
        }
    }
}

impl Default for GdbusLogging {
    /// Same as [`GdbusLogging::new`].
    fn default() -> Self {
        Self::new()
    }
}

impl LoggingControl for GdbusLogging {
    /// Calls `SetLogging(true)`. With the "noop" log source it stops there, because nothing can
    /// echo into it. Otherwise it waits up to the timeout for [`ENABLE_ECHO`] (see
    /// [`wait_for_echo`]), or waits only [`REMEMBERED_ECHO_TIMEOUT`] once an earlier wait has timed out.
    fn ensure_enabled(&self, log: &dyn LogSource) -> LoggingOutcome {
        let since = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        if let Err(e) = (self.set_logging)(true) {
            return LoggingOutcome::Failed(format!(
                "extension timing log could not be enabled: {}",
                e.trim()
            ));
        }
        if log.name() == "noop" {
            return LoggingOutcome::Failed(
                "SetLogging(true) was sent, but the no-op log source cannot show whether the timing log is on"
                    .to_string(),
            );
        }
        // After a timeout the wait shrinks to a short poll; seeing the echo again clears it.
        let remembered = self.memory.timed_out.load(Ordering::SeqCst);
        let timeout = if remembered {
            REMEMBERED_ECHO_TIMEOUT
        } else {
            self.timeout
        };
        if wait_for_echo(log, since, timeout, ECHO_POLL_INTERVAL) {
            self.memory.timed_out.store(false, Ordering::SeqCst);
            return LoggingOutcome::Enabled;
        }
        self.memory.timed_out.store(true, Ordering::SeqCst);
        LoggingOutcome::Failed(echo_timeout_note())
    }

    /// Clears the remembered echo timeout: a captured trace shows the log works.
    fn trace_captured(&self) {
        self.memory.timed_out.store(false, Ordering::SeqCst);
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
    use std::sync::atomic::{AtomicBool, AtomicUsize};

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

    /// Log source named "noop" that counts reads, to show none happen.
    struct CountingNoop(AtomicUsize);

    impl LogSource for CountingNoop {
        /// Returns the provider name "noop".
        fn name(&self) -> &'static str {
            "noop"
        }

        /// Counts the read and returns an empty slice.
        fn capture_slice(&self, _since: u64, _until: u64) -> Result<String, String> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok(String::new())
        }
    }

    static NOOP_SET_CALLS: AtomicUsize = AtomicUsize::new(0);

    /// Stand-in for `gdbus` that succeeds and counts the call.
    fn counting_set_logging(_on: bool) -> Result<(), String> {
        NOOP_SET_CALLS.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    /// With the "noop" source `SetLogging` is still sent once, but the journal is never read and
    /// the call returns in well under one poll interval sequence (a skipped wait would otherwise
    /// cost the full 1000 ms). Fails if the echo wait runs against a source that cannot echo.
    #[test]
    fn test_noop_source_sends_set_logging_but_skips_the_echo_wait() {
        let log = CountingNoop(AtomicUsize::new(0));
        let ctl = GdbusLogging {
            set_logging: counting_set_logging,
            memory: Arc::default(),
            timeout: ECHO_TIMEOUT,
        };
        let began = Instant::now();
        let outcome = ctl.ensure_enabled(&log);
        assert_eq!(NOOP_SET_CALLS.load(Ordering::SeqCst), 1);
        assert_eq!(log.0.load(Ordering::SeqCst), 0, "journal was read");
        assert!(began.elapsed() < Duration::from_millis(100));
        assert!(matches!(outcome, LoggingOutcome::Failed(_)));
    }

    /// Stand-in for `gdbus` that always succeeds.
    fn ok_set_logging(_on: bool) -> Result<(), String> {
        Ok(())
    }

    /// A control with its own memory (so tests do not share the process-wide one) and `timeout`.
    fn control(memory: &Arc<EchoMemory>, timeout: Duration) -> GdbusLogging {
        GdbusLogging {
            set_logging: ok_set_logging,
            memory: memory.clone(),
            timeout,
        }
    }

    /// Two tests in a row with no echo: the first waits the full 1000 ms, the second makes one
    /// read and returns at once, so together they cost about 1 s, not 2 s. Fails if the second
    /// test polls for another second.
    #[test]
    fn test_two_consecutive_echo_timeouts_cost_about_one_second() {
        let memory = Arc::new(EchoMemory::default());
        let ctl = control(&memory, ECHO_TIMEOUT);
        let journal = FakeJournal::new(None);
        let began = Instant::now();
        assert!(matches!(
            ctl.ensure_enabled(&journal),
            LoggingOutcome::Failed(_)
        ));
        let first = began.elapsed();
        let reads_after_first = journal.calls();
        assert!(matches!(
            ctl.ensure_enabled(&journal),
            LoggingOutcome::Failed(_)
        ));
        assert!(
            journal.calls() > reads_after_first + 1,
            "short poll polled repeatedly"
        );
        let total = began.elapsed();
        assert!(
            first >= Duration::from_millis(1000),
            "first took {:?}",
            first
        );
        assert!(
            total < Duration::from_millis(1500),
            "total took {:?}",
            total
        );
    }

    /// Journal whose echo can be switched on after construction.
    struct SwitchJournal(AtomicBool);

    impl LogSource for SwitchJournal {
        /// Returns the provider name "switch-journal".
        fn name(&self) -> &'static str {
            "switch-journal"
        }

        /// The echo line while switched on, else an unrelated line.
        fn capture_slice(&self, _since: u64, _until: u64) -> Result<String, String> {
            Ok(if self.0.load(Ordering::SeqCst) {
                ENABLE_ECHO.to_string()
            } else {
                "other line".to_string()
            })
        }
    }

    /// After a timeout the single check sees the echo: the outcome is Enabled and the memory is
    /// cleared, so the next missing echo gets the full 200 ms wait again. Fails if the memory
    /// stays set (the last wait would then be about 0 ms).
    #[test]
    fn test_echo_reappearing_resets_the_timeout_memory() {
        let memory = Arc::new(EchoMemory::default());
        let ctl = control(&memory, Duration::from_millis(200));
        let journal = SwitchJournal(AtomicBool::new(false));
        assert!(matches!(
            ctl.ensure_enabled(&journal),
            LoggingOutcome::Failed(_)
        ));
        assert!(memory.timed_out.load(Ordering::SeqCst));
        journal.0.store(true, Ordering::SeqCst);
        assert_eq!(ctl.ensure_enabled(&journal), LoggingOutcome::Enabled);
        assert!(!memory.timed_out.load(Ordering::SeqCst));
        journal.0.store(false, Ordering::SeqCst);
        let began = Instant::now();
        assert!(matches!(
            ctl.ensure_enabled(&journal),
            LoggingOutcome::Failed(_)
        ));
        assert!(began.elapsed() >= Duration::from_millis(200));
    }

    /// After a timeout the 150 ms poll catches an echo that shows up on the 4th read (about
    /// 60 ms in). Fails if the remembered wait is a single immediate check.
    #[test]
    fn test_late_echo_is_caught_by_the_short_poll() {
        let memory = Arc::new(EchoMemory::default());
        memory.timed_out.store(true, Ordering::SeqCst);
        let ctl = control(&memory, ECHO_TIMEOUT);
        let journal = FakeJournal::new(Some(4));
        assert_eq!(ctl.ensure_enabled(&journal), LoggingOutcome::Enabled);
        assert!(!memory.timed_out.load(Ordering::SeqCst));
    }

    /// `trace_captured` clears a set memory, so the next missing echo waits the full timeout.
    #[test]
    fn test_trace_captured_clears_the_timeout_memory() {
        let memory = Arc::new(EchoMemory::default());
        memory.timed_out.store(true, Ordering::SeqCst);
        control(&memory, ECHO_TIMEOUT).trace_captured();
        assert!(!memory.timed_out.load(Ordering::SeqCst));
    }
}
