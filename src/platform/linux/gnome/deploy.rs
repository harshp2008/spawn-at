//! Deploying and removing the GNOME Shell extension (`install` / `uninstall`), the embedded
//! extension files, and the session reload helpers.

use super::detect::{detect_gnome_shell_major_version, is_wayland_session};
use super::driver::GnomeWaylandDriver;
use super::logout;
use crate::platform::{DriverError, InstallArgs, UninstallArgs};
use dialoguer::Confirm;
use std::fs;
use std::io::IsTerminal;
use std::path::PathBuf;
use std::process::Command;

/// Extension UUID recognized by GNOME Shell.
const EXTENSION_UUID: &str = "spawn-at@harsh.local";

/// Bundled modern GNOME Shell extension files (GNOME 45–48).
pub const EMBEDDED_EXTENSION_FILES_MODERN: &[(&str, &str)] = &[
    (
        "extension.js",
        include_str!("../../../../assets/gnome/modern/extension.js"),
    ),
    (
        "dbus.js",
        include_str!("../../../../assets/gnome/modern/dbus.js"),
    ),
    (
        "cloak.js",
        include_str!("../../../../assets/gnome/modern/cloak.js"),
    ),
    (
        "commit.js",
        include_str!("../../../../assets/gnome/modern/commit.js"),
    ),
    (
        "pulse.js",
        include_str!("../../../../assets/gnome/modern/pulse.js"),
    ),
    (
        "anchor.js",
        include_str!("../../../../assets/gnome/modern/anchor.js"),
    ),
    (
        "logger.js",
        include_str!("../../../../assets/gnome/modern/logger.js"),
    ),
    (
        "validator.js",
        include_str!("../../../../assets/gnome/modern/validator.js"),
    ),
    // Imported by extension.js: observe-only logging of the paint signals of X11 windows.
    (
        "x11_observe.js",
        include_str!("../../../../assets/gnome/modern/x11_observe.js"),
    ),
    // Not imported by extension.js yet: shipped only because the guard test below requires the
    // list to equal the directory. Inert until the ready-wait machine is wired in.
    (
        "ready_wait.js",
        include_str!("../../../../assets/gnome/modern/ready_wait.js"),
    ),
    (
        "metadata.json",
        include_str!("../../../../assets/gnome/modern/metadata.json"),
    ),
];

/// Bundled legacy GNOME Shell extension files (GNOME 42–44).
pub const EMBEDDED_EXTENSION_FILES_LEGACY: &[(&str, &str)] = &[
    (
        "extension.js",
        include_str!("../../../../assets/gnome/legacy/extension.js"),
    ),
    (
        "dbus.js",
        include_str!("../../../../assets/gnome/legacy/dbus.js"),
    ),
    (
        "cloak.js",
        include_str!("../../../../assets/gnome/legacy/cloak.js"),
    ),
    (
        "commit.js",
        include_str!("../../../../assets/gnome/legacy/commit.js"),
    ),
    (
        "pulse.js",
        include_str!("../../../../assets/gnome/legacy/pulse.js"),
    ),
    (
        "anchor.js",
        include_str!("../../../../assets/gnome/legacy/anchor.js"),
    ),
    (
        "logger.js",
        include_str!("../../../../assets/gnome/legacy/logger.js"),
    ),
    (
        "validator.js",
        include_str!("../../../../assets/gnome/legacy/validator.js"),
    ),
    (
        "metadata.json",
        include_str!("../../../../assets/gnome/legacy/metadata.json"),
    ),
];

// ============================================================================
// 3. Execution & Reload Helpers
// ============================================================================

/// Logs the user out of the Wayland session (`gnome-session-quit --logout --no-prompt`) in a
/// new session group, so it survives this process. The only place that runs it; called by
/// `session::finish()`, which is the last thing a run does.
pub fn log_out_wayland() {
    let mut quit_cmd = Command::new("gnome-session-quit");
    quit_cmd
        .args(["--logout", "--no-prompt"])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        unsafe {
            quit_cmd.pre_exec(|| {
                if libc::setsid() < 0 {
                    libc::setpgid(0, 0);
                }
                Ok(())
            });
        }
    }
    let _ = quit_cmd.spawn();
}

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
        crate::diagnostics::render_warning(&format!(
            "Failed to execute 'xdotool key Alt+F2': {}",
            e
        ));
        return;
    }

    std::thread::sleep(std::time::Duration::from_millis(500));

    if let Err(e) = Command::new("xdotool").args(["type", "r"]).status() {
        crate::diagnostics::render_warning(&format!("Failed to execute 'xdotool type r': {}", e));
        return;
    }

    std::thread::sleep(std::time::Duration::from_millis(100));

    if let Err(e) = Command::new("xdotool").args(["key", "Return"]).status() {
        crate::diagnostics::render_warning(&format!(
            "Failed to execute 'xdotool key Return': {}",
            e
        ));
    }
}

impl GnomeWaylandDriver {
    /// Resolves the user extension directory: `~/.local/share/gnome-shell/extensions/spawn-at@harsh.local`
    fn extension_dir() -> Result<PathBuf, DriverError> {
        let home = std::env::var("HOME").map_err(|e| {
            DriverError::Execution(
                format!(
                    "Failed to determine $HOME directory for extension deployment: {}",
                    e
                )
                .into(),
            )
        })?;
        Ok(PathBuf::from(home)
            .join(".local/share/gnome-shell/extensions")
            .join(EXTENSION_UUID))
    }

    /// Installs the embedded GNOME Shell extension into the user's extensions directory
    /// using dual-mode execution (interactive TUI prompt or headless flag automation).
    pub(super) fn install_impl(&self, args: &InstallArgs) -> Result<(), DriverError> {
        let is_wayland = is_wayland_session();

        let ext_dir = Self::extension_dir()?;
        let major_version = detect_gnome_shell_major_version();
        let is_legacy = major_version.is_some_and(|v| v < 45);
        let files = if is_legacy {
            EMBEDDED_EXTENSION_FILES_LEGACY
        } else {
            EMBEDDED_EXTENSION_FILES_MODERN
        };

        // Read before anything is written. Only on Wayland (X11 reloads the Shell in place);
        // any fact that cannot be read keeps the log-out advice (see logout.rs).
        let logout_not_needed = is_wayland
            && logout::decide(
                &logout::RealFacts {
                    ext_dir: ext_dir.clone(),
                    uuid: EXTENSION_UUID,
                },
                files,
            ) == logout::LogoutNeed::NotNeeded;

        let interactive = !args.headless && std::io::stdout().is_terminal();
        let auto_enable = if interactive {
            crate::ui::section("GNOME Shell Extension Setup");
            Confirm::new()
                .with_prompt(crate::prompts::PROMPT_ENABLE_EXTENSION)
                .default(true)
                .interact()
                .map_err(|e| DriverError::Execution(Box::new(e)))?
        } else {
            args.gnome.gnome_auto_enable
        };
        // The log-out / reload question is NOT asked here: it is the last prompt of the run,
        // asked by session::finish() (see session.rs).
        let session_decision = crate::session::decision(
            interactive,
            is_wayland,
            args.gnome.no_action,
            args.gnome.gnome_wayland_logout,
            args.gnome.should_restart(is_wayland),
        );

        crate::ui::block_start();
        if is_legacy {
            // The untested-platform notice was printed (and confirmed) before the install began.
            println!(
                "Deploying legacy GNOME (42-44) extension to: {}",
                ext_dir.display()
            );
        } else {
            println!(
                "Deploying embedded GNOME extension to: {}",
                ext_dir.display()
            );
        }

        fs::create_dir_all(&ext_dir).map_err(|e| {
            DriverError::Execution(
                format!(
                    "Failed to create extension directory '{}': {}",
                    ext_dir.display(),
                    e
                )
                .into(),
            )
        })?;

        for (filename, content) in files {
            let file_path = ext_dir.join(filename);
            // An identical file is left alone, so its modification time keeps saying when the
            // running Shell could have loaded it (logout.rs compares it with the Shell's start).
            if fs::read(&file_path).is_ok_and(|old| old == content.as_bytes()) {
                continue;
            }
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

        // Nothing is asked, logged out or reloaded here: the facts are recorded and
        // session::finish(), the last statement of the run, asks and acts (see session.rs).
        crate::session::record(crate::session::Pending {
            flow: crate::session::Flow::Install {
                not_needed: logout_not_needed,
                headless: args.headless,
            },
            is_wayland,
            decision: session_decision,
        });

        Ok(())
    }

    /// Disables and removes the GNOME Shell extension using dual-mode execution.
    pub(super) fn uninstall_impl(&self, args: &UninstallArgs) -> Result<(), DriverError> {
        let is_wayland = is_wayland_session();

        let interactive = !args.headless && std::io::stdout().is_terminal();
        let delete_files = if interactive {
            crate::ui::section("GNOME Shell Extension Uninstallation");
            println!("{}", crate::prompts::NOTE_DELETE_EXTENSION_FILES);
            Confirm::new()
                .with_prompt(crate::prompts::PROMPT_DELETE_EXTENSION_FILES)
                .default(true)
                .interact()
                .map_err(|e| DriverError::Execution(Box::new(e)))?
        } else {
            args.gnome.gnome_delete_files
        };
        // The log-out / reload question is asked last, by session::finish().
        let session_decision = crate::session::decision(
            interactive,
            is_wayland,
            args.gnome.no_action,
            args.gnome.gnome_wayland_logout,
            args.gnome.should_restart(is_wayland),
        );

        let ext_dir = Self::extension_dir()?;
        crate::ui::block_start();
        println!("Disabling GNOME extension: {}", EXTENSION_UUID);

        let _ = Command::new("gnome-extensions")
            .args(["disable", EXTENSION_UUID])
            .status();

        if delete_files && ext_dir.exists() {
            println!("Removing extension files from: {}", ext_dir.display());
            fs::remove_dir_all(&ext_dir).map_err(|e| {
                DriverError::Execution(
                    format!(
                        "Failed to remove extension directory '{}': {}",
                        ext_dir.display(),
                        e
                    )
                    .into(),
                )
            })?;
        }

        println!("Spawn-At extension successfully uninstalled.");

        // Recorded, not asked or performed: session::finish() ends the run (see session.rs).
        // The removal phase of a clean install passes no_action, so it records nothing at all.
        if let Some(p) =
            crate::session::uninstall_pending(is_wayland, session_decision, args.gnome.no_action)
        {
            crate::session::record(p);
        }

        Ok(())
    }
}
