//! What ends a run: logging the user out, or reloading the shell. `session.rs` decides when and
//! whether (the question, the advice, the order); this trait is how it is done on this machine,
//! supplied by the platform so `session.rs` names no compositor.
//!
//! On Linux the provider is the GNOME one (`gnome::GnomeSessionActions`), as before this trait
//! existed; elsewhere both actions do nothing (spawn-at stops at `ensure_supported_os` there).

/// The two session actions a run can end with.
pub trait SessionActions {
    /// Logs the user out of the session. Called by `session::finish()` and nowhere else.
    fn log_out(&self);
    /// Reloads the shell in place, which keeps the user's applications open (X11).
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
