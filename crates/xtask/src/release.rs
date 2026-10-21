//! The release checks that `.github/workflows/release.yml` runs:
//!
//! - `cargo xtask check-release-version [<tag>]` fails unless the `version` of all four workspace
//!   `Cargo.toml` files and the four matching `Cargo.lock` entries agree, and, when a tag such as
//!   `v0.2.4` is given, equal the tag's version. Without a tag (the dry run of the workflow with
//!   no input) it only checks that they agree.
//! - `cargo xtask release-notes <version> <out-file>` writes the `CHANGELOG.md` section
//!   `## [<version>]` to `<out-file>` when there is a non-empty one, and writes nothing
//!   otherwise (the release then keeps GitHub's generated notes).
//!
//! Both print what they decided and, when `$GITHUB_OUTPUT` is set, append `key=value` lines to it
//! for later steps: `version`; `found` and `path`. The parsing is line based and only meant for
//! the shapes cargo writes; a manifest that uses `version.workspace = true` counts as unreadable.

use std::fs;
use std::io::Write;

/// Every crate of the workspace as (manifest path, package name). `test_crate_list_matches_the_workspace`
/// pins this against the `members` of the root `Cargo.toml`, so a new crate cannot be forgotten.
const CRATES: [(&str, &str); 4] = [
    ("Cargo.toml", "spawn-at"),
    ("crates/spawn-at-core/Cargo.toml", "spawn-at-core"),
    ("crates/spawn-at-verify/Cargo.toml", "spawn-at-verify"),
    ("crates/xtask/Cargo.toml", "xtask"),
];

/// The version of a release tag: `v0.2.4` gives `0.2.4`, `v0.2.0-beta.6` gives `0.2.0-beta.6`.
/// A leading `v`, three dot-separated numbers without leading zeros, and an optional `-` suffix
/// of letters, digits, dots and hyphens; anything else (build metadata, a missing `v`) is an
/// error naming the tag.
pub fn tag_version(tag: &str) -> Result<String, String> {
    let bad = || {
        format!("the tag '{tag}' is not of the form vX.Y.Z or vX.Y.Z-suffix (for example v0.2.4 or v0.2.0-beta.6)")
    };
    let version = tag.strip_prefix('v').ok_or_else(bad)?;
    let (core, pre) = match version.split_once('-') {
        Some((core, pre)) => (core, Some(pre)),
        None => (version, None),
    };
    let numbers: Vec<&str> = core.split('.').collect();
    let number_ok = |n: &&str| {
        !n.is_empty()
            && n.bytes().all(|b| b.is_ascii_digit())
            && (n.len() == 1 || !n.starts_with('0'))
    };
    if numbers.len() != 3 || !numbers.iter().all(number_ok) {
        return Err(bad());
    }
    if let Some(pre) = pre {
        let ok = !pre.is_empty()
            && pre
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-');
        if !ok {
            return Err(bad());
        }
    }
    Ok(version.to_string())
}

/// The text between the first pair of double quotes of `s`.
fn quoted(s: &str) -> Option<String> {
    let rest = s.strip_prefix('"')?;
    Some(rest[..rest.find('"')?].to_string())
}

/// The `version = "..."` of the `[package]` table of a `Cargo.toml`; `None` when the table has
/// no such line (for example `version.workspace = true`).
pub fn manifest_version(text: &str) -> Option<String> {
    let mut in_package = false;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_package = line == "[package]";
        } else if in_package {
            if let Some(rest) = line.strip_prefix("version") {
                if let Some(value) = rest.trim_start().strip_prefix('=') {
                    return quoted(value.trim());
                }
            }
        }
    }
    None
}

/// The version of the local package `name` in a `Cargo.lock`: the `[[package]]` block with that
/// name and no `source` line (a registry crate of the same name has one). `None` when absent.
pub fn lock_version(text: &str, name: &str) -> Option<String> {
    text.split("[[package]]").find_map(|block| {
        let mut block_name = None;
        let mut version = None;
        let mut has_source = false;
        for line in block.lines() {
            if let Some(v) = line.strip_prefix("name = ") {
                block_name = quoted(v);
            } else if let Some(v) = line.strip_prefix("version = ") {
                version = quoted(v);
            } else if line.starts_with("source = ") {
                has_source = true;
            }
        }
        (block_name.as_deref() == Some(name) && !has_source)
            .then_some(version)
            .flatten()
    })
}

/// Checks the eight versions (four manifests, four lock entries) read through `read`, which maps
/// a path to the file's text. `Ok(version)` when they all agree and, with a non-empty `tag`,
/// equal the tag's version. `Err` lists one problem per line of output: a malformed tag, a file
/// or entry that cannot be read, or each value that differs from the reference (the tag's
/// version, else the first value read).
pub fn check(
    tag: Option<&str>,
    read: &dyn Fn(&str) -> Result<String, String>,
) -> Result<String, Vec<String>> {
    let tag = tag.filter(|t| !t.is_empty());
    let tag_ver = match tag {
        Some(t) => Some(tag_version(t).map_err(|e| vec![e])?),
        None => None,
    };
    let mut problems = Vec::new();
    let mut found: Vec<(String, String)> = Vec::new();
    let lock = read("Cargo.lock");
    for (path, name) in CRATES {
        match read(path) {
            Err(e) => problems.push(format!("{path}: cannot be read ({e})")),
            Ok(text) => match manifest_version(&text) {
                Some(v) => found.push((path.to_string(), v)),
                None => problems.push(format!("{path}: no literal version in [package]")),
            },
        }
        match &lock {
            Err(e) => {
                if !problems.iter().any(|p| p.starts_with("Cargo.lock: cannot")) {
                    problems.push(format!("Cargo.lock: cannot be read ({e})"));
                }
            }
            Ok(text) => match lock_version(text, name) {
                Some(v) => found.push((format!("Cargo.lock entry {name}"), v)),
                None => problems.push(format!("Cargo.lock: no entry for the local package {name}")),
            },
        }
    }
    let reference = tag_ver
        .clone()
        .or_else(|| found.first().map(|(_, v)| v.clone()));
    if let Some(reference) = &reference {
        for (what, v) in &found {
            if v != reference {
                problems.push(match tag {
                    Some(t) => format!("{what} has version {v}, but the tag {t} says {reference}"),
                    None => format!("{what} has version {v}, but {} has {reference}", found[0].0),
                });
            }
        }
    }
    match (problems.is_empty(), reference) {
        (true, Some(r)) => Ok(r),
        _ => Err(problems),
    }
}

/// The body of the `## [<version>]` section of a changelog: the lines after that heading up to the
/// next `## [` heading or the first link-reference line (`[x]: url`), trimmed. The heading must
/// name exactly `version` in brackets (`0.2.0` does not match `[0.2.0-beta.6]`). `None` when there
/// is no such heading or the section is empty.
pub fn changelog_section(changelog: &str, version: &str) -> Option<String> {
    let heading = format!("## [{version}]");
    let mut lines = changelog.lines();
    lines.find(|l| {
        l.strip_prefix(&heading)
            .is_some_and(|rest| rest.is_empty() || rest.starts_with(' '))
    })?;
    let body: Vec<&str> = lines
        .take_while(|l| !l.starts_with("## [") && !(l.starts_with('[') && l.contains("]: ")))
        .collect();
    let body = body.join("\n").trim().to_string();
    (!body.is_empty()).then_some(body)
}

/// Appends `key=value` to the file named by `$GITHUB_OUTPUT`, when it is set.
fn set_output(key: &str, value: &str) -> Result<(), String> {
    if let Ok(path) = std::env::var("GITHUB_OUTPUT") {
        let mut file = fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(&path)
            .map_err(|e| format!("cannot open $GITHUB_OUTPUT ({path}): {e}"))?;
        writeln!(file, "{key}={value}").map_err(|e| format!("cannot write $GITHUB_OUTPUT: {e}"))?;
    }
    Ok(())
}

/// `cargo xtask check-release-version [<tag>]` (see the module docs). Reads the files relative to
/// the current directory, which must be the repository root.
pub fn run_check_version(args: &[String]) -> Result<(), String> {
    if args.len() > 1 {
        return Err("Usage: cargo xtask check-release-version [<tag>]".to_string());
    }
    let tag = args.first().map(String::as_str);
    let read = |path: &str| fs::read_to_string(path).map_err(|e| e.to_string());
    match check(tag, &read) {
        Ok(version) => {
            match tag.filter(|t| !t.is_empty()) {
                Some(t) => println!("release version check: all 8 versions are {version}, matching the tag {t}"),
                None => println!("release version check: all 8 versions agree ({version}); no tag was given to compare with"),
            }
            set_output("version", &version)
        }
        Err(problems) => Err(problems.join("\n")),
    }
}

/// `cargo xtask release-notes <version> <out-file>` (see the module docs).
pub fn run_release_notes(args: &[String]) -> Result<(), String> {
    let [version, out] = args else {
        return Err("Usage: cargo xtask release-notes <version> <out-file>".to_string());
    };
    let changelog = fs::read_to_string("CHANGELOG.md").map_err(|e| format!("CHANGELOG.md: {e}"))?;
    match changelog_section(&changelog, version) {
        Some(body) => {
            fs::write(out, format!("{body}\n")).map_err(|e| format!("{out}: {e}"))?;
            println!("release notes: using the CHANGELOG.md section [{version}] ({} lines) as the release body", body.lines().count());
            set_output("found", "true")?;
            set_output("path", out)
        }
        None => {
            println!("release notes: no non-empty CHANGELOG.md section [{version}]; the release keeps the generated notes");
            set_output("found", "false")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn manifest(name: &str, version: &str) -> String {
        format!("[package]\nname = \"{name}\"\nversion = \"{version}\"\nedition = \"2021\"\n\n[dependencies]\nserde = {{ version = \"1.0\" }}\n")
    }

    fn lock(entries: &[(&str, &str)]) -> String {
        let mut text = String::from("version = 4\n\n[[package]]\nname = \"serde\"\nversion = \"1.0.1\"\nsource = \"registry+https://github.com/rust-lang/crates.io-index\"\n\n");
        for (name, version) in entries {
            text.push_str(&format!(
                "[[package]]\nname = \"{name}\"\nversion = \"{version}\"\n\n"
            ));
        }
        text
    }

    /// The eight files of a workspace at `versions` (manifests) and `locks` (lock entries), in
    /// the order of `CRATES`.
    fn files(versions: [&str; 4], locks: [&str; 4]) -> HashMap<String, String> {
        let mut map = HashMap::new();
        let mut entries = Vec::new();
        for (i, (path, name)) in CRATES.iter().enumerate() {
            map.insert(path.to_string(), manifest(name, versions[i]));
            entries.push((*name, locks[i]));
        }
        map.insert("Cargo.lock".to_string(), lock(&entries));
        map
    }

    fn run(map: &HashMap<String, String>, tag: Option<&str>) -> Result<String, Vec<String>> {
        check(tag, &|p| {
            map.get(p).cloned().ok_or_else(|| "missing".to_string())
        })
    }

    /// The tag shapes the workflow accepts and rejects. A rejected tag fails before any file is
    /// read, with a message that names the tag.
    #[test]
    fn test_tag_version() {
        assert_eq!(tag_version("v0.2.4").unwrap(), "0.2.4");
        assert_eq!(tag_version("v10.20.30").unwrap(), "10.20.30");
        assert_eq!(tag_version("v0.2.0-beta.6").unwrap(), "0.2.0-beta.6");
        for bad in [
            "0.2.4",
            "v0.2",
            "v0.2.4.1",
            "vx.y.z",
            "v0.2.4-",
            "v0.2.4+build",
            "v01.2.3",
            "v",
            "",
            "V0.2.4",
            "v0.2.4 ",
        ] {
            let e = tag_version(bad).unwrap_err();
            assert!(e.contains(&format!("'{bad}'")), "{bad}: {e}");
        }
    }

    /// Only the `[package]` table counts: a dependency's `version`, a dotted-key
    /// `version.workspace`, a comment and a later table do not.
    #[test]
    fn test_manifest_version_reads_only_the_package_table() {
        assert_eq!(
            manifest_version(&manifest("x", "0.2.3")).as_deref(),
            Some("0.2.3")
        );
        assert_eq!(
            manifest_version("[package]\nname = \"x\"\nversion.workspace = true\n"),
            None
        );
        assert_eq!(
            manifest_version("[dependencies]\nfoo = \"1\"\nversion = \"9.9.9\"\n"),
            None
        );
        assert_eq!(
            manifest_version("[package]\n# version = \"1.0.0\"\nversion   =   \"0.3.0\"\n")
                .as_deref(),
            Some("0.3.0")
        );
    }

    /// The lock entry is the local package: a registry crate with the same name (it has a
    /// `source`) is skipped; an absent name gives `None`.
    #[test]
    fn test_lock_version_picks_the_local_package() {
        let text = "[[package]]\nname = \"xtask\"\nversion = \"9.9.9\"\nsource = \"registry+https://x\"\n\n[[package]]\nname = \"xtask\"\nversion = \"0.2.3\"\n\n[[package]]\nname = \"xtask-extra\"\nversion = \"1.0.0\"\n";
        assert_eq!(lock_version(text, "xtask").as_deref(), Some("0.2.3"));
        assert_eq!(lock_version(text, "xtask-extra").as_deref(), Some("1.0.0"));
        assert_eq!(lock_version(text, "other"), None);
    }

    /// All eight values equal the tag: Ok with that version, for a release and a pre-release.
    #[test]
    fn test_check_passes_when_everything_matches_the_tag() {
        let m = files(["0.2.4"; 4], ["0.2.4"; 4]);
        assert_eq!(run(&m, Some("v0.2.4")).unwrap(), "0.2.4");
        let pre = files(["0.2.4-beta.1"; 4], ["0.2.4-beta.1"; 4]);
        assert_eq!(run(&pre, Some("v0.2.4-beta.1")).unwrap(), "0.2.4-beta.1");
    }

    /// One crate's manifest left behind is named, with the tag, and is the only problem reported.
    #[test]
    fn test_check_names_a_manifest_that_was_not_bumped() {
        let m = files(["0.2.4", "0.2.4", "0.2.3", "0.2.4"], ["0.2.4"; 4]);
        let problems = run(&m, Some("v0.2.4")).unwrap_err();
        assert_eq!(
            problems,
            vec!["crates/spawn-at-verify/Cargo.toml has version 0.2.3, but the tag v0.2.4 says 0.2.4"]
        );
    }

    /// A stale `Cargo.lock` entry (the usual mistake after editing a manifest by hand) is named.
    #[test]
    fn test_check_names_a_stale_lock_entry() {
        let m = files(["0.2.4"; 4], ["0.2.4", "0.2.3", "0.2.4", "0.2.4"]);
        let problems = run(&m, Some("v0.2.4")).unwrap_err();
        assert_eq!(
            problems,
            vec!["Cargo.lock entry spawn-at-core has version 0.2.3, but the tag v0.2.4 says 0.2.4"]
        );
    }

    /// A tag that disagrees with a consistent workspace lists all eight values.
    #[test]
    fn test_check_lists_every_value_when_the_tag_is_wrong() {
        let m = files(["0.2.3"; 4], ["0.2.3"; 4]);
        assert_eq!(run(&m, Some("v0.2.4")).unwrap_err().len(), 8);
    }

    /// Without a tag (or with an empty one, which is what an empty dry-run input becomes) only
    /// agreement is checked.
    #[test]
    fn test_check_without_a_tag_only_needs_agreement() {
        let m = files(["0.2.3"; 4], ["0.2.3"; 4]);
        assert_eq!(run(&m, None).unwrap(), "0.2.3");
        assert_eq!(run(&m, Some("")).unwrap(), "0.2.3");
        let split = files(["0.2.3", "0.2.4", "0.2.3", "0.2.3"], ["0.2.3"; 4]);
        let problems = run(&split, None).unwrap_err();
        assert_eq!(
            problems,
            vec!["crates/spawn-at-core/Cargo.toml has version 0.2.4, but Cargo.toml has 0.2.3"]
        );
    }

    /// A missing file, a missing lock entry and a malformed tag are problems, never a pass.
    #[test]
    fn test_check_reports_unreadable_input() {
        let mut m = files(["0.2.4"; 4], ["0.2.4"; 4]);
        m.remove("crates/xtask/Cargo.toml");
        let problems = run(&m, Some("v0.2.4")).unwrap_err();
        assert_eq!(
            problems,
            vec!["crates/xtask/Cargo.toml: cannot be read (missing)"]
        );
        let mut no_lock = files(["0.2.4"; 4], ["0.2.4"; 4]);
        no_lock.remove("Cargo.lock");
        assert_eq!(
            run(&no_lock, Some("v0.2.4")).unwrap_err(),
            vec!["Cargo.lock: cannot be read (missing)"]
        );
        assert!(run(&m, Some("0.2.4")).unwrap_err()[0].contains("'0.2.4'"));
    }

    const LOG: &str = "# Changelog\n\n## [Unreleased]\n\n### Fixed\n\n- unreleased thing\n\n## [0.2.4] - 2026-10-22\n\nIntro.\n\n### Fixed\n\n- the fix\n\n## [0.2.0-beta.6] - 2026-10-17\n\n- beta\n\n## [0.2.0] - 2026-10-18\n\n## [0.1.0] - 2026-01-01\n\n- last\n\n[Unreleased]: https://example.invalid/compare\n[0.1.0]: https://example.invalid/0.1.0\n";

    /// The section runs from its heading to the next `## [` heading, with the heading line itself
    /// and the surrounding blank lines left out.
    #[test]
    fn test_changelog_section_is_cut_at_the_next_heading() {
        assert_eq!(
            changelog_section(LOG, "0.2.4").unwrap(),
            "Intro.\n\n### Fixed\n\n- the fix"
        );
    }

    /// `0.2.0` is the release, not the beta; an empty section, an unknown version and a heading
    /// without brackets give `None`; the last section stops before the link references.
    #[test]
    fn test_changelog_section_exact_heading_and_edges() {
        assert_eq!(changelog_section(LOG, "0.2.0-beta.6").unwrap(), "- beta");
        assert_eq!(
            changelog_section(LOG, "0.2.0"),
            None,
            "the 0.2.0 section is empty"
        );
        assert_eq!(changelog_section(LOG, "9.9.9"), None);
        assert_eq!(changelog_section(LOG, "0.1.0").unwrap(), "- last");
        assert_eq!(changelog_section("## 0.2.4\n\n- x\n", "0.2.4"), None);
        assert_eq!(
            changelog_section("## [0.2.4]\n\n- x\n", "0.2.4").unwrap(),
            "- x",
            "a heading with no date"
        );
        assert_eq!(changelog_section("## [0.2.45] - d\n\n- x\n", "0.2.4"), None);
    }

    /// The real repository: this crate's list of crates is the workspace's `members` (a new
    /// crate has to be added to `CRATES`), and all eight versions agree today. A release-prep
    /// change that bumps only some of them fails here before it fails in the release workflow.
    #[test]
    fn test_crate_list_matches_the_workspace_and_versions_agree() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let root_manifest = fs::read_to_string(root.join("Cargo.toml")).unwrap();
        let members = root_manifest
            .split("members = [")
            .nth(1)
            .and_then(|rest| rest.split(']').next())
            .unwrap();
        let mut from_workspace: Vec<String> = members
            .split('"')
            .skip(1)
            .step_by(2)
            .map(|m| {
                if m == "." {
                    "Cargo.toml".to_string()
                } else {
                    format!("{m}/Cargo.toml")
                }
            })
            .collect();
        from_workspace.sort();
        let mut listed: Vec<String> = CRATES.iter().map(|(p, _)| p.to_string()).collect();
        listed.sort();
        assert_eq!(listed, from_workspace);
        let read = |p: &str| fs::read_to_string(root.join(p)).map_err(|e| e.to_string());
        let version = check(None, &read).unwrap();
        assert_eq!(version, env!("CARGO_PKG_VERSION"));
    }

    /// The text of `release.yml`, for the two pins below.
    fn workflow() -> String {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../.github/workflows/release.yml");
        fs::read_to_string(path).unwrap()
    }

    /// The one step that publishes (the one that uses the release action) runs only for a push of
    /// a `v*` tag, so the `workflow_dispatch` dry run cannot publish, whatever ref it is started
    /// from, and nothing else in the workflow uses that action.
    #[test]
    fn test_release_yml_publishes_only_for_a_tag_push() {
        let yml = workflow();
        assert_eq!(yml.matches("softprops/action-gh-release").count(), 1);
        let steps: Vec<&str> = yml.split("\n      - ").collect();
        let publish = steps
            .iter()
            .find(|s| s.contains("softprops/action-gh-release"))
            .unwrap();
        let guard = publish
            .lines()
            .find(|l| l.trim_start().starts_with("if:"))
            .expect("the publish step has an if");
        assert!(guard.contains("github.event_name == 'push'"), "{guard}");
        assert!(
            guard.contains("startsWith(github.ref, 'refs/tags/v')"),
            "{guard}"
        );
        assert!(
            yml.contains("\n  workflow_dispatch:"),
            "the dry run trigger exists"
        );
        let version_step = steps
            .iter()
            .position(|s| s.contains("check-release-version"))
            .unwrap();
        let build_step = steps
            .iter()
            .position(|s| s.contains("cargo build --release"))
            .unwrap();
        let test_step = steps
            .iter()
            .position(|s| s.contains("run: cargo test"))
            .unwrap();
        assert!(
            version_step < test_step && version_step < build_step,
            "the version check runs before the tests and the build"
        );
    }
}
