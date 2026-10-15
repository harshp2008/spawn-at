//! `SPAWN_AT_BACKEND=sway` through the built binary, against a tiny stand-in for the Sway IPC
//! socket in a temporary directory. No Sway and no desktop is involved.
//!
//! Every run removes `DISPLAY` and `WAYLAND_DISPLAY` from the child's environment first, so even a
//! wrongly selected X11 backend could not reach the X server of the session running the tests.
//! The variables are set on the child process only.

use std::io::{Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

const OUTPUTS: &[u8] = include_bytes!("fixtures/sway/outputs_scale2_bar.json");
const WORKSPACES: &[u8] = include_bytes!("fixtures/sway/workspaces_scale2_bar.json");
const TREE: &[u8] = include_bytes!("fixtures/sway/tree_floating.json");

static SEQ: AtomicUsize = AtomicUsize::new(0);

struct Server {
    path: PathBuf,
    connections: Arc<AtomicUsize>,
    stop: Arc<AtomicBool>,
}

impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        let _ = std::fs::remove_file(&self.path);
    }
}

fn reply(stream: &mut UnixStream, kind: u32, payload: &[u8]) {
    let mut out = b"i3-ipc".to_vec();
    out.extend_from_slice(&(payload.len() as u32).to_ne_bytes());
    out.extend_from_slice(&kind.to_ne_bytes());
    out.extend_from_slice(payload);
    let _ = stream.write_all(&out);
}

fn serve(mut stream: UnixStream, garbage: bool) {
    loop {
        let mut header = [0u8; 14];
        if stream.read_exact(&mut header).is_err() {
            return;
        }
        let len = u32::from_ne_bytes(header[6..10].try_into().unwrap()) as usize;
        let kind = u32::from_ne_bytes(header[10..14].try_into().unwrap());
        let mut payload = vec![0u8; len];
        if stream.read_exact(&mut payload).is_err() {
            return;
        }
        if garbage {
            let _ = stream.write_all(b"HTTP/1.1 400 Bad Request\r\n\r\n");
            continue;
        }
        let body: &[u8] = match kind {
            3 => OUTPUTS,
            1 => WORKSPACES,
            4 => TREE,
            _ => b"[]",
        };
        reply(&mut stream, kind, body);
    }
}

/// Starts the stand-in: it answers GET_OUTPUTS, GET_WORKSPACES and GET_TREE with recorded replies
/// (or, with `garbage`, answers everything with an HTTP error) and counts connections.
fn start_server(garbage: bool) -> Server {
    let path = std::env::temp_dir().join(format!(
        "spawn-at-sway-cli-{}-{}.sock",
        std::process::id(),
        SEQ.fetch_add(1, Ordering::SeqCst)
    ));
    let _ = std::fs::remove_file(&path);
    let listener = UnixListener::bind(&path).unwrap();
    listener.set_nonblocking(true).unwrap();
    let connections = Arc::new(AtomicUsize::new(0));
    let stop = Arc::new(AtomicBool::new(false));
    let (c2, s2) = (connections.clone(), stop.clone());
    std::thread::spawn(move || {
        while !s2.load(Ordering::SeqCst) {
            match listener.accept() {
                Ok((stream, _)) => {
                    c2.fetch_add(1, Ordering::SeqCst);
                    stream.set_nonblocking(false).unwrap();
                    std::thread::spawn(move || serve(stream, garbage));
                }
                Err(_) => std::thread::sleep(Duration::from_millis(5)),
            }
        }
    });
    Server {
        path,
        connections,
        stop,
    }
}

/// Runs `spawn-at <args>` with the given backend variable, `SWAYSOCK` and desktop on the child
/// only, and no display variables.
fn run(args: &[&str], backend: Option<&str>, swaysock: Option<&str>, desktop: &str) -> Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_spawn-at"));
    cmd.args(args)
        .env_remove("DISPLAY")
        .env_remove("WAYLAND_DISPLAY")
        .env_remove("SPAWN_AT_BACKEND")
        .env_remove("SWAYSOCK")
        .env("XDG_CURRENT_DESKTOP", desktop);
    if let Some(b) = backend {
        cmd.env("SPAWN_AT_BACKEND", b);
    }
    if let Some(s) = swaysock {
        cmd.env("SWAYSOCK", s);
    }
    cmd.output().unwrap()
}

fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

/// Forced Sway without a usable `$SWAYSOCK` is an error that names it and exits 1; it never
/// continues with X11 (the message has no X11 words, and nothing is printed on stdout).
fn assert_clear_error(o: &Output, needle: &str) {
    assert_eq!(o.status.code(), Some(1), "stderr: {}", stderr(o));
    let err = stderr(o);
    assert!(err.contains("Driver initialization error"), "{err}");
    assert!(err.contains("$SWAYSOCK") && err.contains(needle), "{err}");
    assert!(
        !err.contains("X11") && !err.contains("DISPLAY"),
        "no X11 fallback: {err}"
    );
    assert!(stdout(o).is_empty(), "stdout: {}", stdout(o));
}

#[test]
fn test_forced_sway_with_swaysock_unset_is_a_clear_error() {
    let o = run(&["query", "layout"], Some("sway"), None, "");
    assert_clear_error(&o, "not set");
}

#[test]
fn test_forced_sway_with_swaysock_empty_is_a_clear_error() {
    let o = run(&["query", "layout"], Some("sway"), Some(""), "");
    assert_clear_error(&o, "empty");
}

#[test]
fn test_forced_sway_with_a_missing_socket_is_a_clear_error() {
    let missing = std::env::temp_dir().join("spawn-at-sway-cli-missing.sock");
    let _ = std::fs::remove_file(&missing);
    let o = run(
        &["query", "windows"],
        Some("sway"),
        Some(missing.to_str().unwrap()),
        "GNOME",
    );
    assert_clear_error(&o, "cannot connect");
}

/// A regular file where a socket should be is "cannot connect", not a hang or a panic.
#[test]
fn test_forced_sway_with_a_file_instead_of_a_socket_is_a_clear_error() {
    let file = std::env::temp_dir().join(format!("spawn-at-sway-cli-file-{}", std::process::id()));
    std::fs::write(&file, b"not a socket").unwrap();
    let o = run(
        &["query", "layout"],
        Some("sway"),
        Some(file.to_str().unwrap()),
        "",
    );
    let _ = std::fs::remove_file(&file);
    assert_clear_error(&o, "cannot connect");
}

/// A socket that answers like an HTTP server is refused, naming i3-ipc.
#[test]
fn test_forced_sway_with_a_socket_that_is_not_sway_is_a_clear_error() {
    let server = start_server(true);
    let o = run(
        &["query", "layout"],
        Some("sway"),
        Some(server.path.to_str().unwrap()),
        "",
    );
    assert_clear_error(&o, "i3-ipc");
}

/// The whole path: the variable selects Sway, the driver reads the recorded outputs and
/// workspaces (scale 2, a 30 px bar) and `query layout` prints them in logical pixels.
#[test]
fn test_forced_sway_prints_the_layout_from_the_socket() {
    let server = start_server(false);
    let o = run(
        &["query", "layout"],
        Some("sway"),
        Some(server.path.to_str().unwrap()),
        "GNOME",
    );
    assert_eq!(o.status.code(), Some(0), "stderr: {}", stderr(&o));
    let out = stdout(&o);
    assert!(
        out.contains("HEADLESS-1") && out.contains("960x540@0,0") && out.contains("960x510@0,30"),
        "{out}"
    );
}

/// The value is lowercased like `x11` is, so `SWAY` selects Sway, and `query windows` lists the
/// window of the recorded tree.
#[test]
fn test_forced_uppercase_sway_lists_the_windows() {
    let server = start_server(false);
    let o = run(
        &["query", "windows"],
        Some("SWAY"),
        Some(server.path.to_str().unwrap()),
        "",
    );
    assert_eq!(o.status.code(), Some(0), "stderr: {}", stderr(&o));
    let out = stdout(&o);
    assert!(
        out.contains("sway-cap-1") && out.contains("800x500@100,150"),
        "{out}"
    );
}

/// A value that only looks like `sway` is ignored, as before: the socket is not touched.
#[test]
fn test_near_values_do_not_select_sway() {
    for value in [" sway", "sway ", "swayx"] {
        let server = start_server(false);
        let _ = run(
            &["query", "layout"],
            Some(value),
            Some(server.path.to_str().unwrap()),
            "",
        );
        assert_eq!(server.connections.load(Ordering::SeqCst), 0, "{value:?}");
    }
}

/// A Sway session without the variable still gets X11, as before (design decision: Sway is never
/// chosen from the desktop). The stand-in socket is offered in `SWAYSOCK` and
/// `XDG_CURRENT_DESKTOP=sway`, and is never connected to.
#[test]
fn test_the_sway_desktop_alone_does_not_select_sway() {
    let server = start_server(false);
    let _ = run(
        &["query", "layout"],
        None,
        Some(server.path.to_str().unwrap()),
        "sway",
    );
    assert_eq!(server.connections.load(Ordering::SeqCst), 0);
}
