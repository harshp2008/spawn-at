//! Integration tests verifying `spawn-at-fixture` process lifecycle and protocol behaviors.

use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, Instant};

/// Resolves absolute filesystem path to the compiled fixture binary.
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

/// Verifies that fixture binary responds cleanly to `--help` flag.
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

/// Asserts that invalid CLI flags are rejected with non-zero exit code.
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

/// Ensures Wayland mode without WAYLAND_DISPLAY environment variable exits non-zero.
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

/// Ensures X11 mode without DISPLAY environment variable exits non-zero.
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

/// Verifies that fixture process terminates cleanly after specified timeout.
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

/// Tests emission of READY synchronization token on standard output.
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

/// Verifies clean process termination upon receiving SIGTERM or SIGINT.
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

/// Resolves absolute filesystem path to the main spawn-at binary.
fn spawn_at_binary_path() -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.pop(); // crates/
    path.pop(); // repo root
    path.push("target");
    path.push("debug");
    path.push("spawn-at");
    path
}

/// Asserts geometry reported by query-windows matches requested fixture dimensions.
#[test]
#[ignore = "requires live display session with spawn-at query windows (opens test windows)"]
fn test_fixture_size_matches_query_windows_on_all_protocols() {
    let fixture_bin = fixture_binary_path();
    let spawn_at_bin = spawn_at_binary_path();
    if !spawn_at_bin.exists() {
        panic!("spawn-at binary not found at {:?}", spawn_at_bin);
    }

    let protocols_to_test = [
        ("wayland", std::env::var_os("WAYLAND_DISPLAY").is_some()),
        ("x11", std::env::var_os("DISPLAY").is_some()),
    ];

    let req_w = 400u32;
    let req_h = 300u32;

    for (proto, available) in protocols_to_test {
        if !available {
            continue;
        }

        // Spawn fixture
        let mut child = Command::new(&fixture_bin)
            .args([
                "--protocol",
                proto,
                "--width",
                &req_w.to_string(),
                "--height",
                &req_h.to_string(),
                "--exit-after-ms",
                "5000",
                "--print-ready",
            ])
            .spawn()
            .unwrap_or_else(|e| panic!("failed to spawn fixture for protocol {}: {}", proto, e));

        let child_pid = child.id() as u64;

        // Poll query windows for up to 3 seconds until window appears
        let mut found_geometry = None;
        let start = Instant::now();
        while start.elapsed() < Duration::from_secs(3) {
            std::thread::sleep(Duration::from_millis(150));
            if let Ok(output) = Command::new(&spawn_at_bin)
                .args(["query", "windows", "--json"])
                .output()
            {
                if let Ok(val) = serde_json::from_slice::<serde_json::Value>(&output.stdout) {
                    if let Some(arr) = val.as_array() {
                        for win in arr {
                            if win.get("pid").and_then(|p| p.as_u64()) == Some(child_pid) {
                                let w = win.get("w").and_then(|v| v.as_u64());
                                let h = win.get("h").and_then(|v| v.as_u64());
                                if let (Some(w), Some(h)) = (w, h) {
                                    found_geometry = Some((w as u32, h as u32));
                                    break;
                                }
                            }
                        }
                    }
                }
            }
            if found_geometry.is_some() {
                break;
            }
        }

        // Terminate fixture process cleanly
        let _ = child.kill();
        let _ = child.wait();

        let (actual_w, actual_h) = found_geometry.unwrap_or_else(|| {
            panic!(
                "fixture window with pid {} was not reported by query windows on protocol {}",
                child_pid, proto
            )
        });

        assert_eq!(
            actual_w, req_w,
            "protocol {}: width mismatch (requested {}, query windows reported {})",
            proto, req_w, actual_w
        );
        assert_eq!(
            actual_h, req_h,
            "protocol {}: height mismatch (requested {}, query windows reported {})",
            proto, req_h, actual_h
        );
    }
}
