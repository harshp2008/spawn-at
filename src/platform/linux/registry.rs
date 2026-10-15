//! Which Linux backend a session gets: the choice that `LinuxBackend::bootstrap` always made,
//! as a pure function so every environment can be tested without a desktop.
//!
//! The order is: `SPAWN_AT_BACKEND=x11` (compared after lowercasing, so `X11` counts too) forces
//! X11; otherwise, if `XDG_CURRENT_DESKTOP` contains `GNOME` (after uppercasing), the GNOME
//! Wayland driver is tried first and used if it connects to the session bus; otherwise, and
//! whenever the GNOME driver cannot connect, X11. Any other value of `SPAWN_AT_BACKEND` is
//! ignored. Every other desktop (KDE, Sway, Hyprland, none at all) therefore gets X11.
//!
//! One exception, and the only way to get the EXPERIMENTAL Sway backend: `SPAWN_AT_BACKEND=sway`
//! (compared after lowercasing like `x11`, nothing trimmed) selects Sway and nothing else. It is
//! never chosen from the desktop: a Sway session without the variable still gets X11.

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

/// The two environment variables the choice reads, normalised the way the old code did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Environment {
    /// `SPAWN_AT_BACKEND`, lowercased; empty when unset or not UTF-8.
    pub forced: String,
    /// `XDG_CURRENT_DESKTOP`, uppercased; empty when unset or not UTF-8.
    pub desktop: String,
}

impl Environment {
    /// The environment of this process.
    pub fn from_process_env() -> Self {
        Self::from_values(
            &std::env::var("SPAWN_AT_BACKEND").unwrap_or_default(),
            &std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default(),
        )
    }

    /// From the raw values of the two variables.
    pub fn from_values(forced: &str, desktop: &str) -> Self {
        Self {
            forced: forced.to_lowercase(),
            desktop: desktop.to_uppercase(),
        }
    }
}

/// The backends to try, in order. The last one is always `X11` (which needs no connection test),
/// except when Sway is forced: then Sway is the only candidate and there is nothing to fall back
/// to, because the user asked for it by name.
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

    /// The selection exactly as `LinuxBackend::bootstrap` wrote it before this module existed
    /// (the D-Bus connection result is a parameter instead of an await).
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
}
