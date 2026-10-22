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

/// An environment every test case passed on: a GNOME Shell major version and a session type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Verified {
    pub major: u32,
    pub wayland: bool,
}

/// An environment the test cases were run on only in part, with what was and was not run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PartlyTested {
    pub major: u32,
    pub wayland: bool,
    /// Completes "is only PARTLY tested (...)" in the notice. Facts only, as measured.
    pub what: &'static str,
}

/// The environments the notice treats as tested: no notice at all. An entry is added only after
/// every test case for that environment has passed in a run (see `docs/KNOWLEDGE.md`, "GNOME and
/// Wayland behavior"); a skipped case is not a pass.
///
/// Matching, here and in [`PARTLY_TESTED`], is by Shell **major** version only. GNOME marks an
/// extension OUT OF DATE by major (the `shell-version` list in `metadata.json`) and
/// `detect_gnome_shell_major_version` reads only the major, so a run on 46.x counts for every
/// 46.y: a minor release is a bug-fix release of the same major. That carries the evidence one
/// step past what was run, which the README matrix states by naming the exact Shell version.
pub const VERIFIED: &[Verified] = &[Verified {
    major: 46,
    wayland: true,
}];

/// The environments that were run, but not with every case passing: the notice is "only PARTLY
/// tested" and says `what`. Not in [`VERIFIED`], and not counted in "has been tested on".
///
/// GNOME Shell 50 on Wayland: one run of the `release-gate` suite on 50.1 (Ubuntu 26.04, one
/// monitor) with a clean build of `c2437c7`: 36 required cases, 16 passed, 9 failed (every one a
/// "request below the minimum size" case: the window came out 300x300, not 300x200), 4 skipped
/// (the cloak cases are limited to Shell 42-48 by their spec) and 7 await a human verdict.
pub const PARTLY_TESTED: &[PartlyTested] = &[
    PartlyTested {
        major: 46,
        wayland: false,
        what: "the test fixture, no real apps",
    },
    PartlyTested {
        major: 50,
        wayland: true,
        what: "one run on 50.1: of 36 required cases 16 passed, 9 failed (windows requested below their minimum size came out too tall), 4 were skipped and 7 await a human verdict",
    },
];

/// `"GNOME Shell 46 and 50 on Wayland"`: the entries of `list`, grouped by session type (Wayland
/// first), majors in ascending order, joined with "and". `None` when the list is empty.
fn describe_tested(list: &[Verified]) -> Option<String> {
    let mut groups = Vec::new();
    for (wayland, name) in [(true, "Wayland"), (false, "X11")] {
        let mut majors: Vec<u32> = list
            .iter()
            .filter(|v| v.wayland == wayland)
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
/// in [`VERIFIED`].
///
/// `major` is the GNOME Shell major version, `None` when it could not be read. The text is two
/// lines: `NOTICE: ...`, then `It may not work. Bug reports are welcome: <ISSUES_URL>`. Wording
/// by case: an entry of [`PARTLY_TESTED`] is "only PARTLY tested" and says what was run; 42 to 44
/// "uses the legacy extension" and is UNTESTED; any other version, and an unreadable one, is
/// UNTESTED and says which environments were tested.
pub fn gnome_notice(major: Option<u32>, wayland: bool) -> Option<String> {
    notice_for(VERIFIED, PARTLY_TESTED, major, wayland)
}

/// [`gnome_notice`] against explicit lists, so tests do not depend on the shipped ones.
fn notice_for(
    verified: &[Verified],
    partly: &[PartlyTested],
    major: Option<u32>,
    wayland: bool,
) -> Option<String> {
    let tail = format!("It may not work. Bug reports are welcome: {ISSUES_URL}");
    let session = if wayland { "Wayland" } else { "X11" };
    let tested = describe_tested(verified)
        .map(|d| format!("spawn-at has been tested on {d}."))
        .unwrap_or_else(|| "spawn-at has not been tested on any GNOME Shell.".to_string());
    let is_verified = major.is_some_and(|m| {
        verified
            .iter()
            .any(|v| v.major == m && v.wayland == wayland)
    });
    let part = major.and_then(|m| partly.iter().find(|p| p.major == m && p.wayland == wayland));
    match (major, part) {
        (Some(_), _) if is_verified => None,
        (Some(m), Some(p)) => Some(format!(
            "NOTICE: GNOME Shell {m} on {session} is only PARTLY tested ({}).\n{tail}",
            p.what
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

    const V: fn(u32, bool) -> Verified = |major, wayland| Verified { major, wayland };
    const VERIFIED_46_48: &[Verified] = &[
        Verified {
            major: 46,
            wayland: true,
        },
        Verified {
            major: 48,
            wayland: true,
        },
    ];
    const PARTLY_46_X11: &[PartlyTested] = &[PartlyTested {
        major: 46,
        wayland: false,
        what: "only the fixture",
    }];

    /// The shipped lists, pinned: adding or dropping an environment is a deliberate edit that must
    /// come with a run (and with the README matrix), so it has to change this test too. 50 on
    /// Wayland is in the partly tested list because 9 of its required cases failed on 50.1.
    #[test]
    fn test_the_shipped_lists() {
        let verified: Vec<(u32, bool)> = VERIFIED.iter().map(|v| (v.major, v.wayland)).collect();
        assert_eq!(verified, vec![(46, true)]);
        let partly: Vec<(u32, bool)> = PARTLY_TESTED.iter().map(|p| (p.major, p.wayland)).collect();
        assert_eq!(partly, vec![(46, false), (50, true)]);
    }

    /// An environment is in at most one list and only once, and a listed modern Shell (45 and up)
    /// must be in the `shell-version` list of the modern `metadata.json`, or GNOME would mark the
    /// extension OUT OF DATE there and the notice would claim a run that could not have loaded it.
    #[test]
    fn test_listed_environments_are_unique_and_loadable() {
        let meta = include_str!("../../assets/gnome/modern/metadata.json");
        let json: serde_json::Value = serde_json::from_str(meta).unwrap();
        let listed: Vec<u64> = json["shell-version"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str()?.parse().ok())
            .collect();
        let all: Vec<(u32, bool)> = VERIFIED
            .iter()
            .map(|v| (v.major, v.wayland))
            .chain(PARTLY_TESTED.iter().map(|p| (p.major, p.wayland)))
            .collect();
        for (i, (major, wayland)) in all.iter().enumerate() {
            assert!(
                !all[..i].contains(&(*major, *wayland)),
                "listed twice: {major} wayland={wayland}"
            );
            if *major >= 45 {
                assert!(
                    listed.contains(&u64::from(*major)),
                    "{major} is not in shell-version {listed:?}"
                );
            }
        }
        for p in PARTLY_TESTED {
            assert!(!p.what.is_empty() && !p.what.contains('\n'), "{p:?}");
        }
    }

    /// A verified environment gets no notice, on that session type only; a partly tested one gets
    /// the "PARTLY" notice with its own version, session and text, and never says UNTESTED.
    #[test]
    fn test_verified_has_no_notice_and_partly_tested_says_what_was_run() {
        assert_eq!(
            notice_for(VERIFIED_46_48, PARTLY_46_X11, Some(46), true),
            None
        );
        assert_eq!(
            notice_for(VERIFIED_46_48, PARTLY_46_X11, Some(48), true),
            None
        );
        assert!(notice_for(VERIFIED_46_48, PARTLY_46_X11, Some(48), false)
            .unwrap()
            .contains("GNOME Shell 48 on X11 is UNTESTED"));
        let partly = notice_for(VERIFIED_46_48, PARTLY_46_X11, Some(46), false).unwrap();
        assert!(
            partly.contains("GNOME Shell 46 on X11 is only PARTLY tested (only the fixture).")
                && !partly.contains("UNTESTED"),
            "{partly}"
        );
        assert!(partly.contains(ISSUES_URL), "{partly}");
        assert_eq!(partly.lines().count(), 2, "{partly}");
    }

    /// An environment in both lists counts as verified (no notice): the lists are not meant to
    /// overlap, and `test_listed_environments_are_unique_and_loadable` keeps the shipped ones apart.
    #[test]
    fn test_verified_wins_over_partly_tested() {
        let both = [PartlyTested {
            major: 46,
            wayland: true,
            what: "x",
        }];
        assert_eq!(notice_for(VERIFIED_46_48, &both, Some(46), true), None);
    }

    /// An unlisted version says what IS tested, built from the verified list only: with 46 and 48
    /// on Wayland it names both; with an empty list it says nothing was tested; a partly tested
    /// environment never counts as tested.
    #[test]
    fn test_untested_notice_names_what_was_tested() {
        let n = notice_for(VERIFIED_46_48, PARTLY_46_X11, Some(49), true).unwrap();
        assert!(
            n.contains(
                "GNOME Shell 49 on Wayland is UNTESTED: spawn-at has been tested on GNOME Shell 46 and 48 on Wayland."
            ),
            "{n}"
        );
        assert!(!n.contains("only been run"), "{n}");
        assert_eq!(n.lines().count(), 2, "{n}");
        let unreadable = notice_for(VERIFIED_46_48, PARTLY_46_X11, None, true).unwrap();
        assert!(
            unreadable.contains("could not be read")
                && unreadable.contains("tested on GNOME Shell 46 and 48 on Wayland."),
            "{unreadable}"
        );
        let none = notice_for(&[], PARTLY_46_X11, Some(47), true).unwrap();
        assert!(
            none.contains("has not been tested on any GNOME Shell"),
            "{none}"
        );
        assert!(!describe_tested(VERIFIED_46_48).unwrap().contains("X11"));
    }

    /// The summary lists majors in ascending order whatever the list order, groups by session and
    /// joins three with commas.
    #[test]
    fn test_describe_tested_orders_and_joins() {
        let list = [V(50, true), V(46, true), V(48, true), V(47, false)];
        assert_eq!(
            describe_tested(&list).as_deref(),
            Some("GNOME Shell 46, 48 and 50 on Wayland and 47 on X11")
        );
        assert_eq!(
            describe_tested(&[V(46, true)]).as_deref(),
            Some("GNOME Shell 46 on Wayland")
        );
        assert_eq!(describe_tested(&[]), None);
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

    /// Through the shipped lists: 46 on Wayland has no notice; 46 on X11 and 50 on Wayland are
    /// "partly" and 50 says what failed; every other version, and an unreadable one, is untested.
    /// The wording does not claim "only 46" (it is built from the list).
    #[test]
    fn test_shipped_lists_through_gnome_notice() {
        assert_eq!(gnome_notice(Some(46), true), None);
        let x11 = gnome_notice(Some(46), false).unwrap();
        assert!(
            x11.contains("PARTLY") && x11.contains("X11") && !x11.contains("UNTESTED"),
            "{x11}"
        );
        let fifty = gnome_notice(Some(50), true).unwrap();
        assert!(
            fifty.contains("GNOME Shell 50 on Wayland is only PARTLY tested")
                && fifty.contains("9 failed")
                && fifty.contains("16 passed")
                && !fifty.contains("UNTESTED"),
            "{fifty}"
        );
        for v in [45, 47, 48, 49] {
            let n = gnome_notice(Some(v), true).unwrap();
            assert!(
                n.contains(&format!("GNOME Shell {v} on Wayland is UNTESTED")),
                "{n}"
            );
            assert!(
                n.contains("spawn-at has been tested on GNOME Shell 46 on Wayland."),
                "{n}"
            );
        }
        assert!(gnome_notice(Some(50), false)
            .unwrap()
            .contains("GNOME Shell 50 on X11 is UNTESTED"));
        assert!(gnome_notice(None, true)
            .unwrap()
            .contains("could not be read"));
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
