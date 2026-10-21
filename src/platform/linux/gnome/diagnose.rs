//! Explains why the spawn-at extension did not answer on the session bus.
//!
//! A failed `ProtocolVersion` read or `ArmSpawn` call has several different causes that need
//! different fixes: the extension was never installed, it is installed but disabled, GNOME marked
//! it OUT OF DATE because the running Shell major is missing from the installed `metadata.json`,
//! or it is loaded but speaks an older protocol. [`diagnose`] tells them apart from [`Facts`],
//! and [`message`] turns the result into the text for the user.
//!
//! [`diagnose`] and [`message`] are pure. [`gather`] is pure over the text it is given;
//! [`gather_real`] is the only function that touches the machine, and it only reads: it runs
//! `gnome-shell --version` and `gnome-extensions info`, and reads the installed `metadata.json`.

use std::fs;
use std::path::Path;
use std::process::Command;

use super::detect::detect_gnome_shell_major_version;

/// What could be read of the installed `metadata.json`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Installed {
    /// No `metadata.json` in the extension directory.
    NotFound,
    /// The file exists but is not JSON with a `shell-version` array of strings.
    Unreadable,
    /// The `shell-version` entries, as written (for example `"49"`).
    Versions(Vec<String>),
}

/// The facts [`diagnose`] needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Facts {
    /// Major version of the running GNOME Shell; `None` when it could not be read.
    pub shell_major: Option<u32>,
    /// The installed `metadata.json`.
    pub installed: Installed,
    /// The `Enabled:` line of `gnome-extensions info`; `None` when it could not be read.
    pub enabled: Option<bool>,
    /// The `ProtocolVersion` the extension answered with; `None` when the read failed.
    pub protocol: Option<u32>,
    /// The lowest protocol the caller accepts.
    pub expected_protocol: u32,
}

/// Why the extension is not usable. The variants are the causes with a distinct fix.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Diagnosis {
    /// No installed extension files.
    NotInstalled,
    /// Installed and in date, but `gnome-extensions info` says `Enabled: No`.
    Disabled,
    /// The installed `metadata.json` does not list the running Shell major, so GNOME never
    /// loads the extension.
    OutOfDate {
        shell_major: u32,
        listed: Vec<String>,
    },
    /// Loaded, and answered with a protocol below the expected one.
    OldProtocol { found: u32, expected: u32 },
    /// Installed, listed for this Shell, not known to be disabled, and still silent.
    NotLoaded { shell_major: Option<u32> },
}

/// True when a `shell-version` entry names `major`. GNOME 40 and later match on the major
/// only, so `"50"` and `"50.1"` both name 50.
fn lists_major(entry: &str, major: u32) -> bool {
    entry
        .split('.')
        .next()
        .and_then(|n| n.parse::<u32>().ok())
        .is_some_and(|n| n == major)
}

/// The cause of the failure, or `None` when the extension answered with a protocol of at least
/// `expected_protocol` (nothing to explain).
///
/// An answered protocol decides alone: a loaded extension is by definition installed and in
/// date. Without an answer the checks run in this order: no files, then a Shell major missing
/// from the metadata, then disabled, then the catch-all. OUT OF DATE comes before disabled
/// because enabling an out-of-date extension does not load it, so the Shell version is the fix
/// that has to come first.
pub fn diagnose(facts: &Facts) -> Option<Diagnosis> {
    if let Some(found) = facts.protocol {
        return (found < facts.expected_protocol).then_some(Diagnosis::OldProtocol {
            found,
            expected: facts.expected_protocol,
        });
    }
    if facts.installed == Installed::NotFound {
        return Some(Diagnosis::NotInstalled);
    }
    if let (Some(major), Installed::Versions(listed)) = (facts.shell_major, &facts.installed) {
        if !listed.iter().any(|e| lists_major(e, major)) {
            return Some(Diagnosis::OutOfDate {
                shell_major: major,
                listed: listed.clone(),
            });
        }
    }
    if facts.enabled == Some(false) {
        return Some(Diagnosis::Disabled);
    }
    Some(Diagnosis::NotLoaded {
        shell_major: facts.shell_major,
    })
}

/// The text shown to the user: what is wrong, then the fix. Only
/// [`Diagnosis::OldProtocol`] says to log out and back in "to load the updated extension".
pub fn message(d: &Diagnosis) -> String {
    match d {
        Diagnosis::NotInstalled => {
            "The spawn-at GNOME extension is not installed.\n\
             Fix: run 'spawn-at install'."
                .to_string()
        }
        Diagnosis::Disabled => {
            "The spawn-at GNOME extension is installed but disabled.\n\
             Fix: run 'gnome-extensions enable spawn-at@harsh.local' (or 'spawn-at install')."
                .to_string()
        }
        Diagnosis::OutOfDate {
            shell_major,
            listed,
        } => format!(
            "The installed extension does not list GNOME Shell {shell_major} (it lists: {}), so GNOME marks it OUT OF DATE and never loads it.\n\
             Fix: upgrade spawn-at to a release that lists this Shell version, run 'spawn-at install', then log out and back in.",
            if listed.is_empty() {
                "nothing".to_string()
            } else {
                listed.join(", ")
            }
        ),
        Diagnosis::OldProtocol { found, expected } => format!(
            "The extension is loaded but reports protocol {found}; this spawn-at expects {expected} or newer.\n\
             Fix: run 'spawn-at install' if you have not, then log out and back in to load the updated extension."
        ),
        Diagnosis::NotLoaded { shell_major } => {
            let shell = shell_major
                .map(|m| format!(" for GNOME Shell {m}"))
                .unwrap_or_default();
            format!(
                "The extension files are installed{shell}, but the extension did not answer on the session bus.\n\
                 Fix: run 'gnome-extensions info spawn-at@harsh.local' and look at its State. On Wayland a newly installed or changed extension is loaded at the next login."
            )
        }
    }
}

/// Reads `<ext_dir>/metadata.json`. See [`Installed`] for the three outcomes.
pub fn read_installed(ext_dir: &Path) -> Installed {
    let text = match fs::read_to_string(ext_dir.join("metadata.json")) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Installed::NotFound,
        Err(_) => return Installed::Unreadable,
    };
    parse_shell_versions(&text).map_or(Installed::Unreadable, Installed::Versions)
}

/// The `shell-version` strings of a `metadata.json` text; `None` when the text is not JSON, has
/// no such array, or an entry is not a string.
pub fn parse_shell_versions(text: &str) -> Option<Vec<String>> {
    let json: serde_json::Value = serde_json::from_str(text).ok()?;
    json.get("shell-version")?
        .as_array()?
        .iter()
        .map(|v| v.as_str().map(str::to_string))
        .collect()
}

/// Reads the `Enabled:` line of the output of `gnome-extensions info <uuid>`: `Some(true)` for
/// "Yes", `Some(false)` for another value, `None` when there is no such line. Whether the label
/// stays "Enabled" in a translated output is not verified.
pub fn parse_info_enabled(info: &str) -> Option<bool> {
    info.lines().find_map(|l| {
        let (key, value) = l.trim().split_once(':')?;
        (key.trim() == "Enabled").then(|| value.trim().eq_ignore_ascii_case("yes"))
    })
}

/// Builds the [`Facts`] from text already read: `info` is the stdout of `gnome-extensions info`
/// (`None` when it could not be run).
pub fn gather(
    ext_dir: &Path,
    shell_major: Option<u32>,
    info: Option<&str>,
    protocol: Option<u32>,
    expected_protocol: u32,
) -> Facts {
    Facts {
        shell_major,
        installed: read_installed(ext_dir),
        enabled: info.and_then(parse_info_enabled),
        protocol,
        expected_protocol,
    }
}

/// [`gather`] with the machine's own Shell version and `gnome-extensions info <uuid>` output.
/// Runs two child processes and waits for them; meant for the failure path only.
pub fn gather_real(
    ext_dir: &Path,
    uuid: &str,
    protocol: Option<u32>,
    expected_protocol: u32,
) -> Facts {
    let info = Command::new("gnome-extensions")
        .args(["info", uuid])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned());
    gather(
        ext_dir,
        detect_gnome_shell_major_version(),
        info.as_deref(),
        protocol,
        expected_protocol,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn versions(v: &[&str]) -> Installed {
        Installed::Versions(v.iter().map(|s| s.to_string()).collect())
    }

    fn silent(installed: Installed, enabled: Option<bool>) -> Facts {
        Facts {
            shell_major: Some(50),
            installed,
            enabled,
            protocol: None,
            expected_protocol: 3,
        }
    }

    /// No metadata.json and no answer is "not installed", whatever `gnome-extensions info`
    /// said; catches a missing install being reported as a version or protocol problem.
    #[test]
    fn test_no_files_is_not_installed() {
        let d = diagnose(&silent(Installed::NotFound, Some(false))).unwrap();
        assert_eq!(d, Diagnosis::NotInstalled);
        assert!(message(&d).contains("not installed"));
        assert!(message(&d).contains("spawn-at install"));
    }

    /// The owner's 2026-10-21 case: Enabled Yes, metadata lists 45-48, Shell 50. Must be OUT OF
    /// DATE with the upgrade-install-relogin advice, and must not claim a protocol mismatch or
    /// say "load the updated extension".
    #[test]
    fn test_shell_major_missing_from_metadata_is_out_of_date() {
        let d = diagnose(&silent(versions(&["45", "46", "47", "48"]), Some(true))).unwrap();
        assert_eq!(
            d,
            Diagnosis::OutOfDate {
                shell_major: 50,
                listed: vec!["45".into(), "46".into(), "47".into(), "48".into()],
            }
        );
        let m = message(&d);
        assert!(m.contains("does not list GNOME Shell 50"), "{m}");
        assert!(m.contains("OUT OF DATE"), "{m}");
        assert!(m.contains("upgrade spawn-at"), "{m}");
        assert!(m.contains("spawn-at install"), "{m}");
        assert!(m.contains("log out and back in"), "{m}");
        assert!(!m.contains("mismatch"), "{m}");
        assert!(!m.contains("updated extension"), "{m}");
    }

    /// OUT OF DATE wins over disabled: enabling an extension that does not list the Shell
    /// leaves it unloaded, so advising 'enable' first would send the user in a circle.
    #[test]
    fn test_out_of_date_wins_over_disabled() {
        let d = diagnose(&silent(versions(&["45", "46"]), Some(false))).unwrap();
        assert!(matches!(d, Diagnosis::OutOfDate { .. }));
    }

    /// A list that names the Shell (including a "50.1" style entry) with `Enabled: No` is the
    /// disabled case with the enable command.
    #[test]
    fn test_listed_but_disabled_is_disabled() {
        for entry in ["50", "50.1"] {
            let d = diagnose(&silent(versions(&["48", entry]), Some(false))).unwrap();
            assert_eq!(d, Diagnosis::Disabled);
            let m = message(&d);
            assert!(m.contains("disabled"), "{m}");
            assert!(
                m.contains("gnome-extensions enable spawn-at@harsh.local"),
                "{m}"
            );
        }
    }

    /// Only a loaded extension with a protocol below the expected one gets the "log out and back
    /// in to load the updated extension" hint; a protocol of 1 against an expected 2 and 3
    /// catches a threshold that is off by one.
    #[test]
    fn test_old_protocol_gets_the_logout_hint() {
        for (found, expected) in [(1, 2), (2, 3)] {
            let mut f = silent(versions(&["48"]), Some(true));
            f.protocol = Some(found);
            f.expected_protocol = expected;
            let d = diagnose(&f).unwrap();
            assert_eq!(d, Diagnosis::OldProtocol { found, expected });
            assert!(message(&d).contains("log out and back in to load the updated extension"));
        }
    }

    /// An answering extension with a protocol at or above the expected one has nothing to
    /// explain, even if the metadata would not list the Shell.
    #[test]
    fn test_current_protocol_has_no_diagnosis() {
        let mut f = silent(versions(&["48"]), Some(true));
        f.protocol = Some(3);
        assert_eq!(diagnose(&f), None);
        f.protocol = Some(6);
        assert_eq!(diagnose(&f), None);
    }

    /// Files fine, listed, not disabled, no answer: the catch-all must not blame a protocol
    /// mismatch or an out-of-date list. An unreadable metadata.json and an unknown Shell
    /// version land here too, since neither proves a cause.
    #[test]
    fn test_listed_enabled_but_silent_is_not_loaded() {
        let d = diagnose(&silent(versions(&["50"]), Some(true))).unwrap();
        assert_eq!(
            d,
            Diagnosis::NotLoaded {
                shell_major: Some(50)
            }
        );
        let m = message(&d);
        assert!(
            !m.contains("mismatch")
                && !m.contains("updated extension")
                && !m.contains("OUT OF DATE"),
            "{m}"
        );

        let mut unknown_shell = silent(versions(&["48"]), None);
        unknown_shell.shell_major = None;
        assert_eq!(
            diagnose(&unknown_shell),
            Some(Diagnosis::NotLoaded { shell_major: None })
        );
        assert!(matches!(
            diagnose(&silent(Installed::Unreadable, Some(true))),
            Some(Diagnosis::NotLoaded { .. })
        ));
    }

    /// Two-digit majors and "major.minor" entries compare on the major only; "5" must not match
    /// 50 (catches a prefix or substring comparison).
    #[test]
    fn test_lists_major_compares_the_major_only() {
        assert!(lists_major("50", 50));
        assert!(lists_major("50.1", 50));
        assert!(!lists_major("5", 50));
        assert!(!lists_major("500", 50));
        assert!(!lists_major("beta", 50));
    }

    /// The `Enabled:` line decides; other labels and a missing line do not.
    #[test]
    fn test_parse_info_enabled() {
        let on = "spawn-at@harsh.local\n  Name: Spawn-At Interceptor\n  Enabled: Yes\n  State: OUT OF DATE\n";
        let off = "spawn-at@harsh.local\n  Enabled: No\n  State: INACTIVE\n";
        assert_eq!(parse_info_enabled(on), Some(true));
        assert_eq!(parse_info_enabled(off), Some(false));
        assert_eq!(parse_info_enabled("  State: ACTIVE\n"), None);
    }

    /// `read_installed` on a temp directory: missing file, broken JSON, and a real list.
    /// Catches a missing file being reported as unreadable (which would hide "not installed").
    #[test]
    fn test_read_installed_on_a_temp_directory() {
        let dir = std::env::temp_dir().join(format!("spawn-at-diagnose-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        assert_eq!(read_installed(&dir), Installed::NotFound);
        fs::write(dir.join("metadata.json"), "{ not json").unwrap();
        assert_eq!(read_installed(&dir), Installed::Unreadable);
        fs::write(
            dir.join("metadata.json"),
            r#"{"uuid":"spawn-at@harsh.local","shell-version":["49","50"]}"#,
        )
        .unwrap();
        assert_eq!(read_installed(&dir), versions(&["49", "50"]));
        fs::remove_dir_all(&dir).unwrap();
    }

    /// `gather` takes its facts from the arguments and the given directory, so a test can build
    /// the whole picture without GNOME: here the owner's case end to end.
    #[test]
    fn test_gather_builds_facts_from_text() {
        let dir = std::env::temp_dir().join(format!("spawn-at-gather-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("metadata.json"),
            r#"{"shell-version":["45","46","47","48"]}"#,
        )
        .unwrap();
        let f = gather(
            &dir,
            Some(50),
            Some("  Enabled: Yes\n  State: OUT OF DATE\n"),
            None,
            3,
        );
        assert_eq!(f.enabled, Some(true));
        assert!(matches!(
            diagnose(&f),
            Some(Diagnosis::OutOfDate {
                shell_major: 50,
                ..
            })
        ));
        fs::remove_dir_all(&dir).unwrap();
    }
}
