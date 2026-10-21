//! Deploying and removing the GNOME Shell extension (`install` / `uninstall`), the embedded
//! extension files, and the session reload helpers.
//!
//! Contents: the two embedded file lists (modern and legacy, compiled in with `include_str!`),
//! `log_out_wayland` and `restart_gnome_shell_x11`, the [`Provisioner`] implementation of
//! [`GnomeWaylandDriver`] (`install`, `uninstall`, `install_notice`), and the GNOME session and
//! update hooks.
//!
//! It acts on the live session: it writes under `$HOME/.local/share/gnome-shell/extensions/`,
//! runs `gnome-extensions`, and can log the user out. The advice on whether a log-out is needed
//! comes from `logout.rs`, and the question to the user about logging out is asked by
//! `session::finish`, not here.

use super::detect::{detect_gnome_shell_major_version, is_wayland_session};
use super::driver::GnomeWaylandDriver;
use super::logout;
use super::needs_logout;
use crate::platform::provisioner::Provisioner;
use crate::platform::{DriverError, InstallArgs, UninstallArgs};
use dialoguer::Confirm;
use std::fs;
use std::io::IsTerminal;
use std::path::PathBuf;
use std::process::Command;

/// The extension's UUID: the name of its directory under the user's extensions folder and the
/// argument of `gnome-extensions enable` / `disable`. It equals `uuid` in both `metadata.json`
/// files.
pub(super) const EXTENSION_UUID: &str = "spawn-at@harsh.local";

/// The files of `assets/gnome/modern/`, as (file name, contents), written by `install` when the
/// detected Shell is 45 or newer or unknown. The `metadata.json` of this set lists Shell
/// versions 45 to 50.
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
    // Not imported by extension.js itself; commit.js and validator.js import it
    // (grep -n "ready_wait" assets/gnome/modern/*.js).
    (
        "ready_wait.js",
        include_str!("../../../../assets/gnome/modern/ready_wait.js"),
    ),
    (
        "metadata.json",
        include_str!("../../../../assets/gnome/modern/metadata.json"),
    ),
];

/// The files of `assets/gnome/legacy/`, written by `install` when the detected Shell major
/// version is below 45. Its `metadata.json` lists Shell versions 42 to 44. This set has no
/// `ready_wait.js` and no `x11_observe.js`.
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

/// Starts `gnome-session-quit --logout --no-prompt` in a new session (`setsid`, falling back to
/// `setpgid(0, 0)`) so it outlives this process, with stdin, stdout and stderr redirected to `/dev/null`.
/// It does not wait for the command. A failure to start it is ignored (`let _ =`), so the user
/// gets no message if `gnome-session-quit` is missing. Side effect: ends the user's session.
// The only place that runs it (grep -rn gnome-session-quit src/); `GnomeSessionActions::log_out`
// below calls it and `session::finish` calls that through the SessionActions trait.
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

/// Reloads GNOME Shell on X11 by sending keys with `xdotool`: `Alt+F2`, 500 ms, type `r`,
/// 100 ms, `Return`. It needs a running X11 session; there is no check for that here.
///
/// If `which xdotool` fails it prints an install hint and returns. If an `xdotool` command
/// cannot be started it prints a warning and stops; a command that starts and exits non-zero
/// is not noticed, so the sequence goes on. Nothing is returned, so the caller cannot tell
/// whether the Shell reloaded.
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

/// `Some(how it ended)` when a `gnome-extensions` call that was started did not exit with 0:
/// `exited with code N`, or `was ended by a signal` when there is no code.
fn exit_failure(status: &std::process::ExitStatus) -> Option<String> {
    exit_failure_from(status.success(), status.code())
}

/// The pure part of [`exit_failure`].
fn exit_failure_from(success: bool, code: Option<i32>) -> Option<String> {
    if success {
        return None;
    }
    Some(match code {
        Some(c) => format!("exited with code {c}"),
        None => "was ended by a signal".to_string(),
    })
}

/// The plain note (not a warning) printed by `install` instead of the success line when
/// `gnome-extensions enable` did not exit with 0. The files are on disk, but the extension is
/// not confirmed enabled; a shell that has not loaded the new files yet may only enable it
/// after the next login.
fn enable_note_text(why: &str) -> String {
    format!(
        "Note: 'gnome-extensions enable' {why}. The extension files were deployed, but the extension is not confirmed enabled. If GNOME Shell has not loaded the new files yet, it may only be enabled after you log out and back in; 'gnome-extensions list --enabled' shows the state."
    )
}

/// The warning printed by `uninstall` instead of the success line when `gnome-extensions
/// disable` could not be run or did not exit with 0. `files_removed` says whether the extension
/// directory was deleted by this run.
fn disable_warning_text(why: &str, files_removed: bool) -> String {
    format!(
        "'gnome-extensions disable' {why}. The extension may still be enabled. {}",
        if files_removed {
            "Its files were removed."
        } else {
            "Its files were not removed."
        }
    )
}

impl GnomeWaylandDriver {
    /// `$HOME/.local/share/gnome-shell/extensions/spawn-at@harsh.local`. Reads `$HOME` only
    /// (not the XDG data dir), so with `HOME` unset it fails with [`DriverError::Execution`].
    pub(super) fn extension_dir() -> Result<PathBuf, DriverError> {
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
}

impl Provisioner for GnomeWaylandDriver {
    /// The notice for the detected GNOME Shell version and session type, from
    /// `install_notice::gnome_notice`; `None` for an environment in `install_notice::VERIFIED`
    /// with full coverage (today Shell 46 on Wayland). Runs `gnome-shell
    /// --version` and `loginctl` (see `detect.rs`).
    fn install_notice(&self) -> Option<String> {
        crate::platform::install_notice::gnome_notice(
            detect_gnome_shell_major_version(),
            is_wayland_session(),
        )
    }

    /// Writes the embedded extension files into the extension directory and, if wanted, runs
    /// `gnome-extensions enable`.
    ///
    /// The file set is the legacy one when the Shell major version is below 45, else the modern
    /// one (also when the version cannot be read). Interactive means `!args.headless` and stdout
    /// is a terminal; then the user is asked whether to enable, otherwise
    /// `gnome_auto_enable` decides. A file whose bytes already match is not rewritten, to keep
    /// its modification time (`logout.rs` compares it with the Shell's start). Files in the
    /// directory that are not in the list, for example from the other set, are left in place.
    ///
    /// Does not log out or reload: it records a `session::Pending` for `session::finish`.
    ///
    /// Errors: [`DriverError::Execution`] when `$HOME` is unset, the directory cannot be
    /// created, a file cannot be written, the prompt fails, or `gnome-extensions` cannot be
    /// started. A non-zero exit of `gnome-extensions enable` is not an error: a note says the
    /// files were deployed but the extension is not confirmed enabled, the "successfully
    /// installed and enabled" line is not printed, and the run goes on and returns `Ok`.
    fn install(&self, args: &InstallArgs) -> Result<(), DriverError> {
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
            args.backend.gnome.gnome_auto_enable
        };
        // The log-out / reload question is NOT asked here: it is the last prompt of the run,
        // asked by session::finish() (see session.rs).
        let session_decision = crate::session::decision(
            interactive,
            is_wayland,
            args.backend.gnome.no_action,
            args.backend.gnome.gnome_wayland_logout,
            args.backend.gnome.should_restart(is_wayland),
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

            if let Some(why) = exit_failure(&status) {
                println!("{}", enable_note_text(&why));
            } else {
                println!("Spawn-At GNOME extension successfully installed and enabled.");
            }
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

    /// Runs `gnome-extensions disable` and, if wanted, deletes the extension directory.
    ///
    /// A missing `gnome-extensions` or a non-zero exit of `disable` does not stop the run: it
    /// goes on to delete the files and returns `Ok`, but a warning replaces the "successfully
    /// uninstalled" line and says whether the files were removed.
    /// Interactive runs ask whether to delete the files; otherwise `gnome_delete_files` decides.
    /// Like `install`, it records a `session::Pending` instead of logging out. The removal phase
    /// of a clean install passes `no_action` and records nothing.
    ///
    /// Errors: [`DriverError::Execution`] when `$HOME` is unset, the prompt fails, or the
    /// directory cannot be removed.
    fn uninstall(&self, args: &UninstallArgs) -> Result<(), DriverError> {
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
            args.backend.gnome.gnome_delete_files
        };
        // The log-out / reload question is asked last, by session::finish().
        let session_decision = crate::session::decision(
            interactive,
            is_wayland,
            args.backend.gnome.no_action,
            args.backend.gnome.gnome_wayland_logout,
            args.backend.gnome.should_restart(is_wayland),
        );

        let ext_dir = Self::extension_dir()?;
        crate::ui::block_start();
        println!("Disabling GNOME extension: {}", EXTENSION_UUID);

        let disable_failure = match Command::new("gnome-extensions")
            .args(["disable", EXTENSION_UUID])
            .status()
        {
            Ok(status) => exit_failure(&status),
            Err(e) => Some(format!("could not be run: {e}")),
        };

        let removing = delete_files && ext_dir.exists();
        if removing {
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

        match disable_failure {
            Some(why) => {
                crate::diagnostics::render_warning(&disable_warning_text(&why, removing));
            }
            None => println!("Spawn-At extension successfully uninstalled."),
        }

        // Recorded, not asked or performed: session::finish() ends the run (see session.rs).
        // The removal phase of a clean install passes no_action, so it records nothing at all.
        if let Some(p) = crate::session::uninstall_pending(
            is_wayland,
            session_decision,
            args.backend.gnome.no_action,
        ) {
            crate::session::record(p);
        }

        Ok(())
    }
}

/// The GNOME implementation of [`SessionActions`](crate::platform::session_actions::SessionActions):
/// `gnome-session-quit` on Wayland, an Alt+F2 `r` reload on X11.
pub struct GnomeSessionActions;

impl crate::platform::session_actions::SessionActions for GnomeSessionActions {
    fn log_out(&self) {
        log_out_wayland()
    }

    fn reload_shell(&self) {
        restart_gnome_shell_x11()
    }
}

/// The GNOME implementation of [`UpdateHooks`](crate::platform::update_hooks::UpdateHooks): active
/// when `XDG_CURRENT_DESKTOP` contains GNOME.
pub struct GnomeUpdateHooks;

impl GnomeUpdateHooks {
    /// True when `$XDG_CURRENT_DESKTOP`, upper-cased, contains "GNOME" (so "ubuntu:GNOME" and
    /// "GNOME-Classic" count). Unset counts as false.
    fn on_gnome() -> bool {
        std::env::var("XDG_CURRENT_DESKTOP")
            .unwrap_or_default()
            .to_uppercase()
            .contains("GNOME")
    }
}

impl crate::platform::update_hooks::UpdateHooks for GnomeUpdateHooks {
    /// On GNOME, runs the NEW binary as `install --skip-bin --headless` and waits for it, so
    /// the extension files come from the new version.
    ///
    /// Only a failure to start that child is reported (as a warning). Its exit status is not
    /// looked at, so an install that fails is silent here.
    ///
    /// The deployed extension directory is read before and after the child runs
    /// (`needs_logout::classify`): `Some` only when files that were already there now differ.
    /// `None` off GNOME, for a first install, for identical files, and when `$HOME` is unset or a
    /// read failed.
    fn after_unpack(
        &self,
        new_binary: &std::path::Path,
    ) -> Option<crate::platform::update_hooks::ChangedExtension> {
        if !Self::on_gnome() {
            return None;
        }
        let facts = GnomeWaylandDriver::extension_dir()
            .ok()
            .map(|ext_dir| logout::RealFacts {
                ext_dir,
                uuid: EXTENSION_UUID,
            });
        let read = |f: &Option<logout::RealFacts>| match f {
            Some(f) => logout::ExtensionFacts::deployed_files(f),
            None => Err("$HOME is not set".to_string()),
        };
        let before = read(&facts);
        println!("Deploying GNOME Shell extension via newly unpacked binary...");
        let ext_status = Command::new(new_binary)
            .args(["install", "--skip-bin", "--headless"])
            .status();
        if let Err(e) = ext_status {
            crate::diagnostics::render_warning(&format!(
                "Failed to install GNOME extension from new binary: {}",
                e
            ));
        }
        needs_logout::classify(before, read(&facts))
    }

    /// Prints [`needs_logout::NOTICE_TEXT`] and, on Wayland, sends the notification and writes the
    /// marker (`needs_logout::announce`). A notification or marker that fails is a warning.
    fn announce_logout_needed(&self, changed: &crate::platform::update_hooks::ChangedExtension) {
        let marker = needs_logout::marker_path();
        let result = needs_logout::announce(
            changed,
            is_wayland_session(),
            marker.as_deref(),
            &mut needs_logout::send_notification,
        );
        match result.announced {
            needs_logout::Announced::Sent => println!("{}", needs_logout::NOTICE_TEXT),
            needs_logout::Announced::SendFailed(why) => {
                crate::diagnostics::render_warning(&format!(
                    "Could not send the desktop notification ({why}). {}",
                    needs_logout::NOTICE_TEXT
                ))
            }
            needs_logout::Announced::NotWayland | needs_logout::Announced::AlreadyFlagged => {}
        }
        if let Some(why) = result.marker_error {
            crate::diagnostics::render_warning(&format!(
                "Could not record the pending log-out ({why}); `spawn` will not remind you."
            ));
        }
    }

    fn offers_session_action(&self) -> bool {
        Self::on_gnome()
    }

    fn session_is_wayland(&self) -> bool {
        is_wayland_session()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Exit 0 is no failure; a code and a signal are told apart. The last two are what
    /// `gnome-extensions` returns when it exits 2 and when it is killed.
    #[test]
    fn test_exit_failure_text() {
        assert_eq!(exit_failure_from(true, Some(0)), None);
        assert_eq!(
            exit_failure_from(false, Some(2)).as_deref(),
            Some("exited with code 2")
        );
        assert_eq!(
            exit_failure_from(false, None).as_deref(),
            Some("was ended by a signal")
        );
    }

    /// After a failed `enable` the note says the files are there but the extension is not
    /// confirmed enabled, never contains the success wording the old code printed anyway, and
    /// is a plain "Note:" line (install prints it with `println!`, not `render_warning`).
    #[test]
    fn test_enable_note_does_not_claim_success() {
        let text = enable_note_text("exited with code 1");
        assert!(text.starts_with("Note: 'gnome-extensions enable' exited with code 1."));
        assert!(text.contains("files were deployed"));
        assert!(text.contains("not confirmed enabled"));
        assert!(text.contains("log out and back in"));
        assert!(!text.contains("successfully"), "{text}");
        assert!(!text.to_lowercase().contains("warning"), "{text}");
    }

    /// After a failed `disable` the warning says the extension may still be enabled and tells
    /// whether the files went; it never says "uninstalled".
    #[test]
    fn test_disable_warning_says_what_happened() {
        let removed = disable_warning_text("could not be run: No such file or directory", true);
        assert!(removed.contains("may still be enabled"));
        assert!(removed.ends_with("Its files were removed."), "{removed}");
        let kept = disable_warning_text("exited with code 1", false);
        assert!(kept.ends_with("Its files were not removed."), "{kept}");
        for t in [&removed, &kept] {
            assert!(
                !t.contains("uninstalled") && !t.contains("successfully"),
                "{t}"
            );
        }
    }
}
