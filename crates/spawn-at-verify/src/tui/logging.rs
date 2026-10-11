//! Extension Logging State Detection & Restoration Guard.
//!
//! Follows V4 Requirement 2:
//! - On start, records the current extension logging state.
//! - On exit, restores THAT state; never blindly forces SetLogging(false).
//! - If the tool enabled logging itself, it turns it off; if the user had it on, it stays on.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

/// Locates the active session log file.
pub fn get_session_log_path() -> PathBuf {
    if let Ok(state_home) = std::env::var("XDG_STATE_HOME") {
        PathBuf::from(state_home)
            .join("spawn-at")
            .join("session.log")
    } else if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home).join(".local/state/spawn-at/session.log")
    } else {
        PathBuf::from("/tmp/spawn-at-session.log")
    }
}

/// Parses the latest `SET_LOGGING enabled=<bool>` transition from log file text.
pub fn parse_logging_state_from_content(content: &str) -> Option<bool> {
    for line in content.lines().rev() {
        if line.contains("SET_LOGGING enabled=true") {
            return Some(true);
        }
        if line.contains("SET_LOGGING enabled=false") {
            return Some(false);
        }
    }
    None
}

/// Detects the currently active extension logging state.
///
/// Priority:
/// 1. Reads the latest `SET_LOGGING enabled=<bool>` entry in `~/.local/state/spawn-at/session.log`.
/// 2. If no entry found, inspects the `SPAWN_AT_DEBUG` environment variable.
/// 3. Defaults to `false` if neither is present.
pub fn detect_current_logging_state() -> bool {
    let log_path = get_session_log_path();
    if let Ok(content) = fs::read_to_string(&log_path) {
        if let Some(state) = parse_logging_state_from_content(&content) {
            return state;
        }
    }

    std::env::var("SPAWN_AT_DEBUG")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

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

/// RAII Guard ensuring extension logging state is faithfully restored on drop or exit.
pub struct ExtensionLoggingGuard {
    /// Whether extension logging was already enabled when this tool started.
    pub initial_state: bool,
    /// Whether `restore` has already run, so dropping the guard does not restore twice.
    pub restored: bool,
}

impl ExtensionLoggingGuard {
    /// Snapshots current logging state and ensures logging is enabled for verification runs.
    pub fn init() -> Self {
        let initial_state = detect_current_logging_state();
        if !initial_state {
            // Enable logging so test traces are captured
            let _ = set_extension_logging(true);
        }
        Self {
            initial_state,
            restored: false,
        }
    }

    /// Restores the logging state recorded at startup.
    ///
    /// If logging was enabled before the tool ran, it remains enabled.
    /// If logging was enabled by this tool, it is restored to disabled.
    pub fn restore(&mut self) {
        if !self.restored {
            self.restored = true;
            let _ = set_extension_logging(self.initial_state);
        }
    }
}

impl Drop for ExtensionLoggingGuard {
    /// Restores extension logging state to its pre-session value upon drop.
    fn drop(&mut self) {
        self.restore();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tests parsing state transition events from extension logging streams.
    #[test]
    fn test_parse_logging_state_transitions() {
        let log_empty = "";
        assert_eq!(parse_logging_state_from_content(log_empty), None);

        let log_enabled =
            "[2026-10-10T20:39:42.259Z] +0.00ms | SET_LOGGING enabled=true\nother lines";
        assert_eq!(parse_logging_state_from_content(log_enabled), Some(true));

        let log_sequence = r#"
[2026-10-10T20:30:00.000Z] +0.00ms | SET_LOGGING enabled=true
[2026-10-10T20:35:00.000Z] +0.00ms | SET_LOGGING enabled=false
"#;
        assert_eq!(parse_logging_state_from_content(log_sequence), Some(false));

        let log_sequence_reversed = r#"
[2026-10-10T20:30:00.000Z] +0.00ms | SET_LOGGING enabled=false
[2026-10-10T20:35:00.000Z] +0.00ms | SET_LOGGING enabled=true
"#;
        assert_eq!(
            parse_logging_state_from_content(log_sequence_reversed),
            Some(true)
        );
    }
}
