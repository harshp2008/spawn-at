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
    fn capture_slice(&self, since_secs: u64, until_secs: u64) -> Result<String, String>;
}

/// GNOME / systemd-journald log source querying `journalctl --user`.
pub struct GnomeJournalSource;

impl LogSource for GnomeJournalSource {
    fn name(&self) -> &'static str {
        "journalctl"
    }

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
        let filtered: Vec<&str> = out_str
            .lines()
            .filter(|line| {
                line.contains("spawn-at")
                    || line.contains("[spawn-at")
                    || line.contains("gnome-shell")
            })
            .collect();

        Ok(filtered.join("\n"))
    }
}

/// No-op log source used on platforms without system log integration.
pub struct NoOpLogSource;

impl LogSource for NoOpLogSource {
    fn name(&self) -> &'static str {
        "noop"
    }

    fn capture_slice(&self, _since_secs: u64, _until_secs: u64) -> Result<String, String> {
        Ok(String::new())
    }
}

/// Fake in-memory log source for deterministic unit testing.
pub struct FakeLogSource {
    entries: Mutex<BTreeMap<u64, Vec<String>>>,
}

impl FakeLogSource {
    pub fn new() -> Self {
        Self {
            entries: Mutex::new(BTreeMap::new()),
        }
    }

    pub fn record(&self, timestamp_secs: u64, line: &str) {
        let mut map = self.entries.lock().unwrap();
        map.entry(timestamp_secs)
            .or_default()
            .push(line.to_string());
    }
}

impl Default for FakeLogSource {
    fn default() -> Self {
        Self::new()
    }
}

impl LogSource for FakeLogSource {
    fn name(&self) -> &'static str {
        "fake"
    }

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

    #[test]
    fn test_noop_log_source_returns_empty() {
        let source = NoOpLogSource;
        assert_eq!(source.name(), "noop");
        let res = source.capture_slice(100, 200).unwrap();
        assert!(res.is_empty());
    }
}
