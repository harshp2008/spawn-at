//! A desktop spawn-at does not support is an error, not a silent switch to X11. Runs the built
//! binary with the desktop variables set on the child only and `DISPLAY` / `WAYLAND_DISPLAY`
//! removed, so even a wrongly chosen X11 backend could not reach the X server of the session
//! running the tests.

use std::process::{Command, Output};

fn run(backend: Option<&str>, desktop: &str, session: Option<&str>) -> Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_spawn-at"));
    cmd.args(["query", "layout"])
        .env_remove("DISPLAY")
        .env_remove("WAYLAND_DISPLAY")
        .env_remove("SPAWN_AT_BACKEND")
        .env_remove("SWAYSOCK")
        .env_remove("XDG_SESSION_TYPE")
        .env("XDG_CURRENT_DESKTOP", desktop);
    if let Some(b) = backend {
        cmd.env("SPAWN_AT_BACKEND", b);
    }
    if let Some(s) = session {
        cmd.env("XDG_SESSION_TYPE", s);
    }
    cmd.output().unwrap()
}

fn err(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

/// Non-GNOME Wayland and an unknown session type exit 1 with the full message and print nothing
/// on stdout.
#[test]
fn test_non_gnome_wayland_exits_1_with_the_message() {
    for (desktop, session) in [
        ("KDE", Some("wayland")),
        ("sway", Some("wayland")),
        ("", None),
    ] {
        let o = run(None, desktop, session);
        let e = err(&o);
        assert_eq!(o.status.code(), Some(1), "{e}");
        assert!(
            e.contains("Driver initialization error: this desktop is not supported"),
            "{e}"
        );
        assert!(e.contains("SPAWN_AT_BACKEND=sway"), "{e}");
        assert!(
            e.contains("set SPAWN_AT_BACKEND=x11 (it may or may not work)"),
            "{e}"
        );
        assert!(o.stdout.is_empty());
    }
}

/// A real X11 session on a non-GNOME desktop, and an explicit `SPAWN_AT_BACKEND=x11` on Wayland,
/// get past the check (they then fail on the missing X display, which is not our message).
#[test]
fn test_x11_session_and_explicit_x11_are_not_refused() {
    for o in [
        run(None, "KDE", Some("x11")),
        run(Some("x11"), "KDE", Some("wayland")),
    ] {
        assert!(!err(&o).contains("not supported"), "{}", err(&o));
    }
}
