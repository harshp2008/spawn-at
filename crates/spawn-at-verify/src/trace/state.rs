//! Whether a log slice holds an analysable trace for a window, and if not, why not.

use super::parser::TraceAnalyser;
use super::report::TraceReport;

/// Outcome of looking for one window's trace in a captured log slice.
#[derive(Debug, Clone)]
pub enum TraceState {
    /// The slice holds at least one event naming the window; the analysis of those events.
    Analysed(TraceReport),
    /// No event for the window was found; the reason, in one line.
    NotCaptured(String),
}

impl TraceState {
    /// The analysis, or `None` when no trace was captured.
    pub fn report(&self) -> Option<&TraceReport> {
        match self {
            TraceState::Analysed(r) => Some(r),
            TraceState::NotCaptured(_) => None,
        }
    }

    /// Why no trace was captured, or `None` when one was.
    pub fn reason(&self) -> Option<&str> {
        match self {
            TraceState::Analysed(_) => None,
            TraceState::NotCaptured(why) => Some(why),
        }
    }
}

/// Analyses `slice` for `window_id` (all windows when `None`) and reports whether any event
/// naming a window survived the scoping.
///
/// An empty report is not a clean one: with no event for the window the analyser has nothing
/// to find violations in, so that case is [`TraceState::NotCaptured`] rather than a pass.
/// `note` is a run-time fact recorded when the slice was captured (a no-op log source, a
/// timing log that could not be enabled); it is appended to the reason.
pub fn assess_trace(slice: Option<&str>, window_id: Option<u64>, note: Option<&str>) -> TraceState {
    let why = |base: String| match note {
        Some(n) => TraceState::NotCaptured(format!("{}; {}", base, n)),
        None => TraceState::NotCaptured(base),
    };
    let Some(slice) = slice else {
        return why("no log slice was captured".into());
    };
    if slice.trim().is_empty() {
        return why("log slice is empty (the extension's timing log was off, or nothing was logged in the run's time window)".into());
    }
    let mut analyser = TraceAnalyser::new();
    if let Some(id) = window_id {
        analyser = analyser.with_target_window_id(id);
    }
    let report = analyser.analyse(slice);
    if report.events.iter().any(|e| e.window_id.is_some()) {
        return TraceState::Analysed(report);
    }
    let lines = slice.lines().filter(|l| !l.trim().is_empty()).count();
    match window_id {
        Some(id) => why(format!(
            "log slice has {} line(s) but no event for window {}",
            lines, id
        )),
        None => why(format!(
            "log slice has {} line(s) but no event naming a window",
            lines
        )),
    }
}
