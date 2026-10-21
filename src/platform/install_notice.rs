//! The notice printed before an install on a platform spawn-at has not been run on, and the
//! confirmation that goes with it. Pure: no I/O and no terminal, so the decision is tested for
//! every GNOME version without a GNOME.
//!
//! It only builds the text and the yes/no decision; printing, prompting and reading the GNOME
//! version belong to the callers.
// Callers: `gnome_notice` from the GNOME provisioner (src/platform/linux/gnome/
// deploy.rs) and `gate` from `main`'s install branch.

/// Where to report problems.
pub const ISSUES_URL: &str = "https://github.com/harshp2008/spawn-at/issues";

/// How much of the repo's own test cases passed in a verified environment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Coverage {
    /// Every test case for the environment passed, so the install prints no notice.
    Full,
    /// Only the test fixture was run, no real apps: the install prints a "PARTLY tested" notice.
    FixtureOnly,
}

/// One environment the test cases were run on: a GNOME Shell major version and a session type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Verified {
    pub major: u32,
    pub wayland: bool,
    pub coverage: Coverage,
}

/// The environments the notice treats as tested; every other GNOME Shell version and session
/// type gets the UNTESTED notice. An entry is added only after every test case for that
/// environment has passed in a run (see `docs/KNOWLEDGE.md`, "GNOME and Wayland behavior").
///
/// Matching is by Shell **major** version only. GNOME marks an extension OUT OF DATE by major
/// (the `shell-version` list in `metadata.json`) and `detect_gnome_shell_major_version` reads
/// only the major, so a run on 50.1 counts for 50.x: a minor release is a bug-fix release of the
/// same major. That carries the evidence one step past what was run, which the README matrix
/// states by naming the exact Shell version each row was run on.
pub const VERIFIED: &[Verified] = &[
    Verified {
        major: 46,
        wayland: true,
        coverage: Coverage::Full,
    },
    Verified {
        major: 46,
        wayland: false,
        coverage: Coverage::FixtureOnly,
    },
];

/// `"GNOME Shell 46 and 50 on Wayland"`: the [`Coverage::Full`] entries of `list`, grouped by
/// session type (Wayland first), majors in ascending order, joined with "and". `None` when no
/// entry is `Full`.
fn describe_tested(list: &[Verified]) -> Option<String> {
    let mut groups = Vec::new();
    for (wayland, name) in [(true, "Wayland"), (false, "X11")] {
        let mut majors: Vec<u32> = list
            .iter()
            .filter(|v| v.wayland == wayland && v.coverage == Coverage::Full)
            .map(|v| v.major)
            .collect();
        majors.sort_unstable();
        majors.dedup();
        if let Some((last, rest)) = majors.split_last() {
            let shells = if rest.is_empty() {
                last.to_string()
            } else {
                let rest: Vec<String> = rest.iter().map(u32::to_string).collect();
                format!("{} and {}", rest.join(", "), last)
            };
            groups.push(format!("{shells} on {name}"));
        }
    }
    if groups.is_empty() {
        None
    } else {
        Some(format!("GNOME Shell {}", groups.join(" and ")))
    }
}

/// What the install should say about the detected GNOME Shell, or `None` when the environment is
/// in [`VERIFIED`] with [`Coverage::Full`].
///
/// `major` is the GNOME Shell major version, `None` when it could not be read. The text is two
/// lines: `NOTICE: ...`, then `It may not work. Bug reports are welcome: <ISSUES_URL>`. Wording
/// by case: a [`Coverage::FixtureOnly`] entry is "only PARTLY tested"; 42 to 44 "uses the legacy
/// extension" and is UNTESTED; any other version, and an unreadable one, is UNTESTED and says
/// which environments were tested.
pub fn gnome_notice(major: Option<u32>, wayland: bool) -> Option<String> {
    notice_for(VERIFIED, major, wayland)
}

/// [`gnome_notice`] against an explicit list, so tests do not depend on the shipped one.
fn notice_for(list: &[Verified], major: Option<u32>, wayland: bool) -> Option<String> {
    let tail = format!("It may not work. Bug reports are welcome: {ISSUES_URL}");
    let session = if wayland { "Wayland" } else { "X11" };
    let tested = describe_tested(list)
        .map(|d| format!("spawn-at has been tested on {d}."))
        .unwrap_or_else(|| "spawn-at has not been tested on any GNOME Shell.".to_string());
    let found = major.and_then(|m| list.iter().find(|v| v.major == m && v.wayland == wayland));
    match (major, found) {
        (Some(_), Some(v)) if v.coverage == Coverage::Full => None,
        (Some(m), Some(_)) => Some(format!(
            "NOTICE: GNOME Shell {m} on {session} is only PARTLY tested (the test fixture, no real apps).\n{tail}"
        )),
        (Some(v @ 42..=44), None) => Some(format!(
            "NOTICE: GNOME Shell {v} uses the legacy extension, which is UNTESTED: it has never been run on a real GNOME 42-44.\n{tail}"
        )),
        (Some(v), None) => Some(format!(
            "NOTICE: GNOME Shell {v} on {session} is UNTESTED: {tested}\n{tail}"
        )),
        (None, _) => Some(format!(
            "NOTICE: the GNOME Shell version could not be read, so this install is UNTESTED: {tested}\n{tail}"
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

    const FULL_46_50: &[Verified] = &[
        Verified {
            major: 46,
            wayland: true,
            coverage: Coverage::Full,
        },
        Verified {
            major: 50,
            wayland: true,
            coverage: Coverage::Full,
        },
        Verified {
            major: 46,
            wayland: false,
            coverage: Coverage::FixtureOnly,
        },
    ];

    /// The shipped list, pinned: adding or dropping an environment is a deliberate edit that
    /// must come with a run (and with the README matrix), so it has to change this test too.
    #[test]
    fn test_the_shipped_verified_list() {
        let pairs: Vec<(u32, bool, Coverage)> = VERIFIED
            .iter()
            .map(|v| (v.major, v.wayland, v.coverage))
            .collect();
        assert_eq!(
            pairs,
            vec![
                (46, true, Coverage::Full),
                (46, false, Coverage::FixtureOnly)
            ]
        );
    }

    /// An environment is only ever listed once, and a verified modern Shell (45 and up) must be in
    /// the `shell-version` list of the modern `metadata.json`, or GNOME would mark the extension
    /// OUT OF DATE there and the notice would claim a test that could not have loaded it.
    #[test]
    fn test_verified_entries_are_unique_and_loadable() {
        let meta = include_str!("../../assets/gnome/modern/metadata.json");
        let json: serde_json::Value = serde_json::from_str(meta).unwrap();
        let listed: Vec<u64> = json["shell-version"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str()?.parse().ok())
            .collect();
        for (i, v) in VERIFIED.iter().enumerate() {
            assert!(
                !VERIFIED[..i]
                    .iter()
                    .any(|w| w.major == v.major && w.wayland == v.wayland),
                "listed twice: {v:?}"
            );
            if v.major >= 45 {
                assert!(
                    listed.contains(&u64::from(v.major)),
                    "{v:?} is not in shell-version {listed:?}"
                );
            }
        }
    }

    /// A `Full` entry gets no notice, on that session type only; a `FixtureOnly` entry gets the
    /// "PARTLY" notice with its own version and session; neither says UNTESTED.
    #[test]
    fn test_listed_environments_get_no_notice_or_the_partly_notice() {
        assert_eq!(notice_for(FULL_46_50, Some(46), true), None);
        assert_eq!(notice_for(FULL_46_50, Some(50), true), None);
        assert!(notice_for(FULL_46_50, Some(50), false)
            .unwrap()
            .contains("GNOME Shell 50 on X11 is UNTESTED"));
        let partly = notice_for(FULL_46_50, Some(46), false).unwrap();
        assert!(
            partly.contains("GNOME Shell 46 on X11 is only PARTLY tested")
                && !partly.contains("UNTESTED"),
            "{partly}"
        );
        assert!(partly.contains(ISSUES_URL), "{partly}");
    }

    /// An unlisted version says what IS tested, built from the list: with 46 and 50 on Wayland it
    /// names both and not the word "only"; with an empty list it says nothing was tested; a
    /// `FixtureOnly` entry never counts as tested.
    #[test]
    fn test_untested_notice_names_what_was_tested() {
        let n = notice_for(FULL_46_50, Some(48), true).unwrap();
        assert!(
            n.contains(
                "GNOME Shell 48 on Wayland is UNTESTED: spawn-at has been tested on GNOME Shell 46 and 50 on Wayland."
            ),
            "{n}"
        );
        assert!(!n.contains("only been run"), "{n}");
        assert_eq!(n.lines().count(), 2, "{n}");
        let unreadable = notice_for(FULL_46_50, None, true).unwrap();
        assert!(
            unreadable.contains("could not be read")
                && unreadable.contains("tested on GNOME Shell 46 and 50 on Wayland."),
            "{unreadable}"
        );
        let none = notice_for(&[], Some(46), true).unwrap();
        assert!(
            none.contains("has not been tested on any GNOME Shell"),
            "{none}"
        );
        assert_eq!(
            describe_tested(&FULL_46_50[2..]),
            None,
            "a FixtureOnly entry is not 'tested'"
        );
    }

    /// The summary lists majors in ascending order whatever the list order, groups by session and
    /// joins three with commas.
    #[test]
    fn test_describe_tested_orders_and_joins() {
        let e = |major, wayland| Verified {
            major,
            wayland,
            coverage: Coverage::Full,
        };
        let list = [e(50, true), e(46, true), e(48, true), e(47, false)];
        assert_eq!(
            describe_tested(&list).as_deref(),
            Some("GNOME Shell 46, 48 and 50 on Wayland and 47 on X11")
        );
        assert_eq!(
            describe_tested(&[e(46, true)]).as_deref(),
            Some("GNOME Shell 46 on Wayland")
        );
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

    /// Through the shipped list: every other version, and an unreadable one, is untested too; 46
    /// on X11 is "partly". The wording does not claim "only 46" (it is built from the list).
    #[test]
    fn test_other_versions_and_x11_are_flagged() {
        for v in [45, 47, 48, 49] {
            let n = gnome_notice(Some(v), true).unwrap();
            assert!(
                n.contains(&format!("GNOME Shell {v} on Wayland is UNTESTED")),
                "{n}"
            );
            assert!(n.contains("spawn-at has been tested on GNOME Shell"), "{n}");
        }
        assert!(gnome_notice(None, true)
            .unwrap()
            .contains("could not be read"));
        let x11 = gnome_notice(Some(46), false).unwrap();
        assert!(
            x11.contains("PARTLY") && x11.contains("X11") && !x11.contains("UNTESTED"),
            "{x11}"
        );
        assert_eq!(gnome_notice(Some(46), true), None);
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
