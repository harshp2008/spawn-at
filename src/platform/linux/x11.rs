//! # X11 Fallback Compositor Driver
//!
//! ## Architectural Notes on X11 Window Management
//!
//! Under traditional X11:
//! - Window hierarchies and global coordinates are exposed across the entire X display.
//! - External window managers and placement tools can manipulate window geometries
//!   using the Extended Window Manager Hints (EWMH) specification (`_NET_MOVERESIZE_WINDOW`,
//!   `_NET_WM_DESKTOP`, etc.).
//! - However, achieving true zero-flicker "cold starts" under X11 is non-trivial without
//!   either:
//!     1. An X11 composite manager plugin (e.g. Picom, KWin X11, or Compiz) that cloaks
//!        unmapped windows before the first frame presentation.
//!     2. An `LD_PRELOAD` interception shim on `XCreateWindow` / `XMapWindow`.
//!
//! This driver provides a reliable baseline implementation, launching the target process
//! while informing the user that compositor-assisted zero-flicker hooks are currently
//! focused on the Wayland engine.

use crate::platform::{
    Armed, Batch, CompositorBackend, Driver, DriverError, Rect, WindowMetadata, WindowState,
};

pub struct X11Driver;

#[async_trait::async_trait]
impl Driver for X11Driver {
    async fn arm(&self, batch: Batch) -> Result<Armed, DriverError> {
        let mut launch_env = Vec::new();
        for entry in &batch.entries {
            launch_env.push(("XDG_ACTIVATION_TOKEN".to_string(), entry.key.clone()));
            launch_env.push(("DESKTOP_STARTUP_ID".to_string(), entry.key.clone()));
        }
        Ok(Armed { launch_env })
    }
}

#[async_trait::async_trait]
impl CompositorBackend for X11Driver {
    fn name(&self) -> &'static str {
        "X11"
    }

    fn supports_runtime_transform(&self) -> bool {
        false
    }

    fn resolve_id(&self, command: &[String], explicit_class: Option<&str>) -> String {
        let bin = command.first().map(|s| s.as_str()).unwrap_or("");
        crate::platform::linux::xdg::resolve_linux_app_id(bin, explicit_class)
    }

    /// Queries pointer position using xdotool.
    async fn get_cursor_position(&self) -> Result<(i32, i32), DriverError> {
        Ok(crate::platform::linux::query_xdotool_cursor().unwrap_or((0, 0)))
    }

    /// Queries active display monitor geometries via xrandr.
    async fn get_monitors(&self) -> Result<Vec<Rect>, DriverError> {
        Ok(crate::platform::linux::query_xrandr_monitors())
    }

    /// Queries active workareas (defaults to active monitor boundaries on X11).
    async fn get_workareas(&self) -> Result<Vec<Rect>, DriverError> {
        self.get_monitors().await
    }

    async fn get_windows(&self) -> Result<Vec<WindowMetadata>, DriverError> {
        Err(DriverError::UnsupportedCapability(
            "Window querying is currently not implemented for X11",
        ))
    }

    async fn move_window(&self, _target_id: &str, _x: i32, _y: i32) -> Result<(), DriverError> {
        Err(DriverError::UnsupportedCapability(
            "Runtime window movement is currently not implemented for X11",
        ))
    }

    async fn move_resize_window(
        &self,
        _target_id: &str,
        _x: i32,
        _y: i32,
        _w: u32,
        _h: u32,
    ) -> Result<(), DriverError> {
        Err(DriverError::UnsupportedCapability(
            "Runtime window move/resize transformation is currently not implemented for X11",
        ))
    }

    async fn set_window_state(
        &self,
        _target_id: &str,
        _state: WindowState,
    ) -> Result<(), DriverError> {
        Err(DriverError::UnsupportedCapability(
            "Window state manipulation is currently not implemented for X11",
        ))
    }

    async fn focus_window(&self, _target_id: &str) -> Result<(), DriverError> {
        Err(DriverError::UnsupportedCapability(
            "Window focus control is currently not implemented for X11",
        ))
    }

    async fn defocus_window(
        &self,
        _target_id: &str,
        _mode: &str,
        _destination: &str,
    ) -> Result<(), DriverError> {
        Err(DriverError::UnsupportedCapability(
            "Window defocus control is currently not implemented for X11",
        ))
    }

    async fn run_daemon(&self) -> Result<(), DriverError> {
        Err(DriverError::UnsupportedCapability(
            "Background daemon mode is currently not implemented for X11",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_x11_unsupported_capabilities() {
        let driver = X11Driver;
        assert_eq!(driver.name(), "X11");
        assert!(!driver.supports_runtime_transform());

        assert!(matches!(
            driver.get_windows().await,
            Err(DriverError::UnsupportedCapability(_))
        ));

        assert!(matches!(
            driver.move_window("test", 10, 20).await,
            Err(DriverError::UnsupportedCapability(_))
        ));

        assert!(matches!(
            driver.move_resize_window("test", 0, 0, 100, 100).await,
            Err(DriverError::UnsupportedCapability(_))
        ));

        assert!(matches!(
            driver.set_window_state("test", WindowState::Maximize).await,
            Err(DriverError::UnsupportedCapability(_))
        ));

        assert!(matches!(
            driver.focus_window("test").await,
            Err(DriverError::UnsupportedCapability(_))
        ));

        assert!(matches!(
            driver.defocus_window("test", "desktop", "").await,
            Err(DriverError::UnsupportedCapability(_))
        ));

        assert!(matches!(
            driver.run_daemon().await,
            Err(DriverError::UnsupportedCapability(_))
        ));
    }
}
