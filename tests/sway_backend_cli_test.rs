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
use std::sync::{Arc, Mutex};
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

// ---------------------------------------------------------------------------------------------
// Window commands through the built binary (audit proposal P5, `docs/TEST_COVERAGE_AUDIT.md`).
//
// A second stand-in records every RUN_COMMAND, answers it with `[{"success":true}]`, and can
// switch the window tree it serves once the first command has arrived (the window moved, or is
// gone), so the commands' own "did it happen" waits finish at once. The recorded tree has one
// floating window: id 5, class `dev.spawnat.swaycap`, focused, at 100,150 and 800x500.
// ---------------------------------------------------------------------------------------------

struct Recorder {
    path: PathBuf,
    commands: Arc<Mutex<Vec<String>>>,
    stop: Arc<AtomicBool>,
}

impl Recorder {
    fn commands(&self) -> Vec<String> {
        self.commands.lock().unwrap().clone()
    }
    fn sock(&self) -> &str {
        self.path.to_str().unwrap()
    }
}

impl Drop for Recorder {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        let _ = std::fs::remove_file(&self.path);
    }
}

fn serve_recording(
    mut stream: UnixStream,
    commands: Arc<Mutex<Vec<String>>>,
    after_command: Arc<AtomicBool>,
    tree_before: Vec<u8>,
    tree_after: Option<Vec<u8>>,
) {
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
        let body: Vec<u8> = match kind {
            0 => {
                commands
                    .lock()
                    .unwrap()
                    .push(String::from_utf8_lossy(&payload).into_owned());
                after_command.store(true, Ordering::SeqCst);
                br#"[{"success":true}]"#.to_vec()
            }
            3 => OUTPUTS.to_vec(),
            1 => WORKSPACES.to_vec(),
            4 => match (&tree_after, after_command.load(Ordering::SeqCst)) {
                (Some(t), true) => t.clone(),
                _ => tree_before.clone(),
            },
            _ => b"[]".to_vec(),
        };
        reply(&mut stream, kind, &body);
    }
}

/// Starts the recording stand-in. `tree_after` is served for GET_TREE once any command was run.
fn start_recorder(tree_after: Option<Vec<u8>>) -> Recorder {
    let path = std::env::temp_dir().join(format!(
        "spawn-at-sway-cli-rec-{}-{}.sock",
        std::process::id(),
        SEQ.fetch_add(1, Ordering::SeqCst)
    ));
    let _ = std::fs::remove_file(&path);
    let listener = UnixListener::bind(&path).unwrap();
    listener.set_nonblocking(true).unwrap();
    let commands = Arc::new(Mutex::new(Vec::new()));
    let after = Arc::new(AtomicBool::new(false));
    let stop = Arc::new(AtomicBool::new(false));
    let (c2, s2) = (commands.clone(), stop.clone());
    std::thread::spawn(move || {
        while !s2.load(Ordering::SeqCst) {
            match listener.accept() {
                Ok((stream, _)) => {
                    stream.set_nonblocking(false).unwrap();
                    let (c, a, t) = (c2.clone(), after.clone(), tree_after.clone());
                    std::thread::spawn(move || serve_recording(stream, c, a, TREE.to_vec(), t));
                }
                Err(_) => std::thread::sleep(Duration::from_millis(5)),
            }
        }
    });
    Recorder {
        path,
        commands,
        stop,
    }
}

/// The recorded tree with window `id` given a new frame and content size.
fn tree_with_window_rect(id: u64, rect: (i64, i64, i64, i64), content: (i64, i64)) -> Vec<u8> {
    fn walk(v: &mut serde_json::Value, id: u64, rect: (i64, i64, i64, i64), content: (i64, i64)) {
        if v["id"] == id {
            v["rect"] =
                serde_json::json!({"x": rect.0, "y": rect.1, "width": rect.2, "height": rect.3});
            v["window_rect"] =
                serde_json::json!({"x": 0, "y": 0, "width": content.0, "height": content.1});
        }
        for key in ["nodes", "floating_nodes"] {
            if let Some(list) = v[key].as_array_mut() {
                for child in list {
                    walk(child, id, rect, content);
                }
            }
        }
    }
    let mut tree: serde_json::Value = serde_json::from_slice(TREE).unwrap();
    walk(&mut tree, id, rect, content);
    serde_json::to_vec(&tree).unwrap()
}

/// The recorded tree with window `id` removed.
fn tree_without_window(id: u64) -> Vec<u8> {
    fn walk(v: &mut serde_json::Value, id: u64) {
        for key in ["nodes", "floating_nodes"] {
            if let Some(list) = v[key].as_array_mut() {
                list.retain(|c| c["id"] != id);
                for child in list {
                    walk(child, id);
                }
            }
        }
    }
    let mut tree: serde_json::Value = serde_json::from_slice(TREE).unwrap();
    walk(&mut tree, id);
    serde_json::to_vec(&tree).unwrap()
}

fn run_sway(args: &[&str], sock: &str) -> Output {
    run(args, Some("sway"), Some(sock), "")
}

/// `focus --id 5` sends exactly one `[con_id=5] focus` and exits 0; by class it resolves to the
/// same container.
#[test]
fn test_cli_focus_sends_focus_for_the_resolved_container() {
    let rec = start_recorder(None);
    let o = run_sway(&["focus", "--id", "5"], rec.sock());
    assert_eq!(o.status.code(), Some(0), "stderr: {}", stderr(&o));
    assert_eq!(rec.commands(), ["[con_id=5] focus"]);

    let rec = start_recorder(None);
    let o = run_sway(&["focus", "-c", "dev.spawnat.swaycap"], rec.sock());
    assert_eq!(o.status.code(), Some(0), "stderr: {}", stderr(&o));
    assert_eq!(rec.commands(), ["[con_id=5] focus"]);
}

#[test]
fn test_cli_focus_unknown_window_fails_and_sends_nothing() {
    let rec = start_recorder(None);
    let o = run_sway(&["focus", "--id", "999"], rec.sock());
    assert_eq!(o.status.code(), Some(1));
    assert!(
        stderr(&o).contains("No window with id 999"),
        "{}",
        stderr(&o)
    );
    assert!(rec.commands().is_empty());
}

/// `close --id 5` sends `[con_id=5] kill`, waits until the window is gone from the tree and
/// exits 0.
#[test]
fn test_cli_close_sends_kill_and_waits_for_the_window_to_leave() {
    let rec = start_recorder(Some(tree_without_window(5)));
    let o = run_sway(&["close", "--id", "5"], rec.sock());
    assert_eq!(o.status.code(), Some(0), "stderr: {}", stderr(&o));
    assert_eq!(rec.commands(), ["[con_id=5] kill"]);
}

#[test]
fn test_cli_close_no_wait_sends_kill_without_checking_the_tree() {
    let rec = start_recorder(None);
    let o = run_sway(&["--no-wait", "close", "--id", "5"], rec.sock());
    assert_eq!(o.status.code(), Some(0), "stderr: {}", stderr(&o));
    assert_eq!(rec.commands(), ["[con_id=5] kill"]);
}

#[test]
fn test_cli_close_unknown_window_fails_and_sends_nothing() {
    let rec = start_recorder(None);
    let o = run_sway(&["close", "--id", "999"], rec.sock());
    assert_eq!(o.status.code(), Some(1));
    assert!(
        stderr(&o).contains("No window with id 999"),
        "{}",
        stderr(&o)
    );
    assert!(rec.commands().is_empty());
}

/// `transform --id 5 --size 400 300 --anchor center --no-focus` on the recorded layout (logical
/// work area 960x510 at 0,30): one command that makes the window floating, sets the size and
/// moves it to the centre, 280,135. The tree served afterwards has the window there, so no
/// correction follows. `--no-focus` keeps the focus command out of the list.
#[test]
fn test_cli_transform_size_and_anchor_sends_one_place_command() {
    let rec = start_recorder(Some(tree_with_window_rect(
        5,
        (280, 135, 400, 300),
        (400, 300),
    )));
    let o = run_sway(
        &[
            "transform",
            "--id",
            "5",
            "--size",
            "400",
            "300",
            "--anchor",
            "center",
            "--no-focus",
        ],
        rec.sock(),
    );
    assert_eq!(o.status.code(), Some(0), "stderr: {}", stderr(&o));
    assert_eq!(
        rec.commands(),
        ["[con_id=5] floating enable, resize set 400 300, move absolute position 280 135"]
    );
}

/// Without a focus modifier a transform also focuses the window: the place command is followed
/// by `[con_id=5] focus` (the default focus policy of `apply_focus_policy`).
#[test]
fn test_cli_transform_focuses_the_window_by_default() {
    let rec = start_recorder(Some(tree_with_window_rect(
        5,
        (280, 135, 400, 300),
        (400, 300),
    )));
    let o = run_sway(
        &[
            "transform",
            "--id",
            "5",
            "--size",
            "400",
            "300",
            "--anchor",
            "center",
        ],
        rec.sock(),
    );
    assert_eq!(o.status.code(), Some(0), "stderr: {}", stderr(&o));
    assert_eq!(
        rec.commands(),
        [
            "[con_id=5] floating enable, resize set 400 300, move absolute position 280 135",
            "[con_id=5] focus"
        ]
    );
}

#[test]
fn test_cli_transform_unknown_window_fails_and_sends_nothing() {
    let rec = start_recorder(None);
    let o = run_sway(
        &["transform", "--id", "999", "--size", "400", "300"],
        rec.sock(),
    );
    assert_eq!(o.status.code(), Some(1));
    assert!(
        stderr(&o).contains("No window with id 999"),
        "{}",
        stderr(&o)
    );
    assert!(rec.commands().is_empty());
}

/// `query layout --json` and `query windows --json` through the binary: the recorded output
/// (scale 2, a 30 px bar) and the one window of the recorded tree.
#[test]
fn test_cli_query_json_matches_the_recorded_session() {
    let rec = start_recorder(None);
    let o = run_sway(&["query", "layout", "--json"], rec.sock());
    assert_eq!(o.status.code(), Some(0), "stderr: {}", stderr(&o));
    let v: serde_json::Value = serde_json::from_str(&stdout(&o)).unwrap();
    assert_eq!(v["monitors"][0]["name"], "HEADLESS-1");
    assert_eq!(v["monitors"][0]["screen"]["w"], 960);
    assert_eq!(v["monitors"][0]["workarea"]["y"], 30);

    let o = run_sway(&["query", "windows", "--json"], rec.sock());
    assert_eq!(o.status.code(), Some(0), "stderr: {}", stderr(&o));
    let v: serde_json::Value = serde_json::from_str(&stdout(&o)).unwrap();
    let w = &v.as_array().expect("a list of windows")[0];
    assert_eq!(w["id"], 5);
    assert_eq!(w["class"], "dev.spawnat.swaycap");
    assert_eq!(w["focused"], true);
    assert_eq!((w["x"].as_i64(), w["y"].as_i64()), (Some(100), Some(150)));
    assert_eq!((w["w"].as_i64(), w["h"].as_i64()), (Some(800), Some(500)));
    assert!(rec.commands().is_empty(), "queries send no command");
}

/// What Sway cannot do exits 1 with the documented "does not support this feature" text, names
/// the missing operation, sends no command and does not fall back to X11.
#[test]
fn test_cli_unsupported_commands_report_the_missing_capability() {
    let cases: [(&[&str], &str); 5] = [
        (&["defocus", "--id", "5"], "defocus_window"),
        (&["maximize", "--id", "5"], "set_window_state"),
        (&["minimize", "--id", "5"], "set_window_state"),
        (&["restore", "--id", "5"], "set_window_state"),
        (&["query", "pointer"], "get_cursor_position"),
    ];
    for (args, operation) in cases {
        let rec = start_recorder(None);
        let o = run_sway(args, rec.sock());
        let err = stderr(&o);
        assert_eq!(o.status.code(), Some(1), "{args:?}: {err}");
        assert!(
            err.contains(&format!(
                "The active compositor does not support this feature: {operation}"
            )),
            "{args:?}: {err}"
        );
        assert!(
            !err.contains("X11") && !err.contains("DISPLAY"),
            "{args:?}: {err}"
        );
        assert!(stdout(&o).is_empty(), "{args:?}: {}", stdout(&o));
        assert!(rec.commands().is_empty(), "{args:?}: {:?}", rec.commands());
    }
}
