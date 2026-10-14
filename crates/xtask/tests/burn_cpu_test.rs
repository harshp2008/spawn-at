//! Runs the built `xtask` binary's `burn-cpu` subcommand as a user would.

use std::process::Command;
use std::time::{Duration, Instant};

fn xtask() -> Command {
    Command::new(env!("CARGO_BIN_EXE_xtask"))
}

/// 2 threads for 0.3 s: exit status 0, at least 0.3 s elapsed, and the summary line reports 2
/// joined threads. The process is waited on, so nothing it started can outlive the call.
#[test]
fn test_burn_cpu_runs_for_the_requested_time_and_exits_zero() {
    let started = Instant::now();
    let out = xtask()
        .args(["burn-cpu", "--threads", "2", "--seconds", "0.3"])
        .output()
        .expect("run xtask");
    assert!(out.status.success(), "{:?}", out);
    assert!(started.elapsed() >= Duration::from_millis(300));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("2 threads joined"), "{}", stderr);
}

/// A missing `--seconds` exits 1 and prints the usage text, without spinning any thread.
#[test]
fn test_burn_cpu_bad_arguments_exit_one_with_usage() {
    let started = Instant::now();
    let out = xtask()
        .args(["burn-cpu", "--threads", "4"])
        .output()
        .expect("run xtask");
    assert_eq!(out.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("--seconds is required"), "{}", stderr);
    assert!(stderr.contains("Usage: cargo xtask burn-cpu"), "{}", stderr);
    assert!(started.elapsed() < Duration::from_secs(5));
}
