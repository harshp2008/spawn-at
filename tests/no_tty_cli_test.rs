//! A menu on a terminal that has no keyboard, through the built binary. The shape: stdout is a
//! terminal (a pty), stdin is /dev/null and the process has no controlling terminal (its own
//! session), as under a service or `setsid`. Before the fix the update menu was drawn, the key
//! read failed with "No such device or address (os error 6)" and the cursor stayed hidden. A
//! stand-in `curl` on `PATH` serves one fake release so `update` reaches its menu without the
//! network; HOME and the config directory are temporary, so nothing on the machine changes.

use std::io::Read;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};

/// Opens a pty pair: (master, slave); `None` when the system has no ptys to give.
fn open_pty() -> Option<(OwnedFd, OwnedFd)> {
    let (mut master, mut slave) = (0, 0);
    // SAFETY: both out-pointers are valid; the name, termios and winsize pointers may be null.
    let rc = unsafe {
        libc::openpty(
            &mut master,
            &mut slave,
            std::ptr::null_mut(),
            std::ptr::null(),
            std::ptr::null(),
        )
    };
    if rc != 0 {
        return None;
    }
    // SAFETY: openpty returned two fresh descriptors that nothing else owns.
    Some(unsafe { (OwnedFd::from_raw_fd(master), OwnedFd::from_raw_fd(slave)) })
}

/// What is waiting on the pty master, without blocking.
fn drain(master: &OwnedFd) -> String {
    // SAFETY: plain fcntl calls on a descriptor this function borrows.
    unsafe {
        let fd = master.as_raw_fd();
        let flags = libc::fcntl(fd, libc::F_GETFL);
        libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK);
    }
    let mut out = Vec::new();
    let mut file = std::fs::File::from(master.try_clone().unwrap());
    let mut buf = [0u8; 4096];
    while let Ok(n) = file.read(&mut buf) {
        if n == 0 {
            break;
        }
        out.extend_from_slice(&buf[..n]);
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[test]
fn test_update_menu_without_a_keyboard_prints_one_clear_line_and_exits_1() {
    let Some((master, slave)) = open_pty() else {
        eprintln!("skipped: this system gave no pty");
        return;
    };
    let dir = std::env::temp_dir().join(format!("spawn-at-notty-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("home")).unwrap();
    std::fs::create_dir_all(dir.join("config")).unwrap();
    // The updater runs curl for the release list and the latest release; any other call fails.
    let curl = dir.join("curl");
    std::fs::write(
        &curl,
        r#"#!/bin/sh
rel='{"tag_name":"v9.9.9","prerelease":false,"draft":false,"assets":[]}'
for last; do :; done
case "$last" in
  */releases/latest) echo "$rel" ;;
  */releases) echo "[$rel]" ;;
  *) exit 22 ;;
esac
"#,
    )
    .unwrap();
    std::fs::set_permissions(&curl, std::fs::Permissions::from_mode(0o755)).unwrap();
    let path = format!(
        "{}:{}",
        dir.display(),
        std::env::var("PATH").unwrap_or_default()
    );

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_spawn-at"));
    cmd.args(["update", "--force"])
        .env("PATH", path)
        .env("HOME", dir.join("home"))
        .env("XDG_CONFIG_HOME", dir.join("config"))
        .env_remove("XDG_STATE_HOME")
        .stdin(Stdio::null())
        .stdout(Stdio::from(slave))
        .stderr(Stdio::piped());
    // SAFETY: setsid in the forked child before exec; it only drops the controlling terminal.
    unsafe {
        cmd.pre_exec(|| {
            libc::setsid();
            Ok(())
        });
    }
    let out = cmd.output().unwrap();
    let on_terminal = drain(&master);
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    let _ = std::fs::remove_dir_all(&dir);

    assert_eq!(out.status.code(), Some(1), "{stderr}");
    assert_eq!(
        stderr.matches("There is no terminal to ask on").count(),
        1,
        "exactly one clear line: {stderr}"
    );
    assert!(
        stderr.contains("--headless") && !stderr.contains("os error"),
        "{stderr}"
    );
    assert!(
        !on_terminal.contains("What would you like to do"),
        "no menu is drawn: {on_terminal:?}"
    );
    assert!(
        !on_terminal.contains("\x1b[?25l"),
        "the cursor is never hidden: {on_terminal:?}"
    );
}
