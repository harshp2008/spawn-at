//! Command-line flags of the GNOME install and uninstall steps.

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
