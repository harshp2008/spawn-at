//! Tests of the version 2 executor (`docs/DESIGN_RUNNER_SETUP.md` sections 2 and 3) with a fake
//! compositor, a fake action runner and real fake-fixture child processes (`test_support.rs`).
//! Nothing here starts `spawn-at`, a display or a compositor.

use super::models::{AutoStatus, FinalStatus, RunOptions, TestResult};
use super::owned::Control;
use super::test_support::{alive, fast, info, out, Env, Fake, FakeActions};
use super::TestRunner;
use crate::loader::TestRegistry;
use crate::oracle::Rect;
use crate::preflight::{PreflightReport, PreflightVerdict};
use crate::probe::EnvironmentInfo;
use crate::schema::v2;
use crate::schema::TestDefinition;
use crate::tracker::WindowTracker;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

const HEAD: &str = r#"
schema_version = 2
id = "transforms/example"
category = "transforms"
title = "example"
description = "example"
tags = ["moves-focus"]
"#;

/// A version 2 spec of kind `kind` with `body`, checked by the loader rules.
fn spec(kind: &str, body: &str) -> TestDefinition {
    let t = unchecked_spec(kind, body);
    v2::validate(&t).expect("the spec passes the loader rules");
    t
}

/// The same without the loader rules, for tests that hand the runner a spec the loader refuses.
fn unchecked_spec(kind: &str, body: &str) -> TestDefinition {
    toml::from_str(&format!("{HEAD}\nkind = \"{kind}\"\n{body}")).expect("spec parses")
}

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

struct Rig {
    env: Env,
    fake: Arc<Fake>,
    tracker: WindowTracker,
    interrupted: Arc<AtomicBool>,
}

impl Rig {
    fn new(mode: &str) -> Rig {
        let env = Env::new(mode);
        let fake = Fake::new(&env);
        Rig {
            env,
            fake,
            tracker: WindowTracker::new_with_preexisting(&[]),
            interrupted: Arc::new(AtomicBool::new(false)),
        }
    }

    fn runner(&self, actions: Arc<FakeActions>) -> TestRunner {
        let options = RunOptions {
            bin_path: self.env.dir.join("spawn-at"),
            interrupted: Arc::clone(&self.interrupted),
            ..Default::default()
        };
        let control: Arc<dyn Control> = self.fake.clone();
        TestRunner::new(TestRegistry::default(), options)
            .with_control(control)
            .with_actions(actions)
            .with_v2_timings(fast())
    }

    fn run(&mut self, test: &TestDefinition, actions: Arc<FakeActions>) -> TestResult {
        let runner = self.runner(actions);
        runner.execute_single_test(test, &preflight(), &env_info(), &mut self.tracker)
    }
}

/// The fixture pids the fake fixtures registered (the children the runner started).
fn child_pids(rig: &Rig) -> Vec<u32> {
    rig.env.fixture_pids()
}

const RESIZE: &str = r#"
[[setup]]
name = "w"
args = ["--width", "400", "--height", "300"]

[action]
command = "transform"
args = ["--id", "${w}", "--size", "600", "450", "--anchor", "center", "--margin", "0", "--no-focus"]

[expect]
exit_code = 0
[expect.rect]
anchor = "center"
size = [600, 450]
margin = 0
tolerance_px = 2
"#;

#[test]
fn test_transform_gets_the_recorded_id_and_passes_on_the_reread_rectangle() {
    let mut rig = Rig::new("normal");
    let fake = Arc::clone(&rig.fake);
    let actions = FakeActions::new(move |args| {
        assert_eq!(args[0], "transform");
        fake.patch(1000, |p| p.rect = Some(Rect::new(660, 315, 600, 450)));
        out(0, "", "")
    });
    let r = rig.run(&spec("auto", RESIZE), Arc::clone(&actions));
    assert_eq!(r.auto_status, AutoStatus::AutoPass, "{:?}", r.message);
    assert_eq!(r.final_status, FinalStatus::Pass);
    assert_eq!(
        actions.calls(),
        vec![vec![
            "transform",
            "--id",
            "1000",
            "--size",
            "600",
            "450",
            "--anchor",
            "center",
            "--margin",
            "0",
            "--no-focus"
        ]]
    );
    assert_eq!(r.final_rect, Some(Rect::new(660, 315, 600, 450)));
    assert_eq!(rig.fake.close_calls(), vec![1000], "torn down by id");
    assert!(child_pids(&rig).iter().all(|p| !alive(*p)));
    assert!(rig.tracker.owned.is_empty());
}

#[test]
fn test_a_rectangle_that_is_off_fails_after_the_settle() {
    let mut rig = Rig::new("normal");
    let fake = Arc::clone(&rig.fake);
    let actions = FakeActions::new(move |_| {
        fake.patch(1000, |p| p.rect = Some(Rect::new(0, 0, 600, 450)));
        out(0, "", "")
    });
    let r = rig.run(&spec("auto", RESIZE), actions);
    assert_eq!(r.auto_status, AutoStatus::AutoFail, "{:?}", r.message);
}

// ---- setup failures are Error and run no action ----

#[test]
fn test_halfway_setup_failure_is_an_error_and_runs_no_action() {
    let mut rig = Rig::new("exit-now");
    let actions = FakeActions::ok();
    let r = rig.run(&spec("auto", RESIZE), Arc::clone(&actions));
    assert_eq!(r.auto_status, AutoStatus::Error);
    assert_eq!(r.final_status, FinalStatus::Error, "never Fail");
    assert!(r.message.as_deref().unwrap().starts_with("setup failed:"));
    assert!(
        actions.calls().is_empty(),
        "no action may run after a failed setup"
    );
    assert!(rig.fake.close_calls().is_empty());
}

#[test]
fn test_second_setup_failing_closes_the_first_and_runs_nothing() {
    let mut rig = Rig::new("normal");
    let body = r#"
[[setup]]
name = "a"
[[setup]]
name = "b"
timeout_ms = 300
[action]
command = "query"
args = ["windows", "--json"]
"#;
    // After `a` registered its window, the fake fixture stops registering: `b` never appears.
    let env_dir = rig.env.dir.clone();
    let actions = FakeActions::ok();
    let t = spec("auto", body);
    let fake = Arc::clone(&rig.fake);
    let flip = std::thread::spawn(move || {
        for _ in 0..200 {
            if !fake.windows().unwrap().is_empty() {
                std::fs::write(env_dir.join("mode"), "no-window").unwrap();
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    });
    let r = rig.run(&t, Arc::clone(&actions));
    flip.join().unwrap();
    assert_eq!(r.auto_status, AutoStatus::Error, "{:?}", r.message);
    assert!(actions.calls().is_empty());
    assert_eq!(
        rig.fake.close_calls(),
        vec![1000],
        "a's recorded window only"
    );
}

#[test]
fn test_a_spec_the_loader_would_refuse_is_refused_by_the_runner_too() {
    let mut rig = Rig::new("normal");
    let body = r#"
[[setup]]
name = "w"
[action]
command = "close"
args = ["--id", "5"]
"#;
    let actions = FakeActions::ok();
    let r = rig.run(&unchecked_spec("auto", body), Arc::clone(&actions));
    assert_eq!(r.auto_status, AutoStatus::Error);
    assert!(r
        .message
        .as_deref()
        .unwrap()
        .contains("invalid version 2 spec"));
    assert!(actions.calls().is_empty());
    assert!(child_pids(&rig).is_empty(), "no fixture was started for it");
}

// ---- the target is re-verified before every action ----

#[test]
fn test_a_changed_target_stops_the_run_before_the_next_action() {
    let mut rig = Rig::new("normal");
    let body = r#"
[[setup]]
name = "w"
[action]
command = "maximize"
args = ["--id", "${w}", "--no-focus"]
[[then]]
command = "restore"
args = ["--id", "${w}", "--no-focus"]
"#;
    let fake = Arc::clone(&rig.fake);
    let actions = FakeActions::new(move |args| {
        if args[0] == "maximize" {
            // Someone else's window now has the id.
            fake.st
                .lock()
                .unwrap()
                .retitle
                .insert(1000, "someone else".into());
        }
        out(0, "", "")
    });
    let r = rig.run(&spec("auto", body), Arc::clone(&actions));
    assert_eq!(r.auto_status, AutoStatus::Error, "{:?}", r.message);
    assert!(r.message.as_deref().unwrap().contains("target changed"));
    assert_eq!(actions.calls().len(), 1, "the second action must not run");
    assert!(
        rig.fake.close_calls().is_empty(),
        "an id with another title is never closed"
    );
    assert!(
        child_pids(&rig).iter().all(|p| !alive(*p)),
        "our own child is still stopped"
    );
}

// ---- exit codes, stdout, nonexistent ids ----

#[test]
fn test_wrong_exit_code_is_a_fail_not_an_error() {
    let mut rig = Rig::new("normal");
    let body = r#"
[[setup]]
name = "w"
[action]
command = "focus"
args = ["--id", "${w}"]
"#;
    let r = rig.run(
        &spec("auto", body),
        FakeActions::new(|_| out(1, "", "boom")),
    );
    assert_eq!(r.auto_status, AutoStatus::AutoFail);
    assert!(r.message.as_deref().unwrap().contains("Exit code mismatch"));
}

#[test]
fn test_a_then_action_that_fails_is_a_fail() {
    let mut rig = Rig::new("normal");
    let body = r#"
[[setup]]
name = "w"
[action]
command = "focus"
args = ["--id", "${w}"]
[[then]]
command = "restore"
args = ["--id", "${w}"]
"#;
    let r = rig.run(
        &spec("auto", body),
        FakeActions::new(|a| {
            if a[0] == "restore" {
                out(2, "", "no")
            } else {
                out(0, "", "")
            }
        }),
    );
    assert_eq!(r.auto_status, AutoStatus::AutoFail);
    assert!(r.message.as_deref().unwrap().contains("[[then]] #1"));
}

#[test]
fn test_nonexistent_id_is_an_absent_id_and_the_expected_error_passes() {
    let mut rig = Rig::new("normal");
    let body = r#"
[[setup]]
name = "w"
[action]
command = "close"
args = ["--id", "${nonexistent}"]
[expect]
exit_code = 1
stderr_present = ["No window with id ${nonexistent}"]
"#;
    let actions = FakeActions::new(|args| out(1, "", &format!("No window with id {}", args[2])));
    let r = rig.run(&spec("auto", body), Arc::clone(&actions));
    assert_eq!(r.auto_status, AutoStatus::AutoPass, "{:?}", r.message);
    assert_eq!(actions.calls()[0], vec!["close", "--id", "1001000"]);
    assert_eq!(
        rig.fake.close_calls(),
        vec![1000],
        "only teardown closed anything, and only w"
    );
}

#[test]
fn test_stdout_expectations_see_the_recorded_window() {
    let body = r#"
[[setup]]
name = "w"
[action]
command = "query"
args = ["windows", "--json"]
[expect]
stdout_lists_window = "w"
stdout_present = ["\"id\":${w}"]
"#;
    let mut rig = Rig::new("normal");
    let fake = Arc::clone(&rig.fake);
    let listing = FakeActions::new(move |_| {
        let w = fake.windows().unwrap();
        let json = serde_json::json!([{"id": w[0].id, "title": w[0].title}]);
        out(0, &json.to_string(), "")
    });
    let r = rig.run(&spec("auto", body), listing);
    assert_eq!(r.auto_status, AutoStatus::AutoPass, "{:?}", r.message);

    let mut rig = Rig::new("normal");
    let other = FakeActions::new(|_| out(0, r#"[{"id":1000,"title":"not ours"}]"#, ""));
    let r = rig.run(&spec("auto", body), other);
    assert_eq!(r.auto_status, AutoStatus::AutoFail);
}

// ---- checks ----

const MAX_RESTORE: &str = r#"
[[setup]]
name = "w"
[action]
command = "maximize"
args = ["--id", "${w}", "--no-focus"]
[[then]]
command = "restore"
args = ["--id", "${w}", "--no-focus"]
[[check]]
after = 0
window = "w"
state = "maximized"
[[check]]
after = 1
window = "w"
state = "normal"
[[check]]
after = 1
window = "w"
same_rect_as = "before"
"#;

#[test]
fn test_checks_run_after_the_action_they_name_and_hold() {
    let mut rig = Rig::new("normal");
    rig.fake
        .patch(1000, |p| p.rect = Some(Rect::new(10, 20, 400, 300)));
    let fake = Arc::clone(&rig.fake);
    let actions = FakeActions::new(move |args| {
        match args[0].as_str() {
            "maximize" => fake.patch(1000, |p| p.maximized = Some(true)),
            "restore" => fake.patch(1000, |p| p.maximized = Some(false)),
            _ => {}
        }
        out(0, "", "")
    });
    let r = rig.run(&spec("auto", MAX_RESTORE), actions);
    assert_eq!(r.auto_status, AutoStatus::AutoPass, "{:?}", r.message);
}

#[test]
fn test_a_check_that_does_not_hold_fails_with_the_action_index() {
    let mut rig = Rig::new("normal");
    // The window never becomes maximized.
    let r = rig.run(&spec("auto", MAX_RESTORE), FakeActions::ok());
    assert_eq!(r.auto_status, AutoStatus::AutoFail);
    let m = r.message.unwrap();
    assert!(
        m.contains("after action 0") && m.contains("Maximized"),
        "{m}"
    );
}

#[test]
fn test_same_rect_as_before_detects_a_moved_window() {
    let mut rig = Rig::new("normal");
    rig.fake
        .patch(1000, |p| p.rect = Some(Rect::new(10, 20, 400, 300)));
    let fake = Arc::clone(&rig.fake);
    let actions = FakeActions::new(move |args| {
        match args[0].as_str() {
            "maximize" => fake.patch(1000, |p| p.maximized = Some(true)),
            "restore" => fake.patch(1000, |p| {
                p.maximized = Some(false);
                p.rect = Some(Rect::new(500, 500, 400, 300)); // restored elsewhere
            }),
            _ => {}
        }
        out(0, "", "")
    });
    let r = rig.run(&spec("auto", MAX_RESTORE), actions);
    assert_eq!(r.auto_status, AutoStatus::AutoFail);
    assert!(r.message.unwrap().contains("was at"));
}

#[test]
fn test_process_exited_check_sees_the_fixture_end() {
    let body = r#"
[[setup]]
name = "w"
[action]
command = "close"
args = ["--id", "${w}"]
[[check]]
window = "w"
state = "gone"
[[check]]
window = "w"
process_exited = true
"#;
    let mut rig = Rig::new("normal");
    let (fake, dir) = (Arc::clone(&rig.fake), rig.env.dir.clone());
    let closes = FakeActions::new(move |_| {
        // `close` makes the window go and the fixture exit, as a real close does.
        fake.st.lock().unwrap().hidden.push(1000);
        let pid: i32 = std::fs::read_to_string(dir.join("state"))
            .unwrap()
            .split(' ')
            .next()
            .unwrap()
            .parse()
            .unwrap();
        // SAFETY: the pid is the fake fixture this test's runner started.
        unsafe { libc::kill(pid, libc::SIGTERM) };
        out(0, "", "")
    });
    let r = rig.run(&spec("auto", body), closes);
    assert_eq!(r.auto_status, AutoStatus::AutoPass, "{:?}", r.message);

    // A close that leaves the process running fails the check.
    let mut rig = Rig::new("normal");
    let fake = Arc::clone(&rig.fake);
    let r = rig.run(
        &spec("auto", body),
        FakeActions::new(move |_| {
            fake.st.lock().unwrap().hidden.push(1000);
            out(0, "", "")
        }),
    );
    assert_eq!(r.auto_status, AutoStatus::AutoFail);
    assert!(r.message.unwrap().contains("still running"));
}

// ---- who keeps the window open, and who closes it ----

const QUERY_ONLY: &str = r#"
[[setup]]
name = "w"
[action]
command = "query"
args = ["windows", "--json"]
"#;

#[test]
fn test_a_test_a_person_judges_keeps_its_window_until_the_tracker_cleans_up() {
    let mut rig = Rig::new("normal");
    let r = rig.run(&spec("both", QUERY_ONLY), FakeActions::ok());
    assert_eq!(r.auto_status, AutoStatus::AutoPass, "{:?}", r.message);
    assert_eq!(
        r.final_status,
        FinalStatus::Pending,
        "a Both test waits for the verdict"
    );
    assert_eq!(rig.tracker.owned.len(), 1);
    assert!(
        rig.fake.close_calls().is_empty(),
        "the window stays up for the reviewer"
    );
    let pid = child_pids(&rig)[0];
    assert!(alive(pid));

    // Every cleanup path (end of run, TUI quit, verdict) goes through this call.
    rig.tracker
        .cleanup_test_windows(&rig.env.dir.join("spawn-at"))
        .unwrap();
    assert!(rig.tracker.owned.is_empty());
    assert_eq!(rig.fake.close_calls(), vec![1000]);
    assert!(!alive(pid));
}

#[test]
fn test_an_auto_test_leaves_nothing_behind() {
    let mut rig = Rig::new("normal");
    let r = rig.run(&spec("auto", QUERY_ONLY), FakeActions::ok());
    assert_eq!(r.auto_status, AutoStatus::AutoPass);
    assert!(rig.tracker.owned.is_empty());
    assert_eq!(rig.fake.close_calls(), vec![1000]);
    assert!(child_pids(&rig).iter().all(|p| !alive(*p)));
}

#[test]
fn test_a_both_test_whose_setup_failed_does_not_hold_anything() {
    let mut rig = Rig::new("exit-now");
    let r = rig.run(&spec("both", QUERY_ONLY), FakeActions::ok());
    assert_eq!(r.auto_status, AutoStatus::Error);
    assert!(rig.tracker.owned.is_empty());
}

#[test]
fn test_a_leftover_window_is_a_warning_not_a_failure() {
    let mut rig = Rig::new("normal");
    rig.fake.st.lock().unwrap().ignore_close = true;
    let r = rig.run(&spec("auto", QUERY_ONLY), FakeActions::ok());
    assert_eq!(r.auto_status, AutoStatus::AutoPass);
    assert!(
        r.message.as_deref().unwrap().contains("left behind"),
        "{:?}",
        r.message
    );
}

// ---- teardown on every way out ----

#[test]
fn test_a_panic_in_an_action_still_tears_down() {
    let mut rig = Rig::new("normal");
    let t = spec("auto", QUERY_ONLY);
    let actions = FakeActions::new(|_| panic!("injected failure in the middle of an action"));
    let runner = rig.runner(actions);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        runner.execute_single_test(&t, &preflight(), &env_info(), &mut rig.tracker)
    }));
    assert!(result.is_err());
    assert_eq!(
        rig.fake.close_calls(),
        vec![1000],
        "unwinding ran Drop, which closed by id"
    );
    assert!(child_pids(&rig).iter().all(|p| !alive(*p)));
}

#[test]
fn test_an_interrupted_run_is_an_error_and_tears_down() {
    let mut rig = Rig::new("normal");
    rig.interrupted.store(true, Ordering::SeqCst);
    let actions = FakeActions::ok();
    let r = rig.run(&spec("auto", QUERY_ONLY), Arc::clone(&actions));
    assert_eq!(r.auto_status, AutoStatus::Error);
    assert!(actions.calls().is_empty());
    assert!(child_pids(&rig).iter().all(|p| !alive(*p)));
}

#[test]
fn test_an_action_that_cannot_run_is_an_error_and_tears_down() {
    use super::owned::ActionRunner;
    struct Broken;
    impl ActionRunner for Broken {
        fn run(
            &self,
            _: &[String],
            _: std::time::Duration,
        ) -> Result<crate::process::ProcessOutput, String> {
            Err("Command timed out after 5000ms (killed process group 1)".into())
        }
    }
    let mut rig = Rig::new("normal");
    let control: Arc<dyn Control> = rig.fake.clone();
    let options = RunOptions {
        bin_path: rig.env.dir.join("spawn-at"),
        ..Default::default()
    };
    let runner = TestRunner::new(TestRegistry::default(), options)
        .with_control(control)
        .with_actions(Arc::new(Broken))
        .with_v2_timings(fast());
    let r = runner.execute_single_test(
        &spec("auto", QUERY_ONLY),
        &preflight(),
        &env_info(),
        &mut rig.tracker,
    );
    assert_eq!(r.auto_status, AutoStatus::Error);
    assert!(r.message.as_deref().unwrap().contains("timed out"));
    assert_eq!(rig.fake.close_calls(), vec![1000]);
    assert!(child_pids(&rig).iter().all(|p| !alive(*p)));
}

// ---- focus goes back to the window that had it ----

#[test]
fn test_focus_goes_back_to_the_window_that_had_it() {
    let mut rig = Rig::new("normal");
    {
        let mut st = rig.fake.st.lock().unwrap();
        let mut user = info(5, Some(99), "the user's terminal");
        user.focused = true;
        st.extra.push(user);
    }
    let fake = Arc::clone(&rig.fake);
    let actions = FakeActions::new(move |_| {
        // The new window took focus.
        fake.patch(5, |p| p.focused = Some(false));
        out(0, "", "")
    });
    let r = rig.run(&spec("auto", QUERY_ONLY), actions);
    assert_eq!(r.auto_status, AutoStatus::AutoPass);
    assert_eq!(rig.fake.focus_calls(), vec![5]);
    assert!(
        rig.fake.close_calls().iter().all(|id| *id != 5),
        "the user's window is never closed"
    );
}

#[test]
fn test_no_refocus_when_that_window_kept_focus_or_is_gone() {
    // Still focused: nothing to restore.
    let mut rig = Rig::new("normal");
    {
        let mut user = info(5, Some(99), "the user's terminal");
        user.focused = true;
        rig.fake.st.lock().unwrap().extra.push(user);
    }
    rig.run(&spec("auto", QUERY_ONLY), FakeActions::ok());
    assert!(rig.fake.focus_calls().is_empty());

    // Gone by teardown: nothing to focus.
    let mut rig = Rig::new("normal");
    {
        let mut user = info(5, Some(99), "the user's terminal");
        user.focused = true;
        rig.fake.st.lock().unwrap().extra.push(user);
    }
    let fake = Arc::clone(&rig.fake);
    rig.run(
        &spec("auto", QUERY_ONLY),
        FakeActions::new(move |_| {
            fake.st.lock().unwrap().extra.clear();
            out(0, "", "")
        }),
    );
    assert!(rig.fake.focus_calls().is_empty());
}

#[test]
fn test_an_interrupt_between_actions_stops_the_run_and_tears_down() {
    let body = r#"
[[setup]]
name = "w"
[action]
command = "maximize"
args = ["--id", "${w}", "--no-focus"]
[[then]]
command = "restore"
args = ["--id", "${w}", "--no-focus"]
"#;
    let mut rig = Rig::new("normal");
    let flag = Arc::clone(&rig.interrupted);
    let actions = FakeActions::new(move |_| {
        flag.store(true, Ordering::SeqCst); // Ctrl-C arrives while the first action runs
        out(0, "", "")
    });
    let r = rig.run(&spec("auto", body), Arc::clone(&actions));
    assert_eq!(r.auto_status, AutoStatus::Error, "{:?}", r.message);
    assert_eq!(
        r.message.as_deref().map(|m| m.starts_with("interrupted")),
        Some(true)
    );
    assert_eq!(actions.calls().len(), 1, "the second action must not start");
    assert_eq!(rig.fake.close_calls(), vec![1000]);
    assert!(child_pids(&rig).iter().all(|p| !alive(*p)));
}

// ---- backends ----

#[test]
fn test_a_spec_for_another_backend_is_skipped_and_starts_nothing() {
    let mut rig = Rig::new("normal");
    let body = r#"
[requires]
backend = ["sway"]
[[setup]]
name = "w"
[action]
command = "defocus"
args = ["--id", "${w}"]
"#;
    let actions = FakeActions::ok();
    let r = rig.run(&spec("auto", body), Arc::clone(&actions));
    assert_eq!(r.auto_status, AutoStatus::Skipped);
    assert_eq!(r.final_status, FinalStatus::Skipped, "Skipped, not Fail");
    let m = r.message.unwrap();
    assert!(m.starts_with("unsupported on this backend ("), "{m}");
    assert!(m.ends_with("spec requires sway"), "{m}");
    assert!(actions.calls().is_empty());
    assert!(
        child_pids(&rig).is_empty(),
        "no fixture may be started for a skipped spec"
    );
}
