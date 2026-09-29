//! # Compositor Driver Abstraction Layer
//!
//! ## Architectural Rationale
//!
//! In Linux desktop ecosystems, window management mechanics differ drastically across
//! display servers:
//!
//! 1. **X11 (Legacy Shared Canvas):**
//!    Under X11, clients communicate with the X server over a shared protocol where window
//!    IDs, coordinates, and properties (`_NET_WM_...`) are globally visible and mutable.
//!    Any client can theoretically move or resize any other window, though race conditions
//!    during initial mapping still cause visual jitter without specialized compositing hooks.
//!
//! 2. **GNOME Wayland (Compositor-Enforced Isolation):**
//!    Wayland intentionally eliminates global coordinate systems and client-driven placement
//!    for security and architectural purity. A client cannot specify where its surface appears.
//!    To achieve "cold-start" placement without visual latency, we must inject hooks into the
//!    compositor itself (via an embedded GNOME Shell extension).
//!
//! 3. **Future Extensibility (Hyprland, wlroots, KDE Plasma):**
//!    Other compositors provide custom IPC protocols (e.g., Hyprland's UNIX socket IPC,
//!    `wlr-foreign-toplevel-management`, or KDE's KWin scripting).
//!
//! The `WindowManager` trait abstracts these radical differences into a unified interface,
//! allowing `spawn-at` to inspect the runtime environment (`XDG_SESSION_TYPE`,
//! `XDG_CURRENT_DESKTOP`) and route execution to the optimal driver without leaking
//! display-server-specific IPC details to the rest of the application.

pub mod gnome;
pub mod x11;

use std::fmt;

/// Errors produced during driver detection, installation, or window positioning.
#[derive(Debug)]
pub enum DriverError {
    /// The requested operation is not supported by the active driver.
    Unsupported(&'static str),
    /// A runtime execution error occurred (e.g. D-Bus communication failure, I/O error).
    Execution(Box<dyn std::error::Error>),
}

impl fmt::Display for DriverError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DriverError::Unsupported(feature) => {
                write!(f, "Feature '{}' is unsupported by the current driver", feature)
            }
            DriverError::Execution(err) => {
                write!(f, "Driver execution error: {}", err)
            }
        }
    }
}

impl std::error::Error for DriverError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            DriverError::Unsupported(_) => None,
            DriverError::Execution(err) => Some(err.as_ref()),
        }
    }
}

/// Target geometry for the window to be spawned.
///
/// Coordinates `(x, y)` define the upper-left corner of the window frame.
/// Dimensions `(w, h)` define the target width and height in pixels.
/// If `w` or `h` are 0, the driver preserves the window's natural or client-negotiated dimensions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TargetGeometry {
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
}

use clap::Args;
use crate::drivers::gnome::{GnomeInstallArgs, GnomeUninstallArgs};

#[derive(clap::ValueEnum, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum InstallScope {
    #[default]
    User,
    System,
}

#[derive(Args, Debug, Clone, Default)]
pub struct InstallArgs {
    /// Target scope for binary installation: 'user' (~/.local/bin) or 'system' (/usr/local/bin)
    #[arg(long, value_enum, default_value_t = InstallScope::User)]
    pub scope: InstallScope,

    /// Skip copying the binary to $PATH (only configure the compositor)
    #[arg(long)]
    pub skip_bin: bool,

    /// Run without interactive prompts (applies flags or safe defaults)
    #[arg(long)]
    pub headless: bool,

    #[command(flatten)]
    pub gnome: GnomeInstallArgs,
    
    // Future compositors go here (e.g., pub hyprland: HyprlandInstallArgs)
}

#[derive(Args, Debug, Clone, Default)]
pub struct UninstallArgs {
    /// Target scope from which to remove the binary: 'user' (~/.local/bin) or 'system' (/usr/local/bin)
    #[arg(long, value_enum, default_value_t = InstallScope::User)]
    pub scope: InstallScope,

    /// Skip removing the binary from $PATH (only unconfigure the compositor)
    #[arg(long)]
    pub skip_bin: bool,

    /// Run without interactive prompts
    #[arg(long)]
    pub headless: bool,

    #[command(flatten)]
    pub gnome: GnomeUninstallArgs,
}

pub use crate::geometry::Rect;

/// Core interface representing a window manager or compositor driver.
///
/// Implementations handle installation requirements (such as deploying compositor extensions),
/// hardware/display queries, and execute zero-flicker window spawns.
pub trait WindowManager {
    /// Human-readable name of the window manager / driver.
    fn name(&self) -> &'static str;

    /// Resolves the application identifier (e.g. Wayland App ID, X11 WM_CLASS, Windows executable name, or macOS Bundle ID)
    /// for the given command and optional explicit class override.
    fn resolve_id(&self, command: &[String], explicit_class: Option<&str>) -> String;

    /// Spawns a process and places its primary window at the specified geometry.
    ///
    /// - `app_id`: Resolved Wayland App ID or X11 WM_CLASS (e.g., "org.gnome.TextEditor" or "*")
    /// - `command`: Full command vector, where `command[0]` is the executable
    /// - `geom`: Target coordinates and dimensions
    fn spawn_at(
        &self,
        _app_id: &str,
        _command: &[String],
        _geom: &TargetGeometry,
    ) -> Result<(), DriverError> {
        Err(DriverError::Unsupported("spawn_at"))
    }

    /// Installs any necessary system hooks or extensions required by this driver.
    fn install(&self, _args: &InstallArgs) -> Result<(), DriverError> {
        println!("No installation required for {}.", self.name());
        Ok(())
    }

    /// Uninstalls any previously installed system hooks or extensions.
    fn uninstall(&self, _args: &UninstallArgs) -> Result<(), DriverError> {
        println!("No uninstallation required for {}.", self.name());
        Ok(())
    }

    /// Query current pointer position (X, Y) in compositor/screen coordinates.
    fn get_cursor_position(&self) -> Option<(i32, i32)> {
        None
    }

    /// Query active display monitor bounding boxes.
    fn get_monitors(&self) -> Vec<Rect> {
        Vec::new()
    }
}

/// Automatically detects the active desktop environment and returns the appropriate driver.
///
/// Routes GNOME Wayland sessions to [`gnome::GnomeWaylandDriver`], and falls back to
/// [`x11::X11Driver`] for X11 or unspecified desktop sessions.
pub fn get_active_driver() -> Box<dyn WindowManager> {
    let session_type = std::env::var("XDG_SESSION_TYPE").unwrap_or_default().to_lowercase();
    let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default().to_uppercase();

    if session_type == "wayland" && desktop.contains("GNOME") {
        Box::new(gnome::GnomeWaylandDriver)
    } else {
        Box::new(x11::X11Driver)
    }
}
