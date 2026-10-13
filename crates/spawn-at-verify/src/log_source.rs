//! Log Capture Abstraction (`LogSource` trait).
//!
//! Provides platform-agnostic log slicing for test execution windows.

use std::collections::BTreeMap;
use std::process::Command;
use std::sync::Mutex;

/// Trait defining system log capture over a discrete execution time window.
pub trait LogSource: Send + Sync {
    /// Identifier of the log source (e.g. "journalctl", "noop", "fake").
    fn name(&self) -> &'static str;

    /// Captures the log slice between the start and end unix epoch timestamps (in seconds).
    /// Captures journalctl entries within the specified timeframe.
    fn capture_slice(&self, since_secs: u64, until_secs: u64) -> Result<String, String>;
}

/// Whether a journal line belongs in a run's slice: it mentions `spawn-at` (every line of the
/// extension's timing log starts with `[spawn-at-time]`, whatever its tag) or `gnome-shell`.
pub fn is_relevant_line(line: &str) -> bool {
    line.contains("spawn-at") || line.contains("[spawn-at") || line.contains("gnome-shell")
}

/// GNOME / systemd-journald log source querying `journalctl --user`.
pub struct GnomeJournalSource;

impl LogSource for GnomeJournalSource {
    /// Returns the provider name "journalctl".
    fn name(&self) -> &'static str {
        "journalctl"
    }

    /// Captures journalctl entries within the specified timeframe.
    fn capture_slice(&self, since_secs: u64, until_secs: u64) -> Result<String, String> {
        let since_arg = format!("@{}", since_secs);
        let until_arg = format!("@{}", until_secs + 1); // small boundary buffer

        let output = Command::new("journalctl")
            .args([
                "--user",
                "--since",
                &since_arg,
                "--until",
                &until_arg,
                "--no-pager",
                "-o",
                "cat",
            ])
            .output()
            .map_err(|e| format!("Failed to invoke journalctl: {}", e))?;

        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            return Err(format!("journalctl exited with error: {}", err));
        }

        let out_str = String::from_utf8_lossy(&output.stdout);
        // Filter to spawn-at or mutter / gnome-shell relevant lines
        let filtered: Vec<&str> = out_str.lines().filter(|l| is_relevant_line(l)).collect();

        Ok(filtered.join("\n"))
    }
}

/// No-op log source used on platforms without system log integration.
pub struct NoOpLogSource;

impl LogSource for NoOpLogSource {
    /// Returns the provider name "noop".
    fn name(&self) -> &'static str {
        "noop"
    }

    /// Always returns an empty string without querying system logs.
    fn capture_slice(&self, _since_secs: u64, _until_secs: u64) -> Result<String, String> {
        Ok(String::new())
    }
}

/// Fake in-memory log source for deterministic unit testing.
pub struct FakeLogSource {
    entries: Mutex<BTreeMap<u64, Vec<String>>>,
}

impl FakeLogSource {
    /// Constructs an empty  instance.
    pub fn new() -> Self {
        Self {
            entries: Mutex::new(BTreeMap::new()),
        }
    }

    /// Records a log message at the given unix timestamp.
    pub fn record(&self, timestamp_secs: u64, line: &str) {
        let mut map = self.entries.lock().unwrap();
        map.entry(timestamp_secs)
            .or_default()
            .push(line.to_string());
    }
}

impl Default for FakeLogSource {
    /// Constructs standard default instance.
    fn default() -> Self {
        Self::new()
    }
}

impl LogSource for FakeLogSource {
    /// Returns the provider name "fake".
    fn name(&self) -> &'static str {
        "fake"
    }

    /// Captures journalctl entries within the specified timeframe.
    /// Retrieves all recorded entries falling within the requested timeframe.
    fn capture_slice(&self, since_secs: u64, until_secs: u64) -> Result<String, String> {
        let map = self.entries.lock().unwrap();
        let mut lines = Vec::new();

        for (&ts, entries) in map.range(since_secs..=until_secs) {
            for entry in entries {
                lines.push(format!("[{}] {}", ts, entry));
            }
        }

        Ok(lines.join("\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tests in-memory log slicing filtering by timestamp range.
    #[test]
    fn test_fake_log_source_captures_time_window_slice() {
        let source = FakeLogSource::new();
        source.record(100, "Early initialization");
        source.record(200, "Test window created");
        source.record(205, "Test window mapped and placed");
        source.record(300, "Later unrelated event");

        // Request slice [200, 210]
        let slice = source.capture_slice(200, 210).unwrap();
        assert!(slice.contains("Test window created"));
        assert!(slice.contains("Test window mapped and placed"));
        assert!(!slice.contains("Early initialization"));
        assert!(!slice.contains("Later unrelated event"));
    }

    /// Every `X11_OBS_*` line of the observe-only X11 logging (extension, `x11_observe.js`) has the
    /// `[spawn-at-time]` prefix, so the journal filter keeps it and it lands in a run's `log_slice`
    /// unchanged. The lines are in the shape `tests/x11_observe_test.js` and the wiring test
    /// produce; unrelated journal lines are still dropped.
    #[test]
    fn test_x11_observation_lines_pass_the_journal_filter() {
        let win = "win=[id=301 pid=7301 class=\"verify.x\" app=\"\" type=NORMAL proto=X11]";
        let lines = [
            format!("[spawn-at-time] +10.00ms | X11_OBS_START {win} kind=XWayland wayland=true client_type=1 id=301 mono_ms=1000.000 damaged=available first_frame=available"),
            format!("[spawn-at-time] +11.00ms | X11_OBS_NOTE {win} damaged unavailable"),
            format!("[spawn-at-time] +12.00ms | X11_OBS_MARK {win} what=set_size id=301 mono_ms=1002.000"),
            format!("[spawn-at-time] +20.00ms | X11_OBS_SIGNAL {win} signal=damaged id=301 mono_ms=1010.000 seq=1 texture=(400x300) actor=(800x600) frame=(10,20,800x600) buffer=(9,19,802x602)"),
            format!("[spawn-at-time] +520.00ms | X11_OBS_END {win} id=301 reason=reveal+500ms mono_ms=1510.000 damaged=3 first_frame=1"),
        ];
        for l in &lines {
            assert!(is_relevant_line(l), "{l}");
        }
        assert!(!is_relevant_line(
            "systemd[1]: Started something unrelated."
        ));
        // Through a source that keeps what it is given, the slice is the lines, byte for byte.
        let source = FakeLogSource::new();
        for l in &lines {
            source.record(100, l);
        }
        let slice = source.capture_slice(100, 100).unwrap();
        for l in &lines {
            assert!(slice.contains(l.as_str()), "{l}");
        }
    }

    /// The lines of the experimental x11-hold-damage wait (`X11_HOLD_DAMAGED` and the existing wait
    /// tags with `strategy=x11-hold-damage`) carry the `[spawn-at-time]` prefix, so they reach a
    /// run's `log_slice` unchanged.
    #[test]
    fn test_x11_hold_lines_pass_the_journal_filter() {
        let win = "win=[id=301 pid=7301 class=\"verify.x\" app=\"\" type=NORMAL proto=XWayland]";
        let lines = [
            format!("[spawn-at-time] +10.00ms | READY_WAIT_START {win} strategy=x11-hold-damage kind=XWayland target=(800x600) cap=250 hard=1500 reveal_on_damaged=2"),
            format!("[spawn-at-time] +14.00ms | X11_HOLD_DAMAGED {win} n=1 ignored=true since_start_ms=4.0"),
            format!("[spawn-at-time] +40.00ms | COMMIT_RESOLVED {win} reason=X11_HOLD_SECOND_DAMAGED finalFrame=(800x600) target=(800x600) safe=(800x600)"),
            format!("[spawn-at-time] +40.10ms | READY_WAIT_RESOLVED {win} strategy=x11-hold-damage outcome=reached elapsed_ms=30.1 waited_ms=30 reason=x11-second-damaged damaged_seen=2 finalFrame=(800x600) target=(800x600)"),
        ];
        let source = FakeLogSource::new();
        for l in &lines {
            assert!(is_relevant_line(l), "{l}");
            source.record(100, l);
        }
        let slice = source.capture_slice(100, 100).unwrap();
        for l in &lines {
            assert!(slice.contains(l.as_str()), "{l}");
        }
    }

    /// Tests that no-op log source returns an empty string.
    #[test]
    fn test_noop_log_source_returns_empty() {
        let source = NoOpLogSource;
        assert_eq!(source.name(), "noop");
        let res = source.capture_slice(100, 200).unwrap();
        assert!(res.is_empty());
    }
}
