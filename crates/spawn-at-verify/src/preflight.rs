//! Preflight Gate: binary freshness & git status inspection.

use std::path::Path;
use std::process::Command;

/// Verdict outcome produced by preflight build inspection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreflightVerdict {
    /// Binary matches current repository commit cleanly.
    Pass,
    /// Binary contains uncommitted modifications or warning conditions.
    Warn(String),
    /// Binary is stale or compiled from a differing commit.
    Fail(String),
    /// Preflight check could not be completed (e.g. outside git repo).
    Skip(String),
}

/// Comprehensive preflight gate report evaluating binary freshness against git tree.
#[derive(Debug, Clone)]
pub struct PreflightReport {
    /// Effective preflight validation verdict.
    pub verdict: PreflightVerdict,
    /// Raw output captured from running .
    pub binary_version_output: String,
    /// Git commit hash extracted from binary version string.
    pub detected_commit: Option<String>,
    /// Current git HEAD commit hash of the working repository.
    pub repo_head: Option<String>,
    /// Whether the git repository working tree contains uncommitted changes.
    pub repo_dirty: bool,
    /// Whether the binary was built from a dirty working tree.
    pub is_dirty_build: bool,
}

/// Runs the preflight inspection on the target binary path.
pub fn inspect_binary(bin_path: &Path) -> Result<PreflightReport, String> {
    if !bin_path.exists() {
        return Err(format!("Binary '{}' does not exist", bin_path.display()));
    }

    // 1. Run binary with --version
    let output = Command::new(bin_path)
        .arg("--version")
        .output()
        .map_err(|e| {
            format!(
                "Failed to execute '{} --version': {}",
                bin_path.display(),
                e
            )
        })?;

    let version_out = String::from_utf8_lossy(&output.stdout).trim().to_string();

    // 2. Query git repository status
    let repo_head = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
            } else {
                None
            }
        });

    let repo_dirty = Command::new("git")
        .args(["status", "--porcelain"])
        .output()
        .ok()
        .map(|o| !o.stdout.is_empty())
        .unwrap_or(false);

    // 3. Extract binary commit or tag
    let (detected_commit, is_dirty_build, is_release_no_commit) =
        parse_binary_version(&version_out);

    // A binary built on an exact release tag prints no commit id; the tag at HEAD is what
    // identifies its commit (see `build.rs`, which uses `git describe --tags --exact-match`).
    let head_tag = Command::new("git")
        .args(["describe", "--tags", "--exact-match", "HEAD"])
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
            } else {
                None
            }
        });

    let verdict = evaluate_preflight_verdict(
        detected_commit.as_deref(),
        is_dirty_build,
        is_release_no_commit,
        repo_head.as_deref(),
        repo_dirty,
        head_tag.as_deref(),
    );

    Ok(PreflightReport {
        verdict,
        binary_version_output: version_out,
        detected_commit,
        repo_head,
        repo_dirty,
        is_dirty_build,
    })
}

/// Parses the output of `spawn-at --version` to extract commit ID and dirty status.
pub fn parse_binary_version(version_str: &str) -> (Option<String>, bool, bool) {
    let is_dirty = version_str.contains("-dirty");

    // Check for "Previous Commit ID: <hash>" (from DEV BUILD format)
    for line in version_str.lines() {
        if let Some(rest) = line.strip_prefix("Previous Commit ID:") {
            let commit = rest.trim();
            if !commit.is_empty() && commit != "unknown" {
                return (Some(commit.to_string()), is_dirty, false);
            }
        }
    }

    // Check for "spawn-at <version>"
    if let Some(rest) = version_str.strip_prefix("spawn-at ") {
        let tag = rest.trim();
        return (Some(tag.to_string()), is_dirty, false);
    }

    (None, false, true)
}

/// Evaluates preflight verdict according to the specification table.
pub fn evaluate_preflight_verdict(
    bin_commit: Option<&str>,
    is_dirty_build: bool,
    is_release_no_commit: bool,
    repo_head: Option<&str>,
    repo_dirty: bool,
    head_tag: Option<&str>,
) -> PreflightVerdict {
    if is_release_no_commit || bin_commit.is_none() {
        return PreflightVerdict::Fail(
            "Unrecognised `spawn-at --version` output: neither a DEV BUILD commit id nor `spawn-at <version>`. Hint: run `cargo build -p spawn-at` first.".into(),
        );
    }

    if is_dirty_build {
        return PreflightVerdict::Warn(
            "Binary built with uncommitted changes (dirty build). Hint: run `cargo build -p spawn-at` first, then run without --allow-dirty.".into(),
        );
    }

    let bin_c = bin_commit.unwrap();

    if let Some(head) = repo_head {
        let clean_bin_c = bin_c.trim_end_matches("-dirty");
        // A commit id is hex and never contains '.'; a release version such as `1.2.3-rc.1`
        // always does. A release binary matches HEAD only when HEAD is exactly tag `v<version>`.
        let matches_head = if clean_bin_c.contains('.') {
            head_tag == Some(format!("v{}", clean_bin_c).as_str())
        } else {
            head.starts_with(clean_bin_c) || clean_bin_c.starts_with(head)
        };

        if matches_head && !repo_dirty {
            PreflightVerdict::Pass
        } else if matches_head && repo_dirty {
            PreflightVerdict::Fail(
                "Stale binary: repo working tree has uncommitted changes since binary was built. Hint: run `cargo build -p spawn-at` first, then run without --allow-dirty."
                    .into(),
            )
        } else {
            PreflightVerdict::Fail(format!(
                "Stale binary: binary commit '{}' does not match repository HEAD '{}'. Hint: run `cargo build -p spawn-at` first, then run without --allow-dirty.",
                clean_bin_c,
                &head[..head.len().min(7)]
            ))
        }
    } else {
        PreflightVerdict::Skip("Git repository not detected; cannot verify binary commit".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tests parsing clean development build version output string.
    #[test]
    fn test_parse_dev_build_version() {
        let text = "DEV BUILD\nlast release was pre-cleanup-beta\n-----------------------------------------------------------------\nPrevious Commit ID: 00d0da4";
        let (commit, dirty, is_rel) = parse_binary_version(text);
        assert_eq!(commit.as_deref(), Some("00d0da4"));
        assert!(!dirty);
        assert!(!is_rel);
    }

    /// Tests parsing dirty development build version output string.
    #[test]
    fn test_parse_dirty_dev_build_version() {
        let text = "DEV BUILD\n-----------------------------------------------------------------\nPrevious Commit ID: 00d0da4-dirty";
        let (commit, dirty, is_rel) = parse_binary_version(text);
        assert_eq!(commit.as_deref(), Some("00d0da4-dirty"));
        assert!(dirty);
        assert!(!is_rel);
    }

    /// Tests that matching clean commit hashes produce a Pass verdict.
    #[test]
    fn test_evaluate_preflight_verdict_clean_pass() {
        let verdict = evaluate_preflight_verdict(
            Some("13a8fed"),
            false,
            false,
            Some("13a8feded2e5268c6081bc22c915864f7cc80f84"),
            false,
            None,
        );
        assert_eq!(verdict, PreflightVerdict::Pass);
    }

    /// Tests that mismatched commit hashes produce a Fail verdict.
    #[test]
    fn test_evaluate_preflight_verdict_stale_fail() {
        let verdict = evaluate_preflight_verdict(
            Some("00d0da4"),
            false,
            false,
            Some("13a8feded2e5268c6081bc22c915864f7cc80f84"),
            false,
            None,
        );
        assert!(matches!(verdict, PreflightVerdict::Fail(_)));
    }

    /// Tests that dirty build version outputs produce a Warn verdict.
    #[test]
    fn test_evaluate_preflight_verdict_dirty_warn() {
        let verdict = evaluate_preflight_verdict(
            Some("13a8fed-dirty"),
            true,
            false,
            Some("13a8feded2e5268c6081bc22c915864f7cc80f84"),
            false,
            None,
        );
        assert!(matches!(verdict, PreflightVerdict::Warn(_)));
    }

    const HEAD: &str = "2e64085aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    /// Runs the full parse + verdict path on `-V` text, as `inspect_binary` does.
    fn verdict_for(text: &str, head_tag: Option<&str>, repo_dirty: bool) -> PreflightVerdict {
        let (commit, dirty, rel) = parse_binary_version(text);
        evaluate_preflight_verdict(
            commit.as_deref(),
            dirty,
            rel,
            Some(HEAD),
            repo_dirty,
            head_tag,
        )
    }

    const DEV_AT_HEAD: &str =
        "DEV BUILD\nlast release was v0.2.0-beta.3\n-----\nPrevious Commit ID: 2e64085";

    /// A dev build whose commit id is HEAD passes, tag or no tag.
    #[test]
    fn test_dev_build_matching_head_passes() {
        assert_eq!(
            verdict_for(DEV_AT_HEAD, None, false),
            PreflightVerdict::Pass
        );
    }

    /// A dev build from another commit still fails with the stale-binary message, even when
    /// HEAD carries a tag (the tag must not rescue a dev build).
    #[test]
    fn test_dev_build_other_commit_fails() {
        let text = "DEV BUILD\n-----\nPrevious Commit ID: 00d0da4";
        for tag in [None, Some("v0.2.0-beta.4")] {
            match verdict_for(text, tag, false) {
                PreflightVerdict::Fail(m) => assert!(
                    m.contains("binary commit '00d0da4' does not match repository HEAD '2e64085'"),
                    "{m}"
                ),
                other => panic!("expected Fail, got {other:?}"),
            }
        }
    }

    /// `spawn-at 0.2.0-beta.4` passes when HEAD is exactly tag `v0.2.0-beta.4`.
    #[test]
    fn test_release_version_equal_to_exact_tag_passes() {
        assert_eq!(
            verdict_for("spawn-at 0.2.0-beta.4", Some("v0.2.0-beta.4"), false),
            PreflightVerdict::Pass
        );
    }

    /// A release version fails when HEAD is untagged or tagged with a different version, with the
    /// existing message naming the version and the short HEAD.
    #[test]
    fn test_release_version_without_matching_tag_fails() {
        for tag in [None, Some("v0.2.0-beta.3"), Some("0.2.0-beta.4")] {
            match verdict_for("spawn-at 0.2.0-beta.4", tag, false) {
                PreflightVerdict::Fail(m) => assert!(
                    m.contains(
                        "binary commit '0.2.0-beta.4' does not match repository HEAD '2e64085'"
                    ),
                    "{m}"
                ),
                other => panic!("tag {tag:?}: expected Fail, got {other:?}"),
            }
        }
    }

    /// Output that is neither a DEV BUILD nor `spawn-at <version>` (or is empty) fails.
    #[test]
    fn test_garbage_version_output_fails() {
        for text in ["", "hello world", "error: unrecognized option"] {
            assert!(
                matches!(
                    verdict_for(text, Some("v0.2.0-beta.4"), false),
                    PreflightVerdict::Fail(_)
                ),
                "{text:?}"
            );
        }
    }
}

/// Optional host tools and the Debian/Ubuntu package that provides each. Both are needed for the
/// independent workarea probe (`xrandr` counts monitors, `xprop` reads mutter's `_NET_WORKAREA`).
pub const OPTIONAL_TOOLS: &[(&str, &str)] =
    &[("xrandr", "x11-xserver-utils"), ("xprop", "x11-utils")];

/// Optional tools for which `exists` returns false, as `(tool, package)` pairs.
pub fn missing_optional_tools_with(
    exists: &dyn Fn(&str) -> bool,
) -> Vec<(&'static str, &'static str)> {
    OPTIONAL_TOOLS
        .iter()
        .copied()
        .filter(|(tool, _)| !exists(tool))
        .collect()
}

/// Optional tools missing from this machine's PATH.
pub fn missing_optional_tools() -> Vec<(&'static str, &'static str)> {
    missing_optional_tools_with(&crate::probe::command_exists)
}

/// Install hint for the missing optional tools, or `None` when nothing is missing.
///
/// Says plainly that geometry checks then fall back to the extension's own workarea values, which
/// are marked as extension-derived and are not independent evidence.
pub fn optional_tools_hint(missing: &[(&str, &str)]) -> Option<String> {
    if missing.is_empty() {
        return None;
    }
    let tools: Vec<String> = missing
        .iter()
        .map(|(t, p)| format!("{} (package {})", t, p))
        .collect();
    let packages: Vec<&str> = missing.iter().map(|(_, p)| *p).collect();
    Some(format!(
        "optional tool(s) missing: {}. On Ubuntu/Debian: sudo apt install {}. Without them the \
         independent workarea probe is unavailable, so geometry checks fall back to \
         extension-derived values (reported as \"workarea from the extension, not independently \
         probed\").",
        tools.join(", "),
        packages.join(" ")
    ))
}

#[cfg(test)]
mod optional_tool_tests {
    use super::*;

    /// With both tools absent both are reported, the hint names each package and the apt
    /// command, and says geometry checks fall back to extension-derived values.
    #[test]
    fn test_hint_when_xrandr_and_xprop_are_absent() {
        let missing = missing_optional_tools_with(&|_| false);
        assert_eq!(
            missing,
            vec![("xrandr", "x11-xserver-utils"), ("xprop", "x11-utils")]
        );
        let hint = optional_tools_hint(&missing).unwrap();
        assert!(
            hint.contains("xrandr (package x11-xserver-utils)"),
            "{}",
            hint
        );
        assert!(hint.contains("xprop (package x11-utils)"), "{}", hint);
        assert!(
            hint.contains("sudo apt install x11-xserver-utils x11-utils"),
            "{}",
            hint
        );
        assert!(hint.contains("extension-derived"), "{}", hint);
    }

    /// Only the missing tool is named; with nothing missing there is no hint.
    #[test]
    fn test_hint_names_only_missing_tools_and_is_silent_when_present() {
        let missing = missing_optional_tools_with(&|t| t == "xrandr");
        assert_eq!(missing, vec![("xprop", "x11-utils")]);
        let hint = optional_tools_hint(&missing).unwrap();
        assert!(
            hint.contains("sudo apt install x11-utils") && !hint.contains("xrandr"),
            "{}",
            hint
        );
        assert_eq!(
            optional_tools_hint(&missing_optional_tools_with(&|_| true)),
            None
        );
    }
}
