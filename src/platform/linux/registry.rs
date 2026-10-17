//! Which Linux backend a session gets, as pure functions of three environment variables so every
//! environment can be tested without a desktop. No I/O and no D-Bus: the caller reads the
//! variables ([`Environment::from_process_env`]), connects the drivers and builds the backend.
//!
//! Order, as `candidates` implements it (values are only case-folded, never trimmed):
//! 1. `SPAWN_AT_BACKEND` equal to `x11` after lowercasing (so `X11` counts, `X11 ` and ` x11`
//!    do not): X11 only.
//! 2. `SPAWN_AT_BACKEND` equal to `sway` after lowercasing: Sway only, with no fallback. It is
//!    the only way to get the EXPERIMENTAL Sway backend; it is never chosen from the desktop.
//! 3. `XDG_CURRENT_DESKTOP` containing `GNOME` after uppercasing (`ubuntu:GNOME` counts):
//!    the GNOME Wayland driver first, then X11 if the caller finds the driver cannot connect.
//! 4. Anything else, including every other desktop (KDE, Hyprland, none at all): X11.
//!
//! Any other value of `SPAWN_AT_BACKEND` (such as `gnome`) is ignored by `candidates`.
//!
//! Unsupported-desktop rule, `unsupported`, checked by the caller before `candidates`: a session
//! is refused when `SPAWN_AT_BACKEND` is neither `x11` nor `sway`, `XDG_CURRENT_DESKTOP` does
//! not contain `GNOME`, and `XDG_SESSION_TYPE` is not `x11` (after lowercasing; unset counts as
//! not `x11`). A non-GNOME X11 session and a forced backend are accepted.
//! `XDG_SESSION_TYPE` is read only by this rule.
// Caller: `LinuxBackend::bootstrap` in src/platform/linux/mod.rs; `select` is test-only.

/// A backend this registry can pick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendId {
    /// The GNOME Shell extension over D-Bus.
    GnomeWayland,
    /// Plain X11 (also what a non-GNOME Wayland session gets, through XWayland).
    X11,
    /// Sway over its IPC socket (EXPERIMENTAL); only ever picked by `SPAWN_AT_BACKEND=sway`.
    Sway,
}

/// The three environment variables the choice reads, with the case normalised as the fields say.
/// A variable that is unset or not valid UTF-8 reads as the empty string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Environment {
    /// `SPAWN_AT_BACKEND`, lowercased; empty when unset or not UTF-8.
    pub forced: String,
    /// `XDG_CURRENT_DESKTOP`, uppercased; empty when unset or not UTF-8.
    pub desktop: String,
    /// `XDG_CURRENT_DESKTOP` as set, for messages.
    pub desktop_raw: String,
    /// `XDG_SESSION_TYPE`, lowercased; empty when unset or not UTF-8. Read only by
    /// `unsupported`; `candidates` ignores it.
    pub session: String,
}

impl Environment {
    /// The environment of this process.
    pub fn from_process_env() -> Self {
        Self::from_values(
            &std::env::var("SPAWN_AT_BACKEND").unwrap_or_default(),
            &std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default(),
        )
        .with_session(&std::env::var("XDG_SESSION_TYPE").unwrap_or_default())
    }

    /// From the raw values of the two variables; the session type starts out empty.
    pub fn from_values(forced: &str, desktop: &str) -> Self {
        Self {
            forced: forced.to_lowercase(),
            desktop: desktop.to_uppercase(),
            desktop_raw: desktop.to_string(),
            session: String::new(),
        }
    }

    /// The same environment with the raw value of `XDG_SESSION_TYPE`.
    pub fn with_session(mut self, session: &str) -> Self {
        self.session = session.to_lowercase();
        self
    }
}

/// `Some(message)` when this session is one spawn-at does not support, `None` otherwise.
///
/// Supported: `SPAWN_AT_BACKEND` forcing `x11` or `sway`; a desktop containing `GNOME`; or an X11
/// session (`XDG_SESSION_TYPE=x11`), where the X11 backend is the right one on any desktop.
/// Everything else (a non-GNOME Wayland session, or no session type at all) is unsupported. The
/// message names `XDG_CURRENT_DESKTOP` and `XDG_SESSION_TYPE` as set (`(unset)` when empty) and
/// mentions `SPAWN_AT_BACKEND=sway` and `SPAWN_AT_BACKEND=x11`.
pub fn unsupported(env: &Environment) -> Option<String> {
    if env.forced == "x11" || env.forced == "sway" {
        return None;
    }
    if env.desktop.contains("GNOME") || env.session == "x11" {
        return None;
    }
    let shown = |v: &str| {
        if v.is_empty() {
            "(unset)".to_string()
        } else {
            format!("'{v}'")
        }
    };
    Some(format!(
        "this desktop is not supported (XDG_CURRENT_DESKTOP is {}, XDG_SESSION_TYPE is {}). \
         spawn-at supports GNOME; Sway is experimental, via SPAWN_AT_BACKEND=sway. \
         To try the X11 fallback anyway, set SPAWN_AT_BACKEND=x11 (it may or may not work).",
        shown(&env.desktop_raw),
        shown(&env.session)
    ))
}

/// The backends to try, in order (see the module header for the rules). The last one is `X11`
/// (which needs no connection test), except when Sway is forced: then Sway is the only candidate
/// and there is nothing to fall back to, because the user asked for it by name. Never empty.
/// Does not look at `XDG_SESSION_TYPE`.
pub fn candidates(env: &Environment) -> Vec<BackendId> {
    if env.forced == "x11" {
        return vec![BackendId::X11];
    }
    if env.forced == "sway" {
        return vec![BackendId::Sway];
    }
    if env.desktop.contains("GNOME") {
        return vec![BackendId::GnomeWayland, BackendId::X11];
    }
    vec![BackendId::X11]
}

// Used by the tests as the synchronous model of the loop in `LinuxBackend::bootstrap` (the real
// GNOME connection is an await there).
#[cfg(test)]
/// The backend picked when `gnome_connects` says whether the GNOME driver can reach the session
/// bus (only asked if GNOME is a candidate).
pub fn select(env: &Environment, mut gnome_connects: impl FnMut() -> bool) -> BackendId {
    for id in candidates(env) {
        match id {
            BackendId::GnomeWayland if !gnome_connects() => continue,
            id => return id,
        }
    }
    BackendId::X11
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A reference model of the selection that knows only `x11` as a forced value (it knows no
    /// `sway`) and takes the D-Bus connection result as a parameter instead of an await;
    /// `select` is compared against it below.
    fn old_selection(forced_raw: &str, desktop_raw: &str, dbus_ok: bool) -> BackendId {
        let forced = forced_raw.to_lowercase();
        if forced == "x11" {
            return BackendId::X11;
        }
        let desktop = desktop_raw.to_uppercase();
        if desktop.contains("GNOME") && dbus_ok {
            return BackendId::GnomeWayland;
        }
        BackendId::X11
    }

    const FORCED: &[&str] = &[
        "", "x11", "X11", "X11 ", " x11", "gnome", "wayland", "xorg", "0",
    ];
    const DESKTOPS: &[&str] = &[
        "",
        "GNOME",
        "gnome",
        "ubuntu:GNOME",
        "GNOME-Classic:GNOME",
        "GNOME-Flashback:GNOME",
        "X-Cinnamon",
        "KDE",
        "sway",
        "Hyprland",
        "XFCE",
        "MATE",
        "Unity",
        "LXQt",
    ];

    /// Every combination of the forced value, the desktop and the D-Bus result: the registry picks
    /// what the old code picked.
    #[test]
    fn test_registry_matches_the_old_selection_in_every_combination() {
        for forced in FORCED {
            for desktop in DESKTOPS {
                for dbus_ok in [true, false] {
                    let env = Environment::from_values(forced, desktop);
                    assert_eq!(
                        select(&env, || dbus_ok),
                        old_selection(forced, desktop, dbus_ok),
                        "forced={forced:?} desktop={desktop:?} dbus_ok={dbus_ok}"
                    );
                }
            }
        }
    }

    /// The table the old code distinguished, written out.
    #[test]
    fn test_candidate_lists() {
        use BackendId::*;
        let cases: &[(&str, &str, &[BackendId])] = &[
            // forced x11 wins over everything, in any case
            ("x11", "GNOME", &[X11]),
            ("X11", "ubuntu:GNOME", &[X11]),
            ("x11", "", &[X11]),
            // other forced values are ignored
            ("gnome", "GNOME", &[GnomeWayland, X11]),
            ("wayland", "KDE", &[X11]),
            (" x11", "GNOME", &[GnomeWayland, X11]),
            // GNOME in the desktop (any case, any position): try GNOME, then X11
            ("", "GNOME", &[GnomeWayland, X11]),
            ("", "gnome", &[GnomeWayland, X11]),
            ("", "ubuntu:GNOME", &[GnomeWayland, X11]),
            ("", "GNOME-Classic:GNOME", &[GnomeWayland, X11]),
            // everything else: X11
            ("", "", &[X11]),
            ("", "KDE", &[X11]),
            ("", "sway", &[X11]),
            ("", "Hyprland", &[X11]),
        ];
        for (forced, desktop, want) in cases {
            let env = Environment::from_values(forced, desktop);
            assert_eq!(&candidates(&env)[..], *want, "{forced:?} {desktop:?}");
        }
    }

    /// The GNOME connection is only attempted when GNOME is a candidate, and X11 needs no test.
    #[test]
    fn test_gnome_is_only_tried_when_it_is_a_candidate() {
        let mut asked = 0;
        let kde = Environment::from_values("", "KDE");
        assert_eq!(
            select(&kde, || {
                asked += 1;
                true
            }),
            BackendId::X11
        );
        let forced = Environment::from_values("x11", "GNOME");
        assert_eq!(
            select(&forced, || {
                asked += 1;
                true
            }),
            BackendId::X11
        );
        assert_eq!(asked, 0);
        let gnome = Environment::from_values("", "GNOME");
        assert_eq!(
            select(&gnome, || {
                asked += 1;
                false
            }),
            BackendId::X11
        );
        assert_eq!(asked, 1);
    }

    /// The new Sway values, kept out of `FORCED` on purpose: `old_selection` above is the code as
    /// it was before Sway existed and knows no `sway`, so for exactly these values the registry
    /// is *supposed* to differ from it. Forced `sway` in any case selects Sway whatever the
    /// desktop and whether or not D-Bus works, and never asks for the GNOME connection.
    #[test]
    fn test_forced_sway_differs_from_the_old_selection_only_for_sway_and_selects_it_everywhere() {
        for forced in ["sway", "SWAY", "Sway", "sWaY"] {
            for desktop in DESKTOPS {
                for dbus_ok in [true, false] {
                    let env = Environment::from_values(forced, desktop);
                    let mut asked = 0;
                    assert_eq!(
                        select(&env, || {
                            asked += 1;
                            dbus_ok
                        }),
                        BackendId::Sway,
                        "forced={forced:?} desktop={desktop:?}"
                    );
                    assert_eq!(asked, 0, "Sway must not ask for the GNOME connection");
                    assert_eq!(
                        candidates(&env),
                        vec![BackendId::Sway],
                        "no X11 behind Sway"
                    );
                    // the old code ignored the value, which is the difference being pinned
                    assert_ne!(old_selection(forced, desktop, dbus_ok), BackendId::Sway);
                }
            }
        }
    }

    /// Values that only look like `sway` are ignored like any other unknown value (nothing is
    /// trimmed, as for `x11`): the registry and the old selection agree on all of them.
    #[test]
    fn test_values_near_sway_are_ignored_like_the_old_code_ignored_them() {
        for forced in [
            " sway", "sway ", "swayx", "sw ay", "sway\n", "swa", "i3", "swaymsg", "sway,x11",
        ] {
            for desktop in DESKTOPS {
                for dbus_ok in [true, false] {
                    let env = Environment::from_values(forced, desktop);
                    assert_eq!(
                        select(&env, || dbus_ok),
                        old_selection(forced, desktop, dbus_ok),
                        "forced={forced:?} desktop={desktop:?} dbus_ok={dbus_ok}"
                    );
                }
            }
        }
    }

    /// Sway is never a candidate unless forced: not for the desktop `sway` in any case, not for
    /// any other desktop, not for any of the values the old table already covered.
    #[test]
    fn test_sway_is_never_selected_without_the_variable() {
        for forced in FORCED {
            for desktop in DESKTOPS
                .iter()
                .chain(["SWAY", "Sway", "sway:wlroots", "i3"].iter())
            {
                let env = Environment::from_values(forced, desktop);
                assert!(
                    !candidates(&env).contains(&BackendId::Sway),
                    "forced={forced:?} desktop={desktop:?}"
                );
            }
        }
        // the Sway desktop still gets X11, the leak recorded as #11 in DESIGN_PLATFORMS.md
        let env = Environment::from_values("", "sway");
        assert_eq!(candidates(&env), vec![BackendId::X11]);
    }

    fn unsupported_for(forced: &str, desktop: &str, session: &str) -> Option<String> {
        unsupported(&Environment::from_values(forced, desktop).with_session(session))
    }

    /// GNOME (any case, any position, any session type) is supported.
    #[test]
    fn test_gnome_is_supported_in_every_session_type() {
        for desktop in ["GNOME", "gnome", "ubuntu:GNOME", "GNOME-Classic:GNOME"] {
            for session in ["wayland", "x11", "", "tty"] {
                assert_eq!(
                    unsupported_for("", desktop, session),
                    None,
                    "{desktop} {session}"
                );
            }
        }
    }

    /// A non-GNOME Wayland session, or one with no session type, without an override is an error
    /// that names the desktop and the session, GNOME, the experimental Sway backend, and the
    /// X11 opt-in with its warning.
    #[test]
    fn test_non_gnome_wayland_without_override_is_unsupported() {
        for (desktop, session) in [
            ("KDE", "wayland"),
            ("sway", "wayland"),
            ("", ""),
            ("XFCE", "tty"),
        ] {
            let m = unsupported_for("", desktop, session).expect("unsupported");
            assert!(m.contains("not supported"), "{m}");
            assert!(m.contains("GNOME"), "{m}");
            assert!(m.contains("SPAWN_AT_BACKEND=sway"), "{m}");
            assert!(
                m.contains("SPAWN_AT_BACKEND=x11") && m.contains("may or may not work"),
                "{m}"
            );
        }
        let m = unsupported_for("", "sway", "wayland").unwrap();
        assert!(
            m.contains("XDG_CURRENT_DESKTOP is 'sway'")
                && m.contains("XDG_SESSION_TYPE is 'wayland'"),
            "{m}"
        );
        let m = unsupported_for("", "", "").unwrap();
        assert!(
            m.contains("XDG_CURRENT_DESKTOP is (unset)")
                && m.contains("XDG_SESSION_TYPE is (unset)"),
            "{m}"
        );
    }

    /// A real X11 session on any desktop is legitimate X11 use: supported, no notice.
    #[test]
    fn test_non_gnome_x11_session_is_supported() {
        for desktop in ["KDE", "XFCE", "", "sway"] {
            for session in ["x11", "X11"] {
                assert_eq!(unsupported_for("", desktop, session), None);
                assert_eq!(
                    candidates(&Environment::from_values("", desktop).with_session(session)),
                    vec![BackendId::X11]
                );
            }
        }
    }

    /// Forcing x11 or sway is explicit and always supported; other values are ignored as before.
    #[test]
    fn test_forced_backends_are_supported_and_unchanged() {
        for forced in ["x11", "X11", "sway", "Sway"] {
            assert_eq!(unsupported_for(forced, "KDE", "wayland"), None, "{forced}");
        }
        let env = Environment::from_values("x11", "KDE").with_session("wayland");
        assert_eq!(candidates(&env), vec![BackendId::X11]);
        let env = Environment::from_values("sway", "KDE").with_session("wayland");
        assert_eq!(candidates(&env), vec![BackendId::Sway]);
        for forced in ["gnome", "wayland", " x11", "0"] {
            assert!(
                unsupported_for(forced, "KDE", "wayland").is_some(),
                "{forced:?}"
            );
        }
    }
}
