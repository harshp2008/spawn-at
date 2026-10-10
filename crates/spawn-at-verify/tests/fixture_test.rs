use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, Instant};

fn fixture_binary_path() -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.pop(); // crates/
    path.pop(); // repo root
    path.push("target");
    path.push("debug");
    path.push("fixture");
    if !path.exists() {
        panic!(
            "Fixture binary not found at {:?}. Build it with `cargo build -p spawn-at-verify --bin fixture --features fixture`",
            path
        );
    }
    path
}

#[test]
fn test_fixture_binary_help() {
    let bin = fixture_binary_path();
    let output = Command::new(&bin)
        .arg("--help")
        .output()
        .expect("failed to execute fixture --help");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Window verification fixture application"));
    assert!(stdout.contains("--protocol"));
    assert!(stdout.contains("--decorations"));
    assert!(stdout.contains("--paint-delay-ms"));
    assert!(stdout.contains("--min-width"));
    assert!(stdout.contains("--min-height"));
    assert!(stdout.contains("--remaximize-after-ms"));
    assert!(stdout.contains("--exit-after-ms"));
    assert!(stdout.contains("--print-ready"));
}

#[test]
fn test_fixture_rejects_invalid_args() {
    let bin = fixture_binary_path();
    let output = Command::new(&bin)
        .arg("--decorations")
        .arg("invalid_mode")
        .output()
        .expect("failed to execute fixture with invalid args");

    assert!(!output.status.success());
}

#[test]
fn test_fixture_protocol_wayland_without_wayland_display_exits_nonzero() {
    let bin = fixture_binary_path();
    let output = Command::new(&bin)
        .env_remove("WAYLAND_DISPLAY")
        .arg("--protocol")
        .arg("wayland")
        .output()
        .expect("failed to execute fixture without WAYLAND_DISPLAY");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Wayland backend requested, but WAYLAND_DISPLAY is not set"),
        "unexpected stderr: {}",
        stderr
    );
}

#[test]
fn test_fixture_protocol_x11_without_display_exits_nonzero() {
    let bin = fixture_binary_path();
    let output = Command::new(&bin)
        .env_remove("DISPLAY")
        .arg("--protocol")
        .arg("x11")
        .output()
        .expect("failed to execute fixture without DISPLAY");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("X11 backend requested, but DISPLAY is not set"),
        "unexpected stderr: {}",
        stderr
    );
}

#[test]
#[ignore = "requires live display session (opens test window)"]
fn test_fixture_clean_exit_timeout() {
    let bin = fixture_binary_path();
    let start = Instant::now();
    let output = Command::new(&bin)
        .args(["--exit-after-ms", "100"])
        .output()
        .expect("failed to execute fixture with --exit-after-ms");

    let elapsed = start.elapsed();
    assert!(output.status.success(), "fixture failed: {:?}", output);
    assert!(
        elapsed >= Duration::from_millis(80),
        "fixture exited too quickly: {:?}",
        elapsed
    );
}

#[test]
#[ignore = "requires live display session (opens test window)"]
fn test_fixture_print_ready_output() {
    let bin = fixture_binary_path();
    let output = Command::new(&bin)
        .args(["--exit-after-ms", "200", "--print-ready"])
        .output()
        .expect("failed to execute fixture with --print-ready");

    assert!(output.status.success(), "fixture failed: {:?}", output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("[fixture] READY protocol="),
        "expected READY line in stdout, got: {}",
        stdout
    );
}

#[test]
#[ignore = "requires live display session (opens test window)"]
fn test_fixture_signal_termination() {
    let bin = fixture_binary_path();
    let mut child = Command::new(&bin)
        .args(["--exit-after-ms", "10000"])
        .spawn()
        .expect("failed to spawn fixture child");

    let pid = child.id() as i32;
    std::thread::sleep(Duration::from_millis(150));

    unsafe {
        libc::kill(pid, libc::SIGTERM);
    }

    let status = child.wait().expect("failed to wait on child");
    assert!(
        status.success() || status.code().is_none(),
        "fixture did not terminate cleanly on SIGTERM: {:?}",
        status
    );
}
