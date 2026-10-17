//! The seam between the neutral commands and the per-desktop drivers.
//!
//! [`CompositorBackend`] is the trait every driver implements; the commands only hold a
//! `Box<dyn CompositorBackend>` from [`init_backend`] and never name a display server. This
//! module also declares the small per-OS traits ([`host::Host`], [`session_actions::SessionActions`],
//! [`update_hooks::UpdateHooks`]) and the types shared by their callers ([`ClaimResult`],
//! [`WindowState`], [`InstallArgs`], [`UninstallArgs`]).
//!
//! It must not contain display-server code: that lives in the driver modules under `linux`.

// Implementors of `CompositorBackend` at this commit (not verified for other commits): the
// `linux::LinuxBackend` adapter that wraps one of `GnomeWaylandDriver`, `SwayDriver` and
// `X11Driver`, and `null_backend::NullBackend`. The rest are test doubles.
pub mod capability;
pub mod escalate;
pub mod host;
pub mod install_notice;
pub mod installer;
pub mod null_backend;
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
    // The GNOME subscription copies these fields from the extension's claim signal and returns
    // `Err` instead of a `ClaimResult` when the signal has `success == false`, so `success` is
    // always true in a result it returns (read in `GnomeClaimSubscription::wait_claim`).
    // The GNOME subscription sets `target_id` to the id in the signal, which can be the
    // wildcard `"*"` when the signal used it.
    /// The target id the claim was reported for.
    pub target_id: String,
    pub success: bool,
    /// The id of the claimed window.
    pub window_id: u64,
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
    // What `size_raised` means is defined by the extension; not verified here.
    pub size_raised: bool,
    pub error: String,
    /// How the size request ended, from the `ResizeOutcome` signal of extension protocol 6.
    /// `None` for an older extension and for windows the extension waits for with the legacy rule.
    pub resize: Option<spawn_at_core::resize_report::ResizeReport>,
}

/// A subscription to window claim notifications issued by the compositor backend.
///
/// Created by [`CompositorBackend::prepare_claim_wait`] before arming, so a signal sent right
/// after arming is not missed.
// Implemented by `GnomeClaimSubscription` (src/platform/linux/gnome/driver.rs); the rest are
// test doubles.
#[async_trait::async_trait]
pub trait ClaimSubscription: Send + Sync {
    /// Awaits the next claim signal matching the armed target, for at most `timeout`.
    ///
    /// Errors: [`DriverError`]; the variants are up to the implementor.
    // The GNOME one returns TargetNotFound on a timeout or a closed signal stream and Execution
    // when the signal reports a failed placement.
    async fn wait_claim(&mut self, timeout: Duration) -> Result<ClaimResult, DriverError>;
}

/// The window state `CompositorBackend::set_window_state` is asked to set.
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

// The scope flag of install and uninstall (`--scope`). Clap derives: a `///` here or on a field
// of `InstallArgs`/`UninstallArgs` would change the `--help` text, so these notes are plain `//`.
// `User` is the default for both commands; where each scope puts the binary is decided by
// `host::Host::bin_dir`, not here.
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

/// What one desktop's driver offers the commands. It extends [`Driver`], so an implementor also
/// provides [`Driver::arm`] (required; [`Driver::disarm`] has a default).
///
/// Required methods: [`CompositorBackend::name`], [`CompositorBackend::resolve_id`] and
/// `Driver::arm`. Every other method has a default; read each default below as "this backend
/// does not do that", so an implementor overrides exactly what it can do and keeps its
/// [`CompositorBackend::capabilities`] consistent with it.
///
/// An implementor must keep [`CompositorBackend::capabilities`] true only for operations whose
/// methods it overrides: a command that checks a capability first refuses with its own message
/// when it is false and never calls the method. Failures are reported as [`DriverError`]; the
/// defaults return [`DriverError::UnsupportedCapability`] carrying the method name, except
/// [`CompositorBackend::close_window`] (a [`DriverError::Execution`] with a fixed message).
// Implementors at this commit: `GnomeWaylandDriver`, `SwayDriver` and `X11Driver` (the real
// drivers), `linux::LinuxBackend` (forwards every method to one of them) and
// `null_backend::NullBackend`; the rest are test doubles in test modules.
// Callers that check a capability first, at this commit: `commands/transform.rs` (`runtime_transform`)
// and `commands/lifecycle.rs` (`close`). Other commands call the methods directly and handle the
// error; `reveal_control` and `needs_helper` are read by no command.
#[async_trait::async_trait]
pub trait CompositorBackend: Driver + Send + Sync {
    /// Short name of the driver, for messages (required, no default).
    fn name(&self) -> &'static str;

    // --- Setup & Teardown ---

    /// What this backend needs on the machine and how to set it up and remove it; `None` when it
    /// needs nothing (see [`provisioner::Provisioner`]). The default is `None`.
    fn provisioner(&self) -> Option<&dyn provisioner::Provisioner> {
        None
    }

    /// A short notice to print (and confirm) before installing when this environment has not been
    /// tested; `None` when it has, or when the driver has nothing to say. The default asks
    /// [`CompositorBackend::provisioner`] and gives `None` without one.
    fn install_notice(&self) -> Option<String> {
        self.provisioner().and_then(|p| p.install_notice())
    }

    /// Installs any necessary system hooks or extensions required by this driver.
    ///
    /// The default calls the provisioner's `install`; without a provisioner it prints
    /// `No installation required for <name>.` to stdout and returns `Ok(())`. Any other side
    /// effect is the provisioner's.
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
    ///
    /// The default calls the provisioner's `uninstall`; without one it prints
    /// `No uninstallation required for <name>.` and returns `Ok(())`.
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

    /// What this backend can do. The default copies `runtime_transform`, `claim_wait` and
    /// `close` from the `supports_*` methods and sets `reveal_control` and `needs_helper` to
    /// false. The GNOME, Sway and X11 drivers return their own values.
    fn capabilities(&self) -> capability::Capabilities {
        capability::Capabilities {
            runtime_transform: self.supports_runtime_transform(),
            claim_wait: self.supports_claim_wait(),
            close: self.supports_close(),
            reveal_control: false,
            needs_helper: false,
        }
    }

    /// True if the compositor supports moving/resizing mapped windows via IPC. Default: false.
    fn supports_runtime_transform(&self) -> bool {
        false
    }

    /// True if the compositor supports asynchronous event/signal notification for claimed windows.
    /// Default: false.
    // Backend contract: implemented by the GNOME driver and forwarded by LinuxBackend, exercised
    // by tests, but no production code asks it today.
    #[allow(dead_code)]
    fn supports_claim_wait(&self) -> bool {
        false
    }

    /// True if the compositor backend supports closing windows. Default: false.
    fn supports_close(&self) -> bool {
        false
    }

    /// Prepares a subscription to wait for window claim notifications before arming.
    ///
    /// The default returns `Ok(None)`: the backend has no claim signal.
    async fn prepare_claim_wait(
        &self,
        _target_id: &str,
    ) -> Result<Option<Box<dyn ClaimSubscription>>, DriverError> {
        Ok(None)
    }

    /// Queries the current geometry of a window by its unique ID.
    ///
    /// The default looks the id up in [`CompositorBackend::get_windows`] and returns
    /// [`DriverError::TargetNotFound`] when no window has it.
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
    /// for the given command and optional explicit class override. Required, no default.
    fn resolve_id(&self, command: &[String], explicit_class: Option<&str>) -> String;

    /// Query current pointer position (X, Y) in compositor/screen coordinates.
    /// The default returns `UnsupportedCapability("get_cursor_position")`.
    async fn get_cursor_position(&self) -> Result<(i32, i32), DriverError> {
        Err(DriverError::UnsupportedCapability("get_cursor_position"))
    }

    /// Query active display monitor bounding boxes. The default is an empty list.
    async fn get_monitors(&self) -> Result<Vec<Rect>, DriverError> {
        Ok(Vec::new())
    }

    /// Query active workareas (monitor bounds minus panels/docks). The default returns
    /// [`CompositorBackend::get_monitors`], i.e. no panels subtracted.
    async fn get_workareas(&self) -> Result<Vec<Rect>, DriverError> {
        self.get_monitors().await
    }

    /// Query active windows. The default returns `UnsupportedCapability("get_windows")`.
    async fn get_windows(&self) -> Result<Vec<WindowMetadata>, DriverError> {
        Err(DriverError::UnsupportedCapability("get_windows"))
    }

    /// Query monitor layout (screens, workareas, scales, and insets).
    ///
    /// The default builds a `schema_version` 1 layout from [`CompositorBackend::get_workareas`]:
    /// one entry per workarea in order, index 0 marked primary, only `index`, `primary` and
    /// `workarea` filled, no `note`.
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

    /// Query compositor extension protocol version if supported (None for X11 / unversioned
    /// backends). The default is `None`.
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
    /// The default returns `UnsupportedCapability("transform_window")`; the same default
    /// pattern (with the method's own name) applies to `move_window`, `move_resize_window`,
    /// `set_window_state`, `focus_window` and `defocus_window`.
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

    /// Requests graceful closure of the specified target window. The default returns
    /// `DriverError::Execution("this backend cannot close windows")`, not `UnsupportedCapability`.
    async fn close_window(&self, _target_id: &str) -> Result<(), DriverError> {
        Err(DriverError::Execution(
            "this backend cannot close windows".into(),
        ))
    }

    /// Optional post-spawn synchronization hook (e.g. for X11 window interception). The default
    /// does nothing and returns `Ok(())`. Only the Sway and X11 drivers override it.
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

/// Builds the backend for this session.
///
/// On Linux this is `LinuxBackend::bootstrap`, which reads `SPAWN_AT_BACKEND`,
/// `XDG_CURRENT_DESKTOP` and `XDG_SESSION_TYPE` (see `linux::registry`) and fails with
/// [`DriverError::Execution`] on a desktop it does not support, or when a forced Sway cannot be
/// reached. On other systems it returns the refusing `NullBackend`.
pub async fn init_backend() -> Result<Box<dyn CompositorBackend>, DriverError> {
    #[cfg(target_os = "linux")]
    return NativeBackend::bootstrap().await;

    #[cfg(not(target_os = "linux"))]
    Ok(Box::new(null_backend::NullBackend))
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
