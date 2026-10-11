//! Compiler-backed documentation check: collects rustc's `missing_docs` warnings.
//!
//! The text scan in `main.rs` only recognises `fn` and `pub struct/enum/trait/...` lines, so it
//! cannot see struct fields, enum variant fields or other items the compiler lints. This module
//! runs `cargo check` with `RUSTFLAGS="-W missing_docs"` (exactly the form to compare against)
//! in a separate target directory, so the normal build cache is not invalidated, and returns
//! the compiler's findings. Test crates carry no `#![warn(missing_docs)]`, so only this form sees
//! their public items.

use std::process::Command;

/// One `missing_docs` warning: file, 1-based line and the compiler's description.
pub struct CompilerMiss {
    /// Path as printed by cargo, relative to the workspace root.
    pub path: String,
    /// 1-based line number.
    pub line: usize,
    /// Compiler message, e.g. "missing documentation for a struct field".
    pub message: String,
}

/// Extracts `missing_docs` warnings from `cargo check --message-format=short` output, keeping
/// only files under one of `target_dirs`.
pub fn parse_missing_docs(output: &str, target_dirs: &[String]) -> Vec<CompilerMiss> {
    let mut misses: Vec<CompilerMiss> = Vec::new();
    for line in output.lines() {
        let Some(idx) = line.find(": warning: missing documentation") else {
            continue;
        };
        let message = line[idx + ": warning: ".len()..].to_string();
        // Location is `path:line:col`.
        let mut parts = line[..idx].rsplitn(3, ':');
        let (_col, line_no, path) = match (parts.next(), parts.next(), parts.next()) {
            (Some(c), Some(l), Some(p)) => (c, l, p),
            _ => continue,
        };
        let Ok(line_no) = line_no.parse::<usize>() else {
            continue;
        };
        let in_scope = target_dirs
            .iter()
            .any(|d| path.starts_with(d.trim_end_matches('/')));
        if in_scope && !misses.iter().any(|m| m.path == path && m.line == line_no) {
            misses.push(CompilerMiss {
                path: path.to_string(),
                line: line_no,
                message,
            });
        }
    }
    misses
}

/// Runs `cargo check` for `spawn-at-verify` with `tui,fixture` features on all targets under
/// `RUSTFLAGS="-W missing_docs"` and returns the in-scope warnings, or the compiler's error
/// output if the build failed.
pub fn compiler_missing_docs(target_dirs: &[String]) -> Result<Vec<CompilerMiss>, String> {
    let out = Command::new("cargo")
        .args([
            "check",
            "-p",
            "spawn-at-verify",
            "--features",
            "tui,fixture",
            "--all-targets",
            "--message-format=short",
        ])
        .env("RUSTFLAGS", "-W missing_docs")
        .env("CARGO_TARGET_DIR", "target/xtask-doc-check")
        .output()
        .map_err(|e| format!("failed to run cargo check: {}", e))?;
    let stderr = String::from_utf8_lossy(&out.stderr);
    if !out.status.success() {
        return Err(format!("cargo check failed:\n{}", stderr));
    }
    Ok(parse_missing_docs(&stderr, target_dirs))
}

#[cfg(test)]
mod tests {
    use super::*;

    const OUT: &str = "\
crates/spawn-at-verify/src/tui/app/state.rs:98:5: warning: missing documentation for a struct field
crates/spawn-at-verify/src/tui/app/state.rs:98:5: warning: missing documentation for a struct field
crates/spawn-at-verify/src/runner/models.rs:137:5: warning: missing documentation for a method
crates/spawn-at-core/src/lib.rs:3:1: warning: missing documentation for a module
crates/spawn-at-verify/src/lib.rs:9:1: warning: unused import: `x`
    Checking spawn-at-verify v0.1.0
";

    /// Only `missing_docs` warnings inside the target directories count, and a duplicate
    /// line (the same item reported for two targets) is counted once.
    #[test]
    fn test_parse_filters_scope_kind_and_duplicates() {
        let dirs = vec!["crates/spawn-at-verify/src".to_string()];
        let misses = parse_missing_docs(OUT, &dirs);
        assert_eq!(misses.len(), 2);
        assert_eq!(
            misses[0].path,
            "crates/spawn-at-verify/src/tui/app/state.rs"
        );
        assert_eq!(misses[0].line, 98);
        assert_eq!(
            misses[0].message,
            "missing documentation for a struct field"
        );
        assert_eq!(misses[1].line, 137);
    }

    /// A trailing slash on a target directory still matches its files.
    #[test]
    fn test_parse_accepts_trailing_slash_dirs() {
        let dirs = vec!["crates/spawn-at-core/src/".to_string()];
        assert_eq!(parse_missing_docs(OUT, &dirs).len(), 1);
    }
}
