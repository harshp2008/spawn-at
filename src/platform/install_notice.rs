//! The notice printed before an install on a platform spawn-at has not been run on, and the
//! confirmation that goes with it. Pure: no I/O and no terminal, so the decision is tested for
//! every GNOME version without a GNOME.
//!
//! It only builds the text and the yes/no decision; printing, prompting and reading the GNOME
//! version belong to the callers.
// Callers at this commit: `gnome_notice` from the GNOME provisioner (src/platform/linux/gnome/
// deploy.rs) and `gate` from `main`'s install branch.

/// Where to report problems.
pub const ISSUES_URL: &str = "https://github.com/harshp2008/spawn-at/issues";

/// What the install should say about the detected GNOME Shell, or `None` for the one
/// combination that gets no notice: major version 46 on Wayland.
///
/// `major` is the GNOME Shell major version, `None` when it could not be read. The text is two
/// lines: `NOTICE: ...`, then `It may not work. Bug reports are welcome: <ISSUES_URL>`. Wording
/// by case: 46 on X11 is "only PARTLY tested"; 42 to 44 "uses the legacy extension" and is
/// UNTESTED; any other version, and an unreadable one, is UNTESTED.
pub fn gnome_notice(major: Option<u32>, wayland: bool) -> Option<String> {
    let tail = format!("It may not work. Bug reports are welcome: {ISSUES_URL}");
    match (major, wayland) {
        (Some(46), true) => None,
        (Some(46), false) => Some(format!(
            "NOTICE: GNOME Shell 46 on X11 is only PARTLY tested (the test fixture, no real apps).\n{tail}"
        )),
        (Some(v @ 42..=44), _) => Some(format!(
            "NOTICE: GNOME Shell {v} uses the legacy extension, which is UNTESTED: it has never been run on a real GNOME 42-44.\n{tail}"
        )),
        (Some(v), _) => Some(format!(
            "NOTICE: GNOME Shell {v} is UNTESTED: spawn-at has only been run on GNOME 46 on Wayland.\n{tail}"
        )),
        (None, _) => Some(format!(
            "NOTICE: the GNOME Shell version could not be read, so this install is UNTESTED: spawn-at has only been run on GNOME 46 on Wayland.\n{tail}"
        )),
    }
}

/// What to do after the notice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gate {
    /// Go on with the install.
    Proceed,
    /// The user said no.
    Declined,
}

/// Decides whether the install goes on: `Proceed` when `yes` is set or `interactive` is false,
/// without calling `ask`; otherwise `ask()` decides (true is `Proceed`). Whether the notice was
/// printed is not this function's business.
pub fn gate(yes: bool, interactive: bool, ask: impl FnOnce() -> bool) -> Gate {
    if yes || !interactive || ask() {
        Gate::Proceed
    } else {
        Gate::Declined
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// GNOME 46 on Wayland, the one configuration that has been run, gets no notice.
    #[test]
    fn test_46_wayland_has_no_notice() {
        assert_eq!(gnome_notice(Some(46), true), None);
    }

    /// 42, 43 and 44 get the legacy notice with the version, the word UNTESTED and the issues URL,
    /// on both session types.
    #[test]
    fn test_legacy_42_to_44_get_the_untested_notice() {
        for v in [42, 43, 44] {
            for wayland in [true, false] {
                let n = gnome_notice(Some(v), wayland).unwrap();
                assert!(n.contains(&format!("GNOME Shell {v}")), "{n}");
                assert!(
                    n.contains("legacy extension") && n.contains("UNTESTED"),
                    "{n}"
                );
                assert!(n.contains("never been run on a real GNOME 42-44"), "{n}");
                assert!(n.contains(ISSUES_URL), "{n}");
                assert!(n.lines().count() == 2, "short: {n}");
            }
        }
    }

    /// Every other version, and an unreadable one, is untested too; 46 on X11 is "partly".
    #[test]
    fn test_other_versions_and_x11_are_flagged() {
        for v in [45, 47, 48, 49] {
            let n = gnome_notice(Some(v), true).unwrap();
            assert!(n.contains(&format!("GNOME Shell {v} is UNTESTED")), "{n}");
        }
        assert!(gnome_notice(None, true)
            .unwrap()
            .contains("could not be read"));
        let x11 = gnome_notice(Some(46), false).unwrap();
        assert!(
            x11.contains("PARTLY") && x11.contains("X11") && !x11.contains("UNTESTED"),
            "{x11}"
        );
    }

    /// `--yes`, a headless run and a run without a terminal never ask; an interactive run asks
    /// and honours the answer.
    #[test]
    fn test_gate() {
        let never = || -> bool { panic!("must not ask") };
        assert_eq!(gate(true, true, never), Gate::Proceed);
        assert_eq!(gate(false, false, never), Gate::Proceed);
        assert_eq!(gate(false, true, || true), Gate::Proceed);
        assert_eq!(gate(false, true, || false), Gate::Declined);
    }
}
