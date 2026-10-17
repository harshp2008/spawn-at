//! What ends a run: logging the user out, or reloading the shell. `session.rs` decides when and
//! whether (the question, the advice, the order); this trait is how it is done on this machine,
//! supplied by the platform so `session.rs` names no compositor.
//!
//! The methods ask nothing and return nothing: an implementation that cannot do its action
//! prints its own message (GNOME's reload does when `xdotool` is missing) and the run goes on.
// Providers: on Linux `gnome::GnomeSessionActions` (src/platform/linux/gnome/
// deploy.rs); elsewhere a stand-in in `non_linux.rs` whose actions do nothing (spawn-at stops
// at `ensure_supported_os` there). `session.rs` is the only caller found by grep.

/// The two session actions a run can end with.
pub trait SessionActions {
    /// Logs the user out of the session. Side effect: ends the user's session and so their open
    /// applications.
    // GNOME: runs `gnome-session-quit --logout --no-prompt` in a new session group, output
    // discarded.
    fn log_out(&self);
    /// Reloads the shell in place, which keeps the user's applications open (X11).
    // GNOME: presses Alt+F2, types `r` and Return with `xdotool`; without `xdotool` it prints how
    // to install it and does nothing else.
    fn reload_shell(&self);
}

#[cfg(test)]
mod tests {
    /// `session.rs` (the part before its tests) names no compositor module: the actions come from
    /// `platform::session_actions()`. Reads the source; nothing is logged out.
    #[test]
    fn test_session_rs_does_not_name_a_compositor() {
        let src = include_str!("../session.rs");
        let production = src.split("#[cfg(test)]").next().unwrap();
        assert!(
            !production.contains("platform::linux"),
            "session.rs names platform::linux"
        );
        assert!(production.contains("platform::session_actions()"));
    }

    /// The provider can be asked for (not called): it is the GNOME one on Linux.
    #[test]
    fn test_the_provider_exists() {
        let _provider: &dyn super::SessionActions = crate::platform::session_actions();
    }
}
