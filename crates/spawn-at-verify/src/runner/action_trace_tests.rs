//! Tests of the per-action trace capture of version 2 tests (`action_trace.rs`) with a fake journal
//! that is written to while the fake actions run, a fake extension switch that counts its calls,
//! and a stand-in fixture script that writes the fixture's paint events when told to. Nothing here
//! reads the user journal, calls `gdbus`, queries the display or starts `spawn-at`.

use super::action_trace::TraceTimings;
use super::models::{AutoStatus, FinalStatus, RunOptions, TestResult};
use super::owned::Control;
use super::test_support::{fast, out, Env, Fake, FakeActions};
use super::TestRunner;
use crate::ext_logging::{LoggingControl, LoggingOutcome};
use crate::loader::TestRegistry;
use crate::log_source::LogSource;
use crate::oracle::Rect;
use crate::preflight::{PreflightReport, PreflightVerdict};
use crate::probe::EnvironmentInfo;
use crate::schema::{v2, TestDefinition};
use crate::trace::action::TraceStatus;
use crate::trace::flash::{monotonic_ns, MonoBracket};
use crate::tracker::WindowTracker;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// What the fake journal and extension switch recorded.
#[derive(Default)]
struct JState {
    lines: Vec<String>,
    /// `(since, until)` of every `capture_slice` call.
    reads: Vec<(u64, u64)>,
    enable_calls: usize,
    /// `n` of every `anchor_brackets` call.
    bracket_calls: Vec<usize>,
    fail_reads: Option<String>,
    enable_error: Option<String>,
}

/// In-memory journal plus logging switch. Lines are stamped with the real monotonic clock
/// (milliseconds since an origin one second before the journal was made), so the stamps of lines
/// written while an action runs have the ordering and spacing the code under test sees.
#[derive(Clone)]
struct Journal {
    origin_ns: u64,
    st: Arc<Mutex<JState>>,
}

impl Journal {
    fn new() -> Self {
        Self {
            origin_ns: monotonic_ns() - 1_000_000_000,
            st: Arc::default(),
        }
    }

    /// Current stamp in ms, rounded to the 10 microseconds the extension prints.
    fn stamp(&self) -> f64 {
        ((monotonic_ns() - self.origin_ns) as f64 / 1e4).round() / 100.0
    }

    /// Monotonic nanoseconds a stamp stands for.
    fn mono_of(&self, stamp_ms: f64) -> u64 {
        self.origin_ns + (stamp_ms * 1e6) as u64
    }

    /// Appends a line for window `id` stamped now; returns its monotonic time.
    fn log(&self, tag: &str, id: u64, class: &str, extra: &str) -> u64 {
        let ms = self.stamp();
        let line = format!(
            "[spawn-at-time] +{ms:.2}ms | {tag} win=[id={id} pid=1 class=\"{class}\" app=\"\" type=NORMAL proto=Wayland] {extra}"
        );
        self.st.lock().unwrap().lines.push(line.trim().to_string());
        self.mono_of(ms)
    }

    fn reads(&self) -> Vec<(u64, u64)> {
        self.st.lock().unwrap().reads.clone()
    }
    fn enable_calls(&self) -> usize {
        self.st.lock().unwrap().enable_calls
    }
    fn bracket_calls(&self) -> Vec<usize> {
        self.st.lock().unwrap().bracket_calls.clone()
    }
    fn fail_reads(&self, why: &str) {
        self.st.lock().unwrap().fail_reads = Some(why.into());
    }
    fn fail_enable(&self, why: &str) {
        self.st.lock().unwrap().enable_error = Some(why.into());
    }
}

impl LogSource for Journal {
    fn name(&self) -> &'static str {
        "fake-journal"
    }
    fn capture_slice(&self, since: u64, until: u64) -> Result<String, String> {
        let mut st = self.st.lock().unwrap();
        st.reads.push((since, until));
        if let Some(e) = &st.fail_reads {
            return Err(format!("journalctl exited with error: {e}"));
        }
        Ok(st.lines.join("\n"))
    }
}

impl LoggingControl for Journal {
    fn ensure_enabled(&self, _log: &dyn LogSource) -> LoggingOutcome {
        let mut st = self.st.lock().unwrap();
        st.enable_calls += 1;
        if let Some(e) = &st.enable_error {
            return LoggingOutcome::Failed(format!(
                "extension timing log could not be enabled: {e}"
            ));
        }
        drop(st);
        let ms = self.stamp();
        self.st.lock().unwrap().lines.push(format!(
            "[spawn-at-time] +{ms:.2}ms | SET_LOGGING enabled=true"
        ));
        LoggingOutcome::Enabled
    }

    /// Each call writes its echo now and is bracketed by 0.1 ms either side, which is how the real
    /// bracketed `gdbus` call bounds the echo.
    fn anchor_brackets(&self, n: usize) -> Vec<MonoBracket> {
        self.st.lock().unwrap().bracket_calls.push(n);
        let mut out = Vec::new();
        for _ in 0..n {
            let ms = self.stamp();
            let t = self.mono_of(ms);
            self.st.lock().unwrap().lines.push(format!(
                "[spawn-at-time] +{ms:.2}ms | SET_LOGGING enabled=true"
            ));
            out.push(MonoBracket {
                before_ns: t - 100_000,
                after_ns: t + 100_000,
            });
        }
        out
    }
}

/// A fixture stand-in that registers itself like the normal one and, whenever the file `emit`
/// appears next to it, appends it to `fixture-<pid>.events` in `SPAWN_AT_FIXTURE_EVENT_DIR` and
/// deletes it (so a test can wait until the paint events are written).
const SCRIPT: &str = r#"#!/bin/sh
D="$(dirname "$0")"
title=""
while [ $# -gt 0 ]; do
  case "$1" in --title) title="$2"; shift ;; esac
  shift
done
printf '%s %s\n' "$$" "$title" >> "$D/state"
while :; do
  if [ -n "$SPAWN_AT_FIXTURE_EVENT_DIR" ] && [ -f "$D/emit" ]; then
    cat "$D/emit" >> "$SPAWN_AT_FIXTURE_EVENT_DIR/fixture-$$.events"
    rm -f "$D/emit"
  fi
  sleep 0.02
done
"#;

const HEAD: &str = r#"
schema_version = 2
id = "transforms/example"
category = "transforms"
title = "example"
description = "example"
tags = ["moves-focus"]
kind = "auto"
"#;

fn spec(body: &str) -> TestDefinition {
    let t: TestDefinition = toml::from_str(&format!("{HEAD}\n{body}")).expect("spec parses");
    v2::validate(&t).expect("the spec passes the loader rules");
    t
}

const FOCUS_A: &str = r#"
[[setup]]
name = "a"
args = ["--width", "400", "--height", "300"]
[action]
command = "focus"
args = ["--id", "${a}"]
[expect]
exit_code = 0
"#;

const FOCUS_AB: &str = r#"
[[setup]]
name = "a"
args = ["--width", "400", "--height", "300"]
[[setup]]
name = "b"
args = ["--width", "400", "--height", "300"]
[action]
command = "focus"
args = ["--id", "${a}"]
[[then]]
command = "focus"
args = ["--id", "${b}"]
[expect]
exit_code = 0
"#;

const TRANSFORM_W: &str = r#"
[[setup]]
name = "w"
args = ["--width", "400", "--height", "300"]
[action]
command = "transform"
args = ["--id", "${w}", "--size", "300", "200"]
[expect]
exit_code = 0
"#;

const CLOSE_A: &str = r#"
[[setup]]
name = "a"
args = ["--width", "400", "--height", "300"]
[action]
command = "close"
args = ["--id", "${a}"]
[expect]
exit_code = 0
"#;

const QUERY_A: &str = r#"
[[setup]]
name = "a"
args = ["--width", "400", "--height", "300"]
[action]
command = "query"
args = ["windows"]
[expect]
exit_code = 0
"#;

fn env_info() -> EnvironmentInfo {
    let wa = Rect::new(0, 0, 1920, 1080);
    EnvironmentInfo {
        session_type: "wayland".into(),
        desktop: "gnome".into(),
        gnome_version: None,
        monitors_count: 1,
        primary_workarea: Some(wa),
        independent_workarea: Some(wa),
    }
}

fn preflight() -> PreflightReport {
    PreflightReport {
        detected_commit: None,
        binary_version_output: String::new(),
        repo_head: None,
        repo_dirty: false,
        is_dirty_build: false,
        verdict: PreflightVerdict::Pass,
    }
}

fn quick() -> TraceTimings {
    TraceTimings {
        settle: Duration::from_millis(10),
        poll: Duration::from_millis(10),
        poll_limit: 5,
        watch_margin: None,
        refresh_hz: Some(60.0),
    }
}

struct Rig {
    env: Env,
    fake: Arc<Fake>,
    journal: Journal,
    tracker: WindowTracker,
}

impl Rig {
    fn new() -> Rig {
        let env = Env::new("normal");
        std::fs::write(env.fixture(), SCRIPT).unwrap();
        let fake = Fake::new(&env);
        Rig {
            env,
            fake,
            journal: Journal::new(),
            tracker: WindowTracker::new_with_preexisting(&[]),
        }
    }

    fn options(&self, action_trace: bool) -> RunOptions {
        RunOptions {
            bin_path: self.env.dir.join("spawn-at"),
            action_trace,
            wire_paint: false,
            ..Default::default()
        }
    }

    fn run_with(
        &mut self,
        runner: TestRunner,
        test: &TestDefinition,
        actions: Arc<FakeActions>,
    ) -> TestResult {
        let control: Arc<dyn Control> = self.fake.clone();
        let runner = runner
            .with_control(control)
            .with_actions(actions)
            .with_v2_timings(fast())
            .with_trace_timings(quick());
        runner.execute_single_test(test, &preflight(), &env_info(), &mut self.tracker)
    }

    /// Runs `test` with the capture on and the fake journal as log source and switch.
    fn run(&mut self, test: &TestDefinition, actions: Arc<FakeActions>) -> TestResult {
        let runner = TestRunner::new(TestRegistry::default(), self.options(true))
            .with_log_source(Box::new(self.journal.clone()))
            .with_logging_control(Box::new(self.journal.clone()));
        self.run_with(runner, test, actions)
    }
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

fn tags(r: &TestResult, action: usize) -> Vec<String> {
    r.flash_info.action_traces[action]
        .events
        .iter()
        .map(|e| e.tag.clone())
        .collect()
}

/// A focus action: the lines of the owned window written during the action are the timeline;
/// the owner's browser (written before and during the action), the owned window's creation lines
/// and an earlier RAISE of the owned window (written before the action started) are not. The
/// verdict is untouched, the result's slice holds the same two lines and nothing else.
#[test]
fn test_focus_timeline_is_the_owned_lines_after_the_action_started() {
    let mut rig = Rig::new();
    let j = &rig.journal;
    j.log("WINDOW_CREATED", 1000, "", "actorFound=true");
    j.log("ACTOR_MAP", 1000, "verify.owned.a", "");
    j.log(
        "RAISE",
        1000,
        "verify.owned.a",
        "opacity=255 frame=(0,0,400x300)",
    );
    j.log("RAISE", 777, "firefox", "opacity=255 frame=(0,0,1920x1080)");
    let jj = j.clone();
    let actions = FakeActions::new(move |args| {
        assert_eq!(args[0], "focus");
        jj.log("RAISE", 777, "firefox", "opacity=255 frame=(0,0,1920x1080)");
        jj.log(
            "RAISE",
            1000,
            "verify.owned.a",
            "opacity=242 frame=(100,140,400x300)",
        );
        jj.log(
            "ACTIVATE",
            1000,
            "verify.owned.a",
            "opacity=242 frame=(100,140,400x300)",
        );
        out(0, "", "")
    });
    let r = rig.run(&spec(FOCUS_A), actions);
    assert_eq!(r.auto_status, AutoStatus::AutoPass, "{:?}", r.message);
    assert_eq!(r.final_status, FinalStatus::Pass);
    let t = &r.flash_info.action_traces;
    assert_eq!(t.len(), 1);
    assert_eq!(t[0].status, TraceStatus::Captured);
    assert_eq!(tags(&r, 0), ["RAISE", "ACTIVATE"]);
    assert_eq!(t[0].events[0].detail, "opacity=242 frame=(100,140,400x300)");
    assert_eq!(t[0].events[0].offset_ms, 0.0);
    let slice = r.log_slice.as_deref().unwrap();
    assert_eq!(slice.lines().count(), 2, "{slice}");
    assert!(!slice.contains("firefox") && !slice.contains("id=777"));
    assert!(!slice.contains("WINDOW_CREATED") && !slice.contains("ACTOR_MAP"));
    assert!(!slice.contains("SET_LOGGING"));
    assert!(
        r.flash_info.flash_basis.is_none(),
        "no flash check for focus"
    );
}

/// The journal is read from the second the test started (not later) to a second at or after the
/// action's end, once before the action and at least once after it; a read before the test began
/// would pull in earlier runs, one that ended early would miss the action.
#[test]
fn test_capture_window_is_from_the_test_start_to_after_the_action() {
    let mut rig = Rig::new();
    let before = now_secs();
    let jj = rig.journal.clone();
    let actions = FakeActions::new(move |_| {
        jj.log("RAISE", 1000, "verify.owned.a", "opacity=1 frame=(0,0,1x1)");
        jj.log(
            "ACTIVATE",
            1000,
            "verify.owned.a",
            "opacity=1 frame=(0,0,1x1)",
        );
        out(0, "", "")
    });
    rig.run(&spec(FOCUS_A), actions);
    let after = now_secs();
    let reads = rig.journal.reads();
    assert!(
        reads.len() >= 2,
        "baseline and after-action reads: {reads:?}"
    );
    for (since, until) in &reads {
        assert!(
            *since >= before && *since <= after,
            "{since} not in {before}..{after}"
        );
        assert!(until >= since && *until <= after + 1);
    }
    assert!(
        reads.iter().all(|(s, _)| *s == reads[0].0),
        "one test start for all reads"
    );
}

/// Two consecutive actions each get their own lines: the second action's slice still holds the
/// first action's lines (the journal is read from the test start), and the floor leaves them out.
#[test]
fn test_two_consecutive_actions_get_separate_timelines() {
    let mut rig = Rig::new();
    let jj = rig.journal.clone();
    let n = Arc::new(Mutex::new(0));
    let actions = FakeActions::new(move |_| {
        let mut n = n.lock().unwrap();
        let (id, class) = if *n == 0 {
            (1000, "verify.owned.a")
        } else {
            (1001, "verify.owned.b")
        };
        *n += 1;
        jj.log("RAISE", id, class, "opacity=255 frame=(0,0,400x300)");
        jj.log("ACTIVATE", id, class, "opacity=255 frame=(0,0,400x300)");
        out(0, "", "")
    });
    let r = rig.run(&spec(FOCUS_AB), actions);
    assert_eq!(r.auto_status, AutoStatus::AutoPass, "{:?}", r.message);
    let t = &r.flash_info.action_traces;
    assert_eq!(t.len(), 2);
    assert_eq!((t[0].index, t[1].index), (0, 1));
    assert_eq!(tags(&r, 0), ["RAISE", "ACTIVATE"]);
    assert_eq!(tags(&r, 1), ["RAISE", "ACTIVATE"]);
    assert!(t[0]
        .events
        .iter()
        .all(|e| e.window == "a" && e.window_id == 1000));
    assert!(t[1]
        .events
        .iter()
        .all(|e| e.window == "b" && e.window_id == 1001));
    assert_eq!(
        t[1].events[0].offset_ms, 0.0,
        "offsets restart at each action"
    );
}

/// A failing action (exit 1) still gets its trace, and the failure is the verdict, not the trace.
#[test]
fn test_a_failing_action_keeps_its_trace_and_its_failure() {
    let mut rig = Rig::new();
    let jj = rig.journal.clone();
    let actions = FakeActions::new(move |_| {
        jj.log("RAISE", 1000, "verify.owned.a", "opacity=1 frame=(0,0,1x1)");
        jj.log(
            "ACTIVATE",
            1000,
            "verify.owned.a",
            "opacity=1 frame=(0,0,1x1)",
        );
        out(1, "", "[spawn-at] ERROR: nope")
    });
    let r = rig.run(&spec(FOCUS_A), actions);
    assert_eq!(r.auto_status, AutoStatus::AutoFail);
    assert_eq!(tags(&r, 0), ["RAISE", "ACTIVATE"]);
}

/// Queries are not traced: no journal read at all for the action, status "not applicable".
#[test]
fn test_a_query_makes_no_journal_read_and_is_not_applicable() {
    let mut rig = Rig::new();
    let r = rig.run(&spec(QUERY_A), FakeActions::ok());
    assert_eq!(r.auto_status, AutoStatus::AutoPass, "{:?}", r.message);
    assert_eq!(
        r.flash_info.action_traces[0].status,
        TraceStatus::NotApplicable
    );
    assert!(rig.journal.reads().is_empty(), "{:?}", rig.journal.reads());
    assert!(r.log_slice.is_none());
}

/// A command that logs nothing and produced no line is "not applicable"; a command that should
/// log and produced no line is "no lines"; neither fails the test. An empty journal is the same.
#[test]
fn test_missing_lines_never_fail_a_test_and_get_their_reason() {
    let mut rig = Rig::new();
    let r = rig.run(&spec(CLOSE_A), FakeActions::ok());
    assert_eq!(r.auto_status, AutoStatus::AutoPass, "{:?}", r.message);
    let t = &r.flash_info.action_traces[0];
    assert_eq!(t.status, TraceStatus::NotApplicable);
    assert_eq!(
        t.status.reason().as_deref(),
        Some("not applicable: the extension logs nothing for this command")
    );
    let mut rig = Rig::new();
    let r = rig.run(&spec(FOCUS_A), FakeActions::ok());
    assert_eq!(r.auto_status, AutoStatus::AutoPass, "{:?}", r.message);
    let t = &r.flash_info.action_traces[0];
    assert_eq!(t.status, TraceStatus::NoLines);
    assert_eq!(
        t.status.reason().as_deref(),
        Some("no lines for this window in the slice")
    );
    assert!(r.log_slice.is_none());
}

/// A journal that cannot be read gives "capture failed: <why>" and the test still passes.
#[test]
fn test_unreadable_journal_is_capture_failed_not_a_failure() {
    let mut rig = Rig::new();
    rig.journal.fail_reads("Permission denied");
    let r = rig.run(&spec(FOCUS_A), FakeActions::ok());
    assert_eq!(r.auto_status, AutoStatus::AutoPass, "{:?}", r.message);
    let reason = r.flash_info.action_traces[0].status.reason().unwrap();
    assert_eq!(
        reason,
        "capture failed: journalctl exited with error: Permission denied"
    );
}

/// A timing log that cannot be enabled gives "capture failed: <why>" for every action, reads the
/// journal not at all, and the test still passes.
#[test]
fn test_logging_that_cannot_be_enabled_is_capture_failed_for_every_action() {
    let mut rig = Rig::new();
    rig.journal.fail_enable("no gdbus");
    let r = rig.run(&spec(FOCUS_AB), FakeActions::ok());
    assert_eq!(r.auto_status, AutoStatus::AutoPass, "{:?}", r.message);
    assert_eq!(r.flash_info.action_traces.len(), 2);
    for t in &r.flash_info.action_traces {
        assert_eq!(
            t.status.reason().as_deref(),
            Some("capture failed: extension timing log could not be enabled: no gdbus")
        );
    }
    assert!(rig.journal.reads().is_empty());
}

/// A log source that is not the extension's journal (the no-op source of a Sway or X11 run) gives
/// "no extension on this backend (<backend>)" and never enables logging.
#[test]
fn test_no_op_log_source_means_no_extension_on_this_backend() {
    let mut rig = Rig::new();
    let runner = TestRunner::new(TestRegistry::default(), rig.options(true))
        .with_log_source(Box::new(crate::log_source::NoOpLogSource));
    let r = rig.run_with(runner, &spec(CLOSE_A), FakeActions::ok());
    assert_eq!(r.auto_status, AutoStatus::AutoPass, "{:?}", r.message);
    assert_eq!(
        r.flash_info.action_traces[0].status.reason().unwrap(),
        format!("no extension on this backend ({})", env_info().backend())
    );
}

/// `RunOptions::default()` captures nothing and never touches the extension: no action trace, no
/// slice, no `SetLogging`, no journal read, even when the log source and switch are present.
#[test]
fn test_capture_is_off_by_default_and_touches_nothing() {
    assert!(!RunOptions::default().action_trace);
    let mut rig = Rig::new();
    let runner = TestRunner::new(TestRegistry::default(), rig.options(false))
        .with_log_source(Box::new(rig.journal.clone()))
        .with_logging_control(Box::new(rig.journal.clone()));
    let jj = rig.journal.clone();
    let actions = FakeActions::new(move |_| {
        jj.log("RAISE", 1000, "verify.owned.a", "opacity=1 frame=(0,0,1x1)");
        out(0, "", "")
    });
    let r = rig.run_with(runner, &spec(FOCUS_A), actions);
    assert_eq!(r.auto_status, AutoStatus::AutoPass, "{:?}", r.message);
    assert!(r.flash_info.action_traces.is_empty());
    assert!(r.log_slice.is_none());
    assert_eq!(rig.journal.enable_calls(), 0);
    assert!(rig.journal.bracket_calls().is_empty());
    assert!(rig.journal.reads().is_empty());
}

/// `SetLogging` budget: one enable per test and no anchor calls for a focus; one enable and one
/// series of four anchor calls (the number a fixture `spawn` makes) for a transform.
#[test]
fn test_set_logging_calls_match_the_existing_mechanism() {
    let mut rig = Rig::new();
    rig.run(&spec(FOCUS_AB), FakeActions::ok());
    assert_eq!(rig.journal.enable_calls(), 1);
    assert!(rig.journal.bracket_calls().is_empty());
    let mut rig = Rig::new();
    rig.run(&spec(TRANSFORM_W), FakeActions::ok());
    assert_eq!(rig.journal.enable_calls(), 1);
    assert_eq!(
        rig.journal.bracket_calls(),
        vec![crate::trace::flash::ANCHOR_SAMPLES]
    );
}

/// Where the paint of the new size lands against the reveal, in ms.
#[derive(Clone, Copy)]
enum Paint {
    /// This long before the reveal.
    Before(f64),
    /// This long after the reveal.
    After(f64),
}

/// Runs a transform whose fake extension logs a full cloak-to-reveal and whose fixture paints
/// 300x200 at `paint` relative to the reveal (after an old 400x300 paint two seconds earlier).
fn transform_with(paint: Paint) -> TestResult {
    let mut rig = Rig::new();
    let jj = rig.journal.clone();
    let emit = rig.env.dir.join("emit");
    let actions = FakeActions::new(move |_| {
        let w = (1000, "verify.owned.w");
        jj.log("BATCH_STEP", w.0, w.1, "inst=Snapshot");
        jj.log(
            "SNAPSHOT_CREATED",
            w.0,
            w.1,
            "visible=true pos=(0,0) size=(400x300) opacity=255",
        );
        jj.log("CLOAK", w.0, w.1, "opacity=0 frame=(0,0,400x300)");
        std::thread::sleep(Duration::from_millis(8));
        let reveal = jj.log("UNCLOAK", w.0, w.1, "opacity=255 frame=(10,10,300x200)");
        jj.log("UNCLOAK_4D_REVEAL", w.0, w.1, "requestedSize=(300x200)");
        jj.log("ANCHOR_ARMED", w.0, w.1, "");
        let new_paint = match paint {
            Paint::Before(ms) => reveal - (ms * 1e6) as u64,
            Paint::After(ms) => reveal + (ms * 1e6) as u64,
        };
        let text = format!(
            "{} FIXTURE FIRST_PAINT 400x300\n{} FIXTURE FIRST_PAINT 300x200\n",
            monotonic_ns() - 2_000_000_000,
            new_paint
        );
        std::fs::write(&emit, text).unwrap();
        // The stand-in fixture appends the file and deletes it; wait for that.
        for _ in 0..200 {
            if !emit.exists() {
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        out(0, "", "")
    });
    rig.run(&spec(TRANSFORM_W), actions)
}

/// A paint 3 ms before the reveal is clean: no flash event, and the timeline still reports the
/// cloak-to-reveal time and the snapshot. The verdict is the same as with no flash check.
#[test]
fn test_transform_paint_before_the_reveal_is_clean() {
    let r = transform_with(Paint::Before(3.0));
    assert_eq!(r.auto_status, AutoStatus::AutoPass, "{:?}", r.message);
    let f = &r.flash_info;
    assert!(!f.flash, "{:?}", f.flash_events);
    assert!(f.flash_events.is_empty());
    assert_eq!(f.flash_basis, Some(crate::trace::flash::FlashBasis::Client));
    let t = &f.action_traces[0];
    assert_eq!(t.status, TraceStatus::Captured);
    assert_eq!(t.snapshot.as_deref(), Some("taken"));
    let c = t
        .cloak_to_reveal_ms
        .expect("CLOAK and UNCLOAK_4D_REVEAL were logged");
    assert!((7.0..500.0).contains(&c), "{c}");
    assert!(t.has_cloak_phase());
}

/// A paint 60 ms after the reveal is a certain flash (longer than any frame of 30 Hz or more):
/// counted, read against the old 400x300 content, and still only informational (AutoPass).
#[test]
fn test_transform_paint_60_ms_after_the_reveal_is_a_certain_flash() {
    let r = transform_with(Paint::After(60.0));
    assert_eq!(r.auto_status, AutoStatus::AutoPass, "{:?}", r.message);
    assert_eq!(r.final_status, FinalStatus::Pass);
    let f = &r.flash_info;
    assert!(f.flash, "{:?} {:?}", f.flash_events, f.flash_notes);
    assert_eq!((f.flash_certain, f.flash_possible), (1, 0));
    assert!(
        f.flash_events[0].detail.contains("content was 400x300"),
        "{}",
        f.flash_events[0].detail
    );
}

/// A paint 1.5 ms after the reveal is a "possible" flash (less than a 60 Hz frame of 16.7 ms).
#[test]
fn test_transform_paint_1_5_ms_after_the_reveal_is_a_possible_flash() {
    let r = transform_with(Paint::After(1.5));
    assert_eq!(r.auto_status, AutoStatus::AutoPass, "{:?}", r.message);
    let f = &r.flash_info;
    assert!(f.flash, "{:?} {:?}", f.flash_events, f.flash_notes);
    assert_eq!((f.flash_certain, f.flash_possible), (0, 1));
}
