//! Giving focus back to the window that had it before a test (the TUI's terminal), with a record
//! of every step so a run on a real desktop can say exactly what happened.
//!
//! The only command ever sent is `focus` to the recorded id; the window is never closed, moved,
//! resized, minimized or signalled. The result is a [`FocusReport`] that the TUI shows in the
//! Result panel whatever the outcome.
//!
//! Reasons a focus can fail on a real compositor, and what this does about each:
//! - the new window takes focus *after* the command (map animation, late activation): the focus
//!   is checked after a short settle and sent again, up to [`FocusTimings::attempts`] times;
//! - the command returns before the compositor applied it: the window list is polled for up to
//!   [`FocusTimings::wait`] after each send (no `--no-wait` guess);
//! - the terminal is not in the window list, or never had focus there: reported as "none found"
//!   with the number of windows listed, and nothing is sent;
//! - the recorded id is gone or is now a window of ours: nothing is sent, and the report says so;
//! - the command fails or is refused (focus-stealing prevention, a wrong id): the exit code and
//!   the first line of the message are in the report.
//!
//! What cannot be known without the real compositor: whether it honours the command (the report
//! shows it), and whether the list's `focused` flag agrees with what the keyboard really targets.

use super::owned::{Control, WindowInfo};
use std::time::{Duration, Instant};

/// Waits and counts of a focus return. The defaults are the real ones; tests shorten them.
#[derive(Debug, Clone, Copy)]
pub struct FocusTimings {
    /// Pause before the first look, so a window that is still taking focus has done so.
    pub settle: Duration,
    /// How many times `focus` is sent before giving up.
    pub attempts: usize,
    /// How long the window list is polled after each send.
    pub wait: Duration,
    /// Pause between two looks at the window list.
    pub poll: Duration,
}

impl Default for FocusTimings {
    /// 150 ms settle, 3 attempts, 600 ms wait after each, 50 ms poll.
    fn default() -> Self {
        Self {
            settle: Duration::from_millis(150),
            attempts: 3,
            wait: Duration::from_millis(600),
            poll: Duration::from_millis(50),
        }
    }
}

/// The outcome of one `focus` command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FocusCommand {
    /// Exit code of the command, `None` when it could not be run or timed out.
    pub exit_code: Option<i32>,
    /// First line of its error output, or the reason it could not be run.
    pub message: String,
}

/// A window as the report names it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowRef {
    /// Compositor id.
    pub id: u64,
    /// Class or application id, when listed.
    pub class: String,
    /// Title.
    pub title: String,
}

impl WindowRef {
    fn of(w: &WindowInfo) -> Self {
        Self {
            id: w.id,
            class: w
                .class
                .clone()
                .or_else(|| w.app_id.clone())
                .unwrap_or_default(),
            title: w.title.clone(),
        }
    }
    fn text(&self) -> String {
        format!(
            "id {} class '{}' title '{}'",
            self.id, self.class, self.title
        )
    }
}

/// Everything a focus return did and saw.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FocusReport {
    /// The window recorded as the terminal.
    pub recorded: Option<WindowRef>,
    /// Why there is none ("no window reported focused (12 listed)", "query failed: ...").
    pub recorded_note: String,
    /// Each `focus` sent, in order.
    pub sent: Vec<FocusCommand>,
    /// Why nothing was sent, when nothing was.
    pub not_sent: String,
    /// The window the list showed focused at the end (`None`: none, or the list failed).
    pub focused_after: Option<WindowRef>,
    /// Why the final look failed, if it did.
    pub after_error: String,
    /// The focused window at the end is the recorded one.
    pub matches: bool,
}

impl FocusReport {
    /// A report for a window that could not be recorded.
    pub fn none_found(note: impl Into<String>) -> Self {
        Self {
            recorded_note: note.into(),
            not_sent: "no terminal window recorded".into(),
            ..Default::default()
        }
    }

    /// The lines shown in the Result panel: the recorded terminal, the commands, the result.
    pub fn lines(&self) -> Vec<String> {
        let mut out = Vec::new();
        out.push(match &self.recorded {
            Some(r) => format!("terminal: {}", r.text()),
            None => format!("terminal: none found ({})", self.recorded_note),
        });
        out.push(if self.sent.is_empty() {
            format!(
                "focus command: not sent ({})",
                if self.not_sent.is_empty() {
                    "already focused"
                } else {
                    &self.not_sent
                }
            )
        } else {
            let each: Vec<String> = self
                .sent
                .iter()
                .enumerate()
                .map(|(i, c)| {
                    format!(
                        "#{} exit {}{}",
                        i + 1,
                        c.exit_code.map_or("none".to_string(), |c| c.to_string()),
                        if c.message.is_empty() {
                            String::new()
                        } else {
                            format!(" '{}'", c.message)
                        }
                    )
                })
                .collect();
            format!(
                "focus command: sent {}x: {}",
                self.sent.len(),
                each.join("; ")
            )
        });
        out.push(if !self.after_error.is_empty() {
            format!("focused afterwards: unknown ({})", self.after_error)
        } else {
            match &self.focused_after {
                Some(f) => format!(
                    "focused afterwards: {} - matches terminal: {}",
                    f.text(),
                    if self.matches { "yes" } else { "NO" }
                ),
                None => {
                    "focused afterwards: no window reported focused - matches terminal: NO".into()
                }
            }
        });
        out
    }
}

/// Why no window is recorded as the terminal, or which one is. `Err` carries the reason.
pub fn find_focused(control: &dyn Control) -> Result<WindowRef, String> {
    let list = control
        .windows()
        .map_err(|e| format!("window query failed: {e}"))?;
    list.iter()
        .find(|w| w.focused)
        .map(WindowRef::of)
        .ok_or_else(|| format!("no window reported focused ({} listed)", list.len()))
}

/// Puts focus back on `recorded` and reports every step. `forbidden` are the ids of windows the
/// runner owns: if the recorded id is one of them, nothing is sent.
pub fn return_focus(
    control: &dyn Control,
    recorded: Option<&WindowRef>,
    recorded_note: &str,
    forbidden: &[u64],
    timings: &FocusTimings,
) -> FocusReport {
    let mut report = FocusReport {
        recorded: recorded.cloned(),
        recorded_note: recorded_note.to_string(),
        ..Default::default()
    };
    let Some(rec) = recorded else {
        report.not_sent = "no terminal window recorded".into();
        return report;
    };
    if forbidden.contains(&rec.id) {
        report.not_sent = "the recorded window is one the runner owns; never targeted".into();
        return report;
    }
    let look = |report: &mut FocusReport| -> Option<bool> {
        match control.windows() {
            Err(e) => {
                report.after_error = format!("window query failed: {e}");
                None
            }
            Ok(list) => {
                report.after_error.clear();
                report.focused_after = list.iter().find(|w| w.focused).map(WindowRef::of);
                report.matches = report
                    .focused_after
                    .as_ref()
                    .is_some_and(|f| f.id == rec.id);
                if !list.iter().any(|w| w.id == rec.id) {
                    report.not_sent = "the recorded window is no longer listed".into();
                    return None;
                }
                Some(report.matches)
            }
        }
    };
    std::thread::sleep(timings.settle);
    for _ in 0..timings.attempts.max(1) {
        match look(&mut report) {
            None => return report,
            Some(true) => return report,
            Some(false) => {}
        }
        report.sent.push(control.focus_detail(rec.id));
        let deadline = Instant::now() + timings.wait;
        loop {
            if look(&mut report) == Some(true) {
                return report;
            }
            if report.focused_after.is_none() && !report.after_error.is_empty() {
                return report;
            }
            if Instant::now() >= deadline {
                break;
            }
            std::thread::sleep(timings.poll);
        }
    }
    report
}
