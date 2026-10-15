//! # Sway backend (EXPERIMENTAL)
//!
//! Design and measurements: `docs/DESIGN_SWAY.md`. Submodules:
//! - `ipc`: the i3-ipc frame format (pure, no sockets).
//! - `types`: the JSON Sway sends and its conversion to `WindowMetadata` and `MonitorLayout`.
//! - `client`: a blocking connection to `$SWAYSOCK`, and the errors that name it.
//!
//! The driver is reached only by `SPAWN_AT_BACKEND=sway`; no environment selects it on its own.

// The write side (placement, close, focus) arrives in the next commit of this series; until
// then some items are only used by the unit tests. Remove this allow with that commit.
#![allow(dead_code)]

pub mod client;
pub mod ipc;
pub mod types;

#[cfg(test)]
mod fake;

use crate::platform::{
    Armed, Batch, CompositorBackend, Driver, DriverError, MonitorLayout, Rect, WindowMetadata,
};
use client::Client;
use ipc::MessageType;
use std::path::{Path, PathBuf};

/// The Sway driver: every call opens its own short connection to `$SWAYSOCK` (the way the X11
/// driver connects per call), so the driver holds only the socket path.
pub struct SwayDriver {
    socket: PathBuf,
}

impl SwayDriver {
    /// Connects using `$SWAYSOCK`. Fails, with a message naming `$SWAYSOCK`, when it is unset,
    /// empty, not connectable or not a Sway socket; the caller must not fall back to another
    /// backend (the user asked for Sway by name).
    pub fn from_env() -> Result<SwayDriver, DriverError> {
        Self::connect(client::socket_path(std::env::var_os("SWAYSOCK"))?)
    }

    /// Connects to the socket at `socket` and checks that it answers like Sway (one
    /// `GET_OUTPUTS`).
    pub fn connect(socket: PathBuf) -> Result<SwayDriver, DriverError> {
        let mut client = Client::connect(&socket)?;
        let reply = client.request(MessageType::GetOutputs, b"")?;
        types::parse_outputs(&reply).map_err(|e| {
            DriverError::IpcError(format!(
                "$SWAYSOCK ({}) answered, but not like Sway: {}",
                socket.display(),
                e
            ))
        })?;
        Ok(SwayDriver { socket })
    }

    /// Runs `work` on a blocking thread with the socket path (the socket I/O is blocking).
    async fn blocking<T, F>(&self, work: F) -> Result<T, DriverError>
    where
        T: Send + 'static,
        F: FnOnce(&Path) -> Result<T, DriverError> + Send + 'static,
    {
        let socket = self.socket.clone();
        tokio::task::spawn_blocking(move || work(&socket))
            .await
            .map_err(|e| DriverError::Execution(e.into()))?
    }
}

fn to_error(e: String) -> DriverError {
    DriverError::IpcError(e)
}

/// `GET_OUTPUTS` and `GET_WORKSPACES` as the monitor layout.
pub fn fetch_layout(socket: &Path) -> Result<MonitorLayout, DriverError> {
    let mut client = Client::connect(socket)?;
    let outputs =
        types::parse_outputs(&client.request(MessageType::GetOutputs, b"")?).map_err(to_error)?;
    let workspaces = types::parse_workspaces(&client.request(MessageType::GetWorkspaces, b"")?)
        .map_err(to_error)?;
    Ok(types::layout_from(&outputs, &workspaces))
}

/// `GET_TREE` and `GET_OUTPUTS` as the window list.
pub fn fetch_windows(socket: &Path) -> Result<Vec<WindowMetadata>, DriverError> {
    let mut client = Client::connect(socket)?;
    let outputs =
        types::parse_outputs(&client.request(MessageType::GetOutputs, b"")?).map_err(to_error)?;
    let tree = types::parse_tree(&client.request(MessageType::GetTree, b"")?).map_err(to_error)?;
    Ok(types::windows_from_tree(&tree, &outputs))
}

#[async_trait::async_trait]
impl Driver for SwayDriver {
    async fn arm(&self, _batch: Batch) -> Result<Armed, DriverError> {
        Ok(Armed::default())
    }
}

#[async_trait::async_trait]
impl CompositorBackend for SwayDriver {
    fn name(&self) -> &'static str {
        "Sway"
    }

    fn resolve_id(&self, command: &[String], explicit_class: Option<&str>) -> String {
        let bin = command.first().map(|s| s.as_str()).unwrap_or("");
        crate::platform::linux::xdg::resolve_linux_app_id(bin, explicit_class)
    }

    /// Screen rectangles of the active outputs, in logical pixels.
    async fn get_monitors(&self) -> Result<Vec<Rect>, DriverError> {
        let layout = self.blocking(fetch_layout).await?;
        Ok(layout
            .monitors
            .iter()
            .filter_map(|m| m.screen.map(Rect::from))
            .collect())
    }

    /// Usable areas (the output minus bars) of the active outputs.
    async fn get_workareas(&self) -> Result<Vec<Rect>, DriverError> {
        let layout = self.blocking(fetch_layout).await?;
        Ok(layout
            .monitors
            .iter()
            .filter_map(|m| m.workarea.map(Rect::from))
            .collect())
    }

    async fn get_layout(&self) -> Result<MonitorLayout, DriverError> {
        self.blocking(fetch_layout).await
    }

    async fn get_windows(&self) -> Result<Vec<WindowMetadata>, DriverError> {
        self.blocking(fetch_windows).await
    }
}

#[cfg(test)]
mod tests {
    use super::fake::{FakeSway, Script};
    use super::*;

    const OUTPUTS: &[u8] =
        include_bytes!("../../../../tests/fixtures/sway/outputs_scale2_bar.json");
    const WORKSPACES: &[u8] =
        include_bytes!("../../../../tests/fixtures/sway/workspaces_scale2_bar.json");
    const TREE: &[u8] = include_bytes!("../../../../tests/fixtures/sway/tree_floating.json");
    const OUTPUTS_ONE: &[u8] = include_bytes!("../../../../tests/fixtures/sway/outputs_one.json");

    fn fake_with_everything() -> FakeSway {
        FakeSway::start(
            Script::default()
                .reply(MessageType::GetOutputs, OUTPUTS)
                .reply(MessageType::GetWorkspaces, WORKSPACES)
                .reply(MessageType::GetTree, TREE),
        )
    }

    /// The driver reads the monitor layout over the socket: the scaled output with a bar, as in
    /// the recorded replies. Screen and work area are the rects Sway reports (logical pixels).
    #[tokio::test]
    async fn test_layout_over_the_socket() {
        let fake = fake_with_everything();
        let driver = SwayDriver::connect(fake.path.clone()).unwrap();
        let layout = driver.get_layout().await.unwrap();
        assert_eq!(layout.monitors.len(), 1);
        assert_eq!(
            layout.monitors[0].workarea.map(|w| (w.x, w.y, w.w, w.h)),
            Some((0, 30, 960, 510))
        );
        let monitors = driver.get_monitors().await.unwrap();
        assert_eq!(
            monitors,
            vec![Rect {
                x: 0,
                y: 0,
                width: 960,
                height: 540
            }]
        );
        let workareas = driver.get_workareas().await.unwrap();
        assert_eq!(
            workareas,
            vec![Rect {
                x: 0,
                y: 30,
                width: 960,
                height: 510
            }]
        );
    }

    /// The driver lists the windows of `GET_TREE`; `get_window_rect` (the trait default) finds
    /// one by its container id.
    #[tokio::test]
    async fn test_windows_over_the_socket() {
        let fake = fake_with_everything();
        let driver = SwayDriver::connect(fake.path.clone()).unwrap();
        let windows = driver.get_windows().await.unwrap();
        assert_eq!(windows.len(), 1);
        assert_eq!(windows[0].title, "sway-cap-1");
        assert_eq!(
            CompositorBackend::get_window_rect(&driver, 5)
                .await
                .unwrap(),
            Rect {
                x: 100,
                y: 150,
                width: 800,
                height: 500
            }
        );
        assert!(CompositorBackend::get_window_rect(&driver, 77)
            .await
            .is_err());
    }

    /// Cursor position is not available over Sway IPC: the standard unsupported error (the
    /// spawn command then uses (0, 0) for `--monitor cursor`).
    #[tokio::test]
    async fn test_cursor_position_is_unsupported() {
        let fake = fake_with_everything();
        let driver = SwayDriver::connect(fake.path.clone()).unwrap();
        assert!(matches!(
            driver.get_cursor_position().await,
            Err(DriverError::UnsupportedCapability(_))
        ));
    }

    /// With a missing socket the error names `$SWAYSOCK` (no other backend is tried here).
    #[test]
    fn test_connect_to_a_missing_socket_is_an_error_naming_the_variable() {
        let path =
            std::env::temp_dir().join(format!("spawn-at-no-sway-{}.sock", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let err = SwayDriver::connect(path)
            .err()
            .expect("must fail")
            .to_string();
        assert!(
            err.contains("$SWAYSOCK") && err.contains("SPAWN_AT_BACKEND=sway"),
            "{err}"
        );
    }

    /// A socket that answers with something other than i3-ipc (an HTTP server, say) is refused
    /// with a message saying so, not used.
    #[test]
    fn test_connect_to_a_socket_that_is_not_sway_is_an_error() {
        let fake = FakeSway::start(Script {
            garbage: Some(b"HTTP/1.1 400 Bad Request\r\n\r\n".to_vec()),
            ..Script::default()
        });
        let err = SwayDriver::connect(fake.path.clone())
            .err()
            .expect("must fail")
            .to_string();
        assert!(err.contains("i3-ipc") && err.contains("$SWAYSOCK"), "{err}");
    }

    /// A socket that speaks i3-ipc but sends JSON that is not a list of outputs is refused.
    #[test]
    fn test_connect_to_a_socket_with_unexpected_json_is_an_error() {
        let fake = FakeSway::start(Script::default().reply(MessageType::GetOutputs, b"not json"));
        let err = SwayDriver::connect(fake.path.clone())
            .err()
            .expect("must fail")
            .to_string();
        assert!(err.contains("not like Sway"), "{err}");
    }

    /// `connect` makes exactly one request, `GET_OUTPUTS`, and nothing that changes the session.
    #[test]
    fn test_connect_only_asks_for_the_outputs() {
        let fake = FakeSway::start(Script::default().reply(MessageType::GetOutputs, OUTPUTS_ONE));
        SwayDriver::connect(fake.path.clone()).unwrap();
        assert_eq!(
            fake.requests(),
            vec![(MessageType::GetOutputs as u32, String::new())]
        );
    }

    /// The driver names itself `Sway`, declares no capability it does not have yet and needs no
    /// helper or installation.
    #[test]
    fn test_name_and_default_capabilities() {
        let fake = FakeSway::start(Script::default().reply(MessageType::GetOutputs, OUTPUTS_ONE));
        let driver = SwayDriver::connect(fake.path.clone()).unwrap();
        assert_eq!(driver.name(), "Sway");
        assert!(driver.provisioner().is_none());
        assert!(!driver.capabilities().needs_helper);
    }
}
