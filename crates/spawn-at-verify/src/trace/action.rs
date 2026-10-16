//! The extension's trace of one action of a version 2 test (a command on an existing window):
//! what each spawn-at command makes the extension log, which of the journal's lines belong to the
//! windows the runner launched itself, and the timeline built from them.
//!
//! Everything here is pure: no journal, no clock, no process. The runner (`runner/action_trace.rs`)
//! decides when to read the journal; this module decides what a slice means.
//!
//! The inventory in [`command_trace`] was read from the extension source
//! (`assets/gnome/modern/extension.js`, the same call sites exist in `legacy/`); the table with
//! line numbers is in `verify/README.md`. Only lines that carry `win=[id=...]` of a window in the
//! caller's `owned` list are ever kept, so another window's lines (the owner's browser, editor or
//! terminal) are dropped before anything is stored, shown or written to a report.

use super::models::TraceEvent;
use super::parser::TraceAnalyser;
use serde::{Deserialize, Serialize};

/// How much of a command the extension logs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TraceSupport {
    /// A timeline with several events carrying the window id (`transform`, `focus`, `spawn`).
    Usable,
    /// Lines only in some cases (`minimize`, `restore`) or for unknown commands.
    Partial,
    /// No line for the target window, except `ANCHOR_RELEASED` when an anchor was armed on it
    /// (`close`, `defocus`, `maximize`) or nothing at all (queries).
    Nothing,
}

/// What the extension logs for one command, as read from its source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandTrace {
    /// How much is logged.
    pub support: TraceSupport,
    /// Event tags that end the command's trace: the runner stops waiting for more lines once one
    /// of them is in the slice. Empty when no line is certain to appear.
    pub completes_with: &'static [&'static str],
    /// One sentence for the UI and the report: which events the extension writes.
    pub logs: &'static str,
}

/// Looks up what the extension logs for `command` (the `spawn-at` subcommand name).
///
/// The extension logs nothing at the start or end of a D-Bus call itself; every line below comes
/// from a `logTime` / `_logActorTrace` call inside the method the command ends up in.
pub fn command_trace(command: &str) -> CommandTrace {
    match command {
        // `transform` is one `ExecuteBatch`; its steps log BATCH_STEP, SNAPSHOT_*, CLOAK,
        // SET_SIZE_REQUEST, READY_WAIT_*, COMMIT_*, APPLY_ANCHOR, POSITION_SET, UNCLOAK*,
        // ANCHOR_ARMED, all with the window id. The call returns before the batch runs, so the
        // trace ends after the CLI has exited.
        "transform" | "spawn" => CommandTrace {
            support: TraceSupport::Usable,
            completes_with: &["UNCLOAK_4D_REVEAL"],
            logs: "BATCH_STEP, SNAPSHOT_*, CLOAK, SET_SIZE_REQUEST, READY_WAIT_*, COMMIT_*, \
                   POSITION_SET, UNCLOAK, UNCLOAK_4D_REVEAL, ANCHOR_ARMED (all with the window id)",
        },
        // `FocusWindow` logs RAISE then ACTIVATE for the target; with a batch pending on the window
        // it logs FOCUS_DEFERRED or FOCUS_BATCH_ALREADY_APPLIED instead.
        "focus" => CommandTrace {
            support: TraceSupport::Usable,
            completes_with: &["ACTIVATE", "FOCUS_DEFERRED", "FOCUS_BATCH_ALREADY_APPLIED"],
            logs: "RAISE and ACTIVATE for the window (no cloak phase)",
        },
        // `DefocusWindow` logs nothing in 'desktop' mode; in 'window' mode and the default it logs
        // RAISE and ACTIVATE for the window that RECEIVES focus, never for the target.
        "defocus" => CommandTrace {
            support: TraceSupport::Nothing,
            completes_with: &[],
            logs: "nothing for the target window (RAISE/ACTIVATE are logged for the window that receives focus)",
        },
        // `SetWindowState`: maximize / unmaximize call Meta directly with no log line.
        "maximize" | "unmaximize" => CommandTrace {
            support: TraceSupport::Nothing,
            completes_with: &[],
            logs: "nothing (ANCHOR_RELEASED only if the window had an armed anchor)",
        },
        "minimize" | "unminimize" | "restore" => CommandTrace {
            support: TraceSupport::Partial,
            completes_with: &[],
            logs: "MINIMIZE / UNMINIMIZE only (restoring a maximized window logs nothing)",
        },
        // `CloseWindow` disarms the anchor (ANCHOR_RELEASED reason=EXTERNAL_CLOSE, only when one
        // was armed) and calls `window.delete()`, which logs nothing.
        "close" => CommandTrace {
            support: TraceSupport::Nothing,
            completes_with: &[],
            logs: "nothing (ANCHOR_RELEASED reason=EXTERNAL_CLOSE only if the window had an armed anchor)",
        },
        // GetWindows, GetLayout, GetWorkareas, GetCursor, GetPointer: no log call at all.
        "query" => CommandTrace {
            support: TraceSupport::Nothing,
            completes_with: &[],
            logs: "nothing",
        },
        _ => CommandTrace {
            support: TraceSupport::Partial,
            completes_with: &[],
            logs: "unknown command: whatever the extension logs for the window",
        },
    }
}

/// Whether a command is a query, which never reaches a window and is not traced at all.
pub fn is_query(command: &str) -> bool {
    command == "query"
}

/// Tags the extension logs when a window is created and mapped. A runner-owned window logs them
/// during setup, so for an action on an existing window they belong to the setup, not the action.
pub const MAP_TIME_TAGS: &[&str] = &["WINDOW_CREATED", "ACTOR_MAP", "REJECT_UNMATCHED"];

/// A window the runner launched itself: the only windows whose lines may be kept.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnedWindow {
    /// The compositor window id recorded at setup.
    pub id: u64,
    /// The setup name (`w` in `--id ${w}`).
    pub name: String,
}

/// Why a timeline is empty, or that it is not. The strings are what the TUI and the report show.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "kebab-case")]
pub enum TraceStatus {
    /// At least one line of an owned window was found.
    Captured,
    /// The extension logs nothing for this command (see [`command_trace`]).
    NotApplicable,
    /// The log source is not the GNOME extension's journal (Sway, X11, a non-GNOME session).
    NoExtension {
        /// The backend that is in use, for example `sway`.
        backend: String,
    },
    /// The journal could not be read or the extension's timing log could not be enabled.
    CaptureFailed {
        /// What failed, in one sentence.
        why: String,
    },
    /// The capture worked but no line carried an owned window's id.
    NoLines,
}

impl TraceStatus {
    /// The reason a timeline is missing, or `None` for [`TraceStatus::Captured`].
    pub fn reason(&self) -> Option<String> {
        match self {
            TraceStatus::Captured => None,
            TraceStatus::NotApplicable => {
                Some("not applicable: the extension logs nothing for this command".into())
            }
            TraceStatus::NoExtension { backend } => {
                Some(format!("no extension on this backend ({backend})"))
            }
            TraceStatus::CaptureFailed { why } => Some(format!("capture failed: {why}")),
            TraceStatus::NoLines => Some("no lines for this window in the slice".into()),
        }
    }
}

/// One line of the timeline of an action.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TimelineEvent {
    /// Milliseconds after the first line of this action (the extension's own clock).
    pub offset_ms: f64,
    /// Event tag (`CLOAK`, `RAISE`, ...).
    pub tag: String,
    /// Window id the line names.
    pub window_id: u64,
    /// Setup name of that window.
    pub window: String,
    /// Everything after the window block: the key fields (`frame=(...)`, `requestedSize=(...)`).
    pub detail: String,
    /// The journal line as logged, trimmed.
    pub raw: String,
}

/// The trace of one action of a test.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActionTrace {
    /// Position of the action in the test: 0 for `[action]`, then 1.. for each `[[then]]`.
    pub index: usize,
    /// The `spawn-at` subcommand (`transform`, `focus`, ...).
    pub command: String,
    /// Setup name of the target (`w` of `--id ${w}`), if the action names one.
    pub target: Option<String>,
    /// The target's window id, once recorded.
    pub window_id: Option<u64>,
    /// Whether there is a timeline and, if not, why.
    pub status: TraceStatus,
    /// What the extension logs for this command (inventory sentence).
    pub logs: String,
    /// The timeline, oldest first; empty unless `status` is `Captured`.
    pub events: Vec<TimelineEvent>,
    /// Milliseconds from the start of the action to its first line, when the extension's clock
    /// could be placed on the runner's (a clock anchor); absent otherwise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lead_in_ms: Option<f64>,
    /// Milliseconds from `CLOAK` to `UNCLOAK_4D_REVEAL` of the target, when both were logged.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cloak_to_reveal_ms: Option<f64>,
    /// Whether the snapshot that covers the window while it is cloaked was `taken` or `skipped:
    /// <reason>`; absent when the action has no snapshot step.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snapshot: Option<String>,
}

impl ActionTrace {
    /// True when the timeline holds a `CLOAK` of the target: the only case in which a cloak
    /// duration, a budget and a flash check mean anything.
    pub fn has_cloak_phase(&self) -> bool {
        self.events.iter().any(|e| e.tag == "CLOAK")
    }

    /// The timeline's raw lines, one per line.
    pub fn raw_lines(&self) -> String {
        self.events
            .iter()
            .map(|e| e.raw.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// The parsed events of `slice` that name a window in `owned`, in log order.
///
/// A line is kept only when its `win=[id=N ...]` block names an owned id; lines with no window
/// block (`SET_LOGGING`, `ARM_SPAWN`) and lines of any other window are dropped.
pub fn owned_events(slice: &str, owned: &[OwnedWindow]) -> Vec<TraceEvent> {
    TraceAnalyser::new()
        .parse_events(slice)
        .into_iter()
        .filter(|e| {
            e.window_id
                .is_some_and(|id| owned.iter().any(|w| w.id == id))
        })
        .collect()
}

/// The largest `+N ms` of any line in `slice`, whatever window it names. Used to remember where
/// the log stood before an action, so that earlier lines are not attributed to it.
pub fn latest_elapsed_ms(slice: &str) -> Option<f64> {
    TraceAnalyser::new()
        .parse_events(slice)
        .iter()
        .map(|e| e.elapsed_ms)
        .fold(None, |acc, ms| Some(acc.map_or(ms, |a: f64| a.max(ms))))
}

/// Drops the lines the action cannot have produced: those at or before `floor_ms` (the log's state
/// before the action), those after `ceiling_ms` (the start of the next action), and, unless the
/// action is a `spawn`, the window-creation lines of the setup ([`MAP_TIME_TAGS`]).
pub fn within_action(
    events: Vec<TraceEvent>,
    command: &str,
    floor_ms: Option<f64>,
    ceiling_ms: Option<f64>,
) -> Vec<TraceEvent> {
    events
        .into_iter()
        .filter(|e| floor_ms.is_none_or(|f| e.elapsed_ms > f))
        .filter(|e| ceiling_ms.is_none_or(|c| e.elapsed_ms <= c))
        .filter(|e| command == "spawn" || !MAP_TIME_TAGS.contains(&e.tag.as_str()))
        .collect()
}

/// What an action is, for [`build_action_trace`].
pub struct ActionSpec<'a> {
    /// Position in the test.
    pub index: usize,
    /// The subcommand.
    pub command: &'a str,
    /// Setup name of the target.
    pub target: Option<&'a str>,
    /// Recorded id of the target.
    pub window_id: Option<u64>,
}

/// Builds the trace of one action from its events (already limited by [`owned_events`] and
/// [`within_action`]).
///
/// `failure` is a fact about the capture (no extension on this backend, journal error); it wins
/// over everything because without a capture the absence of lines says nothing. Otherwise an
/// empty `events` becomes `NotApplicable` for a command that logs nothing and `NoLines` for the
/// rest.
pub fn build_action_trace(
    spec: &ActionSpec,
    events: &[TraceEvent],
    owned: &[OwnedWindow],
    failure: Option<TraceStatus>,
    lead_in_ms: Option<f64>,
) -> ActionTrace {
    let info = command_trace(spec.command);
    let base = ActionTrace {
        index: spec.index,
        command: spec.command.to_string(),
        target: spec.target.map(str::to_string),
        window_id: spec.window_id,
        status: TraceStatus::Captured,
        logs: info.logs.to_string(),
        events: Vec::new(),
        lead_in_ms: None,
        cloak_to_reveal_ms: None,
        snapshot: None,
    };
    if let Some(status) = failure {
        return ActionTrace { status, ..base };
    }
    if events.is_empty() {
        let status = if info.support == TraceSupport::Nothing {
            TraceStatus::NotApplicable
        } else {
            TraceStatus::NoLines
        };
        return ActionTrace { status, ..base };
    }
    let origin = events[0].elapsed_ms;
    let name_of = |id: u64| {
        owned
            .iter()
            .find(|w| w.id == id)
            .map(|w| w.name.clone())
            .unwrap_or_default()
    };
    let timeline: Vec<TimelineEvent> = events
        .iter()
        .filter_map(|e| {
            let id = e.window_id?;
            Some(TimelineEvent {
                offset_ms: round_us((e.elapsed_ms - origin).max(0.0)),
                tag: e.tag.clone(),
                window_id: id,
                window: name_of(id),
                detail: e.extra.trim().to_string(),
                raw: e.raw.trim().to_string(),
            })
        })
        .collect();
    let target_events: Vec<&TraceEvent> = events
        .iter()
        .filter(|e| spec.window_id.is_none() || e.window_id == spec.window_id)
        .collect();
    ActionTrace {
        events: timeline,
        lead_in_ms,
        cloak_to_reveal_ms: cloak_to_reveal_ms(&target_events),
        snapshot: snapshot_state(&target_events),
        ..base
    }
}

/// Milliseconds from the first `CLOAK` to the first `UNCLOAK_4D_REVEAL` after it. The window is
/// hidden (and the snapshot, if one was taken, is what the user sees) between the two.
pub fn cloak_to_reveal_ms(events: &[&TraceEvent]) -> Option<f64> {
    let cloak = events.iter().position(|e| e.tag == "CLOAK")?;
    let reveal = events[cloak..]
        .iter()
        .find(|e| e.tag == "UNCLOAK_4D_REVEAL")?;
    Some(round_us(
        (reveal.elapsed_ms - events[cloak].elapsed_ms).max(0.0),
    ))
}

/// `ms` rounded to a microsecond. The extension prints `+N.NNms` with two decimals, so a
/// difference of two stamps is exact to 10 microseconds; the rounding removes the binary noise
/// of the subtraction (0.5700000002980232 for 0.57), which would otherwise make a timeline
/// differ from itself after a JSON round trip.
fn round_us(ms: f64) -> f64 {
    (ms * 1000.0).round() / 1000.0
}

/// `taken` for a `SNAPSHOT_CREATED`, `skipped: <reason>` for a `SNAPSHOT_SKIPPED reason="..."`.
fn snapshot_state(events: &[&TraceEvent]) -> Option<String> {
    for e in events {
        match e.tag.as_str() {
            "SNAPSHOT_CREATED" => return Some("taken".into()),
            "SNAPSHOT_SKIPPED" => {
                let reason = e
                    .extra
                    .split('"')
                    .nth(1)
                    .filter(|r| !r.is_empty())
                    .unwrap_or("no reason logged");
                return Some(format!("skipped: {reason}"));
            }
            _ => {}
        }
    }
    None
}

/// The raw lines of every action's timeline, in action order: the slice a result carries, which
/// holds nothing but lines of owned windows.
pub fn joined_raw(traces: &[ActionTrace]) -> String {
    traces
        .iter()
        .map(ActionTrace::raw_lines)
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

/// The caveat shown wherever a flash result for a `transform` is shown. The extension's log and
/// the fixture's paint events are the only evidence: no pixel is ever captured.
pub const FLASH_CAVEAT: &str = "informational: a clean result means no late paint was found in the runs tested, not a guarantee; 'possible' flashes have a known noise level; no pixels are captured";

/// Why a command has no flash analysis: it never hides the window, so there is no cloak to end.
pub const NO_CLOAK_NOTE: &str = "no flash analysis: this command has no cloak phase";

impl ActionTrace {
    /// The cloak duration as shown in the Cloak row: the time from `CLOAK` to
    /// `UNCLOAK_4D_REVEAL`, or `n/a` with the reason it does not apply.
    pub fn cloak_text(&self) -> String {
        match self.cloak_to_reveal_ms {
            Some(ms) => format!("{ms:.1} ms (CLOAK to UNCLOAK_4D_REVEAL)"),
            None if self.has_cloak_phase() => "n/a (CLOAK logged, no UNCLOAK_4D_REVEAL)".into(),
            None => format!("n/a (no cloak phase for {})", self.command),
        }
    }

    /// One line for the Trace row: the event count and window, or the reason there is no timeline.
    pub fn headline(&self) -> String {
        let what = match &self.target {
            Some(t) => format!("{} on '{}'", self.command, t),
            None => self.command.clone(),
        };
        match self.status.reason() {
            Some(why) => format!("#{} {}: {}", self.index, what, why),
            None => format!("#{} {}: {} event(s)", self.index, what, self.events.len()),
        }
    }
}

/// Why the cloak checks are not run for an action that has lines but no cloak phase (`focus`,
/// `minimize`, ...).
pub const NO_CLOAK_CHECKS: &str = "checks not applied: this command has no cloak phase";

/// What the lifecycle checks found in one action's timeline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActionVerdict {
    /// The checks ran and found nothing.
    Clean,
    /// The checks ran; each entry is one problem, in plain words.
    Violations(Vec<String>),
    /// The checks cannot be applied; the reason is shown instead of a verdict.
    NotApplied(String),
}

impl ActionTrace {
    /// The window's process id, read from the `pid=N` of the captured lines. `None` when no line
    /// has one, or the extension printed `-1` (unknown): nothing is made up.
    pub fn pid(&self) -> Option<u64> {
        self.events.iter().find_map(|e| {
            let rest = e.raw.split("pid=").nth(1)?;
            let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
            digits.parse().ok()
        })
    }

    /// `#0 transform on 'w'`: which action this is.
    pub fn label(&self) -> String {
        match &self.target {
            Some(t) => format!("#{} {} on '{}'", self.index, self.command, t),
            None => format!("#{} {}", self.index, self.command),
        }
    }

    /// Applies the same kind of checks the spawn analysis makes to a transform's lines: the cloak
    /// comes before the reveal, there is one reveal, it comes after the last size and position
    /// step, and nothing changes size or position after it. Only an action with a cloak phase is
    /// checked; for the others the reason is given, never a fake "clean".
    pub fn verdict(&self) -> ActionVerdict {
        if let Some(why) = self.status.reason() {
            return ActionVerdict::NotApplied(why);
        }
        if !self.has_cloak_phase() {
            return ActionVerdict::NotApplied(NO_CLOAK_CHECKS.into());
        }
        let tags: Vec<&str> = self.events.iter().map(|e| e.tag.as_str()).collect();
        let first = |t: &str| tags.iter().position(|x| *x == t);
        let mut problems = Vec::new();
        let reveals = tags.iter().filter(|t| **t == "UNCLOAK").count();
        if reveals > 1 {
            problems.push(format!("{reveals} reveals (UNCLOAK) in one action"));
        }
        match (first("CLOAK"), first("UNCLOAK")) {
            (Some(c), Some(u)) if c > u => problems.push("revealed before it was cloaked".into()),
            (Some(_), None) => problems.push("cloaked but never revealed".into()),
            _ => {}
        }
        if let Some(u) = first("UNCLOAK") {
            for (i, t) in tags.iter().enumerate() {
                let step = matches!(
                    *t,
                    "AFTER_SET_SIZE"
                        | "AFTER_SET_POSITION_ANCHORED"
                        | "POSITION_SET"
                        | "COMMIT_RESOLVED"
                        | "SET_SIZE_REQUEST"
                );
                if step && i > u {
                    problems.push(format!("{t} after the reveal"));
                }
                if matches!(*t, "ANCHOR_CORRECTION" | "LATE_RESIZE_CORRECTED") && i > u {
                    problems.push(format!("{t}: size or position changed after the reveal"));
                }
            }
        }
        if problems.is_empty() {
            ActionVerdict::Clean
        } else {
            ActionVerdict::Violations(problems)
        }
    }

    /// The Trace row text: the verdict (or the reason there is none) and the event count, with
    /// the action label when `labelled` (more than one action ran).
    pub fn trace_text(&self, labelled: bool) -> String {
        let n = self.events.len();
        let count = if n == 1 {
            "1 event".to_string()
        } else {
            format!("{n} events")
        };
        let body = match self.verdict() {
            ActionVerdict::Clean => format!("✔ clean, no violations · {count}"),
            ActionVerdict::Violations(v) => format!(
                "✖ {}{} · {count}",
                v[0],
                if v.len() > 1 {
                    format!(" (+{} more)", v.len() - 1)
                } else {
                    String::new()
                }
            ),
            ActionVerdict::NotApplied(why) if n > 0 => format!("{why} · {count}"),
            ActionVerdict::NotApplied(why) => why,
        };
        if labelled {
            format!("{}: {}", self.label(), body)
        } else {
            body
        }
    }
}

impl ActionTrace {
    /// The Result panel line, worded like the spawn one (`Trace: 0 violations`) plus the event
    /// count; the action label only when more than one action ran.
    pub fn panel_text(&self, labelled: bool) -> String {
        let n = self.events.len();
        let tail = if n > 0 {
            format!(" · {n} event{}", if n == 1 { "" } else { "s" })
        } else {
            String::new()
        };
        let body = match self.verdict() {
            ActionVerdict::Clean => format!("Trace: 0 violations{tail}"),
            ActionVerdict::Violations(v) => format!(
                "Trace: {} violation{}{tail}",
                v.len(),
                if v.len() == 1 { "" } else { "s" }
            ),
            ActionVerdict::NotApplied(why) => format!("Trace: {why}{tail}"),
        };
        if labelled {
            format!("{} · {}", self.label(), body)
        } else {
            body
        }
    }
}
