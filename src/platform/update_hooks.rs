//! What `spawn-at update` needs to know about the desktop: whether a helper has to be deployed
//! from the new binary, and whether the run should end with the log-out / reload question.
//! Supplied by the platform so `update.rs` reads no desktop environment variable.
//!
//! None of the methods returns an error: a failing step is reported by the implementation on
//! its own output and the upgrade goes on.
// Providers at this commit: `gnome::GnomeUpdateHooks` on Linux (src/platform/linux/gnome/
// deploy.rs) and a do-nothing stand-in in `non_linux.rs`. The only callers found by grep are in
// `update.rs`.

use std::path::Path;

/// Desktop-specific steps of an upgrade.
pub trait UpdateHooks {
    /// After the new binary is unpacked and checked, before it replaces the running one: deploys
    /// what the desktop needs from it (GNOME: the Shell extension, via `install --skip-bin
    /// --headless`). Prints its own progress and warnings.
    ///
    /// Side effect: runs `new_binary` as a child process.
    // GNOME does this only when `XDG_CURRENT_DESKTOP` contains GNOME (uppercased), and warns only
    // when the child cannot be started; a child that runs and exits non-zero is not reported
    // here (the status is not inspected).
    fn after_unpack(&self, new_binary: &Path);

    /// True if this desktop has a session action to offer after the upgrade (a log-out or a Shell
    /// reload). GNOME: `XDG_CURRENT_DESKTOP` contains GNOME.
    fn offers_session_action(&self) -> bool;

    /// True if the session is Wayland (the question and the action differ from X11).
    fn session_is_wayland(&self) -> bool;
}

#[cfg(test)]
mod tests {
    /// The release asset is named after the target triple; for the one target released so far
    /// it is the name `update.rs` always used, byte for byte.
    #[test]
    fn test_asset_name_follows_the_target_triple() {
        assert_eq!(
            crate::update::asset_name_for("x86_64-unknown-linux-gnu"),
            "spawn-at-x86_64-unknown-linux-gnu.tar.gz"
        );
        assert_eq!(
            crate::update::asset_name_for("aarch64-apple-darwin"),
            "spawn-at-aarch64-apple-darwin.tar.gz"
        );
        #[cfg(all(target_os = "linux", target_arch = "x86_64", target_env = "gnu"))]
        assert_eq!(
            crate::update::asset_name_for(env!("SPAWN_AT_TARGET")),
            "spawn-at-x86_64-unknown-linux-gnu.tar.gz"
        );
    }

    /// `update.rs` (the part before its tests) reads no desktop environment variable and names no
    /// compositor module: that is the hooks' job.
    #[test]
    fn test_update_rs_names_no_desktop() {
        let src = include_str!("../update.rs");
        let production = src.split("#[cfg(test)]").next().unwrap();
        assert!(!production.contains("XDG_CURRENT_DESKTOP"));
        assert!(!production.contains("platform::linux"));
        assert!(production.contains("platform::update_hooks()"));
    }

    /// The provider can be asked for (not called with a binary).
    #[test]
    fn test_the_provider_exists() {
        let hooks: &dyn super::UpdateHooks = crate::platform::update_hooks();
        // Same rule as before the hooks existed: a session action is offered exactly when
        // XDG_CURRENT_DESKTOP contains GNOME (no test changes that variable).
        let on_gnome = std::env::var("XDG_CURRENT_DESKTOP")
            .unwrap_or_default()
            .to_uppercase()
            .contains("GNOME");
        assert_eq!(hooks.offers_session_action(), on_gnome);
    }
}
