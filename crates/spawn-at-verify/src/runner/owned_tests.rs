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
            ..Default::default()
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

// ---- a real program instead of the fixture (`[[setup]] command`) ----

/// A stand-in for a real program: a script with a bare name that registers a window the way the
/// fake fixture does (`<pid> <title>` in `state`), then stays alive. Returns the name, and puts
/// `env.dir` in front of the child's `PATH` through the session.
fn real_program(env: &Env) -> String {
    let name = format!("vrf-real-{}", SEQ.fetch_add(1, Ordering::SeqCst));
    let path = env.dir.join(&name);
    std::fs::write(
        &path,
        "#!/bin/sh\nprintf '%s %s\\n' \"$$\" \"Real Program Window\" >> \"$(dirname \"$0\")/state\"\nexec sleep 30\n",
    )
    .unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    name
}

fn real_request(name: &str, program: &str) -> SetupRequest {
    SetupRequest {
        name: name.into(),
        command: vec![program.into()],
        timeout: Some(Duration::from_millis(1500)),
        ..Default::default()
    }
}

fn with_program_on_path(s: SetupSession, env: &Env) -> SetupSession {
    let old = std::env::var("PATH").unwrap_or_default();
    s.with_env("PATH", &format!("{}:{old}", env.dir.display()))
}

#[test]
fn test_a_real_program_window_is_the_new_window_with_the_childs_pid() {
    let env = Env::new("normal");
    let fake = Fake::new(&env);
    let program = real_program(&env);
    let mut s = with_program_on_path(session(&env, &fake), &env);
    // A window that was there before the program started, with another pid: never taken.
    fake.st
        .lock()
        .unwrap()
        .extra
        .push(info(77, Some(1), "Old window"));

    s.launch(&real_request("w", &program)).unwrap();

    assert_eq!(s.window_id("w"), Some(1000));
    let pid = u64::from(s.child_pid("w").unwrap());
    assert_eq!(fake.windows().unwrap()[0].pid, Some(pid));
    // The title is the program's own, recorded, and the target check uses it.
    assert_eq!(s.verify_target("w").unwrap().title, "Real Program Window");
    s.teardown();
    assert_eq!(
        fake.close_calls(),
        vec![1000],
        "only its own window is closed"
    );
    assert!(!alive(pid as u32));
}

#[test]
fn test_a_real_program_with_two_new_windows_is_ambiguous_and_nothing_is_closed() {
    let env = Env::new("normal");
    let fake = Fake::new(&env);
    let name = format!("vrf-real-{}", SEQ.fetch_add(1, Ordering::SeqCst));
    let path = env.dir.join(&name);
    std::fs::write(
        &path,
        "#!/bin/sh\nD=\"$(dirname \"$0\")\"\nprintf '%s %s\\n' \"$$\" \"First\" >> \"$D/state\"\nprintf '%s %s\\n' \"$$\" \"Second\" >> \"$D/state\"\nexec sleep 30\n",
    )
    .unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    let mut s = with_program_on_path(session(&env, &fake), &env);

    let err = s.launch(&real_request("w", &name)).unwrap_err();
    assert!(matches!(err, SetupError::Ambiguous(_)), "{err}");
    assert_eq!(s.window_id("w"), None);
    s.teardown();
    assert!(
        fake.close_calls().is_empty(),
        "no window is closed when none was recorded"
    );
}

#[test]
fn test_a_real_program_that_ends_before_showing_a_window_is_a_setup_error() {
    let env = Env::new("exit-now");
    let fake = Fake::new(&env);
    let name = format!("vrf-real-{}", SEQ.fetch_add(1, Ordering::SeqCst));
    let path = env.dir.join(&name);
    std::fs::write(&path, "#!/bin/sh\necho gone >&2\nexit 3\n").unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    let mut s = with_program_on_path(session(&env, &fake), &env);

    let err = s.launch(&real_request("w", &name)).unwrap_err();
    assert!(matches!(err, SetupError::ExitedEarly { .. }), "{err}");
    assert!(fake.close_calls().is_empty());
}

#[test]
fn test_a_real_program_request_is_refused_when_it_is_not_a_bare_name_or_not_allowed_env() {
    let env = Env::new("normal");
    let fake = Fake::new(&env);
    let mut s = session(&env, &fake);

    let mut path = real_request("w", "/bin/sleep");
    assert!(matches!(s.launch(&path), Err(SetupError::BadRequest(_))));
    path.command = vec!["sleep".into(), "1\0".into()];
    assert!(matches!(s.launch(&path), Err(SetupError::BadRequest(_))));

    let mut ld = real_request("w", "sleep");
    ld.env.insert("LD_PRELOAD".into(), "/tmp/x.so".into());
    assert!(matches!(s.launch(&ld), Err(SetupError::BadRequest(_))));

    let mut value = real_request("w", "sleep");
    value.env.insert("GDK_BACKEND".into(), "mir".into());
    assert!(matches!(s.launch(&value), Err(SetupError::BadRequest(_))));

    let mut mixed = real_request("w", "sleep");
    mixed.args = vec!["--width".into(), "400".into()];
    assert!(matches!(s.launch(&mixed), Err(SetupError::BadRequest(_))));

    let mut env_only = req("w");
    env_only.env.insert("GDK_BACKEND".into(), "x11".into());
    assert!(matches!(
        s.launch(&env_only),
        Err(SetupError::BadRequest(_))
    ));
    assert!(env.fixture_pids().is_empty(), "nothing was started");
}

// ---- the Shell identity behind `shell_alive` ----

#[test]
fn test_parse_stat_start_reads_field_22_after_the_command_name() {
    let line = "1234 (gnome-shell) S 1 1234 1234 0 -1 4194560 100 0 0 0 5 6 0 0 20 0 14 0 987654 12345 678 18446744073709551615";
    assert_eq!(super::owned::parse_stat_start(line), Some(987654));
    // A name with spaces and a ')' in it must not shift the fields.
    let odd = "77 (a b) c) R 1 77 77 0 -1 0 0 0 0 0 0 0 0 0 20 0 1 0 424242 1 1 1";
    assert_eq!(super::owned::parse_stat_start(odd), Some(424242));
    assert_eq!(super::owned::parse_stat_start("garbage"), None);
    assert_eq!(super::owned::parse_stat_start("1 (x) S 1 2"), None);
}

use super::owned::ShellId;
static PROBE_PID: AtomicUsize = AtomicUsize::new(0);
static PROBE_START: AtomicUsize = AtomicUsize::new(0);
/// The probe is a plain `fn`, so it reads these statics; the tests that set them take turns.
static PROBE_LOCK: Mutex<()> = Mutex::new(());
fn probe_now() -> Option<ShellId> {
    match PROBE_PID.load(Ordering::SeqCst) {
        0 => None,
        pid => Some(ShellId {
            pid: pid as u32,
            start: PROBE_START.load(Ordering::SeqCst) as u64,
        }),
    }
}

#[test]
fn test_shell_alive_holds_for_the_same_process_and_fails_for_a_new_or_missing_one() {
    let _only = PROBE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let env = Env::new("normal");
    let fake = Fake::new(&env);
    PROBE_PID.store(500, Ordering::SeqCst);
    PROBE_START.store(9000, Ordering::SeqCst);
    let s = session(&env, &fake).with_shell_probe(probe_now);
    assert_eq!(s.check_shell_alive(), Ok(()));

    // Same pid, other start time: the pid was reused by a new Shell.
    PROBE_START.store(9500, Ordering::SeqCst);
    let e = s.check_shell_alive().unwrap_err();
    assert!(e.contains("500") && e.contains("new one started"), "{e}");

    // Another pid: the session restarted.
    PROBE_PID.store(731, Ordering::SeqCst);
    PROBE_START.store(9000, Ordering::SeqCst);
    assert!(s.check_shell_alive().is_err());

    // No Shell at all (or several): not alive.
    PROBE_PID.store(0, Ordering::SeqCst);
    let e = s.check_shell_alive().unwrap_err();
    assert!(e.contains("no longer running"), "{e}");
}

#[test]
fn test_shell_alive_is_an_error_when_the_shell_was_not_identified_at_the_start() {
    let _only = PROBE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let env = Env::new("normal");
    let fake = Fake::new(&env);
    PROBE_PID.store(0, Ordering::SeqCst);
    let s = session(&env, &fake).with_shell_probe(probe_now);
    // Identified now but not at the start: nothing to compare with, so not a pass.
    PROBE_PID.store(500, Ordering::SeqCst);
    let e = s.check_shell_alive().unwrap_err();
    assert!(e.contains("could not be identified"), "{e}");
}
