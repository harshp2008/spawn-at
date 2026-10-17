//! Command-line flags of the GNOME install and uninstall steps.
//!
//! The two structs are flattened into the install and uninstall arguments of the CLI. They hold
//! only parsed flag values plus [`GnomeInstallArgs::should_restart`] and
//! [`GnomeUninstallArgs::should_restart`], the pure rule that turns the flags and the session
//! type into one yes/no. They do no I/O and do not detect the session themselves: the caller
//! passes `is_wayland`.
//!
//! The `///` text on the fields is the `--help` text of clap, so it is not a place for notes.

use clap::Args;

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
    /// Whether the flags ask for a log-out (Wayland) or an in-place Shell reload (X11) after
    /// the install. `no_action` wins over everything; otherwise `gnome_wayland_logout` is read
    /// on Wayland and `gnome_x11_reload` on X11. The other session type's flag is ignored.
    ///
    /// Pure: no I/O, no prompt.
    // Callers: the install and uninstall in deploy.rs pass the result to `session::decision`,
    // which may still ask the user on an interactive run (grep -n "should_restart" src/).
    // `gnome_auto_enable` and `gnome_delete_files` are read only on a non-interactive run
    // (grep -n "gnome_auto_enable" src/platform/linux/gnome/deploy.rs).
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
    /// Same rule as [`GnomeInstallArgs::should_restart`], applied to the uninstall flags:
    /// `no_action` wins, then `gnome_wayland_logout` on Wayland or `gnome_x11_reload` on X11.
    /// Pure: no I/O, no prompt.
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
