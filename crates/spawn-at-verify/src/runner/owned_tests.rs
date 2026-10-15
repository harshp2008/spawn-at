//! Tests of the ownership and teardown machinery (`docs/DESIGN_RUNNER_SETUP.md` section 3).
//!
//! A shell script stands in for the fixture and an in-memory fake for `spawn-at query windows` /
//! `close`. The fixtures are real child processes, so "the child is dead", "the decoy is alive"
//! and "the signal came after the close" are observed, not assumed. No display, no compositor,
//! no `spawn-at` binary.

use super::owned::{
    parse_windows, CliControl, Control, SetupError, SetupRequest, SetupSession, Timings, WindowInfo,
};
use std::collections::HashMap;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

static SEQ: AtomicUsize = AtomicUsize::new(0);

const SCRIPT: &str = r#"#!/bin/sh
D="$(dirname "$0")"
title=""
while [ $# -gt 0 ]; do
  case "$1" in --title) title="$2"; shift ;; esac
  shift
done
mode="$(cat "$D/mode" 2>/dev/null)"
case "$mode" in
  exit-now) echo "boom" >&2; exit 3 ;;
  no-window) exec sleep 30 ;;
  ignore-term) trap '' TERM; printf '%s %s\n' "$$" "$title" >> "$D/state"; while :; do sleep 0.05; done ;;
  record-term) trap 'echo TERM >> "$D/events"; exit 0' TERM; printf '%s %s\n' "$$" "$title" >> "$D/state"; while :; do sleep 0.05; done ;;
  *) printf '%s %s\n' "$$" "$title" >> "$D/state"; exec sleep 30 ;;
esac
"#;

/// Scratch directory holding the fake fixture and the files it and the fake control share.
struct Env {
    dir: PathBuf,
}

impl Env {
    fn new(mode: &str) -> Env {
        let dir = std::env::temp_dir().join(format!(
            "spawn-at-owned-test-{}-{}",
            std::process::id(),
            SEQ.fetch_add(1, Ordering::SeqCst)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let script = dir.join("fixture");
        std::fs::write(&script, SCRIPT).unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        let env = Env { dir };
        env.set_mode(mode);
        env
    }
    fn set_mode(&self, mode: &str) {
        std::fs::write(self.dir.join("mode"), mode).unwrap();
    }
    fn fixture(&self) -> PathBuf {
        self.dir.join("fixture")
    }
    fn events(&self) -> Vec<String> {
        std::fs::read_to_string(self.dir.join("events"))
            .unwrap_or_default()
            .lines()
            .map(str::to_string)
            .collect()
    }
}

impl Drop for Env {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

#[derive(Default)]
struct FakeState {
    hidden: Vec<u64>,
    extra: Vec<WindowInfo>,
    retitle: HashMap<u64, String>,
    fail_query: bool,
    ignore_close: bool,
    close_calls: Vec<u64>,
    /// List the `extra` windows only once the fake fixture has registered its own, so a test
    /// that wants "our window plus a second one" does not race the fixture's start.
    extra_needs_state: bool,
}

/// In-memory `spawn-at`: the windows are the lines the fake fixture wrote (`<pid> <title>`), ids
/// 1000, 1001, ..., plus whatever the test adds.
struct Fake {
    dir: PathBuf,
    st: Mutex<FakeState>,
}

impl Fake {
    fn new(env: &Env) -> Arc<Fake> {
        Arc::new(Fake {
            dir: env.dir.clone(),
            st: Mutex::new(FakeState::default()),
        })
    }
    fn close_calls(&self) -> Vec<u64> {
        self.st.lock().unwrap().close_calls.clone()
    }
}

fn info(id: u64, pid: Option<u64>, title: &str) -> WindowInfo {
    WindowInfo {
        id,
        pid,
        title: title.to_string(),
        app_id: None,
        class: None,
        rect: None,
        focused: false,
        maximized: false,
        minimized: false,
    }
}

impl Control for Fake {
    fn windows(&self) -> Result<Vec<WindowInfo>, String> {
        let st = self.st.lock().unwrap();
        if st.fail_query {
            return Err("query windows failed (exit Some(1)): no extension".into());
        }
        let text = std::fs::read_to_string(self.dir.join("state")).unwrap_or_default();
        let mut out = Vec::new();
        for (i, line) in text.lines().enumerate() {
            let Some((pid, title)) = line.split_once(' ') else {
                continue; // a line still being written
            };
            let id = 1000 + i as u64;
            if st.hidden.contains(&id) {
                continue;
            }
            let title = st.retitle.get(&id).cloned().unwrap_or(title.to_string());
            out.push(info(id, Some(pid.parse().unwrap()), &title));
        }
        if !st.extra_needs_state || !out.is_empty() {
            out.extend(st.extra.iter().cloned());
        }
        Ok(out)
    }

    fn close(&self, id: u64) -> bool {
        use std::io::Write;
        let mut st = self.st.lock().unwrap();
        st.close_calls.push(id);
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.dir.join("events"))
            .unwrap();
        writeln!(f, "close {id}").unwrap();
        if !st.ignore_close {
            st.hidden.push(id);
        }
        true
    }
}

fn fast() -> Timings {
    Timings {
        setup_default: Duration::from_millis(1500),
        poll: Duration::from_millis(20),
        exit_grace: Duration::from_millis(300),
        term_grace: Duration::from_millis(300),
    }
}

fn session(env: &Env, fake: &Arc<Fake>) -> SetupSession {
    let ctl: Arc<dyn Control> = fake.clone();
    SetupSession::new(
        ctl,
        env.fixture(),
        "t",
        10_000,
        fast(),
        Arc::new(AtomicBool::new(false)),
    )
    .unwrap()
}

fn req(name: &str) -> SetupRequest {
    SetupRequest {
        name: name.to_string(),
        args: vec!["--width".into(), "400".into()],
        timeout: Some(Duration::from_millis(400)),
    }
}

/// True while a process with this pid exists (a zombie counts; a reaped child does not).
fn alive(pid: u32) -> bool {
    // SAFETY: signal 0 only checks that the pid exists.
    unsafe { libc::kill(pid as libc::pid_t, 0) == 0 }
}

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
