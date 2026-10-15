//! Execution of a version 2 test: launch its own fixture windows, act on them by their recorded
//! ids, assert, tear down (`docs/DESIGN_RUNNER_SETUP.md`). Version 1 tests never come here.
//!
//! Result statuses, in the design's words: a setup that fails halfway, a target that changed, a
//! failed query and an interrupt are `Error` (nothing about the product was judged); a command
//! that exits wrongly, a check that does not hold and a rectangle that is off are `AutoFail`.

use super::engine::TestRunner;
use super::models::{resolve_final_status, AutoStatus, HumanVerdict, TestResult};
use super::owned::{
    ActionRunner, CliActions, CliControl, Control, SetupRequest, SetupSession, Timings,
    EXIT_AFTER_AUTO_MS, EXIT_AFTER_HUMAN_MS,
};
use super::validation::{pass_assessment, validate_expectations, validate_geometry_final};
use crate::oracle::Rect;
use crate::probe::EnvironmentInfo;
use crate::schema::v2::{self, Bindings, NONEXISTENT};
use crate::schema::{CheckState, TestAction, TestCheck, TestDefinition, TestKind};
use crate::tracker::WindowTracker;
use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// What a test ended with, before it becomes a [`TestResult`].
struct Outcome {
    status: AutoStatus,
    message: String,
    exit_code: Option<i32>,
    stdout: String,
    stderr: String,
    final_rect: Option<Rect>,
}

impl Outcome {
    fn new(status: AutoStatus, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
            exit_code: None,
            stdout: String::new(),
            stderr: String::new(),
            final_rect: None,
        }
    }
    fn error(message: impl Into<String>) -> Self {
        Self::new(AutoStatus::Error, message)
    }
    fn fail(message: impl Into<String>) -> Self {
        Self::new(AutoStatus::AutoFail, message)
    }
}

/// Everything the actions produced that the final result carries.
#[derive(Default)]
struct Captured {
    exit_code: Option<i32>,
    stdout: String,
    stderr: String,
    final_rect: Option<Rect>,
}

/// The name in `--id ${name}` of `action`, if it has one.
fn target_name(action: &TestAction) -> Option<String> {
    let pos = action.args.iter().position(|a| a == "--id")?;
    v2::whole_placeholder(action.args.get(pos + 1)?).map(|p| p.name)
}

/// The test with its expectation strings filled in from `b`.
fn substituted(test: &TestDefinition, b: &Bindings) -> Result<TestDefinition, String> {
    let mut t = test.clone();
    for list in [
        &mut t.expect.stderr_present,
        &mut t.expect.stderr_absent,
        &mut t.expect.stdout_present,
    ] {
        for s in list.iter_mut() {
            *s = v2::substitute(s, b)?;
        }
    }
    Ok(t)
}

fn rects_close(a: Rect, b: Rect) -> bool {
    a.x.abs_diff(b.x) <= 2
        && a.y.abs_diff(b.y) <= 2
        && a.w.abs_diff(b.w) <= 2
        && a.h.abs_diff(b.h) <= 2
}

/// One look at whether `check` holds now.
fn check_once(
    session: &SetupSession,
    check: &TestCheck,
    before: &HashMap<String, Option<Rect>>,
) -> Result<(), String> {
    let window = session.read(&check.window)?;
    if let Some(state) = check.state {
        return match (state, window) {
            (CheckState::Gone, None) => Ok(()),
            (CheckState::Gone, Some(_)) => {
                Err(format!("window '{}' is still listed", check.window))
            }
            (_, None) => Err(format!("window '{}' is not listed", check.window)),
            (CheckState::Present, Some(_)) => Ok(()),
            (CheckState::Maximized, Some(w)) if w.maximized => Ok(()),
            (CheckState::Minimized, Some(w)) if w.minimized => Ok(()),
            (CheckState::Normal, Some(w)) if !w.maximized && !w.minimized => Ok(()),
            (CheckState::Focused, Some(w)) if w.focused => Ok(()),
            (CheckState::Unfocused, Some(w)) if !w.focused => Ok(()),
            (s, Some(w)) => Err(format!(
                "window '{}' should be {:?} but is maximized={} minimized={} focused={}",
                check.window, s, w.maximized, w.minimized, w.focused
            )),
        };
    }
    if check.same_rect_as.is_some() {
        let was = before
            .get(&check.window)
            .copied()
            .flatten()
            .ok_or_else(|| {
                format!(
                    "no rectangle of '{}' was recorded before the action",
                    check.window
                )
            })?;
        let now = window
            .and_then(|w| w.rect)
            .ok_or_else(|| format!("window '{}' has no readable rectangle", check.window))?;
        return if rects_close(was, now) {
            Ok(())
        } else {
            Err(format!(
                "window '{}' is at {:?} but was at {:?} before the action",
                check.window, now, was
            ))
        };
    }
    Ok(())
}

/// Evaluates `check`, looking again until it holds or `wait` is over.
fn eval_check(
    session: &mut SetupSession,
    check: &TestCheck,
    before: &HashMap<String, Option<Rect>>,
    timings: &Timings,
) -> Result<(), String> {
    if check.process_exited {
        return if session.wait_process_exit(&check.window, timings.check_wait) {
            Ok(())
        } else {
            Err(format!(
                "the fixture process of '{}' is still running {} ms after the action",
                check.window,
                timings.check_wait.as_millis()
            ))
        };
    }
    let deadline = Instant::now() + timings.check_wait;
    loop {
        match check_once(session, check, before) {
            Ok(()) => return Ok(()),
            Err(e) if Instant::now() >= deadline => return Err(e),
            Err(_) => std::thread::sleep(timings.poll),
        }
    }
}

impl TestRunner {
    /// Runs a version 2 test. Called once from `execute_single_test`, after the preflight,
    /// environment and dry-run handling it shares with version 1.
    pub(super) fn execute_v2_test(
        &self,
        test: &TestDefinition,
        env_info: &EnvironmentInfo,
        tracker: &mut WindowTracker,
        start: Instant,
    ) -> TestResult {
        // The loader already checked this; the runner does not rely on it.
        if let Err(e) = v2::validate(test) {
            return finish(
                test,
                start,
                Outcome::error(format!("invalid version 2 spec: {e}")),
            );
        }
        let bin = &self.options.bin_path;
        let control: Arc<dyn Control> = self
            .control
            .clone()
            .unwrap_or_else(|| Arc::new(CliControl { bin: bin.clone() }));
        let actions: Arc<dyn ActionRunner> = self
            .actions
            .clone()
            .unwrap_or_else(|| Arc::new(CliActions { bin: bin.clone() }));
        let fixture_bin = bin
            .parent()
            .unwrap_or(std::path::Path::new("."))
            .join("fixture");
        let exit_after = match test.kind {
            TestKind::Auto => EXIT_AFTER_AUTO_MS,
            TestKind::Human | TestKind::Both => EXIT_AFTER_HUMAN_MS,
        };
        let mut session = match SetupSession::new(
            Arc::clone(&control),
            fixture_bin,
            &test.id,
            exit_after,
            self.v2_timings,
            Arc::clone(&self.options.interrupted),
        ) {
            Ok(s) => s,
            Err(e) => {
                return finish(test, start, Outcome::error(format!("setup failed: {e}")));
            }
        };
        if !test.v2.setup.is_empty() {
            session.remember_focus();
        }

        let mut captured = Captured::default();
        let mut outcome = self.run_v2_steps(test, env_info, &mut session, &*actions, &mut captured);
        outcome.exit_code = captured.exit_code;
        outcome.stdout = captured.stdout;
        outcome.stderr = captured.stderr;
        outcome.final_rect = captured.final_rect;

        // A test a person judges keeps its window until the verdict; the tracker tears it down.
        let keep_open = matches!(test.kind, TestKind::Human | TestKind::Both)
            && outcome.status != AutoStatus::Error
            && !session.is_empty();
        if keep_open {
            outcome.message.push_str(" (window left open for review)");
            tracker.owned.push(session);
        } else {
            let report = session.teardown();
            for w in &report.warnings {
                outcome.message.push_str(&format!(" (warning: {w})"));
            }
            if !report.left_behind.is_empty() {
                outcome.message.push_str(&format!(
                    " (warning: left behind after teardown: window ids {:?})",
                    report.left_behind
                ));
            }
        }
        finish(test, start, outcome)
    }

    /// Setup, actions and checks. The session is torn down by the caller (or, on a panic, by its
    /// `Drop`).
    fn run_v2_steps(
        &self,
        test: &TestDefinition,
        env_info: &EnvironmentInfo,
        session: &mut SetupSession,
        actions: &dyn ActionRunner,
        captured: &mut Captured,
    ) -> Outcome {
        let timings = self.v2_timings;
        for s in &test.v2.setup {
            let req = SetupRequest {
                name: s.name.clone(),
                args: s.args.clone(),
                timeout: s.timeout_ms.map(Duration::from_millis),
            };
            if let Err(e) = session.launch(&req) {
                return Outcome::error(format!("setup failed: {e}"));
            }
        }

        let all: Vec<&TestAction> = std::iter::once(&test.action)
            .chain(test.v2.then.iter())
            .collect();
        let last = all.len() - 1;
        let mut before: HashMap<String, Option<Rect>> = HashMap::new();
        let mut status = AutoStatus::AutoPass;
        let mut message = String::from("Assertions passed");

        for (i, action) in all.iter().enumerate() {
            if self.options.interrupted.load(Ordering::SeqCst) {
                return Outcome::error("interrupted");
            }
            let mut b = session.bindings();
            let uses_nonexistent = action.args.iter().any(|a| a.contains("${nonexistent}"));
            if uses_nonexistent {
                match session.choose_nonexistent() {
                    Ok(id) => b.nonexistent = Some(id),
                    Err(e) => return Outcome::error(e),
                }
            }
            // The target must still be the window this run recorded.
            if let Some(name) = target_name(action) {
                if name != NONEXISTENT {
                    if let Err(e) = session.verify_target(&name) {
                        return Outcome::error(e);
                    }
                }
            }
            if let Some(id) = b.nonexistent {
                if let Err(e) = session.ensure_absent(id) {
                    return Outcome::error(e);
                }
            }
            if i == 0 {
                for s in &test.v2.setup {
                    let rect = session.read(&s.name).ok().flatten().and_then(|w| w.rect);
                    before.insert(s.name.clone(), rect);
                }
            }

            let mut args = vec![action.command.clone()];
            for a in &action.args {
                match v2::substitute(a, &b) {
                    Ok(s) => args.push(s),
                    Err(e) => return Outcome::error(e),
                }
            }
            let out = match actions.run(&args, self.options.step_timeout) {
                Ok(o) => o,
                Err(e) => return Outcome::error(format!("Harness error executing command: {e}")),
            };
            let code = out.exit_code.unwrap_or(-1);
            let stdout = String::from_utf8_lossy(&out.stdout).to_string();
            let stderr = String::from_utf8_lossy(&out.stderr).to_string();

            if i == 0 {
                captured.exit_code = Some(code);
                captured.stdout = stdout.clone();
                captured.stderr = stderr.clone();
                let judged = match substituted(test, &b) {
                    Ok(t) => t,
                    Err(e) => return Outcome::error(e),
                };
                if let Err(e) = validate_expectations(&judged, code, &stderr, env_info) {
                    return Outcome::fail(e);
                }
                for needle in &judged.expect.stdout_present {
                    if !stdout.contains(needle.as_str()) {
                        return Outcome::fail(format!(
                            "Required text '{needle}' missing from stdout:\n{stdout}"
                        ));
                    }
                }
                if let Some(name) = &judged.expect.stdout_lists_window {
                    if let Err(e) = stdout_lists_window(&stdout, session, name) {
                        return Outcome::fail(e);
                    }
                }
                if judged.expect.rect.is_some() {
                    let name = target_name(action)
                        .filter(|n| n != NONEXISTENT)
                        .or_else(|| test.v2.setup.first().map(|s| s.name.clone()))
                        .unwrap_or_default();
                    match self.settle_rect(&judged, env_info, session, &name, &stdout) {
                        Ok((rect, s, m)) => {
                            captured.final_rect = rect;
                            status = s;
                            message = m;
                        }
                        Err(o) => return o,
                    }
                }
            } else if code != 0 {
                return Outcome::fail(format!(
                    "[[then]] #{i} (`{}`) exited with {code}:\n{}",
                    args.join(" "),
                    stderr.trim()
                ));
            }

            for c in &test.v2.check {
                if c.after.unwrap_or(last) != i {
                    continue;
                }
                if let Err(e) = eval_check(session, c, &before, &timings) {
                    return Outcome::fail(format!(
                        "Check on '{}' after action {i} failed: {e}",
                        c.window
                    ));
                }
            }
        }
        Outcome::new(status, message)
    }

    /// Looks at the recorded window until its rectangle satisfies `[expect.rect]` or the settle
    /// time is over. Returns the rectangle read and the pass status and message, or the failure.
    #[allow(clippy::type_complexity)]
    fn settle_rect(
        &self,
        judged: &TestDefinition,
        env_info: &EnvironmentInfo,
        session: &SetupSession,
        name: &str,
        stdout: &str,
    ) -> Result<(Option<Rect>, AutoStatus, String), Outcome> {
        let timings = self.v2_timings;
        let deadline = Instant::now() + timings.settle;
        loop {
            let rect = match session.read(name) {
                Ok(w) => w.and_then(|w| w.rect),
                Err(e) => return Err(Outcome::error(e)),
            };
            let last_err = if rect.is_some() {
                match validate_geometry_final(judged, stdout, rect, None, env_info) {
                    Ok(check) => {
                        let (status, message) = pass_assessment(check.as_ref());
                        return Ok((rect, status, message));
                    }
                    Err(e) => e,
                }
            } else {
                format!("the rectangle of window '{name}' could not be read")
            };
            if Instant::now() >= deadline {
                return Err(Outcome::fail(last_err));
            }
            std::thread::sleep(timings.poll);
        }
    }
}

/// `stdout` is a JSON array of windows holding the recorded window of setup `name` (same id,
/// same unique title).
fn stdout_lists_window(stdout: &str, session: &SetupSession, name: &str) -> Result<(), String> {
    let id = session
        .window_id(name)
        .ok_or_else(|| format!("setup '{name}' has no recorded window"))?;
    let title = session
        .bindings()
        .titles
        .get(name)
        .cloned()
        .unwrap_or_default();
    let listed = super::owned::parse_windows(stdout.as_bytes())?;
    if listed.iter().any(|w| w.id == id && w.title == title) {
        Ok(())
    } else {
        Err(format!(
            "stdout does not list window {id} ('{name}') with its unique title"
        ))
    }
}

/// Builds the result of a version 2 test from its outcome.
fn finish(test: &TestDefinition, start: Instant, o: Outcome) -> TestResult {
    let (final_status, disagreement) =
        resolve_final_status(test.kind, o.status, &HumanVerdict::default());
    TestResult {
        id: test.id.clone(),
        title: test.title.clone(),
        kind: test.kind,
        required: test.required,
        auto_status: o.status,
        human_verdict: HumanVerdict::default(),
        final_status,
        disagreement,
        duration_ms: start.elapsed().as_millis() as u64,
        exit_code: o.exit_code,
        stdout: o.stdout,
        stderr: o.stderr,
        message: Some(o.message),
        log_slice: None,
        trace_note: None,
        final_rect: o.final_rect,
        flash_info: Default::default(),
        wait_info: Default::default(),
    }
}
