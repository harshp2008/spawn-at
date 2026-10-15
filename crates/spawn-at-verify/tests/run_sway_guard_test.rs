//! `run` aimed at Sway (`SPAWN_AT_BACKEND=sway`) from a GNOME user's shell
//! (`docs/DESIGN_RUNNER_SETUP.md` section 4.3):
//!
//! - the GNOME journal log source is not used (the run prints `Timing log source: noop`), so no
//!   `SetLogging` call can reach the live extension;
//! - a `SWAYSOCK` that is not inside a marked `spawn-at-sway.*` directory blocks the test before
//!   anything is started: no fixture, no `spawn-at` action;
//! - a `SWAYSOCK` inside a marked private directory lets the test run.
//!
//! The binary under test is a shell script in a temporary directory; no display, no D-Bus.

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

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
  focus) echo "focus" >> "$D/events" ;;
esac
"#;

const SPEC: &str = r#"
schema_version = 2
id = "transforms/for-sway"
category = "transforms"
title = "for sway"
description = "focus a fixture window"
kind = "auto"
tags = ["moves-focus"]

[requires]
backend = ["sway"]

[[setup]]
name = "w"

[action]
command = "focus"
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
            "spawn-at-verify-swayguard-{}-{}",
            std::process::id(),
            tag
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("bin")).unwrap();
        std::fs::create_dir_all(root.join("verify/transforms")).unwrap();
        write_exec(&root.join("bin/spawn-at"), SPAWN_AT);
        write_exec(&root.join("bin/fixture"), FIXTURE);
        std::fs::write(root.join("verify/transforms/for-sway.toml"), SPEC).unwrap();
        Rig { root }
    }

    /// Runs `run` as a GNOME user's shell would have it, aimed at Sway with `swaysock`.
    fn run(&self, swaysock: Option<&str>) -> (Output, String) {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_spawn-at-verify"));
        cmd.args([
            "run",
            "--bin",
            self.root.join("bin/spawn-at").to_str().unwrap(),
            "--dir",
            self.root.join("verify").to_str().unwrap(),
            "--glob",
            "transforms/*",
        ])
        .current_dir(&self.root)
        .env_remove("DISPLAY")
        .env_remove("WAYLAND_DISPLAY")
        .env_remove("DBUS_SESSION_BUS_ADDRESS")
        .env_remove("SWAYSOCK")
        .env("XDG_CURRENT_DESKTOP", "ubuntu:GNOME")
        .env("XDG_SESSION_TYPE", "wayland")
        .env("SPAWN_AT_BACKEND", "sway");
        if let Some(s) = swaysock {
            cmd.env("SWAYSOCK", s);
        }
        let out = cmd.output().unwrap();
        let text = String::from_utf8_lossy(&out.stdout).to_string()
            + &String::from_utf8_lossy(&out.stderr);
        (out, text)
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

#[test]
fn test_gnome_journal_is_not_used_when_sway_is_forced() {
    let rig = Rig::new("journal");
    let (_, text) = rig.run(None);
    assert!(
        text.contains("Timing log source: noop"),
        "a Sway run must not use the GNOME journal / SetLogging:\n{text}"
    );
}

#[test]
fn test_a_sway_that_is_not_the_private_session_blocks_the_test() {
    let rig = Rig::new("blocked");
    // A socket path in the user's own runtime directory, as a real Sway session has it.
    let (out, text) = rig.run(Some("/run/user/1000/sway-ipc.1000.1.sock"));
    assert!(
        text.contains("BLOCK") || text.to_lowercase().contains("blocked"),
        "{text}"
    );
    assert!(text.contains("private headless session"), "{text}");
    assert!(!out.status.success() || text.contains("Blocked"), "{text}");
    assert!(rig.lines("state").is_empty(), "no fixture may be started");
    assert!(rig.lines("events").is_empty(), "no action may be run");
}

#[test]
fn test_no_swaysock_at_all_blocks_the_test() {
    let rig = Rig::new("nosock");
    let (_, text) = rig.run(None);
    assert!(text.contains("SWAYSOCK is not set"), "{text}");
    assert!(rig.lines("state").is_empty());
}

#[test]
fn test_the_private_harness_directory_lets_the_test_run() {
    let rig = Rig::new("private");
    let dir = rig.root.join("spawn-at-sway.TEST01");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).unwrap();
    std::fs::write(dir.join(".spawn-at-sway-harness"), "").unwrap();
    let sock = dir.join("sway-ipc.1.1.sock");
    let (out, text) = rig.run(Some(sock.to_str().unwrap()));
    assert!(out.status.success(), "{text}");
    assert_eq!(rig.lines("events"), vec!["focus", "close 1000"], "{text}");
}
