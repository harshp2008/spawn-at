//! `spawn-at-verify run` ends cleanly on Ctrl-C, SIGTERM and SIGHUP (`docs/DESIGN_RUNNER_SETUP.md`
//! section 3.4). The real `run` binary is started against stand-in scripts in a temporary
//! directory, with no display and no D-Bus in its environment, and signalled while its test is
//! waiting inside an action. It must close the window it recorded (by id), stop the fixture
//! process it started, run no further action, write nothing outside the temporary directory and
//! exit with 130.

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Stand-in for the fixture: registers `<pid> <title>` in `state`, then waits.
const FIXTURE: &str = r#"#!/bin/sh
D="$(dirname "$0")"
title=""
while [ $# -gt 0 ]; do
  case "$1" in --title) title="$2"; shift ;; esac
  shift
done
printf '%s %s\n' "$$" "$title" >> "$D/state"
exec sleep 60
"#;

/// Stand-in for `spawn-at`: windows are the lines of `state`; `focus` takes 3 s; `close` and
/// `restore` leave a line in `events`.
const SPAWN_AT: &str = r#"#!/bin/sh
D="$(dirname "$0")"
case "$1" in
  --version) printf 'spawn-at test double\nPrevious Commit ID: abc1234\n' ;;
  query)
    case "$2" in
      windows)
        [ -f "$D/state" ] || { echo '[]'; exit 0; }
        awk 'BEGIN{printf "["} {sep=(NR>1)?",":""; printf "%s{\"id\":%d,\"pid\":%d,\"title\":\"%s\",\"x\":0,\"y\":0,\"w\":400,\"h\":300,\"focused\":false,\"maximized\":false,\"minimized\":false}", sep, 999+NR, $1, $2} END{print "]"}' "$D/state" ;;
      layout) echo '{"schema_version":1,"monitors":[{"index":0,"primary":true,"workarea":{"x":0,"y":0,"w":1920,"h":1080}}]}' ;;
    esac ;;
  close) echo "close $3" >> "$D/events" ;;
  focus) echo "focus" >> "$D/events"; sleep 3 ;;
  restore) echo "restore" >> "$D/events" ;;
esac
"#;

const SPEC: &str = r#"
schema_version = 2
id = "transforms/slow"
category = "transforms"
title = "slow"
description = "a first action that takes 3 s, then a second that must never start once interrupted"
kind = "auto"
tags = ["moves-focus"]

[[setup]]
name = "w"

[action]
command = "focus"
args = ["--id", "${w}"]

[[then]]
command = "restore"
args = ["--id", "${w}"]
"#;

fn write_exec(path: &Path, text: &str) {
    std::fs::write(path, text).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

struct Rig {
    root: PathBuf,
}

impl Rig {
    fn new(tag: &str) -> Rig {
        let root = std::env::temp_dir().join(format!(
            "spawn-at-verify-signal-{}-{}",
            std::process::id(),
            tag
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("bin")).unwrap();
        std::fs::create_dir_all(root.join("verify/transforms")).unwrap();
        write_exec(&root.join("bin/spawn-at"), SPAWN_AT);
        write_exec(&root.join("bin/fixture"), FIXTURE);
        std::fs::write(root.join("verify/transforms/slow.toml"), SPEC).unwrap();
        Rig { root }
    }
    fn lines(&self, file: &str) -> Vec<String> {
        std::fs::read_to_string(self.root.join("bin").join(file))
            .unwrap_or_default()
            .lines()
            .map(str::to_string)
            .collect()
    }
}

impl Drop for Rig {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn alive(pid: i32) -> bool {
    // SAFETY: signal 0 only checks that the pid exists.
    unsafe { libc::kill(pid, 0) == 0 }
}

/// Starts `run`, waits until the fixture has registered, sends `signal`, and returns the exit
/// code, the stderr text and the rig for inspection.
fn run_and_signal(tag: &str, signal: i32) -> (Option<i32>, String, Rig) {
    let rig = Rig::new(tag);
    let mut child = Command::new(env!("CARGO_BIN_EXE_spawn-at-verify"))
        .args([
            "run",
            "--bin",
            rig.root.join("bin/spawn-at").to_str().unwrap(),
            "--dir",
            rig.root.join("verify").to_str().unwrap(),
            "--glob",
            "transforms/*",
        ])
        .current_dir(&rig.root)
        .env_remove("DISPLAY")
        .env_remove("WAYLAND_DISPLAY")
        .env_remove("XDG_CURRENT_DESKTOP")
        .env_remove("DBUS_SESSION_BUS_ADDRESS")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    // The first action (`focus`) is running once `events` says so: the window is recorded.
    let deadline = Instant::now() + Duration::from_secs(20);
    while !rig.lines("events").contains(&"focus".to_string()) {
        assert!(
            Instant::now() < deadline,
            "the run never reached the first action"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
    // SAFETY: signalling the child this test started.
    unsafe { libc::kill(child.id() as i32, signal) };

    let deadline = Instant::now() + Duration::from_secs(20);
    let status = loop {
        if let Some(s) = child.try_wait().unwrap() {
            break s;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("run did not end within 20 s of the signal");
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    let out = child.wait_with_output().unwrap();
    (
        status.code(),
        String::from_utf8_lossy(&out.stderr).to_string(),
        rig,
    )
}

fn assert_clean_interrupted_run(code: Option<i32>, stderr: &str, rig: &Rig) {
    assert_eq!(code, Some(130), "stderr: {stderr}");
    assert!(stderr.contains("Interrupted"), "{stderr}");
    let events = rig.lines("events");
    assert_eq!(events.first().map(String::as_str), Some("focus"));
    assert!(
        !events.contains(&"restore".to_string()),
        "the second action must not start after the signal: {events:?}"
    );
    assert!(
        events.contains(&"close 1000".to_string()),
        "the recorded window is closed by its id: {events:?}"
    );
    let state = rig.lines("state");
    assert_eq!(state.len(), 1, "one fixture was started: {state:?}");
    let pid: i32 = state[0].split(' ').next().unwrap().parse().unwrap();
    assert!(
        !alive(pid),
        "the fixture process the run started is stopped"
    );
}

#[test]
fn test_sigterm_ends_run_with_teardown_and_exit_130() {
    let (code, stderr, rig) = run_and_signal("term", libc::SIGTERM);
    assert_clean_interrupted_run(code, &stderr, &rig);
}

#[test]
fn test_sigint_ends_run_with_teardown_and_exit_130() {
    let (code, stderr, rig) = run_and_signal("int", libc::SIGINT);
    assert_clean_interrupted_run(code, &stderr, &rig);
}

#[test]
fn test_sighup_ends_run_with_teardown_and_exit_130() {
    let (code, stderr, rig) = run_and_signal("hup", libc::SIGHUP);
    assert_clean_interrupted_run(code, &stderr, &rig);
}

/// Without a signal the same spec runs to the end: both actions start, nothing is left.
#[test]
fn test_without_a_signal_both_actions_run_and_the_window_is_closed() {
    let rig = Rig::new("none");
    let out = Command::new(env!("CARGO_BIN_EXE_spawn-at-verify"))
        .args([
            "run",
            "--bin",
            rig.root.join("bin/spawn-at").to_str().unwrap(),
            "--dir",
            rig.root.join("verify").to_str().unwrap(),
            "--glob",
            "transforms/*",
        ])
        .current_dir(&rig.root)
        .env_remove("DISPLAY")
        .env_remove("WAYLAND_DISPLAY")
        .env_remove("XDG_CURRENT_DESKTOP")
        .env_remove("DBUS_SESSION_BUS_ADDRESS")
        .output()
        .unwrap();
    let text =
        String::from_utf8_lossy(&out.stdout).to_string() + &String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "{text}");
    let events = rig.lines("events");
    assert_eq!(events, vec!["focus", "restore", "close 1000"], "{text}");
    let pid: i32 = rig.lines("state")[0]
        .split(' ')
        .next()
        .unwrap()
        .parse()
        .unwrap();
    assert!(!alive(pid));
}

/// With `--repeat`, a signal ends the repetitions: the cut-short run is dropped, the test is not
/// reported as flaky, and no further repetition starts a fixture.
#[test]
fn test_signal_during_repeat_is_not_reported_as_flaky_and_starts_nothing_more() {
    let rig = Rig::new("repeat");
    let child = Command::new(env!("CARGO_BIN_EXE_spawn-at-verify"))
        .args([
            "run",
            "--bin",
            rig.root.join("bin/spawn-at").to_str().unwrap(),
            "--dir",
            rig.root.join("verify").to_str().unwrap(),
            "--glob",
            "transforms/*",
            "--repeat",
            "3",
        ])
        .current_dir(&rig.root)
        .env_remove("DISPLAY")
        .env_remove("WAYLAND_DISPLAY")
        .env_remove("XDG_CURRENT_DESKTOP")
        .env_remove("DBUS_SESSION_BUS_ADDRESS")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    // The first repetition runs to the end; the signal comes while the second one is in its
    // first action.
    let deadline = Instant::now() + Duration::from_secs(30);
    while rig.lines("events").iter().filter(|l| *l == "focus").count() < 2 {
        assert!(
            Instant::now() < deadline,
            "the run never reached the second repetition"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
    // SAFETY: signalling the child this test started.
    unsafe { libc::kill(child.id() as i32, libc::SIGTERM) };
    let out = child.wait_with_output().unwrap();
    let text =
        String::from_utf8_lossy(&out.stdout).to_string() + &String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(130), "{text}");
    assert!(
        !text.contains("FLAKY"),
        "an interrupted repetition is not a different outcome:\n{text}"
    );
    assert!(
        text.contains("Passed: 1"),
        "the finished repetition counts:\n{text}"
    );
    assert_eq!(
        rig.lines("state").len(),
        2,
        "the third repetition must not start a fixture"
    );
    let pid: i32 = rig.lines("state")[1]
        .split(' ')
        .next()
        .unwrap()
        .parse()
        .unwrap();
    assert!(!alive(pid));
}
