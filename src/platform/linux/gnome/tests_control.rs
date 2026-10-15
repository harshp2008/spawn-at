//! D-Bus stub tests for the window-control wrappers of `GnomeWaylandDriver`: `focus_window`,
//! `defocus_window`, `set_window_state`, `transform_window` and `move_window` (audit proposal P2,
//! gap 4). Same method as the close tests in `tests.rs`: a `zbus` interface served on the test's
//! own connection, a proxy aimed at that connection's unique name, so nothing reaches the real
//! `org.gnome.Shell` service.
//!
//! The stub records one line per call (`FocusWindow(42)`, `SetWindowState(42,maximize)`, ...).
//! A method that the driver called under another wire name would never reach the stub and the
//! test would see an `IpcError` instead, so the recorded names are the real D-Bus method names.

use super::dbus;
use super::driver::GnomeWaylandDriver;
use crate::platform::{CompositorBackend, DriverError, WindowState};
use spawn_at_core::geometry::{Anchor, PlacementParams};
use std::sync::{Arc, Mutex};

#[derive(Clone)]
struct StubControl {
    calls: Arc<Mutex<Vec<String>>>,
    /// Return value of the boolean methods (the extension answers `false` for "window not found").
    reply: bool,
    /// Make `ExecuteBatch` fail with a D-Bus error.
    fail_batch: bool,
}

#[zbus::interface(name = "org.gnome.Shell.Extensions.SpawnAt")]
impl StubControl {
    #[zbus(property)]
    fn protocol_version(&self) -> u32 {
        6
    }

    fn focus_window(&self, target: &str) -> bool {
        self.calls
            .lock()
            .unwrap()
            .push(format!("FocusWindow({target})"));
        self.reply
    }

    fn defocus_window(&self, target: &str, mode: &str, destination: &str) -> bool {
        self.calls
            .lock()
            .unwrap()
            .push(format!("DefocusWindow({target},{mode},{destination})"));
        self.reply
    }

    fn set_window_state(&self, target: &str, state: &str) -> bool {
        self.calls
            .lock()
            .unwrap()
            .push(format!("SetWindowState({target},{state})"));
        self.reply
    }

    fn move_window(&self, app_id: &str, x: i32, y: i32) {
        self.calls
            .lock()
            .unwrap()
            .push(format!("MoveWindow({app_id},{x},{y})"));
    }

    fn execute_batch(&self, target_id: &str, instructions_json: &str) -> zbus::fdo::Result<()> {
        self.calls
            .lock()
            .unwrap()
            .push(format!("ExecuteBatch({target_id},{instructions_json})"));
        if self.fail_batch {
            return Err(zbus::fdo::Error::Failed("batch refused".into()));
        }
        Ok(())
    }
}

/// An extension that has none of the control methods (only the protocol property).
struct StubNoMethods;

#[zbus::interface(name = "org.gnome.Shell.Extensions.SpawnAt")]
impl StubNoMethods {
    #[zbus(property)]
    fn protocol_version(&self) -> u32 {
        6
    }
}

/// Serves `stub` at `path` on a fresh session-bus connection and returns a driver that talks to
/// it. The connection is returned too: the caller must keep it alive for the whole test, or the
/// stub stops answering and the call hangs.
async fn driver_for<T>(path: &str, stub: T) -> (GnomeWaylandDriver, zbus::Connection)
where
    T: zbus::object_server::Interface,
{
    let conn = zbus::Connection::session().await.unwrap();
    conn.object_server().at(path, stub).await.unwrap();
    let unique_name = conn.unique_name().unwrap().to_owned();
    let proxy = dbus::SpawnAtProxy::builder(&conn)
        .destination(unique_name)
        .unwrap()
        .path(path.to_string())
        .unwrap()
        .build()
        .await
        .unwrap();
    let driver = GnomeWaylandDriver::from_proxy(proxy);
    // Read the property once before any method call. Found while writing these tests: when the
    // stub lives on the same connection as the client, a method call that is the first thing sent
    // through a fresh proxy never gets an answer (the test hangs, observed with 11 of 11 tests,
    // each run alone). The close tests in `tests.rs` avoid it because `close_window` reads the
    // protocol version first. Cause inside zbus 4.4 not investigated.
    assert_eq!(driver.protocol_version().await, Some(6));
    (driver, conn)
}

fn stub(reply: bool) -> (StubControl, Arc<Mutex<Vec<String>>>) {
    let calls = Arc::new(Mutex::new(Vec::new()));
    (
        StubControl {
            calls: calls.clone(),
            reply,
            fail_batch: false,
        },
        calls,
    )
}

fn recorded(calls: &Arc<Mutex<Vec<String>>>) -> Vec<String> {
    calls.lock().unwrap().clone()
}

#[tokio::test]
async fn test_gnome_driver_focus_window_sends_focus_window_with_the_target() {
    let (s, calls) = stub(true);
    let (d, _conn) = driver_for("/org/gnome/Shell/Extensions/SpawnAtTestFocusOk", s).await;
    d.focus_window("42").await.unwrap();
    assert_eq!(recorded(&calls), vec!["FocusWindow(42)"]);
}

#[tokio::test]
async fn test_gnome_driver_focus_window_false_reply_is_target_not_found() {
    let (s, _calls) = stub(false);
    let (d, _conn) = driver_for("/org/gnome/Shell/Extensions/SpawnAtTestFocusFalse", s).await;
    match d.focus_window("42").await.unwrap_err() {
        DriverError::TargetNotFound(msg) => {
            assert_eq!(msg, "Compositor failed to focus window target '42'")
        }
        other => panic!("Expected TargetNotFound, got {:?}", other),
    }
}

#[tokio::test]
async fn test_gnome_driver_defocus_window_sends_target_mode_and_destination() {
    let (s, calls) = stub(true);
    let (d, _conn) = driver_for("/org/gnome/Shell/Extensions/SpawnAtTestDefocusOk", s).await;
    d.defocus_window("42", "mru", "").await.unwrap();
    d.defocus_window("42", "desktop", "").await.unwrap();
    d.defocus_window("42", "window", "Terminal").await.unwrap();
    assert_eq!(
        recorded(&calls),
        vec![
            "DefocusWindow(42,mru,)",
            "DefocusWindow(42,desktop,)",
            "DefocusWindow(42,window,Terminal)"
        ]
    );
}

#[tokio::test]
async fn test_gnome_driver_defocus_window_false_reply_is_target_not_found() {
    let (s, _calls) = stub(false);
    let (d, _conn) = driver_for("/org/gnome/Shell/Extensions/SpawnAtTestDefocusFalse", s).await;
    match d.defocus_window("42", "mru", "").await.unwrap_err() {
        DriverError::TargetNotFound(msg) => {
            assert_eq!(msg, "Compositor failed to defocus window target '42'")
        }
        other => panic!("Expected TargetNotFound, got {:?}", other),
    }
}

#[tokio::test]
async fn test_gnome_driver_set_window_state_sends_the_four_state_strings() {
    let (s, calls) = stub(true);
    let (d, _conn) = driver_for("/org/gnome/Shell/Extensions/SpawnAtTestStateOk", s).await;
    d.set_window_state("42", WindowState::Maximize)
        .await
        .unwrap();
    d.set_window_state("42", WindowState::Minimize)
        .await
        .unwrap();
    d.set_window_state("42", WindowState::Unminimize)
        .await
        .unwrap();
    d.set_window_state("42", WindowState::Restore)
        .await
        .unwrap();
    assert_eq!(
        recorded(&calls),
        vec![
            "SetWindowState(42,maximize)",
            "SetWindowState(42,minimize)",
            "SetWindowState(42,unminimize)",
            "SetWindowState(42,restore)"
        ]
    );
}

#[tokio::test]
async fn test_gnome_driver_set_window_state_false_reply_is_target_not_found() {
    let (s, _calls) = stub(false);
    let (d, _conn) = driver_for("/org/gnome/Shell/Extensions/SpawnAtTestStateFalse", s).await;
    match d
        .set_window_state("42", WindowState::Maximize)
        .await
        .unwrap_err()
    {
        DriverError::TargetNotFound(msg) => {
            assert_eq!(
                msg,
                "Compositor failed to set state 'Maximize' on target '42'"
            )
        }
        other => panic!("Expected TargetNotFound, got {:?}", other),
    }
}

/// An extension without the methods (an old or broken one): the D-Bus error is wrapped in
/// `IpcError`, not turned into `TargetNotFound`. Pinned for all three boolean wrappers.
#[tokio::test]
async fn test_gnome_driver_control_calls_on_an_extension_without_the_methods_are_ipc_errors() {
    let (d, _conn) = driver_for(
        "/org/gnome/Shell/Extensions/SpawnAtTestNoControlMethods",
        StubNoMethods,
    )
    .await;
    let errs = [
        d.focus_window("42").await.unwrap_err(),
        d.defocus_window("42", "mru", "").await.unwrap_err(),
        d.set_window_state("42", WindowState::Restore)
            .await
            .unwrap_err(),
    ];
    for e in errs {
        match &e {
            DriverError::IpcError(msg) => assert!(msg.contains("UnknownMethod"), "{msg}"),
            other => panic!("Expected IpcError, got {:?}", other),
        }
        assert!(e.to_string().starts_with("IPC Error: "));
    }
}

#[tokio::test]
async fn test_gnome_driver_move_window_sends_move_window_with_coordinates() {
    let (s, calls) = stub(true);
    let (d, _conn) = driver_for("/org/gnome/Shell/Extensions/SpawnAtTestMove", s).await;
    d.move_window("42", -10, 250).await.unwrap();
    assert_eq!(recorded(&calls), vec!["MoveWindow(42,-10,250)"]);
}

/// Size-only transform: exactly Snapshot, Cloak, SetSize, WaitForCommit(500 ms), Uncloak,
/// DestroySnapshot, with no SetPositionAnchored. The bytes are the serde output of the
/// instruction enum; the extension parses them (`tests/dispatcher_test.js`).
#[tokio::test]
async fn test_gnome_driver_transform_window_size_only_sends_exact_batch_json() {
    let (s, calls) = stub(true);
    let (d, _conn) = driver_for("/org/gnome/Shell/Extensions/SpawnAtTestTransformSize", s).await;
    let params = PlacementParams {
        size: Some((500, 400)),
        ..Default::default()
    };
    d.transform_window("42", params, 300, 200).await.unwrap();
    assert_eq!(
        recorded(&calls),
        vec![
            r#"ExecuteBatch(42,["Snapshot","Cloak",{"SetSize":{"w":500,"h":400}},{"WaitForCommit":{"timeout_ms":500}},"Uncloak","DestroySnapshot"])"#
        ]
    );
}

/// Position-only transform (anchor set, no size): no SetSize and no WaitForCommit, and the
/// payload carries the window's current 300x200 size.
#[tokio::test]
async fn test_gnome_driver_transform_window_anchor_only_sends_position_batch() {
    let (s, calls) = stub(true);
    let (d, _conn) = driver_for("/org/gnome/Shell/Extensions/SpawnAtTestTransformAnchor", s).await;
    let params = PlacementParams {
        anchor: Some(Anchor::Center),
        ..Default::default()
    };
    d.transform_window("42", params, 300, 200).await.unwrap();
    let sent = recorded(&calls);
    assert_eq!(sent.len(), 1);
    let json = sent[0]
        .strip_prefix("ExecuteBatch(42,")
        .and_then(|s| s.strip_suffix(')'))
        .unwrap();
    let v: serde_json::Value = serde_json::from_str(json).unwrap();
    let arr = v.as_array().unwrap();
    assert_eq!(arr.len(), 5);
    assert_eq!(arr[0], "Snapshot");
    assert_eq!(arr[1], "Cloak");
    assert_eq!(arr[2]["SetPositionAnchored"]["intended_w"], 300);
    assert_eq!(arr[2]["SetPositionAnchored"]["intended_h"], 200);
    assert_eq!(arr[3], "Uncloak");
    assert_eq!(arr[4], "DestroySnapshot");
}

#[tokio::test]
async fn test_gnome_driver_transform_window_batch_error_is_ipc_error() {
    let (mut s, _calls) = stub(true);
    s.fail_batch = true;
    let (d, _conn) = driver_for("/org/gnome/Shell/Extensions/SpawnAtTestTransformFail", s).await;
    let params = PlacementParams {
        size: Some((500, 400)),
        ..Default::default()
    };
    match d
        .transform_window("42", params, 300, 200)
        .await
        .unwrap_err()
    {
        DriverError::IpcError(msg) => assert!(msg.contains("batch refused"), "{msg}"),
        other => panic!("Expected IpcError, got {:?}", other),
    }
}
