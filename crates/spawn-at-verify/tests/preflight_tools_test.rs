//! `run` prints an install hint on stderr when the optional X11 tools are not on PATH.

use std::process::Command;

/// With an empty PATH `xrandr` and `xprop` cannot be found, so `run --dry-run` prints the hint
/// (packages and apt command, and the extension-derived fallback) on stderr.
#[test]
fn test_run_prints_install_hint_when_tools_are_not_on_path() {
    let exe = env!("CARGO_BIN_EXE_spawn-at-verify");
    let out = Command::new(exe)
        .args(["run", "--dry-run", "--bin", exe])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .env("PATH", "")
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("Note: optional tool(s) missing: xrandr (package x11-xserver-utils), xprop (package x11-utils)"), "{}", stderr);
    assert!(
        stderr.contains("sudo apt install x11-xserver-utils x11-utils"),
        "{}",
        stderr
    );
    assert!(stderr.contains("extension-derived"), "{}", stderr);
    assert!(
        !String::from_utf8_lossy(&out.stdout).contains("optional tool"),
        "hint must not go to stdout"
    );
}
