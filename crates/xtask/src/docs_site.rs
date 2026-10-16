//! `docs-site`: builds the whole documentation site the way CI does, and checks it.
//!
//! Steps, in order, each of which stops the run on failure: build the `spawn-at` binary; write
//! the CLI reference from its `--help`; check the book's markdown; `mdbook build` (any `WARN`
//! line is a failure); `cargo doc` with warnings as errors; copy the book and the rustdoc output
//! (under `api/`) into one directory; check that directory for broken links and third-party
//! requests.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::{docs_check, docs_cli, docs_html};

/// Warnings are errors, except one lint. The help text of `--monitor` is the doc comment
/// `Target monitor selector (<INDEX>, 'cursor', or 'primary')` in `src/cli/args.rs`; clap prints
/// that comment as `--help`, and rustdoc reads `<INDEX>` as an unclosed HTML tag. Fixing it would
/// change the help text, so the lint is allowed until the owner decides. Every other rustdoc
/// warning still fails the build.
const RUSTDOC_FLAGS: &str = "-D warnings -A rustdoc::invalid_html_tags";

const BOOK_DIR: &str = "docs-site";
const BOOK_OUT: &str = "target/docs-site/book";
const DEFAULT_SITE: &str = "target/docs-site/site";

fn step(text: &str) {
    println!("\n==> {text}");
}

fn run_cmd(cmd: &mut Command, what: &str) -> Result<(), String> {
    let status = cmd
        .status()
        .map_err(|e| format!("cannot run {what}: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{what} failed with {:?}", status.code()))
    }
}

/// Copies the directory `from` into `to` (created), following nothing but plain files and dirs.
pub fn copy_dir(from: &Path, to: &Path) -> std::io::Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else if entry.file_type()?.is_file() {
            fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

/// True if mdBook's output holds a warning line. mdBook exits 0 on warnings, so the output is
/// the only place they show.
pub fn has_warning(output: &str) -> bool {
    output.lines().any(|l| {
        l.contains(" WARN ") || l.contains("[WARN]") || l.to_lowercase().contains("warning")
    })
}

fn mdbook_build(mdbook: &str) -> Result<(), String> {
    let out = Command::new(mdbook)
        .args(["build", BOOK_DIR])
        .output()
        .map_err(|e| {
            format!("cannot run `{mdbook}`: {e}. Install mdBook v0.5.4 (see .github/workflows/docs.yml for the checked download) or pass --mdbook <path>")
        })?;
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    print!("{text}");
    if !out.status.success() {
        return Err(format!("mdbook build failed with {:?}", out.status.code()));
    }
    if has_warning(&text) {
        return Err("mdbook build printed a warning; warnings are errors here".to_string());
    }
    Ok(())
}

/// Entry point of `cargo xtask docs-site [--mdbook P] [--bin P] [--out DIR]`.
pub fn run(args: &[String]) -> Result<(), String> {
    let mut mdbook = "mdbook".to_string();
    let mut bin = PathBuf::from("target/debug/spawn-at");
    let mut site = PathBuf::from(DEFAULT_SITE);
    let mut it = args.iter();
    while let Some(flag) = it.next() {
        let value = it.next().ok_or_else(|| format!("{flag} needs a value"))?;
        match flag.as_str() {
            "--mdbook" => mdbook = value.clone(),
            "--bin" => bin = PathBuf::from(value),
            "--out" => site = PathBuf::from(value),
            other => return Err(format!("unknown option {other}")),
        }
    }

    step("build the spawn-at binary (its --help is the CLI reference)");
    run_cmd(
        Command::new("cargo").args(["build", "--locked", "-p", "spawn-at"]),
        "cargo build",
    )?;

    step("generate the CLI reference");
    docs_cli::run(&["--bin".to_string(), bin.display().to_string()])?;

    step("check the book source");
    docs_check::run(&[format!("{BOOK_DIR}/src")])?;

    step("mdbook build");
    mdbook_build(&mdbook)?;

    step("cargo doc (warnings are errors)");
    // Old output of another crate set (for example xtask) must not end up in the site.
    if Path::new("target/doc").exists() {
        fs::remove_dir_all("target/doc").map_err(|e| format!("cannot clear target/doc: {e}"))?;
    }
    run_cmd(
        Command::new("cargo")
            .args([
                "doc",
                "--no-deps",
                "--workspace",
                "--exclude",
                "xtask",
                "--locked",
            ])
            .env("RUSTDOCFLAGS", RUSTDOC_FLAGS),
        "cargo doc",
    )?;

    step(&format!("assemble {}", site.display()));
    if site.exists() {
        fs::remove_dir_all(&site).map_err(|e| format!("cannot clear {}: {e}", site.display()))?;
    }
    copy_dir(Path::new(BOOK_OUT), &site).map_err(|e| format!("cannot copy the book: {e}"))?;
    copy_dir(Path::new("target/doc"), &site.join("api"))
        .map_err(|e| format!("cannot copy the rustdoc output: {e}"))?;

    step("check the assembled site");
    docs_html::run(&[
        site.display().to_string(),
        "--generated-dir".to_string(),
        "api/".to_string(),
    ])?;
    println!("\n==> site ready in {}", site.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// mdBook's warning lines are recognised; ordinary info lines are not.
    #[test]
    fn test_has_warning() {
        assert!(has_warning("2026-10-16 WARN mdbook_core: broken thing\n"));
        assert!(has_warning("[WARN] x"));
        assert!(!has_warning("2026-10-16 INFO Book building has started\n"));
    }

    /// Nested directories and files are copied, and the source is left in place.
    #[test]
    fn test_copy_dir_copies_nested_files() {
        let base = std::env::temp_dir().join(format!("xtask-docs-copy-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(base.join("from/a/b")).unwrap();
        fs::write(base.join("from/a/b/f.txt"), "x").unwrap();
        copy_dir(&base.join("from"), &base.join("to")).unwrap();
        assert_eq!(fs::read_to_string(base.join("to/a/b/f.txt")).unwrap(), "x");
        assert!(base.join("from/a/b/f.txt").is_file());
        let _ = fs::remove_dir_all(&base);
    }
}
