//! # Platform & Compositor Driver Abstraction Layer
//!
//! ## Architectural Rationale
//!
//! In desktop ecosystems, window management mechanics differ drastically across
//! display servers (GNOME Wayland, X11, and future macOS/Windows backends).
//! The `CompositorBackend` trait abstracts these differences into a unified interface,
//! allowing `spawn-at` to inspect the runtime environment and route execution to the
//! optimal driver without leaking display-server-specific IPC details to high-level commands.

pub mod capability;
pub mod escalate;
pub mod host;
pub mod install_notice;
pub mod installer;
pub mod provisioner;
pub mod session_actions;
pub mod update_hooks;
pub mod upgrade;

#[cfg(target_os = "linux")]
pub mod linux;

// Other operating systems are not supported yet. This stub provides the few `platform::linux`
// names the neutral code calls, so the crate compiles; see `non_linux.rs`.
#[cfg(not(target_os = "linux"))]
#[path = "non_linux.rs"]
pub mod linux;

#[cfg(target_os = "linux")]
pub use linux::LinuxBackend as NativeBackend;

// The per-OS flags of the install and uninstall commands (see `InstallArgs::backend`); off Linux
// `linux` is the compile-only stub, which has empty ones.
pub use linux::{BackendInstallArgs, BackendUninstallArgs};

pub use crate::target::WindowMetadata;
use clap::Args;
pub use spawn_at_core::driver::{Armed, Batch, Driver, DriverError};
#[cfg(test)]
pub use spawn_at_core::driver::{FocusIntent, Reveal, Urgency};
#[cfg(test)]
pub use spawn_at_core::geometry::LayoutInsets;
pub use spawn_at_core::geometry::{LayoutRect, MonitorInfo, MonitorLayout, PlacementParams, Rect};
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
    /// How the size request ended, from the `ResizeOutcome` signal of extension protocol 6.
    /// `None` for an older extension and for windows the extension waits for with the legacy rule.
    pub resize: Option<spawn_at_core::resize_report::ResizeReport>,
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
    // Only `run_unminimize` constructs this (see the note there); the GNOME and X11 drivers
    // still map it, and the extension still implements the `unminimize` state string.
    #[allow(dead_code)]
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
    /// Go on without asking after the notice for an untested platform (the notice is still printed)
    #[arg(short = 'y', long)]
    pub yes: bool,
    /// Print the notice and what would be installed, then stop without changing anything
    #[arg(long)]
    pub dry_run: bool,
    /// If spawn-at is already installed, remove it first (binary, extension, login update entry) without asking; your configuration and settings are kept
    #[arg(long)]
    pub clean_install: bool,
    /// Update release channel: 'stable' (official) or 'beta' (includes pre-releases); 'all' is a deprecated alias of 'beta'
    #[arg(long, value_parser = ["stable", "beta", "all"])]
    pub update_channel: Option<String>,
    /// Enable visual update notifications in interactive sessions
    #[arg(long, num_args = 0..=1, default_missing_value = "true")]
    pub update_notify: Option<bool>,
    // Flags of the active backend (the GNOME flags on Linux; none elsewhere).
    #[command(flatten)]
    pub backend: BackendInstallArgs,
}

#[derive(Args, Debug, Clone, Default)]
pub struct UninstallArgs {
    #[arg(long, value_enum, default_value_t = InstallScope::User)]
    pub scope: InstallScope,
    #[arg(long)]
    pub skip_bin: bool,
    #[arg(long)]
    pub headless: bool,
    // Flags of the active backend (the GNOME flags on Linux; none elsewhere).
    #[command(flatten)]
    pub backend: BackendUninstallArgs,
}

/// Core interface representing a window manager or compositor driver.
#[async_trait::async_trait]
pub trait CompositorBackend: Driver + Send + Sync {
    /// Human-readable name of the window manager / driver.
    fn name(&self) -> &'static str;

    // --- Setup & Teardown ---

    /// What this backend needs on the machine and how to set it up and remove it; `None` when it
    /// needs nothing (see [`provisioner::Provisioner`]).
    fn provisioner(&self) -> Option<&dyn provisioner::Provisioner> {
        None
    }

    /// A short notice to print (and confirm) before installing when this environment has not been
    /// tested; `None` when it has, or when the driver has nothing to say.
    fn install_notice(&self) -> Option<String> {
        self.provisioner().and_then(|p| p.install_notice())
    }

    /// Installs any necessary system hooks or extensions required by this driver.
    fn install(&self, args: &InstallArgs) -> Result<(), DriverError> {
        match self.provisioner() {
            Some(p) => p.install(args),
            None => {
                println!("No installation required for {}.", self.name());
                Ok(())
            }
        }
    }

    /// Uninstalls any previously installed system hooks or extensions.
    fn uninstall(&self, args: &UninstallArgs) -> Result<(), DriverError> {
        match self.provisioner() {
            Some(p) => p.uninstall(args),
            None => {
                println!("No uninstallation required for {}.", self.name());
                Ok(())
            }
        }
    }

    // --- Capability Checking ---

    /// What this backend can do. The default is built from the `supports_*` methods, so a
    /// backend (or a test double) that only has those behaves as before; the GNOME and X11
    /// drivers return their own values.
    fn capabilities(&self) -> capability::Capabilities {
        capability::Capabilities {
            runtime_transform: self.supports_runtime_transform(),
            claim_wait: self.supports_claim_wait(),
            close: self.supports_close(),
            reveal_control: false,
            needs_helper: false,
        }
    }

    /// True if the compositor supports moving/resizing mapped windows via IPC.
    fn supports_runtime_transform(&self) -> bool {
        false
    }

    /// True if the compositor supports asynchronous event/signal notification for claimed windows.
    // Backend contract: implemented by the GNOME driver and forwarded by LinuxBackend, exercised
    // by tests, but no production code asks it today.
    #[allow(dead_code)]
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

    /// Query monitor layout (screens, workareas, scales, and insets).
    async fn get_layout(&self) -> Result<MonitorLayout, DriverError> {
        let workareas = self.get_workareas().await?;
        let monitors = workareas
            .into_iter()
            .enumerate()
            .map(|(i, wa)| MonitorInfo {
                index: i,
                name: None,
                primary: i == 0,
                scale: None,
                screen: None,
                workarea: Some(wa.into()),
                insets: None,
            })
            .collect();
        Ok(MonitorLayout {
            schema_version: 1,
            monitors,
            note: None,
        })
    }

    /// Query compositor extension protocol version if supported (None for X11 / unversioned backends).
    async fn protocol_version(&self) -> Option<u32> {
        None
    }

    /// Whether the backend's helper is new enough for `need`. The default compares the
    /// `protocol_version` the backend reports with the protocol `need` requires, so the
    /// commands hold no protocol numbers or messages.
    async fn helper_status(&self, need: capability::HelperNeed) -> capability::HelperStatus {
        capability::HelperStatus::from_protocol(self.protocol_version().await, need)
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
    // Backend contract for the extension's MoveWindow D-Bus method (protocol surface); no
    // command calls it today, only tests do.
    #[allow(dead_code)]
    async fn move_window(&self, _target_id: &str, _x: i32, _y: i32) -> Result<(), DriverError> {
        Err(DriverError::UnsupportedCapability("move_window"))
    }

    /// Moves and resizes a target window to the specified geometry.
    // Same as `move_window`: backend contract, no production caller, tests only.
    #[allow(dead_code)]
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

/// Stops the program with one clear message on an operating system spawn-at does not support
/// yet. Does nothing on Linux.
pub fn ensure_supported_os() {
    #[cfg(not(target_os = "linux"))]
    {
        crate::diagnostics::render_error("spawn-at does not support this operating system yet");
        std::process::exit(1);
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

/// The [`host::Host`] of this operating system.
pub fn host() -> &'static dyn host::Host {
    &linux::host::NativeHost
}

/// How this machine logs out or reloads the shell (see [`session_actions::SessionActions`]).
pub fn session_actions() -> &'static dyn session_actions::SessionActions {
    linux::session_actions()
}

/// The desktop-specific steps of `spawn-at update` (see [`update_hooks::UpdateHooks`]).
pub fn update_hooks() -> &'static dyn update_hooks::UpdateHooks {
    linux::update_hooks()
}
