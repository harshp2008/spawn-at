//! # GNOME Wayland Compositor Driver
//!
//! ## Deep Dive: The Mechanics of Wayland Window Placement on GNOME
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
//! the sequence of events must be inverted:
//!
//! - **Conventional (Faulty) Flow:**
//!   1. Spawn process -> 2. Wait for window to map -> 3. Reposition window.
//!   Result: Window flickers at default location for 1-5 frames before jumping.
//!
//! - **Spawn-At (Zero-Flicker) Flow:**
//!   1. **D-Bus Arm:** Call `Arm(app_id, x, y, w, h)` on the GNOME Shell extension.
//!      The compositor registers an expected window target in memory *before* the
//!      application process is even spawned.
//!   2. **Process Spawn:** The child process is launched. It connects to the Wayland
//!      display socket and submits its `xdg_surface`.
//!   3. **Synchronous Interception:** Mutter triggers `window-created`. The extension
//!      matches the window's `wm_class` / `app_id` (or wildcard `*`) with the armed target.
//!   4. **Opacity Cloaking:** As soon as Mutter creates the `ClutterActor` during `map`,
//!      the extension sets `actor.opacity = 0` synchronously. Mutter renders nothing to the
//!      screen while the initial frame geometry settles.
//!   5. **Atomic Reveal:** Once `window.get_frame_rect()` matches the target coordinates,
//!      `actor.opacity = 255` is restored.
//!
//! ### 3. Session Restart Semantics on Wayland
//! Under X11, GNOME Shell could be restarted in-place without dropping running apps by
//! typing `r` in the `Alt+F2` prompt. Under Wayland, the compositor IS the display server;
//! terminating or restarting the GNOME Shell process tears down the Wayland socket and
//! destroys all client connections. Therefore, when loading shell extensions in older
//! environments, logging out via `gnome-session-quit` is the standard clean reload method.

use super::{DriverError, InstallArgs, TargetGeometry, UninstallArgs, WindowManager};
use clap::Args;
use dialoguer::{Confirm, Select};
use std::fs;
use std::io::IsTerminal;
use std::path::PathBuf;
use std::process::Command;

#[derive(Args, Debug, Clone, Default)]
pub struct GnomeInstallArgs {
    /// [GNOME] Automatically enable the GNOME Shell extension
    #[arg(long, default_value_t = true, num_args = 0..=1, default_missing_value = "true")]
    pub gnome_auto_enable: bool,

    /// [GNOME] Automatically log out / restart session to load extension
    #[arg(long, default_value_t = false, num_args = 0..=1, default_missing_value = "true")]
    pub gnome_restart: bool,
}

#[derive(Args, Debug, Clone, Default)]
pub struct GnomeUninstallArgs {
    /// [GNOME] Completely remove extension files from disk
    #[arg(long, default_value_t = true, num_args = 0..=1, default_missing_value = "true")]
    pub gnome_delete_files: bool,

    /// [GNOME] Automatically log out / restart session after uninstallation
    #[arg(long, default_value_t = false, num_args = 0..=1, default_missing_value = "true")]
    pub gnome_restart: bool,
}

const EXTENSION_UUID: &str = "spawn-at@harsh.local";
const EXTENSION_JS: &str = include_str!("../../assets/gnome/extension.esm.js");
const METADATA_JSON: &str = include_str!("../../assets/gnome/metadata.json");

pub struct GnomeWaylandDriver;

impl GnomeWaylandDriver {
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

impl WindowManager for GnomeWaylandDriver {
    fn name(&self) -> &'static str {
        "GNOME Wayland"
    }

    fn resolve_id(&self, command: &[String], explicit_class: Option<&str>) -> String {
        let bin = command.first().map(|s| s.as_str()).unwrap_or("");
        crate::platform::linux::xdg::resolve_linux_app_id(bin, explicit_class)
    }

    /// Installs the embedded GNOME Shell extension into the user's extensions directory
    /// using dual-mode execution (interactive TUI prompt or headless flag automation).
    fn install(&self, args: &InstallArgs) -> Result<(), DriverError> {
        // Resolve configuration: interactive TUI vs headless
        let (auto_enable, restart) = if !args.headless && std::io::stdout().is_terminal() {
            println!("\x1b[1;36m=== GNOME Shell Extension Setup ===\x1b[0m\n");

            let auto_enable = Confirm::new()
                .with_prompt("Automatically enable the spawn-at GNOME Shell extension?")
                .default(true)
                .interact()
                .map_err(|e| DriverError::Execution(Box::new(e)))?;

            let restart_choices = &[
                "No - Keep session running (I'll restart/log out later if needed)",
                "Yes - Log out now to ensure extension is cleanly loaded",
            ];
            let restart_choice = Select::new()
                .with_prompt("Do you want to log out / restart your session now?")
                .items(restart_choices)
                .default(0)
                .interact()
                .map_err(|e| DriverError::Execution(Box::new(e)))?;

            (auto_enable, restart_choice == 1)
        } else {
            (args.gnome.gnome_auto_enable, args.gnome.gnome_restart)
        };

        let ext_dir = Self::extension_dir()?;
        println!("Deploying embedded GNOME extension to: {}", ext_dir.display());

        fs::create_dir_all(&ext_dir).map_err(|e| {
            DriverError::Execution(
                format!("Failed to create extension directory '{}': {}", ext_dir.display(), e).into(),
            )
        })?;

        let js_path = ext_dir.join("extension.js");
        let meta_path = ext_dir.join("metadata.json");

        fs::write(&js_path, EXTENSION_JS).map_err(|e| {
            DriverError::Execution(format!("Failed to write extension.js: {}", e).into())
        })?;

        fs::write(&meta_path, METADATA_JSON).map_err(|e| {
            DriverError::Execution(format!("Failed to write metadata.json: {}", e).into())
        })?;

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

        if restart {
            println!("Logging out to complete GNOME Shell extension initialization...");
            let _ = Command::new("gnome-session-quit")
                .args(["--logout", "--no-prompt"])
                .spawn();
        } else {
            println!("Session restart skipped.");
            println!("If the extension is not immediately active, log out and back in to complete loading.");
        }

        Ok(())
    }

    /// Disables and removes the GNOME Shell extension using dual-mode execution.
    fn uninstall(&self, args: &UninstallArgs) -> Result<(), DriverError> {
        let (delete_files, restart) = if !args.headless && std::io::stdout().is_terminal() {
            println!("\x1b[1;36m=== GNOME Shell Extension Uninstallation ===\x1b[0m\n");

            let delete_files = Confirm::new()
                .with_prompt("Completely delete extension files from ~/.local/share/gnome-shell/extensions?")
                .default(true)
                .interact()
                .map_err(|e| DriverError::Execution(Box::new(e)))?;

            let restart_choices = &[
                "No - Keep session running",
                "Yes - Log out now to refresh GNOME Shell state",
            ];
            let restart_choice = Select::new()
                .with_prompt("Do you want to log out / restart your session now?")
                .items(restart_choices)
                .default(0)
                .interact()
                .map_err(|e| DriverError::Execution(Box::new(e)))?;

            (delete_files, restart_choice == 1)
        } else {
            (args.gnome.gnome_delete_files, args.gnome.gnome_restart)
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

        if restart {
            println!("Logging out to complete uninstallation...");
            let _ = Command::new("gnome-session-quit")
                .args(["--logout", "--no-prompt"])
                .spawn();
        }

        Ok(())
    }

    /// Primes the GNOME Shell extension via D-Bus and launches the command.
    ///
    /// The D-Bus call to `Arm` informs Mutter of the target coordinates and application ID
    /// prior to child process execution, ensuring that the window is intercepted and cloaked
    /// at the moment of creation.
    fn spawn_at(
        &self,
        app_id: &str,
        command: &[String],
        geom: &TargetGeometry,
    ) -> Result<(), DriverError> {
        if command.is_empty() {
            return Err(DriverError::Execution(
                "Cannot spawn application: command vector is empty".into(),
            ));
        }

        // 1. Connect to user D-Bus session bus
        let connection = zbus::blocking::Connection::session().map_err(|e| {
            DriverError::Execution(
                format!("Failed to connect to D-Bus session bus: {}", e).into(),
            )
        })?;

        // 2. Call Arm(identifier: s, x: i, y: i, w: u, h: u)
        let arm_result: Result<zbus::message::Message, zbus::Error> = connection.call_method(
            Some("org.gnome.Shell"),
            "/org/gnome/Shell/Extensions/SpawnAt",
            Some("org.gnome.Shell.Extensions.SpawnAt"),
            "Arm",
            &(app_id, geom.x, geom.y, geom.w as i32, geom.h as i32),
        );

        if let Err(e) = arm_result {
            return Err(DriverError::Execution(
                format!(
                    "Failed to communicate with SpawnAt GNOME extension via D-Bus: {}\n\
                     Reason: The extension does not appear to be running on the session bus.\n\
                     Fix: Run 'spawn-at install' to install and activate the extension.",
                    e
                )
                .into(),
            ));
        }

        // 3. Launch the requested process
        Command::new(&command[0])
            .args(&command[1..])
            .spawn()
            .map_err(|e| {
                DriverError::Execution(
                    format!("Failed to spawn command '{}': {}", command[0], e).into(),
                )
            })?;

        Ok(())
    }

    /// Queries pointer position from the GNOME extension over D-Bus, with fallback to xdotool.
    fn get_cursor_position(&self) -> Option<(i32, i32)> {
        if let Ok(conn) = zbus::blocking::Connection::session() {
            // 1. Try querying SpawnAt GNOME extension via D-Bus
            if let Ok(reply) = conn.call_method(
                Some("org.gnome.Shell"),
                "/org/gnome/Shell/Extensions/SpawnAt",
                Some("org.gnome.Shell.Extensions.SpawnAt"),
                "GetCursor",
                &(),
            ) {
                if let Ok(coords) = reply.body().deserialize::<(i32, i32)>() {
                    return Some(coords);
                }
            }

            // 2. Try MousePos extension if present
            if let Ok(reply) = conn.call_method(
                Some("org.gnome.Shell"),
                "/org/gnome/Shell/Extensions/MousePos",
                Some("org.gnome.Shell.Extensions.MousePos"),
                "GetCoordinates",
                &(),
            ) {
                if let Ok(coords) = reply.body().deserialize::<(i32, i32)>() {
                    return Some(coords);
                }
            }
        }

        // Fallback for XWayland
        crate::platform::linux::query_xdotool_cursor()
    }

    /// Queries active display monitor geometries via xrandr.
    fn get_monitors(&self) -> Vec<crate::geometry::Rect> {
        crate::platform::linux::query_xrandr_monitors()
    }
}
