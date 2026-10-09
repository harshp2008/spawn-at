//! # Platform & Compositor Driver Abstraction Layer
//!
//! ## Architectural Rationale
//!
//! In desktop ecosystems, window management mechanics differ drastically across
//! display servers (GNOME Wayland, X11, and future macOS/Windows backends).
//! The `CompositorBackend` trait abstracts these differences into a unified interface,
//! allowing `spawn-at` to inspect the runtime environment and route execution to the
//! optimal driver without leaking display-server-specific IPC details to high-level commands.

pub mod escalate;
pub mod installer;

#[cfg(target_os = "linux")]
pub mod linux;

#[cfg(target_os = "linux")]
pub use linux::LinuxBackend as NativeBackend;

pub use crate::target::WindowMetadata;
use clap::Args;
pub use spawn_at_core::driver::{
    Armed, Batch, Driver, DriverError, Entry, FocusIntent, Reveal, Urgency,
};
pub use spawn_at_core::geometry::{PlacementParams, Rect};
use std::time::Duration;

/// The result reported when a compositor claims and finishes placement of an armed window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaimResult {
    pub target_id: String,
    pub success: bool,
    pub window_id: u64,
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
    pub size_raised: bool,
    pub error: String,
}

/// A subscription to window claim notifications issued by the compositor backend.
#[async_trait::async_trait]
pub trait ClaimSubscription: Send + Sync {
    /// Awaits the next claim signal matching the armed target or times out.
    async fn wait_claim(&mut self, timeout: Duration) -> Result<ClaimResult, DriverError>;
}

/// The desired window state for transformations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowState {
    Maximize,
    Minimize,
    Unminimize,
    Restore,
}

#[derive(clap::ValueEnum, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum InstallScope {
    #[default]
    User,
    System,
}

#[derive(Args, Debug, Clone, Default)]
pub struct InstallArgs {
    #[arg(long, value_enum, default_value_t = InstallScope::User)]
    pub scope: InstallScope,
    #[arg(long)]
    pub skip_bin: bool,
    #[arg(long)]
    pub headless: bool,
    /// Update release channel: 'stable' (official) or 'all' (beta/pre-releases)
    #[arg(long, value_parser = ["stable", "all"])]
    pub update_channel: Option<String>,
    /// Enable visual update notifications in interactive sessions
    #[arg(long, num_args = 0..=1, default_missing_value = "true")]
    pub update_notify: Option<bool>,
    #[cfg(target_os = "linux")]
    #[command(flatten)]
    pub gnome: crate::platform::linux::gnome::GnomeInstallArgs,
}

#[derive(Args, Debug, Clone, Default)]
pub struct UninstallArgs {
    #[arg(long, value_enum, default_value_t = InstallScope::User)]
    pub scope: InstallScope,
    #[arg(long)]
    pub skip_bin: bool,
    #[arg(long)]
    pub headless: bool,
    #[cfg(target_os = "linux")]
    #[command(flatten)]
    pub gnome: crate::platform::linux::gnome::GnomeUninstallArgs,
}

/// Core interface representing a window manager or compositor driver.
#[async_trait::async_trait]
pub trait CompositorBackend: Driver + Send + Sync {
    /// Human-readable name of the window manager / driver.
    fn name(&self) -> &'static str;

    // --- Setup & Teardown ---

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

    // --- Capability Checking ---

    /// True if the compositor supports moving/resizing mapped windows via IPC.
    fn supports_runtime_transform(&self) -> bool {
        false
    }

    /// True if the compositor supports asynchronous event/signal notification for claimed windows.
    fn supports_claim_wait(&self) -> bool {
        false
    }

    /// True if the compositor backend supports closing windows.
    fn supports_close(&self) -> bool {
        false
    }

    /// Prepares a subscription to wait for window claim notifications before arming.
    async fn prepare_claim_wait(
        &self,
        _target_id: &str,
    ) -> Result<Option<Box<dyn ClaimSubscription>>, DriverError> {
        Ok(None)
    }

    /// Queries the current geometry of a window by its unique ID.
    async fn get_window_rect(&self, id: u64) -> Result<Rect, DriverError> {
        let windows = self.get_windows().await?;
        windows
            .into_iter()
            .find(|w| w.id == Some(id))
            .map(|w| Rect {
                x: w.x,
                y: w.y,
                width: w.w as u32,
                height: w.h as u32,
            })
            .ok_or_else(|| DriverError::TargetNotFound(format!("Window ID {} not found", id)))
    }

    // --- Environment Queries ---

    /// Resolves the application identifier (e.g. Wayland App ID, X11 WM_CLASS, Windows executable name, or macOS Bundle ID)
    /// for the given command and optional explicit class override.
    fn resolve_id(&self, command: &[String], explicit_class: Option<&str>) -> String;

    /// Query current pointer position (X, Y) in compositor/screen coordinates.
    async fn get_cursor_position(&self) -> Result<(i32, i32), DriverError> {
        Err(DriverError::UnsupportedCapability("get_cursor_position"))
    }

    /// Query active display monitor bounding boxes.
    async fn get_monitors(&self) -> Result<Vec<Rect>, DriverError> {
        Ok(Vec::new())
    }

    /// Query active workareas (monitor bounds minus panels/docks).
    async fn get_workareas(&self) -> Result<Vec<Rect>, DriverError> {
        self.get_monitors().await
    }

    /// Query active windows.
    async fn get_windows(&self) -> Result<Vec<WindowMetadata>, DriverError> {
        Err(DriverError::UnsupportedCapability("get_windows"))
    }

    // --- Core Actions ---

    /// Performs an animated or atomic transform on an existing window.
    async fn transform_window(
        &self,
        _target_id: &str,
        _params: PlacementParams,
        _current_w: u32,
        _current_h: u32,
    ) -> Result<(), DriverError> {
        Err(DriverError::UnsupportedCapability("transform_window"))
    }

    /// Moves a window to the specified coordinates without altering its dimensions.
    async fn move_window(&self, _target_id: &str, _x: i32, _y: i32) -> Result<(), DriverError> {
        Err(DriverError::UnsupportedCapability("move_window"))
    }

    /// Moves and resizes a target window to the specified geometry.
    async fn move_resize_window(
        &self,
        _target_id: &str,
        _x: i32,
        _y: i32,
        _w: u32,
        _h: u32,
    ) -> Result<(), DriverError> {
        Err(DriverError::UnsupportedCapability("move_resize_window"))
    }

    /// Modifies the window state (maximize, minimize, unminimize, restore).
    async fn set_window_state(
        &self,
        _target_id: &str,
        _state: WindowState,
    ) -> Result<(), DriverError> {
        Err(DriverError::UnsupportedCapability("set_window_state"))
    }

    /// Gives focus to the specified target window.
    async fn focus_window(&self, _target_id: &str) -> Result<(), DriverError> {
        Err(DriverError::UnsupportedCapability("focus_window"))
    }

    /// Evicts focus from the target window to another target (or desktop/previous).
    async fn defocus_window(
        &self,
        _target_id: &str,
        _mode: &str,
        _destination: &str,
    ) -> Result<(), DriverError> {
        Err(DriverError::UnsupportedCapability("defocus_window"))
    }

    /// Requests graceful closure of the specified target window.
    async fn close_window(&self, _target_id: &str) -> Result<(), DriverError> {
        Err(DriverError::Execution(
            "this backend cannot close windows".into(),
        ))
    }

    /// Optional post-spawn synchronization hook (e.g. for X11 window interception).
    async fn post_spawn(&self, _child_pid: u32, _batch: &Batch) -> Result<(), DriverError> {
        Ok(())
    }
}

/// Automatically detects the active desktop environment and returns the appropriate driver.
pub async fn init_backend() -> Result<Box<dyn CompositorBackend>, DriverError> {
    #[cfg(target_os = "linux")]
    return NativeBackend::bootstrap().await;

    #[cfg(not(target_os = "linux"))]
    Err(DriverError::UnsupportedCapability(
        "Unsupported operating system",
    ))
}

/// Alias for `init_backend` to preserve backwards compatibility.
pub async fn get_active_driver() -> Result<Box<dyn CompositorBackend>, DriverError> {
    init_backend().await
}
