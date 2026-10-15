//! Tests of the ownership and teardown machinery (`docs/DESIGN_RUNNER_SETUP.md` section 3).
//!
//! A shell script stands in for the fixture and an in-memory fake for `spawn-at query windows` /
//! `close` (`test_support.rs`). The fixtures are real child processes, so "the child is dead",
//! "the decoy is alive" and "the signal came after the close" are observed, not assumed. No
//! display, no compositor, no `spawn-at` binary.

use super::owned::{parse_windows, CliControl, Control, SetupError, SetupRequest, SetupSession};
use super::test_support::{alive, fast, info, req, session, Env, Fake};
use std::os::unix::fs::PermissionsExt;
use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

static SEQ: AtomicUsize = AtomicUsize::new(0);

// ---- recording the window ----

#[test]
fn test_launch_records_the_window_by_unique_title_and_child_pid() {
    let env = Env::new("normal");
    let fake = Fake::new(&env);
    let mut s = session(&env, &fake);
    s.launch(&req("w")).unwrap();
    assert_eq!(s.window_id("w"), Some(1000));
    let title = s.title_of("w");
    assert!(
        title.starts_with("verify-") && title.ends_with("-w"),
        "{title}"
    );
    let b = s.bindings();
    assert_eq!(b.ids["w"], 1000);
    assert_eq!(b.titles["w"], title);
    // The window list says the same pid as the child the session started.
    let pid = u64::from(s.child_pid("w").unwrap());
    assert_eq!(fake.windows().unwrap()[0].pid, Some(pid));
}

#[test]
fn test_titles_differ_between_sessions_and_setups() {
    let env = Env::new("normal");
    let fake = Fake::new(&env);
    let a = session(&env, &fake);
    let b = session(&env, &fake);
    assert_ne!(a.title_of("w"), b.title_of("w"));
    assert_ne!(a.title_of("w"), a.title_of("v"));
}

#[test]
fn test_two_windows_with_the_title_are_ambiguous_and_nothing_is_closed() {
    let env = Env::new("normal");
    let fake = Fake::new(&env);
    let mut s = session(&env, &fake);
    let title = s.title_of("w");
    {
        let mut st = fake.st.lock().unwrap();
        st.extra.push(info(77, None, &title));
        st.extra_needs_state = true;
    }
    let err = s.launch(&req("w")).unwrap_err();
    assert_eq!(err, SetupError::Ambiguous(vec![1000, 77]));
    assert_eq!(s.window_id("w"), None, "an ambiguous match records nothing");
    let pid = s.child_pid("w").unwrap();
    let report = s.teardown();
    assert!(
        fake.close_calls().is_empty(),
        "neither window may be closed"
    );
    assert!(report.left_behind.is_empty());
    assert!(!alive(pid), "our own child is still stopped");
}

#[test]
fn test_a_window_with_our_title_but_another_pid_is_not_ours_and_survives() {
    let env = Env::new("no-window");
    let fake = Fake::new(&env);
    // A process the test started, standing for someone else's window.
    let mut decoy = Command::new("sleep").arg("30").spawn().unwrap();
    let mut s = session(&env, &fake);
    let title = s.title_of("w");
    fake.st
        .lock()
        .unwrap()
        .extra
        .push(info(55, Some(u64::from(decoy.id())), &title));
    let err = s.launch(&req("w")).unwrap_err();
    let child = s.child_pid("w").unwrap();
    assert_eq!(
        err,
        SetupError::PidMismatch {
            expected: u64::from(child),
            got: u64::from(decoy.id())
        }
    );
    drop(s);
    assert!(
        fake.close_calls().is_empty(),
        "the foreign window must not be closed"
    );
    assert!(
        alive(decoy.id()),
        "the foreign process must not be signalled"
    );
    assert!(!alive(child), "our own child is stopped through its handle");
    let _ = decoy.kill();
    let _ = decoy.wait();
}

#[test]
fn test_no_window_is_a_setup_error_and_the_child_is_stopped() {
    let env = Env::new("no-window");
    let fake = Fake::new(&env);
    let mut s = session(&env, &fake);
    let err = s.launch(&req("w")).unwrap_err();
    assert!(matches!(err, SetupError::NoWindow(_)), "{err:?}");
    let pid = s.child_pid("w").unwrap();
    drop(s);
    assert!(!alive(pid));
    assert!(
        fake.close_calls().is_empty(),
        "no id was recorded, so nothing is closed by id"
    );
}

#[test]
fn test_a_fixture_that_exits_at_once_is_reported_with_its_stderr() {
    let env = Env::new("exit-now");
    let fake = Fake::new(&env);
    let mut s = session(&env, &fake);
    match s.launch(&req("w")).unwrap_err() {
        SetupError::ExitedEarly { code, stderr } => {
            assert_eq!(code, Some(3));
            assert!(stderr.contains("boom"), "{stderr}");
        }
        other => panic!("expected ExitedEarly, got {other:?}"),
    }
    assert!(fake.close_calls().is_empty());
}

#[test]
fn test_a_failed_query_is_an_error_not_an_empty_list() {
    let env = Env::new("normal");
    let fake = Fake::new(&env);
    fake.st.lock().unwrap().fail_query = true;
    let mut s = session(&env, &fake);
    let err = s.launch(&req("w")).unwrap_err();
    assert!(matches!(err, SetupError::QueryFailed(_)), "{err:?}");
    let pid = s.child_pid("w").unwrap();
    drop(s);
    assert!(!alive(pid));
    assert!(fake.close_calls().is_empty());
}

#[test]
fn test_interrupt_flag_ends_the_wait_and_tears_down() {
    let env = Env::new("no-window");
    let fake = Fake::new(&env);
    let flag = Arc::new(AtomicBool::new(true));
    let ctl: Arc<dyn Control> = fake.clone();
    let mut s = SetupSession::new(ctl, env.fixture(), "t", 10_000, fast(), flag).unwrap();
    let err = s.launch(&req("w")).unwrap_err();
    assert_eq!(err, SetupError::Interrupted);
    let pid = s.child_pid("w").unwrap();
    drop(s);
    assert!(!alive(pid));
}

#[test]
fn test_flags_the_runner_owns_are_refused_at_launch() {
    let env = Env::new("normal");
    let fake = Fake::new(&env);
    let mut s = session(&env, &fake);
    for bad in [
        vec!["--title", "mine"],
        vec!["--app-id", "mine"],
        vec!["--exit-after-ms", "99999999"],
        vec!["--protocol", "x11"],
        vec!["--print-ready"],
    ] {
        let r = SetupRequest {
            name: "w".into(),
            args: bad.iter().map(|s| s.to_string()).collect(),
            timeout: None,
        };
        assert!(
            matches!(s.launch(&r), Err(SetupError::BadRequest(_))),
            "{bad:?}"
        );
        assert_eq!(
            s.len(),
            0,
            "no process may be started for a refused request"
        );
    }
}

#[test]
fn test_missing_fixture_binary_is_reported_before_anything_starts() {
    let env = Env::new("normal");
    let fake = Fake::new(&env);
    let ctl: Arc<dyn Control> = fake.clone();
    let mut s = SetupSession::new(
        ctl,
        env.dir.join("no-such-fixture"),
        "t",
        10_000,
        fast(),
        Arc::new(AtomicBool::new(false)),
    )
    .unwrap();
    assert!(matches!(
        s.launch(&req("w")),
        Err(SetupError::FixtureMissing(_))
    ));
    assert!(s.is_empty());
}

#[test]
fn test_one_name_cannot_be_launched_twice() {
    let env = Env::new("normal");
    let fake = Fake::new(&env);
    let mut s = session(&env, &fake);
    s.launch(&req("w")).unwrap();
    assert!(matches!(
        s.launch(&req("w")),
        Err(SetupError::BadRequest(_))
    ));
    assert_eq!(s.len(), 1);
}

#[test]
fn test_halfway_failure_closes_the_first_window_by_its_recorded_id() {
    let env = Env::new("normal");
    let fake = Fake::new(&env);
    let mut s = session(&env, &fake);
    s.launch(&req("a")).unwrap();
    let a_pid = s.child_pid("a").unwrap();
    env.set_mode("exit-now");
    assert!(s.launch(&req("b")).is_err());
    drop(s);
    assert_eq!(
        fake.close_calls(),
        vec![1000],
        "only a's recorded window is closed"
    );
    assert!(!alive(a_pid));
}

// ---- re-verification before an action ----

#[test]
fn test_verify_target_accepts_the_recorded_window_and_refuses_any_change() {
    let env = Env::new("normal");
    let fake = Fake::new(&env);
    let mut s = session(&env, &fake);
    s.launch(&req("w")).unwrap();
    assert_eq!(s.verify_target("w").unwrap().id, 1000);

    // The id now carries another title.
    fake.st
        .lock()
        .unwrap()
        .retitle
        .insert(1000, "someone else".into());
    let e = s.verify_target("w").unwrap_err();
    assert!(
        e.contains("target changed") && e.contains("someone else"),
        "{e}"
    );
    fake.st.lock().unwrap().retitle.clear();

    // The window is gone.
    fake.st.lock().unwrap().hidden.push(1000);
    assert!(s
        .verify_target("w")
        .unwrap_err()
        .contains("no longer listed"));
    fake.st.lock().unwrap().hidden.clear();

    // The window is listed with our title but another pid.
    {
        let mut st = fake.st.lock().unwrap();
        st.hidden.push(1000);
        let title = s.title_of("w");
        st.extra.push(info(1000, Some(1), &title));
    }
    assert!(s
        .verify_target("w")
        .unwrap_err()
        .contains("belongs to pid 1"));

    // An unknown name is an error, not a lookup of something else.
    assert!(s.verify_target("zzz").is_err());
}

#[test]
fn test_read_returns_none_for_a_missing_window_and_ignores_a_retitled_one() {
    let env = Env::new("normal");
    let fake = Fake::new(&env);
    let mut s = session(&env, &fake);
    s.launch(&req("w")).unwrap();
    assert!(s.read("w").unwrap().is_some());
    fake.st.lock().unwrap().retitle.insert(1000, "other".into());
    assert!(s.read("w").unwrap().is_none());
}

#[test]
fn test_nonexistent_id_is_above_every_listed_id_and_rechecked() {
    let env = Env::new("normal");
    let fake = Fake::new(&env);
    let mut s = session(&env, &fake);
    s.launch(&req("w")).unwrap();
    let id = s.choose_nonexistent().unwrap();
    assert_eq!(id, 1000 + 1_000_000);
    assert!(s.ensure_absent(id).is_ok());
    fake.st
        .lock()
        .unwrap()
        .extra
        .push(info(id, None, "appeared meanwhile"));
    assert!(s.ensure_absent(id).unwrap_err().contains("was not run"));
}

// ---- teardown ----

#[test]
fn test_teardown_closes_by_id_before_any_signal() {
    let env = Env::new("record-term");
    let fake = Fake::new(&env);
    fake.st.lock().unwrap().ignore_close = true;
    let mut s = session(&env, &fake);
    s.launch(&req("w")).unwrap();
    drop(s);
    assert_eq!(
        env.events(),
        vec!["close 1000", "TERM"],
        "close first, then SIGTERM"
    );
}

#[test]
fn test_teardown_skips_the_close_when_the_id_carries_another_title() {
    let env = Env::new("normal");
    let fake = Fake::new(&env);
    let mut s = session(&env, &fake);
    s.launch(&req("w")).unwrap();
    let pid = s.child_pid("w").unwrap();
    fake.st
        .lock()
        .unwrap()
        .retitle
        .insert(1000, "not ours".into());
    let report = s.teardown();
    assert!(
        fake.close_calls().is_empty(),
        "an id must not be closed because we remember it"
    );
    assert!(
        report
            .warnings
            .iter()
            .any(|w| w.contains("did not close window 1000")),
        "{report:?}"
    );
    assert!(!alive(pid), "our own child is still stopped");
}

#[test]
fn test_teardown_skips_the_close_when_the_window_is_gone() {
    let env = Env::new("normal");
    let fake = Fake::new(&env);
    let mut s = session(&env, &fake);
    s.launch(&req("w")).unwrap();
    fake.st.lock().unwrap().hidden.push(1000);
    let report = s.teardown();
    assert!(fake.close_calls().is_empty());
    assert!(report.warnings.is_empty() && report.left_behind.is_empty());
}

#[test]
fn test_teardown_does_not_close_when_the_title_cannot_be_checked() {
    let env = Env::new("normal");
    let fake = Fake::new(&env);
    let mut s = session(&env, &fake);
    s.launch(&req("w")).unwrap();
    let pid = s.child_pid("w").unwrap();
    fake.st.lock().unwrap().fail_query = true;
    let report = s.teardown();
    assert!(fake.close_calls().is_empty());
    assert!(
        report
            .warnings
            .iter()
            .any(|w| w.contains("cannot verify its title")),
        "{report:?}"
    );
    assert!(!alive(pid));
}

#[test]
fn test_a_child_that_ignores_sigterm_is_killed() {
    let env = Env::new("ignore-term");
    let fake = Fake::new(&env);
    let mut s = session(&env, &fake);
    s.launch(&req("w")).unwrap();
    let pid = s.child_pid("w").unwrap();
    assert!(alive(pid));
    let start = Instant::now();
    s.teardown();
    assert!(!alive(pid), "SIGKILL through the handle ends it");
    assert!(start.elapsed() < Duration::from_secs(3));
}

#[test]
fn test_leftover_window_is_reported() {
    let env = Env::new("normal");
    let fake = Fake::new(&env);
    fake.st.lock().unwrap().ignore_close = true;
    let mut s = session(&env, &fake);
    s.launch(&req("w")).unwrap();
    let report = s.teardown();
    assert_eq!(report.left_behind, vec![1000]);
}

#[test]
fn test_teardown_runs_once() {
    let env = Env::new("normal");
    let fake = Fake::new(&env);
    let mut s = session(&env, &fake);
    s.launch(&req("w")).unwrap();
    let first = s.teardown();
    let second = s.teardown();
    drop(s);
    assert_eq!(first, second);
    assert_eq!(
        fake.close_calls(),
        vec![1000],
        "one close, however often teardown is called"
    );
}

#[test]
fn test_dropping_the_session_tears_down() {
    let env = Env::new("normal");
    let fake = Fake::new(&env);
    let mut s = session(&env, &fake);
    s.launch(&req("w")).unwrap();
    let pid = s.child_pid("w").unwrap();
    drop(s);
    assert_eq!(fake.close_calls(), vec![1000]);
    assert!(!alive(pid));
}

#[test]
fn test_a_panic_still_tears_down() {
    let env = Env::new("normal");
    let fake = Fake::new(&env);
    let pid = Arc::new(Mutex::new(0u32));
    let pid2 = Arc::clone(&pid);
    let (env_ref, fake_ref) = (&env, &fake);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut s = session(env_ref, fake_ref);
        s.launch(&req("w")).unwrap();
        *pid2.lock().unwrap() = s.child_pid("w").unwrap();
        panic!("injected failure in the middle of a test");
    }));
    assert!(result.is_err());
    assert_eq!(
        fake.close_calls(),
        vec![1000],
        "unwinding ran Drop, which closed by id"
    );
    assert!(!alive(*pid.lock().unwrap()));
}

// ---- the real Control over a stand-in spawn-at ----

#[test]
fn test_cli_control_parses_the_window_list_and_reports_failures() {
    let dir = std::env::temp_dir().join(format!(
        "spawn-at-owned-cli-{}-{}",
        std::process::id(),
        SEQ.fetch_add(1, Ordering::SeqCst)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let bin = dir.join("spawn-at");
    std::fs::write(
        &bin,
        r#"#!/bin/sh
case "$1" in
  query) [ -f "$(dirname "$0")/fail" ] && { echo "extension down" >&2; exit 1; }
         echo '[{"id": 5, "pid": 99, "title": "T", "class": "c", "app_id": "a", "x": 1, "y": 2, "w": 3, "h": 4, "focused": true, "maximized": false, "minimized": false}]' ;;
  close) [ "$3" = "5" ] && exit 0; exit 1 ;;
esac
"#,
    )
    .unwrap();
    std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).unwrap();
    let c = CliControl { bin: bin.clone() };
    let w = c.windows().unwrap();
    assert_eq!(w.len(), 1);
    assert_eq!((w[0].id, w[0].pid, w[0].title.as_str()), (5, Some(99), "T"));
    assert!(w[0].focused && !w[0].maximized);
    assert!(w[0].rect.is_some());
    assert!(c.close(5));
    assert!(!c.close(6));
    std::fs::write(dir.join("fail"), "").unwrap();
    let e = c.windows().unwrap_err();
    assert!(e.contains("extension down"), "{e}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_parse_windows_rejects_non_arrays_and_skips_entries_without_an_id() {
    assert!(parse_windows(b"{}").is_err());
    assert!(parse_windows(b"not json").is_err());
    let w = parse_windows(br#"[{"title":"no id"},{"id":2,"title":"x"}]"#).unwrap();
    assert_eq!(w.len(), 1);
    assert_eq!(w[0].id, 2);
    assert!(w[0].rect.is_none());
}
