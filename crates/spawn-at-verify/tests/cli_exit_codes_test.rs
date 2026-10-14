//! The `run` exit codes are documented in `run --help` and match the summary policy.

use spawn_at_verify::cli::{EXIT_FAILED, EXIT_UNCHECKED_REQUIRED};
use std::process::Command;

/// `run --help` (and `-h`) print the exit-code section, including the unchecked-required code.
#[test]
fn test_run_help_documents_exit_codes() {
    for flag in ["--help", "-h"] {
        let out = Command::new(env!("CARGO_BIN_EXE_spawn-at-verify"))
            .args(["run", flag])
            .output()
            .unwrap();
        assert!(out.status.success());
        let text = String::from_utf8_lossy(&out.stdout);
        assert!(text.contains("Exit codes:"), "{}", text);
        assert!(text.contains("  0  no test failed"), "{}", text);
        assert!(text.contains("  1  at least one test failed"), "{}", text);
        assert!(
            text.contains("  2  nothing failed, but at least one REQUIRED test passed with part of its evidence unchecked"),
            "{}",
            text
        );
        // Both kinds of unchecked pass are named under code 2.
        assert!(text.contains("its geometry (no workarea"), "{}", text);
        assert!(
            text.contains("or its\n     trace (the extension's timing log held no events"),
            "{}",
            text
        );
    }
}

/// The codes named in the help are the ones the code uses.
#[test]
fn test_exit_code_constants_match_help() {
    assert_eq!(EXIT_FAILED, 1);
    assert_eq!(EXIT_UNCHECKED_REQUIRED, 2);
}

/// `run --help` lists `--strict-flash` and the exit-code text says a PASS (flash) keeps the code
/// unless the flag is given; `list --help` does not take the flag.
#[test]
fn test_run_help_documents_strict_flash() {
    let out = Command::new(env!("CARGO_BIN_EXE_spawn-at-verify"))
        .args(["run", "--help"])
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("--strict-flash"), "{}", text);
    assert!(text.contains("A PASS (flash)"), "{}", text);
    assert!(text.contains("with --strict-flash it is a"), "{}", text);
    let list = Command::new(env!("CARGO_BIN_EXE_spawn-at-verify"))
        .args(["list", "--help"])
        .output()
        .unwrap();
    assert!(!String::from_utf8_lossy(&list.stdout).contains("--strict-flash"));
}

/// `run --help` lists `--flash-threshold-ms`, which defaults to 0.
#[test]
fn test_run_help_documents_flash_threshold() {
    let out = Command::new(env!("CARGO_BIN_EXE_spawn-at-verify"))
        .args(["run", "--help"])
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("--flash-threshold-ms"), "{}", text);
    assert!(text.contains("[default: 0]"), "{}", text);
}

/// `run --help` lists the two same-login A/B options and what they do.
#[test]
fn test_run_help_documents_the_wait_options() {
    let out = Command::new(env!("CARGO_BIN_EXE_spawn-at-verify"))
        .args(["run", "--help"])
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(
        text.contains("--wait-strategy <auto|legacy|ready-wait>"),
        "{}",
        text
    );
    assert!(text.contains("--wait-cap-ms <MS>"), "{}", text);
    assert!(text.contains("original 30 ms rule"), "{}", text);
    // They belong to `run`: `list` does not take them.
    let list = Command::new(env!("CARGO_BIN_EXE_spawn-at-verify"))
        .args(["list", "--help"])
        .output()
        .unwrap();
    assert!(!String::from_utf8_lossy(&list.stdout).contains("--wait-"));
}

/// The options parse into the request the runner passes to every spawn, and mean "nothing" when
/// they are not given.
#[test]
fn test_run_wait_options_become_the_runner_request() {
    use clap::Parser;
    use spawn_at_verify::cli::args::{Cli, Commands};
    let parse = |extra: &[&str]| -> spawn_at_verify::trace::wait::WaitRequest {
        let mut argv = vec!["spawn-at-verify", "run"];
        argv.extend_from_slice(extra);
        match Cli::try_parse_from(argv).unwrap().command {
            Commands::Run(run) => run.wait_request(),
            _ => panic!("expected the run subcommand"),
        }
    };
    let none = parse(&[]);
    assert!(none.is_empty());
    let both = parse(&["--wait-strategy", "legacy", "--wait-cap-ms", "90"]);
    assert_eq!(both.strategy.as_deref(), Some("legacy"));
    assert_eq!(both.cap_ms.as_deref(), Some("90"));
    // Passed as typed, even when wrong: spawn-at warns about it.
    let typo = parse(&["--wait-strategy", "legcy", "--wait-cap-ms", "x"]);
    assert_eq!(typo.strategy.as_deref(), Some("legcy"));
    assert_eq!(typo.cap_ms.as_deref(), Some("x"));
}

/// `run --help` lists `--no-wire-paint` (the wire paint time is the default) and `list --help`
/// does not take it.
#[test]
fn test_run_help_documents_no_wire_paint() {
    let out = Command::new(env!("CARGO_BIN_EXE_spawn-at-verify"))
        .args(["run", "--help"])
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("--no-wire-paint"), "{}", text);
    assert!(text.contains("wl_surface.commit"), "{}", text);
    assert!(!text.contains("--wire-paint"), "{}", text);
    let list = Command::new(env!("CARGO_BIN_EXE_spawn-at-verify"))
        .args(["list", "--help"])
        .output()
        .unwrap();
    assert!(!String::from_utf8_lossy(&list.stdout).contains("wire-paint"));
}
