//! # GNOME Compositor Backend & Extension Driver
//!
//! Provides the primary compositor backend for GNOME Shell on Linux across both
//! Wayland and X11 sessions.
//!
//! ## Architectural Overview: Zero-Flicker Window Placement on GNOME
//!
//! ### 1. The Wayland Isolation Barrier
//! Wayland compositors (specifically Mutter in GNOME) enforce a strict isolation model:
//! client applications do not know their absolute desktop coordinates and are denied
//! any protocol requests to position their own windows globally on screen.
//! Unlike X11, there is no `XMoveWindow` or EWMH `_NET_MOVERESIZE_WINDOW` equivalent
//! available to arbitrary processes.
//!
//! ### 2. The Arming & Pre-Registration Pattern
//! To place a window at exact coordinates on its very first frame without flash-and-jump,
//! the sequence of events is inverted:
//!
//! - **Conventional (Faulty) Flow:**
//!   1. Spawn process -> 2. Wait for window to map -> 3. Reposition window.
//!      Result: Window flickers at default location for 1-5 frames before jumping.
//!
//! - **Spawn-At (Zero-Flicker) Flow:**
//!   1. **D-Bus Arm:** Call `ArmSpawn(target_id, instructions_json)` on the GNOME Shell extension.
//!      The compositor registers an expected window target in memory *before* the
//!      application process is even spawned.
//!   2. **Process Spawn:** The child process is launched with `XDG_ACTIVATION_TOKEN` / `DESKTOP_STARTUP_ID`.
//!      It connects to the display socket and submits its surface.
//!   3. **Synchronous Interception:** Mutter triggers `window-created`. The extension
//!      matches the window's startup ID / `app_id` with the armed target.
//!   4. **Opacity Cloaking:** As soon as Mutter creates the `ClutterActor` during `map`,
//!      the extension sets `actor.opacity = 0` synchronously. Mutter renders nothing to the
//!      screen while the initial frame geometry settles.
//!   5. **Atomic Reveal:** Once `window.get_frame_rect()` matches the target coordinates,
//!      `actor.opacity = 255` is restored.
//!
//! ## Table of Contents
//! - **1. CLI Configuration & Arguments**
//!   - [`GnomeInstallArgs`]: Flags controlling extension enablement, X11 reload, and Wayland logout during install.
//!   - [`GnomeUninstallArgs`]: Flags controlling file cleanup, X11 reload, and Wayland logout during uninstall.
//! - **2. Session & Desktop Detection**
//!   - [`is_wayland_session`]: Detects whether current desktop is running on Wayland via `loginctl`.
//! - **3. Execution & Reload Helpers**
//!   - [`restart_gnome_shell_x11`]: Simulates `Alt+F2` -> `r` -> `Enter` via `xdotool` on X11.
//! - **4. Driver Construction & Internal State**
//!   - [`GnomeWaylandDriver`]: Core driver managing D-Bus communication with the GNOME extension.
//! - **5. Pre-Map Arming Driver Trait**
//!   - [`Driver`]: Pre-registers window targets and provides launch environment variables.
//! - **6. Runtime Compositor Backend Trait**
//!   - [`CompositorBackend`]: Implements `install`, `uninstall`, window transformations, state queries, and daemon mode.
//! - **7. Unit Tests**
//!   - Test suite verifying argument defaults, `--no-action` overrides, flag toggles, and session queries.

pub mod dbus;
pub mod mechanics;

use crate::platform::{
    Armed, Batch, CompositorBackend, Driver, DriverError, InstallArgs, PlacementParams, Rect,
    UninstallArgs, WindowMetadata, WindowState,
};
use clap::Args;
use dialoguer::{Confirm, Select};
use std::fs;
use std::io::IsTerminal;
use std::path::PathBuf;
use std::process::Command;

// ============================================================================
// 1. CLI Configuration & Arguments
// ============================================================================

/// Command-line arguments for the GNOME extension installation step.
#[derive(Args, Debug, Clone, Default)]
pub struct GnomeInstallArgs {
    /// \[GNOME\] Automatically enable the GNOME Shell extension
    #[arg(long, default_value_t = true, num_args = 0..=1, default_missing_value = "true")]
    pub gnome_auto_enable: bool,

    /// \[GNOME X11\] Reload GNOME Shell in-place on X11 (non-destructive: apps stay open)
    #[arg(long, default_value_t = true, num_args = 0..=1, default_missing_value = "true")]
    pub gnome_x11_reload: bool,

    /// \[GNOME Wayland\] Log out of session on Wayland to load extension (destructive: closes apps)
    #[arg(long, default_value_t = false, num_args = 0..=1, default_missing_value = "true")]
    pub gnome_wayland_logout: bool,

    /// \[GNOME\] Do not reload or log out (alias for --gnome-x11-reload=false --gnome-wayland-logout=false)
    #[arg(long)]
    pub no_action: bool,
}

impl GnomeInstallArgs {
    /// Determines whether shell reload or logout should be executed based on session type and flag precedence.
    pub fn should_restart(&self, is_wayland: bool) -> bool {
        if self.no_action {
            false
        } else if is_wayland {
            self.gnome_wayland_logout
        } else {
            self.gnome_x11_reload
        }
    }
}

/// Command-line arguments for the GNOME extension uninstallation step.
#[derive(Args, Debug, Clone, Default)]
pub struct GnomeUninstallArgs {
    /// \[GNOME\] Completely remove extension files from disk
    #[arg(long, default_value_t = true, num_args = 0..=1, default_missing_value = "true")]
    pub gnome_delete_files: bool,

    /// \[GNOME X11\] Reload GNOME Shell in-place on X11 after uninstallation (non-destructive: apps stay open)
    #[arg(long, default_value_t = true, num_args = 0..=1, default_missing_value = "true")]
    pub gnome_x11_reload: bool,

    /// \[GNOME Wayland\] Log out of session on Wayland after uninstallation (destructive: closes apps)
    #[arg(long, default_value_t = false, num_args = 0..=1, default_missing_value = "true")]
    pub gnome_wayland_logout: bool,

    /// \[GNOME\] Do not reload or log out (alias for --gnome-x11-reload=false --gnome-wayland-logout=false)
    #[arg(long)]
    pub no_action: bool,
}

impl GnomeUninstallArgs {
    /// Determines whether shell reload or logout should be executed based on session type and flag precedence.
    pub fn should_restart(&self, is_wayland: bool) -> bool {
        if self.no_action {
            false
        } else if is_wayland {
            self.gnome_wayland_logout
        } else {
            self.gnome_x11_reload
        }
    }
}

// ============================================================================
// 2. Session & Desktop Detection
// ============================================================================

/// Extension UUID recognized by GNOME Shell.
const EXTENSION_UUID: &str = "spawn-at@harsh.local";

/// Bundled modern GNOME Shell extension files (GNOME 45–48).
pub const EMBEDDED_EXTENSION_FILES_MODERN: &[(&str, &str)] = &[
    ("extension.js", include_str!("../../../../assets/gnome/modern/extension.js")),
    ("dbus.js", include_str!("../../../../assets/gnome/modern/dbus.js")),
    ("cloak.js", include_str!("../../../../assets/gnome/modern/cloak.js")),
    ("commit.js", include_str!("../../../../assets/gnome/modern/commit.js")),
    ("pulse.js", include_str!("../../../../assets/gnome/modern/pulse.js")),
    ("anchor.js", include_str!("../../../../assets/gnome/modern/anchor.js")),
    ("logger.js", include_str!("../../../../assets/gnome/modern/logger.js")),
    ("validator.js", include_str!("../../../../assets/gnome/modern/validator.js")),
    ("metadata.json", include_str!("../../../../assets/gnome/modern/metadata.json")),
];

/// Bundled legacy GNOME Shell extension files (GNOME 42–44).
pub const EMBEDDED_EXTENSION_FILES_LEGACY: &[(&str, &str)] = &[
    ("extension.js", include_str!("../../../../assets/gnome/legacy/extension.js")),
    ("dbus.js", include_str!("../../../../assets/gnome/legacy/dbus.js")),
    ("cloak.js", include_str!("../../../../assets/gnome/legacy/cloak.js")),
    ("commit.js", include_str!("../../../../assets/gnome/legacy/commit.js")),
    ("pulse.js", include_str!("../../../../assets/gnome/legacy/pulse.js")),
    ("anchor.js", include_str!("../../../../assets/gnome/legacy/anchor.js")),
    ("logger.js", include_str!("../../../../assets/gnome/legacy/logger.js")),
    ("validator.js", include_str!("../../../../assets/gnome/legacy/validator.js")),
    ("metadata.json", include_str!("../../../../assets/gnome/legacy/metadata.json")),
];

/// Default embedded extension files (modern).
pub const EMBEDDED_EXTENSION_FILES: &[(&str, &str)] = EMBEDDED_EXTENSION_FILES_MODERN;

/// Detects the GNOME Shell major version (e.g. 42, 43, 44, 45, 46, etc.).
pub fn detect_gnome_shell_major_version() -> Option<u32> {
    let output = Command::new("gnome-shell").arg("--version").output().ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_gnome_shell_major_version(&stdout)
}

pub fn parse_gnome_shell_major_version(version_str: &str) -> Option<u32> {
    for part in version_str.split_whitespace() {
        if let Some(first_num) = part.split('.').next() {
            if let Ok(ver) = first_num.parse::<u32>() {
                if ver >= 40 && ver <= 60 {
                    return Some(ver);
                }
            }
        }
    }
    None
}

/// Resolves session identifier to pass into `loginctl show-session <id> -p Type --value`.
fn get_session_id() -> Option<String> {
    if let Ok(id) = std::env::var("XDG_SESSION_ID") {
        let trimmed = id.trim().to_string();
        if !trimmed.is_empty() {
            return Some(trimmed);
        }
    }

    if let Ok(uid_out) = Command::new("id").arg("-u").output() {
        let uid = String::from_utf8_lossy(&uid_out.stdout).trim().to_string();
        if !uid.is_empty() {
            if let Ok(display_out) = Command::new("loginctl")
                .args(["show-user", &uid, "-p", "Display", "--value"])
                .output()
            {
                let sess = String::from_utf8_lossy(&display_out.stdout).trim().to_string();
                if !sess.is_empty() {
                    return Some(sess);
                }
            }
        }
    }

    if let Ok(out) = Command::new("loginctl")
        .args(["list-sessions", "--no-legend"])
        .output()
    {
        let text = String::from_utf8_lossy(&out.stdout);
        for line in text.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if let Some(first) = parts.first() {
                if !first.is_empty() {
                    return Some((*first).to_string());
                }
            }
        }
    }

    None
}

/// Returns true if the active desktop session is running on Wayland.
///
/// Decides X11 vs Wayland with `loginctl show-session <id> -p Type --value`,
/// NOT `$XDG_SESSION_TYPE` (which has been unreliable).
pub fn is_wayland_session() -> bool {
    if let Some(id) = get_session_id() {
        if let Ok(output) = Command::new("loginctl")
            .args(["show-session", &id, "-p", "Type", "--value"])
            .output()
        {
            let sess_type = String::from_utf8_lossy(&output.stdout).trim().to_lowercase();
            if sess_type == "wayland" {
                return true;
            } else if sess_type == "x11" {
                return false;
            }
        }
    }

    if let Ok(output) = Command::new("loginctl")
        .args(["show-session", "auto", "-p", "Type", "--value"])
        .output()
    {
        let sess_type = String::from_utf8_lossy(&output.stdout).trim().to_lowercase();
        if sess_type == "wayland" {
            return true;
        } else if sess_type == "x11" {
            return false;
        }
    }

    std::env::var("WAYLAND_DISPLAY").is_ok()
}

// ============================================================================
// 3. Execution & Reload Helpers
// ============================================================================

/// Restarts GNOME Shell on X11 using synthetic keypresses via xdotool:
/// Alt+F2, ~0.5s wait, type "r", press Return.
///
/// If xdotool is missing, prints a clear message telling the user to install it,
/// and continues normally without failing the installation.
pub fn restart_gnome_shell_x11() {
    let which_res = Command::new("which").arg("xdotool").output();
    let installed = which_res.map(|o| o.status.success()).unwrap_or(false);

    if !installed {
        println!("xdotool is not installed. To reload GNOME Shell automatically on X11, please install xdotool (e.g. 'sudo apt install xdotool').");
        return;
    }

    println!("Reloading GNOME Shell on X11 via key simulation (Alt+F2 -> 'r' -> Enter)...");

    if let Err(e) = Command::new("xdotool").args(["key", "Alt+F2"]).status() {
        crate::diagnostics::render_warning(&format!("Failed to execute 'xdotool key Alt+F2': {}", e));
        return;
    }

    std::thread::sleep(std::time::Duration::from_millis(500));

    if let Err(e) = Command::new("xdotool").args(["type", "r"]).status() {
        crate::diagnostics::render_warning(&format!("Failed to execute 'xdotool type r': {}", e));
        return;
    }

    std::thread::sleep(std::time::Duration::from_millis(100));

    if let Err(e) = Command::new("xdotool").args(["key", "Return"]).status() {
        crate::diagnostics::render_warning(&format!("Failed to execute 'xdotool key Return': {}", e));
    }
}

// ============================================================================
// 4. Driver Construction & Internal State
// ============================================================================

/// Compositor backend driver for GNOME sessions, communicating via the SpawnAt D-Bus extension.
pub struct GnomeWaylandDriver {
    proxy: dbus::SpawnAtProxy<'static>,
}

impl GnomeWaylandDriver {
    /// Constructs a new `GnomeWaylandDriver` by establishing a session bus D-Bus connection.
    pub async fn new() -> Result<Self, DriverError> {
        let connection = zbus::Connection::session()
            .await
            .map_err(|e| DriverError::IpcError(format!("Failed to connect to D-Bus session bus: {}", e)))?;
        let proxy = dbus::SpawnAtProxy::new(&connection)
            .await
            .map_err(|e| DriverError::IpcError(format!("Failed to initialize SpawnAt D-Bus proxy: {}", e)))?;

        Ok(Self { proxy })
    }

    /// Resolves the user extension directory: `~/.local/share/gnome-shell/extensions/spawn-at@harsh.local`
    fn extension_dir() -> Result<PathBuf, DriverError> {
        let home = std::env::var("HOME").map_err(|e| {
            DriverError::Execution(
                format!("Failed to determine $HOME directory for extension deployment: {}", e).into(),
            )
        })?;
        Ok(PathBuf::from(home)
            .join(".local/share/gnome-shell/extensions")
            .join(EXTENSION_UUID))
    }
}

// ============================================================================
// 5. Pre-Map Arming Driver Trait
// ============================================================================

pub const EXPECTED_PROTOCOL_VERSION: u32 = 2;

use futures_util::StreamExt;
use std::time::Duration;
use crate::platform::{ClaimResult, ClaimSubscription};

pub struct GnomeClaimSubscription {
    stream: dbus::SpawnClaimedStream<'static>,
    target_id: String,
}

#[async_trait::async_trait]
impl ClaimSubscription for GnomeClaimSubscription {
    async fn wait_claim(&mut self, timeout: Duration) -> Result<ClaimResult, DriverError> {
        let deadline = tokio::time::Instant::now() + timeout;
        while tokio::time::Instant::now() < deadline {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            let next_signal = tokio::time::timeout(remaining, self.stream.next()).await;
            match next_signal {
                Ok(Some(sig)) => {
                    if let Ok(args) = sig.args() {
                        let sig_target = args.target_id;
                        if self.target_id == "*" || sig_target == self.target_id || sig_target == "*" {
                            if !args.success {
                                return Err(DriverError::Execution(
                                    format!("Compositor placement failed: {}", args.error).into(),
                                ));
                            }
                            return Ok(ClaimResult {
                                target_id: sig_target.to_string(),
                                success: args.success,
                                window_id: args.window_id,
                                x: args.x,
                                y: args.y,
                                w: args.w,
                                h: args.h,
                                size_raised: args.size_raised,
                                error: args.error.to_string(),
                            });
                        }
                    }
                }
                Ok(None) => break,
                Err(_) => break,
            }
        }
        Err(DriverError::TargetNotFound(format!(
            "Timed out waiting for claim signal for target '{}'",
            self.target_id
        )))
    }
}

#[async_trait::async_trait]
impl Driver for GnomeWaylandDriver {
    /// Arms the GNOME Shell extension with declarative batch entries translated to low-level instructions.
    async fn arm(&self, batch: Batch) -> Result<Armed, DriverError> {
        // Check extension protocol version; warn on mismatch without hard-failing
        let ext_version = self.proxy.protocol_version().await.unwrap_or(0);
        if ext_version < EXPECTED_PROTOCOL_VERSION {
            crate::diagnostics::render_warning(
                "Extension protocol mismatch (detected older extension). \
                 Please log out and back in to load the updated extension."
            );
        }

        let mut launch_env = Vec::new();
        let mut armed_token = None;
        let is_solitary = batch.entries.len() == 1;

        for entry in &batch.entries {
            let target_id = if is_solitary {
                if !entry.app_hint.is_empty() {
                    entry.app_hint.as_str()
                } else {
                    "*"
                }
            } else {
                entry.key.as_str()
            };

            let instructions = mechanics::build_instructions_for_entry(entry);
            let instructions_json = serde_json::to_string(&instructions)
                .map_err(|e| DriverError::Execution(format!("Failed to serialize instructions: {}", e).into()))?;

            self.proxy
                .arm_spawn(target_id, &instructions_json)
                .await
                .map_err(|e| {
                    DriverError::IpcError(format!(
                        "Failed to communicate with SpawnAt GNOME extension via D-Bus: {}\n\
                         Reason: The extension does not appear to be running on the session bus.\n\
                         Fix: Run 'spawn-at install' to install and activate the extension.",
                        e
                    ))
                })?;

            launch_env.push(("XDG_ACTIVATION_TOKEN".to_string(), entry.key.clone()));
            launch_env.push(("DESKTOP_STARTUP_ID".to_string(), entry.key.clone()));
            armed_token = Some(target_id.to_string());
        }

        Ok(Armed {
            launch_env,
            token: armed_token,
        })
    }

    /// Disarms a pending target in the GNOME Shell extension (best-effort, idempotent).
    async fn disarm(&self, token: &str) -> Result<bool, DriverError> {
        self.proxy
            .disarm_spawn(token)
            .await
            .map_err(|e| DriverError::IpcError(format!("Failed to disarm spawn on D-Bus: {}", e)))
    }
}

// ============================================================================
// 6. Runtime Compositor Backend Trait
// ============================================================================

#[async_trait::async_trait]
impl CompositorBackend for GnomeWaylandDriver {
    fn name(&self) -> &'static str {
        "GNOME Wayland"
    }

    fn supports_runtime_transform(&self) -> bool {
        true
    }

    fn supports_claim_wait(&self) -> bool {
        true
    }

    async fn prepare_claim_wait(
        &self,
        target_id: &str,
    ) -> Result<Option<Box<dyn ClaimSubscription>>, DriverError> {
        let ext_version = self.proxy.protocol_version().await.unwrap_or(0);
        if ext_version < EXPECTED_PROTOCOL_VERSION {
            crate::diagnostics::render_info(&format!(
                "GNOME Shell extension does not support SpawnClaimed signal (protocol version < {}); falling back to polling.",
                EXPECTED_PROTOCOL_VERSION
            ));
            return Ok(None);
        }

        let stream = self
            .proxy
            .receive_spawn_claimed()
            .await
            .map_err(|e| DriverError::IpcError(format!("Failed to subscribe to SpawnClaimed: {}", e)))?;

        Ok(Some(Box::new(GnomeClaimSubscription {
            stream,
            target_id: target_id.to_string(),
        })))
    }

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

    fn resolve_id(&self, command: &[String], explicit_class: Option<&str>) -> String {
        let bin = command.first().map(|s| s.as_str()).unwrap_or("");
        crate::platform::linux::xdg::resolve_linux_app_id(bin, explicit_class)
    }

    /// Installs the embedded GNOME Shell extension into the user's extensions directory
    /// using dual-mode execution (interactive TUI prompt or headless flag automation).
    fn install(&self, args: &InstallArgs) -> Result<(), DriverError> {
        let is_wayland = is_wayland_session();

        let (auto_enable, should_restart) = if !args.headless && std::io::stdout().is_terminal() {
            println!("\x1b[1;36m=== GNOME Shell Extension Setup ===\x1b[0m\n");

            let auto_enable = Confirm::new()
                .with_prompt("Automatically enable the spawn-at GNOME Shell extension?")
                .default(true)
                .interact()
                .map_err(|e| DriverError::Execution(Box::new(e)))?;

            // Session-tailored TUI prompt:
            // - X11 reload is non-destructive (apps stay open) -> Default: Yes (choice 0)
            // - Wayland logout is destructive (closes all apps) -> Default: No (choice 0)
            let restart_choice = if is_wayland {
                let restart_choices = &[
                    "No - Keep session running (I'll log out later if needed) [Default]",
                    "Yes - Log out now to ensure extension is cleanly loaded (Destructive: closes apps)",
                ];
                let choice = Select::new()
                    .with_prompt("Do you want to log out of your session now? (Destructive on Wayland)")
                    .items(restart_choices)
                    .default(0)
                    .interact()
                    .map_err(|e| DriverError::Execution(Box::new(e)))?;
                choice == 1
            } else {
                let restart_choices = &[
                    "Yes - Reload GNOME Shell in-place now (Non-destructive: apps stay open) [Default]",
                    "No - Keep session running (I'll reload shell manually with Alt+F2 -> 'r' if needed)",
                ];
                let choice = Select::new()
                    .with_prompt("Do you want to reload GNOME Shell now? (Non-destructive on X11)")
                    .items(restart_choices)
                    .default(0)
                    .interact()
                    .map_err(|e| DriverError::Execution(Box::new(e)))?;
                choice == 0
            };

            (auto_enable, restart_choice)
        } else {
            (args.gnome.gnome_auto_enable, args.gnome.should_restart(is_wayland))
        };

        let ext_dir = Self::extension_dir()?;
        let major_version = detect_gnome_shell_major_version();
        let is_legacy = major_version.map_or(false, |v| v < 45);

        let files = if is_legacy {
            crate::diagnostics::render_warning("Support for GNOME Shell 42-44 is experimental and has NOT been tested on a real session yet. It may not work at all. Please report problems at https://github.com/harsh/spawn-at/issues.");
            println!("Deploying legacy GNOME (42-44) extension to: {}", ext_dir.display());
            EMBEDDED_EXTENSION_FILES_LEGACY
        } else {
            println!("Deploying embedded GNOME extension to: {}", ext_dir.display());
            EMBEDDED_EXTENSION_FILES_MODERN
        };

        fs::create_dir_all(&ext_dir).map_err(|e| {
            DriverError::Execution(
                format!("Failed to create extension directory '{}': {}", ext_dir.display(), e).into(),
            )
        })?;

        for (filename, content) in files {
            let file_path = ext_dir.join(filename);
            fs::write(&file_path, content).map_err(|e| {
                DriverError::Execution(format!("Failed to write {}: {}", filename, e).into())
            })?;
        }

        if auto_enable {
            println!("Enabling extension via gnome-extensions CLI...");
            let status = Command::new("gnome-extensions")
                .args(["enable", EXTENSION_UUID])
                .status()
                .map_err(|e| {
                    DriverError::Execution(
                        format!("Failed to execute 'gnome-extensions enable': {}", e).into(),
                    )
                })?;

            if !status.success() {
                println!(
                    "Note: 'gnome-extensions enable' returned a non-zero exit code (the extension might already be enabled or pending reload)."
                );
            }
            println!("Spawn-At GNOME extension successfully installed and enabled.");
        } else {
            println!("Extension files deployed. Skipping automatic enablement (--gnome-auto-enable=false).");
        }

        if should_restart {
            if is_wayland {
                println!("Logging out to complete GNOME Shell extension initialization...");
                let _ = Command::new("gnome-session-quit")
                    .args(["--logout", "--no-prompt"])
                    .spawn();
            } else {
                restart_gnome_shell_x11();
            }
        } else {
            if is_wayland {
                println!("Wayland session logout skipped (--gnome-wayland-logout=false).");
                println!("If the extension is not immediately active, log out and back in to complete loading.");
            } else {
                println!("GNOME Shell reload skipped (--gnome-x11-reload=false).");
                println!("You can reload the shell anytime on X11 by pressing Alt+F2, typing 'r', and pressing Enter.");
            }
        }

        Ok(())
    }

    /// Disables and removes the GNOME Shell extension using dual-mode execution.
    fn uninstall(&self, args: &UninstallArgs) -> Result<(), DriverError> {
        let is_wayland = is_wayland_session();

        let (delete_files, should_restart) = if !args.headless && std::io::stdout().is_terminal() {
            println!("\x1b[1;36m=== GNOME Shell Extension Uninstallation ===\x1b[0m\n");

            let delete_files = Confirm::new()
                .with_prompt("Completely delete extension files from ~/.local/share/gnome-shell/extensions?")
                .default(true)
                .interact()
                .map_err(|e| DriverError::Execution(Box::new(e)))?;

            let restart_choice = if is_wayland {
                let restart_choices = &[
                    "No - Keep session running [Default]",
                    "Yes - Log out now to refresh GNOME Shell state (Destructive: closes apps)",
                ];
                let choice = Select::new()
                    .with_prompt("Do you want to log out of your session now? (Destructive on Wayland)")
                    .items(restart_choices)
                    .default(0)
                    .interact()
                    .map_err(|e| DriverError::Execution(Box::new(e)))?;
                choice == 1
            } else {
                let restart_choices = &[
                    "Yes - Reload GNOME Shell in-place now (Non-destructive: apps stay open) [Default]",
                    "No - Keep session running",
                ];
                let choice = Select::new()
                    .with_prompt("Do you want to reload GNOME Shell now? (Non-destructive on X11)")
                    .items(restart_choices)
                    .default(0)
                    .interact()
                    .map_err(|e| DriverError::Execution(Box::new(e)))?;
                choice == 0
            };

            (delete_files, restart_choice)
        } else {
            (args.gnome.gnome_delete_files, args.gnome.should_restart(is_wayland))
        };

        let ext_dir = Self::extension_dir()?;
        println!("Disabling GNOME extension: {}", EXTENSION_UUID);

        let _ = Command::new("gnome-extensions")
            .args(["disable", EXTENSION_UUID])
            .status();

        if delete_files && ext_dir.exists() {
            println!("Removing extension files from: {}", ext_dir.display());
            fs::remove_dir_all(&ext_dir).map_err(|e| {
                DriverError::Execution(
                    format!("Failed to remove extension directory '{}': {}", ext_dir.display(), e).into(),
                )
            })?;
        }

        println!("Spawn-At extension successfully uninstalled.");

        if should_restart {
            if is_wayland {
                println!("Logging out to complete uninstallation...");
                let _ = Command::new("gnome-session-quit")
                    .args(["--logout", "--no-prompt"])
                    .spawn();
            } else {
                restart_gnome_shell_x11();
            }
        } else {
            if is_wayland {
                println!("Wayland session logout skipped (--gnome-wayland-logout=false).");
            } else {
                println!("GNOME Shell reload skipped (--gnome-x11-reload=false).");
            }
        }

        Ok(())
    }

    async fn transform_window(
        &self,
        target_id: &str,
        params: PlacementParams,
        current_w: u32,
        current_h: u32,
    ) -> Result<(), DriverError> {
        let payload = mechanics::calculate_placement(&params, current_w, current_h);
        let instructions = vec![
            mechanics::Instruction::Snapshot,
            mechanics::Instruction::Cloak,
            mechanics::Instruction::SetSize { w: payload.intended_w, h: payload.intended_h },
            mechanics::Instruction::WaitForCommit { timeout_ms: 500 },
            mechanics::Instruction::SetPositionAnchored(payload),
            mechanics::Instruction::Uncloak,
            mechanics::Instruction::DestroySnapshot,
        ];
        let instructions_json = serde_json::to_string(&instructions)
            .map_err(|e| DriverError::Execution(format!("Serialization error: {}", e).into()))?;

        self.proxy
            .execute_batch(target_id, &instructions_json)
            .await
            .map_err(|e| DriverError::IpcError(e.to_string()))?;

        Ok(())
    }

    /// Queries pointer position from the GNOME extension over D-Bus, with fallback to xdotool.
    async fn get_cursor_position(&self) -> Result<(i32, i32), DriverError> {
        if let Ok(coords) = self.proxy.get_cursor().await {
            return Ok(coords);
        }
        if let Ok(coords) = self.proxy.get_pointer().await {
            return Ok(coords);
        }
        Ok(crate::platform::linux::query_x11_cursor().unwrap_or((0, 0)))
    }

    async fn get_monitors(&self) -> Result<Vec<Rect>, DriverError> {
        match self.get_workareas().await {
            Ok(areas) if !areas.is_empty() => Ok(areas),
            _ => Ok(crate::platform::linux::query_x11_monitors()),
        }
    }

    async fn get_workareas(&self) -> Result<Vec<Rect>, DriverError> {
        let json = self
            .proxy
            .get_workareas()
            .await
            .map_err(|e| DriverError::IpcError(e.to_string()))?;
        serde_json::from_str(&json)
            .map_err(|e| DriverError::IpcError(format!("Failed to parse workareas: {}", e)))
    }

    async fn get_windows(&self) -> Result<Vec<WindowMetadata>, DriverError> {
        let json = self
            .proxy
            .get_windows()
            .await
            .map_err(|e| DriverError::IpcError(e.to_string()))?;
        serde_json::from_str(&json)
            .map_err(|e| DriverError::IpcError(format!("Failed to parse windows: {}", e)))
    }

    async fn move_window(&self, target_id: &str, x: i32, y: i32) -> Result<(), DriverError> {
        self.proxy
            .move_window(target_id, x, y)
            .await
            .map_err(|e| DriverError::IpcError(e.to_string()))
    }

    async fn set_window_state(
        &self,
        target_id: &str,
        state: WindowState,
    ) -> Result<(), DriverError> {
        let state_str = match state {
            WindowState::Maximize => "maximize",
            WindowState::Minimize => "minimize",
            WindowState::Unminimize => "unminimize",
            WindowState::Restore => "restore",
        };
        let success = self
            .proxy
            .set_window_state(target_id, state_str)
            .await
            .map_err(|e| DriverError::IpcError(e.to_string()))?;

        if !success {
            return Err(DriverError::TargetNotFound(format!(
                "Compositor failed to set state '{:?}' on target '{}'",
                state, target_id
            )));
        }
        Ok(())
    }

    async fn focus_window(&self, target_id: &str) -> Result<(), DriverError> {
        let success = self
            .proxy
            .focus_window(target_id)
            .await
            .map_err(|e| DriverError::IpcError(e.to_string()))?;

        if !success {
            return Err(DriverError::TargetNotFound(format!(
                "Compositor failed to focus window target '{}'",
                target_id
            )));
        }
        Ok(())
    }

    async fn defocus_window(
        &self,
        target_id: &str,
        mode: &str,
        destination: &str,
    ) -> Result<(), DriverError> {
        let success = self
            .proxy
            .defocus_window(target_id, mode, destination)
            .await
            .map_err(|e| DriverError::IpcError(e.to_string()))?;

        if !success {
            return Err(DriverError::TargetNotFound(format!(
                "Compositor failed to defocus window target '{}'",
                target_id
            )));
        }
        Ok(())
    }

    async fn close_window(&self, target_id: &str) -> Result<(), DriverError> {
        let version = self.proxy.protocol_version().await.unwrap_or(0);
        if version < 3 {
            return Err(DriverError::UnsupportedCapability(
                "CloseWindow is not supported by the active GNOME Shell extension (ProtocolVersion < 3). Please reload or update the extension.",
            ));
        }

        let success = match self.proxy.close_window(target_id).await {
            Ok(s) => s,
            Err(e) => {
                let err_str = e.to_string();
                if err_str.contains("UnknownMethod") || err_str.contains("MethodNotFound") {
                    return Err(DriverError::UnsupportedCapability(
                        "CloseWindow is not supported by the active GNOME Shell extension. Please reload or update the extension.",
                    ));
                }
                return Err(DriverError::IpcError(err_str));
            }
        };

        if !success {
            return Err(DriverError::TargetNotFound(format!(
                "Compositor failed to close window target '{}' (window not found)",
                target_id
            )));
        }
        Ok(())
    }
}

// ============================================================================
// 7. Unit Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;
    use crate::cli::Cli;

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
        let parsed = Cli::try_parse_from(["spawn-at", "install", "--scope", "user", "--headless"]).unwrap();
        if let crate::cli::Commands::Install(args) = parsed.command {
            assert!(args.gnome.gnome_auto_enable);
            assert!(args.gnome.gnome_x11_reload);
            assert!(!args.gnome.gnome_wayland_logout);
            assert!(!args.gnome.no_action);

            assert!(args.gnome.should_restart(false)); // X11 defaults to reload (true)
            assert!(!args.gnome.should_restart(true)); // Wayland defaults to safe skip (false)
        } else {
            panic!("Expected Install command");
        }
    }

    #[test]
    fn test_gnome_install_args_no_action_precedence() {
        // Passing --no-action should override both X11 and Wayland to false
        let parsed = Cli::try_parse_from([
            "spawn-at", "install", "--scope", "user", "--headless",
            "--gnome-x11-reload=true", "--gnome-wayland-logout=true", "--no-action"
        ]).unwrap();
        if let crate::cli::Commands::Install(args) = parsed.command {
            assert!(args.gnome.no_action);
            assert!(!args.gnome.should_restart(false));
            assert!(!args.gnome.should_restart(true));
        } else {
            panic!("Expected Install command");
        }
    }

    #[test]
    fn test_gnome_install_args_flag_toggles() {
        // Explicitly disable X11 reload
        let parsed = Cli::try_parse_from([
            "spawn-at", "install", "--scope", "user", "--headless", "--gnome-x11-reload=false"
        ]).unwrap();
        if let crate::cli::Commands::Install(args) = parsed.command {
            assert!(!args.gnome.gnome_x11_reload);
            assert!(!args.gnome.should_restart(false));
        } else {
            panic!("Expected Install command");
        }

        // Explicitly enable Wayland logout
        let parsed = Cli::try_parse_from([
            "spawn-at", "install", "--scope", "user", "--headless", "--gnome-wayland-logout=true"
        ]).unwrap();
        if let crate::cli::Commands::Install(args) = parsed.command {
            assert!(args.gnome.gnome_wayland_logout);
            assert!(args.gnome.should_restart(true));
        } else {
            panic!("Expected Install command");
        }
    }

    #[test]
    fn test_gnome_uninstall_args_defaults_and_overrides() {
        // Defaults:
        let parsed = Cli::try_parse_from(["spawn-at", "uninstall", "--scope", "user", "--headless"]).unwrap();
        if let crate::cli::Commands::Uninstall(args) = parsed.command {
            assert!(args.gnome.gnome_delete_files);
            assert!(args.gnome.gnome_x11_reload);
            assert!(!args.gnome.gnome_wayland_logout);
            assert!(!args.gnome.no_action);

            assert!(args.gnome.should_restart(false));
            assert!(!args.gnome.should_restart(true));
        } else {
            panic!("Expected Uninstall command");
        }

        // --no-action override:
        let parsed = Cli::try_parse_from([
            "spawn-at", "uninstall", "--scope", "user", "--headless", "--no-action"
        ]).unwrap();
        if let crate::cli::Commands::Uninstall(args) = parsed.command {
            assert!(args.gnome.no_action);
            assert!(!args.gnome.should_restart(false));
            assert!(!args.gnome.should_restart(true));
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

        let modern_xml = extract_xml(modern_dbus);
        let legacy_xml = extract_xml(legacy_dbus);

        assert_eq!(modern_xml, legacy_xml, "D-Bus interface XML must match between modern and legacy extensions");
        assert!(modern_xml.contains("name=\"SpawnClaimed\""));
        assert!(modern_xml.contains("name=\"ProtocolVersion\""));
    }

    #[test]
    fn test_parse_gnome_shell_major_version() {
        assert_eq!(parse_gnome_shell_major_version("GNOME Shell 46.0"), Some(46));
        assert_eq!(parse_gnome_shell_major_version("GNOME Shell 42.9"), Some(42));
        assert_eq!(parse_gnome_shell_major_version("GNOME Shell 45.beta"), Some(45));
        assert_eq!(parse_gnome_shell_major_version("unknown"), None);
    }
}
