//! Unit tests of the GNOME backend: the install and uninstall flag defaults, the Shell version
//! parser, the embedded extension file lists against `assets/gnome`, the legacy and modern D-Bus
//! XML, and `GnomeWaylandDriver` against stub services on the session bus (close, layout,
//! windows, claims, capabilities, provisioner).
//!
//! The stubs implement only the methods a test needs, on a connection the test opened with
//! `Connection::session()` (so a session bus must be reachable), and each proxy is aimed at that
//! connection's unique name, so the real `org.gnome.Shell` service is never called. `test_is_wayland_session` runs the real
//! `loginctl` and asserts nothing about the result, only that it does not panic. The window
//! control wrappers are in `tests_control.rs`.

use super::dbus;
use super::deploy::{EMBEDDED_EXTENSION_FILES_LEGACY, EMBEDDED_EXTENSION_FILES_MODERN};
use super::detect::{is_wayland_session, parse_gnome_shell_major_version};
use super::driver::GnomeWaylandDriver;
use crate::cli::Cli;
use crate::platform::{CompositorBackend, DriverError, Rect};
use clap::Parser;
use spawn_at_core::resize_report::ResizeReport;
use std::time::Duration;

#[test]
fn test_is_wayland_session() {
    // Verify is_wayland_session executes without panicking
    let _ = is_wayland_session();
}

#[test]
fn test_gnome_install_args_defaults() {
    // Headless default run:
    // On X11: reload is enabled by default (true)
    // On Wayland: logout is disabled by default (false)
    let parsed =
        Cli::try_parse_from(["spawn-at", "install", "--scope", "user", "--headless"]).unwrap();
    if let crate::cli::Commands::Install(args) = parsed.command {
        assert!(args.backend.gnome.gnome_auto_enable);
        assert!(args.backend.gnome.gnome_x11_reload);
        assert!(!args.backend.gnome.gnome_wayland_logout);
        assert!(!args.backend.gnome.no_action);

        assert!(args.backend.gnome.should_restart(false)); // X11 defaults to reload (true)
        assert!(!args.backend.gnome.should_restart(true)); // Wayland defaults to safe skip (false)
    } else {
        panic!("Expected Install command");
    }
}

#[test]
fn test_gnome_install_args_no_action_precedence() {
    // Passing --no-action should override both X11 and Wayland to false
    let parsed = Cli::try_parse_from([
        "spawn-at",
        "install",
        "--scope",
        "user",
        "--headless",
        "--gnome-x11-reload=true",
        "--gnome-wayland-logout=true",
        "--no-action",
    ])
    .unwrap();
    if let crate::cli::Commands::Install(args) = parsed.command {
        assert!(args.backend.gnome.no_action);
        assert!(!args.backend.gnome.should_restart(false));
        assert!(!args.backend.gnome.should_restart(true));
    } else {
        panic!("Expected Install command");
    }
}

#[test]
fn test_gnome_install_args_flag_toggles() {
    // Explicitly disable X11 reload
    let parsed = Cli::try_parse_from([
        "spawn-at",
        "install",
        "--scope",
        "user",
        "--headless",
        "--gnome-x11-reload=false",
    ])
    .unwrap();
    if let crate::cli::Commands::Install(args) = parsed.command {
        assert!(!args.backend.gnome.gnome_x11_reload);
        assert!(!args.backend.gnome.should_restart(false));
    } else {
        panic!("Expected Install command");
    }

    // Explicitly enable Wayland logout
    let parsed = Cli::try_parse_from([
        "spawn-at",
        "install",
        "--scope",
        "user",
        "--headless",
        "--gnome-wayland-logout=true",
    ])
    .unwrap();
    if let crate::cli::Commands::Install(args) = parsed.command {
        assert!(args.backend.gnome.gnome_wayland_logout);
        assert!(args.backend.gnome.should_restart(true));
    } else {
        panic!("Expected Install command");
    }
}

#[test]
fn test_gnome_uninstall_args_defaults_and_overrides() {
    // Defaults:
    let parsed =
        Cli::try_parse_from(["spawn-at", "uninstall", "--scope", "user", "--headless"]).unwrap();
    if let crate::cli::Commands::Uninstall(args) = parsed.command {
        assert!(args.backend.gnome.gnome_delete_files);
        assert!(args.backend.gnome.gnome_x11_reload);
        assert!(!args.backend.gnome.gnome_wayland_logout);
        assert!(!args.backend.gnome.no_action);

        assert!(args.backend.gnome.should_restart(false));
        assert!(!args.backend.gnome.should_restart(true));
    } else {
        panic!("Expected Uninstall command");
    }

    // --no-action override:
    let parsed = Cli::try_parse_from([
        "spawn-at",
        "uninstall",
        "--scope",
        "user",
        "--headless",
        "--no-action",
    ])
    .unwrap();
    if let crate::cli::Commands::Uninstall(args) = parsed.command {
        assert!(args.backend.gnome.no_action);
        assert!(!args.backend.gnome.should_restart(false));
        assert!(!args.backend.gnome.should_restart(true));
    } else {
        panic!("Expected Uninstall command");
    }
}

#[test]
fn test_embedded_extension_files_match_disk() {
    use std::collections::HashSet;

    // Verify modern extension files
    let modern_embedded_names: HashSet<&str> = EMBEDDED_EXTENSION_FILES_MODERN
        .iter()
        .map(|(name, _)| *name)
        .collect();

    let modern_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/gnome/modern");
    let modern_disk_names: HashSet<String> = std::fs::read_dir(modern_dir)
        .expect("Failed to read assets/gnome/modern directory")
        .filter_map(|entry| {
            let entry = entry.ok()?;
            if entry.file_type().ok()?.is_file() {
                Some(entry.file_name().to_string_lossy().to_string())
            } else {
                None
            }
        })
        .collect();

    let modern_refs: HashSet<&str> = modern_disk_names.iter().map(|s| s.as_str()).collect();
    assert_eq!(
        modern_embedded_names, modern_refs,
        "EMBEDDED_EXTENSION_FILES_MODERN in Rust must exactly match assets/gnome/modern on disk"
    );

    // Verify legacy extension files
    let legacy_embedded_names: HashSet<&str> = EMBEDDED_EXTENSION_FILES_LEGACY
        .iter()
        .map(|(name, _)| *name)
        .collect();

    let legacy_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/gnome/legacy");
    let legacy_disk_names: HashSet<String> = std::fs::read_dir(legacy_dir)
        .expect("Failed to read assets/gnome/legacy directory")
        .filter_map(|entry| {
            let entry = entry.ok()?;
            if entry.file_type().ok()?.is_file() {
                Some(entry.file_name().to_string_lossy().to_string())
            } else {
                None
            }
        })
        .collect();

    let legacy_refs: HashSet<&str> = legacy_disk_names.iter().map(|s| s.as_str()).collect();
    assert_eq!(
        legacy_embedded_names, legacy_refs,
        "EMBEDDED_EXTENSION_FILES_LEGACY in Rust must exactly match assets/gnome/legacy on disk"
    );
}

#[test]
fn test_legacy_and_modern_dbus_xml_parity() {
    let modern_dbus = include_str!("../../../../assets/gnome/modern/dbus.js");
    let legacy_dbus = include_str!("../../../../assets/gnome/legacy/dbus.js");

    fn extract_xml(src: &str) -> String {
        let start = src.find("<node>").expect("D-Bus XML <node> not found");
        let end = src.find("</node>").expect("D-Bus XML </node> not found") + "</node>".len();
        src[start..end].trim().to_string()
    }

    // The legacy tree (GNOME 42-44) stays at protocol 5; protocol 6 added the ResizeOutcome
    // signal to the modern tree only. Everything else must still be identical.
    fn without_resize_outcome(xml: &str) -> String {
        let start = xml
            .find("    <signal name=\"ResizeOutcome\">")
            .expect("modern XML must declare ResizeOutcome (protocol 6)");
        let end_tag = "</signal>\n";
        let end = start + xml[start..].find(end_tag).unwrap() + end_tag.len();
        format!("{}{}", &xml[..start], &xml[end..])
    }

    let modern_xml = extract_xml(modern_dbus);
    let legacy_xml = extract_xml(legacy_dbus);

    assert!(!legacy_xml.contains("ResizeOutcome"));
    assert_eq!(
            without_resize_outcome(&modern_xml),
            legacy_xml,
            "D-Bus interface XML must match between modern and legacy extensions, apart from the protocol-6 ResizeOutcome signal"
        );
    assert!(modern_xml.contains("name=\"SpawnClaimed\""));
    assert!(modern_xml.contains("name=\"ProtocolVersion\""));
}

#[test]
fn test_parse_gnome_shell_major_version() {
    assert_eq!(
        parse_gnome_shell_major_version("GNOME Shell 46.0"),
        Some(46)
    );
    assert_eq!(
        parse_gnome_shell_major_version("GNOME Shell 42.9"),
        Some(42)
    );
    assert_eq!(
        parse_gnome_shell_major_version("GNOME Shell 45.beta"),
        Some(45)
    );
    assert_eq!(parse_gnome_shell_major_version("unknown"), None);
}

struct StubProtocol3;

#[zbus::interface(name = "org.gnome.Shell.Extensions.SpawnAt")]
impl StubProtocol3 {
    #[zbus(property)]
    fn protocol_version(&self) -> u32 {
        3
    }

    fn close_window(&self, target_id: &str) -> bool {
        target_id == "valid_id"
    }

    fn get_workareas(&self) -> String {
        r#"[{"x": 0, "y": 40, "width": 1920, "height": 1040}]"#.to_string()
    }
}

struct StubProtocol4;

#[zbus::interface(name = "org.gnome.Shell.Extensions.SpawnAt")]
impl StubProtocol4 {
    #[zbus(property)]
    fn protocol_version(&self) -> u32 {
        4
    }

    fn get_layout(&self) -> String {
        r#"{
                "schema_version": 1,
                "monitors": [
                    {
                        "index": 0,
                        "name": "DP-1",
                        "primary": true,
                        "scale": 1.0,
                        "screen": { "x": 0, "y": 0, "w": 1920, "h": 1080 },
                        "workarea": { "x": 0, "y": 40, "w": 1920, "h": 1040 }
                    }
                ]
            }"#
        .to_string()
    }

    fn get_workareas(&self) -> String {
        r#"[{"x": 0, "y": 40, "width": 1920, "height": 1040}]"#.to_string()
    }

    fn get_windows(&self) -> String {
        r#"[
                {
                    "id": 101,
                    "pid": 202,
                    "title": "Calculator",
                    "class": "gnome-calculator",
                    "app_id": "gnome-calculator",
                    "x": 100,
                    "y": 150,
                    "w": 500,
                    "h": 400,
                    "focused": true,
                    "maximized": false,
                    "minimized": false
                }
            ]"#
        .to_string()
    }
}

struct StubProtocol5;

#[zbus::interface(name = "org.gnome.Shell.Extensions.SpawnAt")]
impl StubProtocol5 {
    #[zbus(property)]
    fn protocol_version(&self) -> u32 {
        5
    }

    fn get_windows(&self) -> String {
        r#"[
                {
                    "id": 101,
                    "pid": 202,
                    "title": "Calculator",
                    "class": "gnome-calculator",
                    "app_id": "gnome-calculator",
                    "x": 100,
                    "y": 150,
                    "w": 500,
                    "h": 400,
                    "focused": true,
                    "maximized": false,
                    "minimized": false,
                    "monitor": 0,
                    "state": "normal",
                    "workspace": 1
                }
            ]"#
        .to_string()
    }
}

struct StubProtocol2;

#[zbus::interface(name = "org.gnome.Shell.Extensions.SpawnAt")]
impl StubProtocol2 {
    #[zbus(property)]
    fn protocol_version(&self) -> u32 {
        2
    }
}

#[tokio::test]
async fn test_gnome_driver_close_window_against_stub_protocol_3() {
    let conn = zbus::Connection::session().await.unwrap();
    let path = "/org/gnome/Shell/Extensions/SpawnAtTestProtocol3";
    conn.object_server().at(path, StubProtocol3).await.unwrap();

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
    assert!(driver.supports_close());

    let linux_backend = crate::platform::linux::LinuxBackend::new(Box::new(driver));
    assert!(linux_backend.supports_close());

    let res = linux_backend.close_window("valid_id").await;
    assert!(res.is_ok(), "Expected Ok(()), got {:?}", res);

    let res_not_found = linux_backend.close_window("invalid_id").await;
    assert!(res_not_found.is_err());
    match res_not_found.unwrap_err() {
        DriverError::TargetNotFound(msg) => {
            assert!(msg.contains("invalid_id"));
        }
        other => panic!("Expected TargetNotFound, got {:?}", other),
    }
}

#[tokio::test]
async fn test_gnome_driver_close_window_against_stub_protocol_2_fails() {
    let conn = zbus::Connection::session().await.unwrap();
    let path = "/org/gnome/Shell/Extensions/SpawnAtTestProtocol2";
    conn.object_server().at(path, StubProtocol2).await.unwrap();

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
    assert!(driver.supports_close());

    let linux_backend = crate::platform::linux::LinuxBackend::new(Box::new(driver));
    assert!(linux_backend.supports_close());

    let res = linux_backend.close_window("any_id").await;
    assert!(res.is_err());
    let err_str = res.unwrap_err().to_string();
    assert_eq!(
            err_str,
            "extension is older than the CLI expects (protocol 2, need 3): run `spawn-at install` and log out and back in"
        );
}

struct StubProtocol3MissingMethod;

#[zbus::interface(name = "org.gnome.Shell.Extensions.SpawnAt")]
impl StubProtocol3MissingMethod {
    #[zbus(property)]
    fn protocol_version(&self) -> u32 {
        3
    }
}

#[tokio::test]
async fn test_gnome_driver_close_window_missing_method_reports_stale_extension() {
    let conn = zbus::Connection::session().await.unwrap();
    let path = "/org/gnome/Shell/Extensions/SpawnAtTestProtocol3MissingMethod";
    conn.object_server()
        .at(path, StubProtocol3MissingMethod)
        .await
        .unwrap();

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
    assert!(driver.supports_close());

    let linux_backend = crate::platform::linux::LinuxBackend::new(Box::new(driver));
    assert!(linux_backend.supports_close());

    let res = linux_backend.close_window("any_id").await;
    assert!(res.is_err());
    let err_str = res.unwrap_err().to_string();
    assert_eq!(
            err_str,
            "extension is older than the CLI expects (protocol 3, need 3): run `spawn-at install` and log out and back in"
        );
}

#[tokio::test]
async fn test_gnome_driver_get_layout_against_stub_protocol_4() {
    let conn = zbus::Connection::session().await.unwrap();
    let path = "/org/gnome/Shell/Extensions/SpawnAtTestProtocol4";
    conn.object_server().at(path, StubProtocol4).await.unwrap();

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
    assert_eq!(driver.protocol_version().await, Some(4));

    let layout = driver.get_layout().await.unwrap();
    assert_eq!(layout.schema_version, 1);
    assert_eq!(layout.monitors.len(), 1);
    let m = &layout.monitors[0];
    assert_eq!(m.index, 0);
    assert_eq!(m.name.as_deref(), Some("DP-1"));
    assert!(m.primary);
    assert_eq!(m.scale, Some(1.0));
    assert_eq!(
        m.screen,
        Some(crate::platform::LayoutRect {
            x: 0,
            y: 0,
            w: 1920,
            h: 1080
        })
    );
    assert_eq!(
        m.workarea,
        Some(crate::platform::LayoutRect {
            x: 0,
            y: 40,
            w: 1920,
            h: 1040
        })
    );
    assert_eq!(
        m.insets,
        Some(crate::platform::LayoutInsets {
            top: 40,
            right: 0,
            bottom: 0,
            left: 0
        })
    );
    assert!(layout.note.is_none());

    let screens = driver.get_monitors().await.unwrap();
    assert_eq!(
        screens,
        vec![Rect {
            x: 0,
            y: 0,
            width: 1920,
            height: 1080
        }]
    );
}

#[tokio::test]
async fn test_gnome_driver_get_layout_stale_extension_degradation_protocol_3() {
    let conn = zbus::Connection::session().await.unwrap();
    let path = "/org/gnome/Shell/Extensions/SpawnAtTestProtocol3Degradation";
    conn.object_server().at(path, StubProtocol3).await.unwrap();

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
    assert_eq!(driver.protocol_version().await, Some(3));

    let layout = driver.get_layout().await.unwrap();
    assert_eq!(layout.schema_version, 1);
    assert_eq!(layout.monitors.len(), 1);
    let m = &layout.monitors[0];
    assert_eq!(m.index, 0);
    assert!(m.screen.is_none());
    assert!(m.insets.is_none());
    assert_eq!(
        m.workarea,
        Some(crate::platform::LayoutRect {
            x: 0,
            y: 40,
            w: 1920,
            h: 1040
        })
    );
    assert!(layout.note.is_some());
    assert!(layout.note.unwrap().contains("protocol 3, need 4"));
}

#[tokio::test]
async fn test_gnome_driver_get_windows_against_stub_protocol_5() {
    let conn = zbus::Connection::session().await.unwrap();
    let path = "/org/gnome/Shell/Extensions/SpawnAtTestProtocol5";
    conn.object_server().at(path, StubProtocol5).await.unwrap();

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
    assert_eq!(driver.protocol_version().await, Some(5));

    let windows = driver.get_windows().await.unwrap();
    assert_eq!(windows.len(), 1);
    let win = &windows[0];
    assert_eq!(win.id, Some(101));
    assert_eq!(win.monitor, Some(0));
    assert_eq!(win.state.as_deref(), Some("normal"));
    assert_eq!(win.workspace, Some(1));
}

#[tokio::test]
async fn test_gnome_driver_get_windows_against_stub_protocol_4_older_extension() {
    let conn = zbus::Connection::session().await.unwrap();
    let path = "/org/gnome/Shell/Extensions/SpawnAtTestProtocol4Windows";
    conn.object_server().at(path, StubProtocol4).await.unwrap();

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
    assert_eq!(driver.protocol_version().await, Some(4));

    let windows = driver.get_windows().await.unwrap();
    assert_eq!(windows.len(), 1);
    let win = &windows[0];
    assert_eq!(win.id, Some(101));
    assert!(win.monitor.is_none());
    assert!(win.state.is_none());
    assert!(win.workspace.is_none());
}

/// A stand-in for the extension's claim signals. `version` is the protocol it reports; the
/// ResizeOutcome signal is declared on both versions (the object is only a signal source, a
/// protocol 5 extension simply never emits it).
struct StubClaim {
    version: u32,
}

#[zbus::interface(name = "org.gnome.Shell.Extensions.SpawnAt")]
impl StubClaim {
    #[zbus(property)]
    fn protocol_version(&self) -> u32 {
        self.version
    }

    #[zbus(signal)]
    #[allow(clippy::too_many_arguments)]
    async fn spawn_claimed(
        ctxt: &zbus::SignalContext<'_>,
        target_id: &str,
        success: bool,
        window_id: u64,
        x: i32,
        y: i32,
        w: u32,
        h: u32,
        size_raised: bool,
        error: &str,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn resize_outcome(
        ctxt: &zbus::SignalContext<'_>,
        target_id: &str,
        outcome: &str,
        final_w: u32,
        final_h: u32,
        waited_ms: u32,
        reason: &str,
    ) -> zbus::Result<()>;
}

/// Serves `StubClaim` on one session-bus connection and returns a driver on a second
/// connection, so the signals really travel through the bus. The first connection is returned
/// too and must be kept alive, with the path to emit from.
async fn claim_driver(version: u32, path: &'static str) -> (zbus::Connection, GnomeWaylandDriver) {
    let server = zbus::Connection::session().await.unwrap();
    server
        .object_server()
        .at(path, StubClaim { version })
        .await
        .unwrap();
    let client = zbus::Connection::session().await.unwrap();
    let proxy = dbus::SpawnAtProxy::builder(&client)
        .destination(server.unique_name().unwrap().to_owned())
        .unwrap()
        .path(path.to_string())
        .unwrap()
        .build()
        .await
        .unwrap();
    (server, GnomeWaylandDriver::from_proxy(proxy))
}

async fn emit_claim(server: &zbus::Connection, path: &'static str, target: &str) {
    let ctxt = zbus::SignalContext::new(server, path).unwrap();
    StubClaim::spawn_claimed(&ctxt, target, true, 7, 1104, 464, 494, 292, false, "")
        .await
        .unwrap();
}

/// Protocol 6: the outcome emitted before the claim is attached to the claim, and one for
/// another target is not.
#[tokio::test]
async fn test_claim_carries_resize_outcome_with_protocol_6() {
    let path = "/org/gnome/Shell/Extensions/SpawnAtTestClaim6";
    let (server, driver) = claim_driver(6, path).await;
    let mut sub = driver.prepare_claim_wait("t-1").await.unwrap().unwrap();

    let ctxt = zbus::SignalContext::new(&server, path).unwrap();
    StubClaim::resize_outcome(&ctxt, "other", "timed-out", 400, 300, 150, "cap")
        .await
        .unwrap();
    StubClaim::resize_outcome(&ctxt, "t-1", "adjusted", 494, 292, 41, "quiet")
        .await
        .unwrap();
    emit_claim(&server, path, "t-1").await;

    let claim = sub.wait_claim(Duration::from_secs(2)).await.unwrap();
    assert_eq!(claim.window_id, 7);
    assert_eq!((claim.w, claim.h), (494, 292));
    assert_eq!(
        claim.resize,
        Some(ResizeReport::from_wire("adjusted", 494, 292, 41, "quiet"))
    );
}

/// Protocol 6 with a window the extension waits for with the legacy rule: no outcome is sent,
/// the claim has none, and the CLI does not wait for one.
#[tokio::test]
async fn test_claim_without_outcome_is_not_delayed_with_protocol_6() {
    let path = "/org/gnome/Shell/Extensions/SpawnAtTestClaim6Legacy";
    let (server, driver) = claim_driver(6, path).await;
    let mut sub = driver.prepare_claim_wait("t-2").await.unwrap().unwrap();

    emit_claim(&server, path, "t-2").await;
    let started = std::time::Instant::now();
    let claim = sub.wait_claim(Duration::from_secs(2)).await.unwrap();
    assert!(claim.resize.is_none());
    assert!(
        started.elapsed() < Duration::from_millis(500),
        "waited {:?} for an outcome that was never sent",
        started.elapsed()
    );
}

/// Protocol 5 (an extension that was not reinstalled): the claim works and has no outcome,
/// even if something emitted a ResizeOutcome, because the CLI does not subscribe below 6.
#[tokio::test]
async fn test_claim_against_protocol_5_has_no_resize_outcome() {
    let path = "/org/gnome/Shell/Extensions/SpawnAtTestClaim5";
    let (server, driver) = claim_driver(5, path).await;
    let mut sub = driver.prepare_claim_wait("t-3").await.unwrap().unwrap();

    let ctxt = zbus::SignalContext::new(&server, path).unwrap();
    StubClaim::resize_outcome(&ctxt, "t-3", "reached", 800, 600, 20, "at-target")
        .await
        .unwrap();
    emit_claim(&server, path, "t-3").await;

    let claim = sub.wait_claim(Duration::from_secs(2)).await.unwrap();
    assert_eq!((claim.x, claim.y), (1104, 464));
    assert!(claim.resize.is_none());
}

/// The GNOME driver reports what its `supports_*` methods say, needs the helper (the extension)
/// and can hold a new window invisible (cloak).
#[tokio::test]
async fn test_gnome_capabilities_equal_its_supports_methods() {
    let (_server, driver) = claim_driver(6, "/org/gnome/Shell/Extensions/SpawnAtCaps").await;
    let c = driver.capabilities();
    assert_eq!(c.runtime_transform, driver.supports_runtime_transform());
    assert_eq!(c.claim_wait, driver.supports_claim_wait());
    assert_eq!(c.close, driver.supports_close());
    assert!(c.reveal_control && c.needs_helper);
}

/// The GNOME driver provides the provisioner, and its notice is the one `gnome_notice` gives for
/// the detected Shell (`CompositorBackend::install_notice` forwards to it).
#[tokio::test]
async fn test_gnome_driver_is_its_own_provisioner() {
    let (_server, driver) = claim_driver(6, "/org/gnome/Shell/Extensions/SpawnAtProv").await;
    let p = driver
        .provisioner()
        .expect("GNOME needs the extension installed");
    assert_eq!(p.install_notice(), driver.install_notice());
}
