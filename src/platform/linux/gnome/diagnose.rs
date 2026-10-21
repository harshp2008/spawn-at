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
