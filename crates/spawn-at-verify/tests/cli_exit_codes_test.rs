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
