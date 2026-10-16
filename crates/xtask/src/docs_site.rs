//! `docs-site`: builds the whole documentation site the way CI does, and checks it.
//!
//! Steps, in order, each of which stops the run on failure: build the `spawn-at` binary; write
//! the CLI reference from its `--help`; check the site's markdown and Starlight config; install
//! the npm packages (`npm ci`, or reuse `docs-site/node_modules` when it is newer than
//! `package.json` and `package-lock.json`); `astro build` (Starlight, with Pagefind search);
//! `cargo doc` with warnings as errors; copy the Astro output and the rustdoc output (under
//! `api/`) into one directory; check that directory for broken links, missing ids, rewritten
//! flags and third-party requests. No step needs a network except `npm ci`, and only when it runs.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::SystemTime;

use crate::{docs_check, docs_cli, docs_html};

/// Warnings are errors, except one lint. The help text of `--monitor` is the doc comment
/// `Target monitor selector (<INDEX>, 'cursor', or 'primary')` in `src/cli/args.rs`; clap prints
/// that comment as `--help`, and rustdoc reads `<INDEX>` as an unclosed HTML tag. Fixing it would
/// change the help text, so the lint is allowed until the owner decides. Every other rustdoc
/// warning still fails the build.
const RUSTDOC_FLAGS: &str = "-D warnings -A rustdoc::invalid_html_tags";

/// The Astro project: `package.json`, `astro.config.mjs` and `src/content/docs/`.
const SITE_DIR: &str = "docs-site";
/// Where `astro build` writes (Astro's default `outDir`, inside the project; git ignores it).
const ASTRO_OUT: &str = "docs-site/dist";
const DEFAULT_SITE: &str = "target/docs-site/site";

/// Files the built site must contain: the home page, the 404 page, the Pagefind search script
/// (the site's search runs from these local files) and the favicon the pages link to.
const REQUIRED_FILES: [&str; 4] = [
    "index.html",
    "404.html",
    "pagefind/pagefind.js",
    "favicon.svg",
];

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

fn modified(path: &Path) -> Option<SystemTime> {
    fs::metadata(path).and_then(|m| m.modified()).ok()
}

/// True if the packages in `project/node_modules` can be reused: npm's own record of what it
/// installed (`node_modules/.package-lock.json`) exists and is newer than both `package.json` and
/// `package-lock.json`. When it is not, the run does a clean `npm ci`.
pub fn node_modules_fresh(project: &Path) -> bool {
    let Some(installed) = modified(&project.join("node_modules/.package-lock.json")) else {
        return false;
    };
    ["package.json", "package-lock.json"]
        .iter()
        .all(|f| modified(&project.join(f)).is_some_and(|m| m <= installed))
}

/// The names in `REQUIRED_FILES` that `site` does not contain.
pub fn missing_files(site: &Path) -> Vec<&'static str> {
    REQUIRED_FILES
        .iter()
        .copied()
        .filter(|f| !site.join(f).is_file())
        .collect()
}

/// An `npm` command run in the Astro project with Astro's telemetry off.
fn npm(args: &[&str]) -> Command {
    let mut cmd = Command::new("npm");
    cmd.args(args)
        .current_dir(SITE_DIR)
        .env("ASTRO_TELEMETRY_DISABLED", "1");
    cmd
}

/// Entry point of `cargo xtask docs-site [--bin P] [--out DIR] [--clean-install]`.
pub fn run(args: &[String]) -> Result<(), String> {
    let mut bin = PathBuf::from("target/debug/spawn-at");
    let mut site = PathBuf::from(DEFAULT_SITE);
    let mut clean_install = false;
    let mut it = args.iter();
    while let Some(flag) = it.next() {
        if flag == "--clean-install" {
            clean_install = true;
            continue;
        }
        let value = it.next().ok_or_else(|| format!("{flag} needs a value"))?;
        match flag.as_str() {
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

    step("check the site source");
    docs_check::run(&[])?;

    if clean_install || !node_modules_fresh(Path::new(SITE_DIR)) {
        // --ignore-scripts: no package's install script runs. The build does not need one
        // (checked on a clean install); a package that starts needing one fails here, loudly.
        step("npm ci (clean install from package-lock.json)");
        run_cmd(
            &mut npm(&["ci", "--ignore-scripts", "--no-audit", "--no-fund"]),
            "npm ci",
        )?;
    } else {
        step("reuse docs-site/node_modules (newer than package.json and package-lock.json)");
    }

    step("astro build");
    let dist = Path::new(ASTRO_OUT);
    if dist.exists() {
        fs::remove_dir_all(dist).map_err(|e| format!("cannot clear {ASTRO_OUT}: {e}"))?;
    }
    run_cmd(&mut npm(&["run", "build"]), "npm run build")?;

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
    if dist.join("api").exists() {
        return Err(format!(
            "{ASTRO_OUT}/api exists: the site has a page or file at /api/, which is the rustdoc output's place"
        ));
    }
    copy_dir(dist, &site).map_err(|e| format!("cannot copy the site: {e}"))?;
    copy_dir(Path::new("target/doc"), &site.join("api"))
        .map_err(|e| format!("cannot copy the rustdoc output: {e}"))?;
    let missing = missing_files(&site);
    if !missing.is_empty() {
        return Err(format!(
            "the built site lacks {} (is search or the favicon switched off?)",
            missing.join(", ")
        ));
    }

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
    use std::fs::File;
    use std::time::Duration;

    fn temp(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("xtask-docs-site-{}-{}", std::process::id(), tag));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn touch(path: &Path, age_secs: u64) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let file = File::create(path).unwrap();
        file.set_modified(SystemTime::now() - Duration::from_secs(age_secs))
            .unwrap();
    }

    /// Nested directories and files are copied, and the source is left in place.
    #[test]
    fn test_copy_dir_copies_nested_files() {
        let base = temp("copy");
        fs::create_dir_all(base.join("from/a/b")).unwrap();
        fs::write(base.join("from/a/b/f.txt"), "x").unwrap();
        copy_dir(&base.join("from"), &base.join("to")).unwrap();
        assert_eq!(fs::read_to_string(base.join("to/a/b/f.txt")).unwrap(), "x");
        assert!(base.join("from/a/b/f.txt").is_file());
        let _ = fs::remove_dir_all(&base);
    }

    /// Installed packages are reused only when npm's record of them is newer than both
    /// `package.json` and `package-lock.json`; no record, or an older one, means `npm ci`.
    #[test]
    fn test_node_modules_fresh() {
        let dir = temp("fresh");
        touch(&dir.join("package.json"), 100);
        touch(&dir.join("package-lock.json"), 100);
        assert!(!node_modules_fresh(&dir), "no node_modules yet");
        touch(&dir.join("node_modules/.package-lock.json"), 10);
        assert!(node_modules_fresh(&dir));
        touch(&dir.join("package-lock.json"), 1);
        assert!(
            !node_modules_fresh(&dir),
            "lock file changed after the install"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    /// A built site without the Pagefind script (search switched off) or without a 404 page is
    /// reported by name; a complete one reports nothing.
    #[test]
    fn test_missing_files() {
        let dir = temp("missing");
        for f in ["index.html", "404.html", "favicon.svg"] {
            touch(&dir.join(f), 1);
        }
        assert_eq!(missing_files(&dir), vec!["pagefind/pagefind.js"]);
        touch(&dir.join("pagefind/pagefind.js"), 1);
        assert!(missing_files(&dir).is_empty());
        let _ = fs::remove_dir_all(&dir);
    }
}
