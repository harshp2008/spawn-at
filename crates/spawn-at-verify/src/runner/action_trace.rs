//! Capture of the extension's trace for the actions of a version 2 test, and the transform flash
//! check built on it (`docs/DESIGN_RUNNER_SETUP.md` section 5, `verify/README.md` "What is traced").
//!
//! One [`ActionTraceRun`] lives for one test. It turns the extension's timing log on once (the
//! same `ensure_enabled` every version 1 run makes), takes a clock anchor with the same bracketed
//! `SetLogging` calls a fixture `spawn` takes when the test has a `transform`, and then, around
//! each action:
//!
//! 1. **before**: reads the journal slice since the test started and remembers the newest `+N ms`
//!    in it (`floor_ms`): everything at or before it happened before the action;
//! 2. the action runs;
//! 3. **after**: waits a short settle (the extension writes its log from an idle callback), reads
//!    the slice again, keeps the lines of runner-owned windows after the floor, and for a command
//!    whose trace has a known last event (`transform`, `focus`) keeps looking until that event is
//!    in, up to [`TraceTimings::poll_limit`] more looks.
//!
//! The slice is whole seconds wide (`journalctl --since @N`); the floor, not the slice bounds,
//! is what separates one action from the next. Missing, empty or failed captures never fail a
//! test: they become the exact reason in [`TraceStatus`].

use super::engine::TestRunner;
use super::flash::{evaluate_action, FlashCapture};
use super::owned::SetupSession;
use crate::ext_logging::LoggingOutcome;
use crate::probe::EnvironmentInfo;
use crate::schema::{TestAction, TestDefinition};
use crate::trace::action::{
    build_action_trace, command_trace, is_query, joined_raw, latest_elapsed_ms, owned_events,
    within_action, ActionSpec, ActionTrace, OwnedWindow, TraceStatus,
};
use crate::trace::flash::{
    echo_times_ms, monotonic_ns, ClockAnchor, FlashInfo, FlashParams, MonoBracket,
    POST_REVEAL_WATCH_MS,
};
use crate::trace::TraceEvent;
use std::time::{Duration, Instant, SystemTime};

/// Waits of the capture. The defaults are the real ones; tests with a fake journal shorten them.
#[derive(Debug, Clone, Copy)]
pub struct TraceTimings {
    /// Pause between the end of an action and the first look at the journal: the extension writes
    /// its lines from an idle callback, a little after the event.
    pub settle: Duration,
    /// Pause between two looks while waiting for the last event of a command.
    pub poll: Duration,
    /// Number of further looks for the last event before giving up.
    pub poll_limit: usize,
    /// Margin added to the post-reveal watch of a `transform` so an event exactly
    /// [`POST_REVEAL_WATCH_MS`] after the reveal is read, not raced. `None` skips the wait.
    pub watch_margin: Option<Duration>,
    /// Refresh rate for the flash labels; `None` reads the display (a test sets it so that no
    /// display query is made).
    pub refresh_hz: Option<f64>,
}

impl Default for TraceTimings {
    /// 250 ms settle, 100 ms between looks, 10 further looks, 100 ms watch margin: the pacing
    /// the version 1 flash check already uses.
    fn default() -> Self {
        Self {
            settle: Duration::from_millis(250),
            poll: Duration::from_millis(100),
            poll_limit: 10,
            watch_margin: Some(Duration::from_millis(100)),
            refresh_hz: None,
        }
    }
}

/// Seconds since the epoch, for slice bounds.
fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// How the capture of a whole test stands.
enum Plan {
    /// Not asked for (`RunOptions::action_trace` off) or nothing to trace: no action trace at all.
    Off,
    /// No capture is possible for the whole test; every action gets this status.
    Unavailable(TraceStatus),
    /// Journal and timing log are usable.
    Active,
}

/// An action as seen before it ran.
pub(super) struct Probe {
    index: usize,
    command: String,
    target: Option<String>,
    window_id: Option<u64>,
    /// Newest `+N ms` in the journal before the action; `None` when the baseline read failed.
    floor_ms: Option<f64>,
    /// Why the baseline could not be read.
    baseline_error: Option<String>,
    /// `CLOCK_MONOTONIC` nanoseconds when the action started.
    start_ns: u64,
}

/// What is kept of one action for the rest of the test.
struct Recorded {
    trace: ActionTrace,
    /// The same lines as parsed events (absolute `+N ms`), for the flash check.
    events: Vec<TraceEvent>,
    floor_ms: Option<f64>,
    /// Start of the next action on the extension's clock; later lines are the next action's.
    ceiling_ms: Option<f64>,
    start_ns: u64,
}

/// The capture of one test.
pub(super) struct ActionTraceRun {
    plan: Plan,
    test_start_secs: u64,
    capture: Option<FlashCapture>,
    brackets: Vec<MonoBracket>,
    recorded: Vec<Recorded>,
    timings: TraceTimings,
    backend: String,
    /// Time spent on the capture, which the result's duration does not count.
    spent: Duration,
}

/// What the capture adds to the result.
pub(super) struct TraceOutcome {
    /// The lines of runner-owned windows, one per line; `None` when there are none.
    pub log_slice: Option<String>,
    /// The flash result of the first `transform`, with the actions' traces in `action_traces`.
    pub flash: FlashInfo,
    /// Time the capture took, to leave out of the result's duration.
    pub spent: Duration,
}

impl ActionTraceRun {
    /// A run that captures nothing.
    pub(super) fn off() -> Self {
        Self {
            plan: Plan::Off,
            test_start_secs: 0,
            capture: None,
            brackets: Vec::new(),
            recorded: Vec::new(),
            timings: TraceTimings::default(),
            backend: String::new(),
            spent: Duration::ZERO,
        }
    }

    /// Environment variables the fixtures of this test must be started with (the event directory
    /// of the flash check); empty unless the test has a `transform`.
    pub(super) fn fixture_env(&self) -> Vec<(&'static str, String)> {
        self.capture
            .as_ref()
            .map(FlashCapture::env_pairs)
            .unwrap_or_default()
    }

    /// Time spent so far.
    pub(super) fn spent(&self) -> Duration {
        self.spent
    }

    /// The window ids the runner owns right now, with their setup names.
    fn owned(session: &SetupSession) -> Vec<OwnedWindow> {
        session
            .named_ids()
            .into_iter()
            .map(|(name, id)| OwnedWindow { id, name })
            .collect()
    }

    /// Clock anchor from the brackets and the echoes in `slice`, when both exist.
    fn anchor(&self, slice: &str) -> Option<ClockAnchor> {
        if self.brackets.is_empty() {
            return None;
        }
        ClockAnchor::from_samples(&self.brackets, &echo_times_ms(slice)).ok()
    }
}

impl TestRunner {
    /// Decides how `test` is traced and, when it can be, turns the timing log on and takes the
    /// clock anchor. Called before the fixtures start, because their environment depends on it.
    pub(super) fn plan_action_trace(
        &self,
        test: &TestDefinition,
        env_info: &EnvironmentInfo,
    ) -> ActionTraceRun {
        let began = Instant::now();
        let mut run = ActionTraceRun::off();
        run.timings = self.trace_timings;
        run.backend = env_info.backend().to_string();
        // A runner whose compositor is faked, with no log source of its own, must not reach for
        // the live extension: `action_trace` is off unless a caller switched it on.
        if !self.options.action_trace || test.v2.setup.is_empty() {
            return run;
        }
        run.test_start_secs = now_secs();
        if self.log_source.name() == "noop" {
            run.plan = Plan::Unavailable(TraceStatus::NoExtension {
                backend: run.backend.clone(),
            });
            return run;
        }
        match self.logging.ensure_enabled(&*self.log_source) {
            LoggingOutcome::Failed(why) => {
                run.plan = Plan::Unavailable(TraceStatus::CaptureFailed { why });
                return run;
            }
            LoggingOutcome::Enabled | LoggingOutcome::Unneeded => run.plan = Plan::Active,
        }
        let has_transform = std::iter::once(&test.action)
            .chain(test.v2.then.iter())
            .any(|a| a.command == "transform");
        if has_transform {
            run.capture = FlashCapture::start(&*self.logging, self.options.wire_paint);
            run.brackets = run
                .capture
                .as_ref()
                .map(|c| c.brackets.clone())
                .unwrap_or_default();
        }
        run.spent += began.elapsed();
        run
    }

    /// Reads the journal slice since the test started.
    fn read_slice(&self, run: &ActionTraceRun) -> Result<String, String> {
        self.log_source
            .capture_slice(run.test_start_secs, now_secs())
    }

    /// Called just before an action runs: remembers where the journal stands.
    pub(super) fn trace_before(
        &self,
        run: &mut ActionTraceRun,
        index: usize,
        action: &TestAction,
        target: Option<String>,
        session: &SetupSession,
    ) -> Probe {
        let began = Instant::now();
        let window_id = target.as_deref().and_then(|n| session.window_id(n));
        let (floor_ms, baseline_error) =
            if matches!(run.plan, Plan::Active) && !is_query(&action.command) {
                match self.read_slice(run) {
                    Ok(slice) => (latest_elapsed_ms(&slice), None),
                    Err(e) => (None, Some(e)),
                }
            } else {
                (None, None)
            };
        // The previous action's lines end where this one's floor begins.
        if let Some(prev) = run.recorded.last_mut() {
            prev.ceiling_ms = floor_ms;
        }
        run.spent += began.elapsed();
        Probe {
            index,
            command: action.command.clone(),
            target,
            window_id,
            floor_ms,
            baseline_error,
            start_ns: monotonic_ns(),
        }
    }

    /// Called right after an action returned: waits for its lines and records its trace.
    pub(super) fn trace_after(
        &self,
        run: &mut ActionTraceRun,
        probe: Probe,
        session: &SetupSession,
    ) {
        if matches!(run.plan, Plan::Off) {
            return;
        }
        let began = Instant::now();
        let owned = ActionTraceRun::owned(session);
        let spec = ActionSpec {
            index: probe.index,
            command: &probe.command,
            target: probe.target.as_deref(),
            window_id: probe.window_id,
        };
        let mut events: Vec<TraceEvent> = Vec::new();
        let mut failure: Option<TraceStatus> = match &run.plan {
            Plan::Unavailable(s) => Some(s.clone()),
            _ => None,
        };
        if failure.is_none() && is_query(&probe.command) {
            // Reads never reach a window; nothing was logged and nothing was waited for.
            failure = Some(TraceStatus::NotApplicable);
        }
        let mut slice = String::new();
        if failure.is_none() {
            if let Some(why) = &probe.baseline_error {
                failure = Some(TraceStatus::CaptureFailed { why: why.clone() });
            }
        }
        if failure.is_none() {
            let info = command_trace(&probe.command);
            std::thread::sleep(run.timings.settle);
            let mut looks = 0;
            loop {
                match self.read_slice(run) {
                    Ok(s) => {
                        events = within_action(
                            owned_events(&s, &owned),
                            &probe.command,
                            probe.floor_ms,
                            None,
                        );
                        slice = s;
                    }
                    Err(e) => {
                        failure = Some(TraceStatus::CaptureFailed { why: e });
                        break;
                    }
                }
                let done = info.completes_with.is_empty()
                    || events
                        .iter()
                        .any(|e| info.completes_with.contains(&e.tag.as_str()));
                if done || looks >= run.timings.poll_limit {
                    break;
                }
                looks += 1;
                std::thread::sleep(run.timings.poll);
            }
        }
        let lead_in_ms = events.first().and_then(|first| {
            let a = run.anchor(&slice)?;
            let origin = (a.lo_ns + a.hi_ns) / 2.0;
            let lead = (origin + first.elapsed_ms * 1e6 - probe.start_ns as f64) / 1e6;
            // An event that precedes the start by more than the anchor allows is not this
            // action's; a smaller negative value is the anchor's own uncertainty.
            (lead >= -a.half_width_ms()).then_some(lead.max(0.0))
        });
        let trace = build_action_trace(&spec, &events, &owned, failure, lead_in_ms);
        run.recorded.push(Recorded {
            trace,
            events,
            floor_ms: probe.floor_ms,
            ceiling_ms: None,
            start_ns: probe.start_ns,
        });
        run.spent += began.elapsed();
    }

    /// Ends the capture: for a `transform` waits out the post-reveal watch and measures flash,
    /// then returns what goes into the result.
    pub(super) fn trace_finish(
        &self,
        mut run: ActionTraceRun,
        session: &SetupSession,
    ) -> TraceOutcome {
        let began = Instant::now();
        let mut flash = FlashInfo::default();
        if matches!(run.plan, Plan::Active) {
            let first_transform = run.recorded.iter().position(|r| {
                r.trace.command == "transform"
                    && r.trace.status == TraceStatus::Captured
                    && r.events.iter().any(|e| e.tag == "UNCLOAK")
            });
            if let Some(i) = first_transform {
                flash = self.transform_flash(&mut run, i, session);
            } else if run
                .recorded
                .iter()
                .any(|r| r.trace.command == "transform" && r.trace.status == TraceStatus::Captured)
            {
                flash
                    .flash_notes
                    .push("no UNCLOAK in the transform's trace: the reveal time is unknown".into());
            }
        }
        flash.action_traces = run.recorded.drain(..).map(|r| r.trace).collect();
        let raw = joined_raw(&flash.action_traces);
        run.spent += began.elapsed();
        TraceOutcome {
            log_slice: (!raw.is_empty()).then_some(raw),
            flash,
            spent: run.spent,
        }
    }

    /// Waits until [`POST_REVEAL_WATCH_MS`] after the reveal of recorded action `i`, re-reads its
    /// lines (a correction made during the wait is in them) and compares the fixture's paint
    /// events from the action's start with the reveal.
    fn transform_flash(
        &self,
        run: &mut ActionTraceRun,
        i: usize,
        session: &SetupSession,
    ) -> FlashInfo {
        let owned = ActionTraceRun::owned(session);
        let mut slice = self.read_slice(run).unwrap_or_default();
        if let (Some(margin), Some(anchor)) = (run.timings.watch_margin, run.anchor(&slice)) {
            let reveal_ms = run.recorded[i]
                .events
                .iter()
                .find(|e| e.tag == "UNCLOAK")
                .map(|e| e.elapsed_ms);
            if let Some(ms) = reveal_ms {
                let reveal_ns = (anchor.lo_ns + anchor.hi_ns) / 2.0 + ms * 1e6;
                let until = reveal_ns + POST_REVEAL_WATCH_MS * 1e6 + margin.as_nanos() as f64;
                let now = monotonic_ns() as f64;
                if until > now {
                    std::thread::sleep(Duration::from_nanos((until - now) as u64));
                }
                slice = self.read_slice(run).unwrap_or(slice);
            }
        }
        // Rebuild the action's lines from the later slice so a late correction shows.
        let rec = &run.recorded[i];
        let events = within_action(
            owned_events(&slice, &owned),
            &rec.trace.command,
            rec.floor_ms,
            rec.ceiling_ms,
        );
        let target_events: Vec<TraceEvent> = events
            .iter()
            .filter(|e| rec.trace.window_id.is_none() || e.window_id == rec.trace.window_id)
            .cloned()
            .collect();
        let pid = rec
            .trace
            .target
            .as_deref()
            .and_then(|n| session.child_pid(n))
            .map(u64::from);
        let (client_text, wire_text) = match &run.capture {
            Some(cap) => (
                cap.read_events(pid),
                cap.wire_enabled().then(|| cap.read_wire(pid)),
            ),
            None => (String::new(), None),
        };
        // A test with a fake journal passes the rate, so no display query is made.
        let refresh = run
            .timings
            .refresh_hz
            .map(|hz| (hz, false, String::new()))
            .unwrap_or_else(|| {
                let r = crate::display::refresh();
                (r.hz, r.assumed, r.source.to_string())
            });
        let params = FlashParams {
            refresh_hz: refresh.0,
            threshold_ms: self.options.flash_threshold_ms,
        };
        let mut info = evaluate_action(
            &target_events,
            &slice,
            &client_text,
            wire_text.as_deref(),
            &run.brackets,
            rec.start_ns,
            params,
        );
        if refresh.1 && info.flash_basis.is_some() {
            info.flash_notes
                .push(format!("refresh rate not read: {}", refresh.2));
        }
        // The trace shown is the later, fuller one.
        let spec_target = rec.trace.target.clone();
        let spec = ActionSpec {
            index: rec.trace.index,
            command: &rec.trace.command,
            target: spec_target.as_deref(),
            window_id: rec.trace.window_id,
        };
        let lead = rec.trace.lead_in_ms;
        let rebuilt = build_action_trace(&spec, &events, &owned, None, lead);
        let rec = &mut run.recorded[i];
        rec.events = events;
        rec.trace = rebuilt;
        info
    }
}
