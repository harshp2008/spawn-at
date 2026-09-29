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

use super::{DriverError, TargetGeometry, WindowManager};

pub struct X11Driver;

impl WindowManager for X11Driver {
    fn name(&self) -> &'static str {
        "X11"
    }

    fn resolve_id(&self, command: &[String], explicit_class: Option<&str>) -> String {
        let bin = command.first().map(|s| s.as_str()).unwrap_or("");
        crate::platform::linux::xdg::resolve_linux_app_id(bin, explicit_class)
    }

    fn spawn_at(
        &self,
        _app_id: &str,
        command: &[String],
        _geom: &TargetGeometry,
    ) -> Result<(), DriverError> {
        println!("Warning: Perfect cold-starts not fully supported on X11 yet.");
        std::process::Command::new(&command[0])
            .args(&command[1..])
            .spawn()
            .map_err(|e| DriverError::Execution(Box::new(e)))?;
        Ok(())
    }

    /// Queries pointer position using xdotool.
    fn get_cursor_position(&self) -> Option<(i32, i32)> {
        crate::platform::linux::query_xdotool_cursor()
    }

    /// Queries active display monitor geometries via xrandr.
    fn get_monitors(&self) -> Vec<crate::geometry::Rect> {
        crate::platform::linux::query_xrandr_monitors()
    }
}
