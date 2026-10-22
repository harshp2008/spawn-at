//! D-Bus stub tests for the keyed arm of `GnomeWaylandDriver`: `arm` against an extension at
//! protocol 7 (`ArmSpawnEntry`) and at protocol 6 (only `ArmSpawn`), and `disarm_entry`.
//!
//! Same method as `tests_control.rs`: a `zbus` interface served on the test's own connection, a
//! proxy aimed at that connection's unique name, so nothing reaches the real `org.gnome.Shell`.
//! The protocol 6 stub has no entry methods at all, so a driver that called `ArmSpawnEntry` or
//! `DisarmEntry` on it would get an error instead of a recorded call: the same thing a real
//! protocol 6 extension answers.

use super::dbus;
use super::driver::GnomeWaylandDriver;
use crate::platform::{Batch, CompositorBackend, Driver, DriverError};
use spawn_at_core::driver::{Entry, FocusIntent, Reveal, Urgency};
use spawn_at_core::geometry::PlacementParams;
use spawn_at_core::wait_override::WaitOverride;
use std::sync::{Arc, Mutex};
use std::time::Duration;

type Calls = Arc<Mutex<Vec<String>>>;

/// The extension at protocol 7.
struct StubProtocol7 {
    calls: Calls,
    /// The one entry key `DisarmEntry` finds; any other key answers `false`.
    live_key: &'static str,
    /// Make `DisarmEntry` fail with a D-Bus error.
    fail_disarm: bool,
}

#[zbus::interface(name = "org.gnome.Shell.Extensions.SpawnAt")]
impl StubProtocol7 {
    #[zbus(property)]
    fn protocol_version(&self) -> u32 {
        7
    }

    fn arm_spawn(&self, target_id: &str, _instructions_json: &str) {
        self.calls
            .lock()
            .unwrap()
            .push(format!("ArmSpawn({target_id})"));
    }

    fn arm_spawn_entry(&self, target_id: &str, entry_key: &str, _instructions_json: &str) {
        self.calls
            .lock()
            .unwrap()
            .push(format!("ArmSpawnEntry({target_id},{entry_key})"));
    }

    fn disarm_entry(&self, target_id: &str, entry_key: &str) -> zbus::fdo::Result<bool> {
        self.calls
            .lock()
            .unwrap()
            .push(format!("DisarmEntry({target_id},{entry_key})"));
        if self.fail_disarm {
            return Err(zbus::fdo::Error::Failed("disarm refused".into()));
        }
        Ok(entry_key == self.live_key)
    }
}

/// An extension at protocol 6: `ArmSpawn` only.
struct StubProtocol6 {
    calls: Calls,
}

#[zbus::interface(name = "org.gnome.Shell.Extensions.SpawnAt")]
impl StubProtocol6 {
    #[zbus(property)]
    fn protocol_version(&self) -> u32 {
        6
    }

    fn arm_spawn(&self, target_id: &str, _instructions_json: &str) {
        self.calls
            .lock()
            .unwrap()
            .push(format!("ArmSpawn({target_id})"));
    }
}

/// Serves `stub` on a fresh session-bus connection (keep the returned connection alive for the
/// whole test) and returns a driver aimed at it. Reads the protocol property once before any
/// method call, for the reason given in `tests_control.rs`.
async fn driver_for<T>(path: &str, stub: T, protocol: u32) -> (GnomeWaylandDriver, zbus::Connection)
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
    assert_eq!(driver.protocol_version().await, Some(protocol));
    (driver, conn)
}

fn entry(key: &str, app_hint: &str) -> Entry {
    Entry {
        key: key.into(),
        app_hint: app_hint.into(),
        placement: PlacementParams::default(),
        wait: WaitOverride::default(),
    }
}

fn batch(entries: Vec<Entry>) -> Batch {
    Batch {
        id: 1,
        entries,
        reveal: Reveal::Together,
        focus: FocusIntent::Exclusive,
        urgency: Urgency::Normal,
        deadline: Duration::from_millis(15000),
    }
}

fn recorded(calls: &Calls) -> Vec<String> {
    calls.lock().unwrap().clone()
}

#[tokio::test]
async fn test_arm_on_protocol_7_sends_arm_spawn_entry_with_the_entry_key() {
    let calls = Calls::default();
    let stub = StubProtocol7 {
        calls: calls.clone(),
        live_key: "",
        fail_disarm: false,
    };
    let (driver, _conn) = driver_for("/test/entry/arm7", stub, 7).await;

    let armed = driver
        .arm(batch(vec![entry("spawn-at-1-2_TIME2", "clip-app")]))
        .await
        .unwrap();

    assert_eq!(
        recorded(&calls),
        vec!["ArmSpawnEntry(clip-app,spawn-at-1-2_TIME2)"]
    );
    assert_eq!(armed.token.as_deref(), Some("clip-app"));
}

#[tokio::test]
async fn test_arm_on_protocol_6_sends_the_plain_arm_spawn() {
    let calls = Calls::default();
    let (driver, _conn) = driver_for(
        "/test/entry/arm6",
        StubProtocol6 {
            calls: calls.clone(),
        },
        6,
    )
    .await;

    let armed = driver
        .arm(batch(vec![entry("spawn-at-1-2_TIME2", "clip-app")]))
        .await
        .unwrap();

    assert_eq!(recorded(&calls), vec!["ArmSpawn(clip-app)"]);
    assert_eq!(armed.token.as_deref(), Some("clip-app"));
}

#[tokio::test]
async fn test_arm_of_several_entries_names_each_under_its_own_key() {
    let calls = Calls::default();
    let stub = StubProtocol7 {
        calls: calls.clone(),
        live_key: "",
        fail_disarm: false,
    };
    let (driver, _conn) = driver_for("/test/entry/armmany", stub, 7).await;

    driver
        .arm(batch(vec![entry("k-1", "term"), entry("k-2", "term")]))
        .await
        .unwrap();

    assert_eq!(
        recorded(&calls),
        vec!["ArmSpawnEntry(k-1,k-1)", "ArmSpawnEntry(k-2,k-2)"]
    );
}

#[tokio::test]
async fn test_disarm_entry_on_protocol_7_sends_the_target_and_key_and_returns_the_answer() {
    let calls = Calls::default();
    let stub = StubProtocol7 {
        calls: calls.clone(),
        live_key: "k-live",
        fail_disarm: false,
    };
    let (driver, _conn) = driver_for("/test/entry/disarm7", stub, 7).await;

    assert!(driver.disarm_entry("clip-app", "k-live").await.unwrap());
    assert!(!driver.disarm_entry("clip-app", "k-gone").await.unwrap());
    assert_eq!(
        recorded(&calls),
        vec![
            "DisarmEntry(clip-app,k-live)",
            "DisarmEntry(clip-app,k-gone)"
        ]
    );
}

#[tokio::test]
async fn test_disarm_entry_on_protocol_6_sends_nothing_and_returns_false() {
    let calls = Calls::default();
    let (driver, _conn) = driver_for(
        "/test/entry/disarm6",
        StubProtocol6 {
            calls: calls.clone(),
        },
        6,
    )
    .await;

    assert!(!driver.disarm_entry("clip-app", "k-1").await.unwrap());
    assert!(recorded(&calls).is_empty());
}

#[tokio::test]
async fn test_disarm_entry_dbus_failure_is_an_ipc_error() {
    let stub = StubProtocol7 {
        calls: Calls::default(),
        live_key: "",
        fail_disarm: true,
    };
    let (driver, _conn) = driver_for("/test/entry/disarmfail", stub, 7).await;

    match driver.disarm_entry("clip-app", "k-1").await {
        Err(DriverError::IpcError(msg)) => assert!(msg.contains("disarm entry"), "{msg}"),
        other => panic!("expected IpcError, got {other:?}"),
    }
}
