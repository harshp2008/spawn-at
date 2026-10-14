//! Detects an existing spawn-at install and plans what `spawn-at install` does about it:
//! keep and upgrade in place (today's behaviour), do nothing, or a clean install that removes
//! the old binary, extension and autostart entry first.
//!
//! A clean install never touches the config file or the settings in it: `spawn-at uninstall`
//! does not remove them either, and nothing here does. The decision logic takes its facts and
//! its prompt as arguments, so tests need no terminal, no real home and no real install.

use crate::platform::installer;
use crate::platform::{CompositorBackend, InstallScope, UninstallArgs};
use semver::Version;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// How long `<existing> -V` may take before it is given up on.
pub const VERSION_TIMEOUT: Duration = Duration::from_secs(2);

/// An existing spawn-at binary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    pub path: PathBuf,
    /// Version text for the user; `None` when `-V` failed or timed out.
    pub version: Option<String>,
    pub semver: Option<Version>,
}

/// The version being installed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewVersion {
    pub display: String,
    pub semver: Option<Version>,
}

/// Executable `spawn-at` files in the user location, the system location and on PATH, each once
/// (symlinks resolved), never the running binary. Order: user, system, then PATH order.
pub fn find_existing(
    user_bin: Option<&Path>,
    system_bin: &Path,
    path_env: Option<&OsStr>,
    current_exe: &Path,
) -> Vec<PathBuf> {
    let current = current_exe.canonicalize().ok();
    let mut candidates: Vec<PathBuf> = Vec::new();
    candidates.extend(user_bin.map(|d| d.join("spawn-at")));
    candidates.push(system_bin.join("spawn-at"));
    if let Some(p) = path_env {
        candidates.extend(std::env::split_paths(p).map(|d| d.join("spawn-at")));
    }
    let mut seen: Vec<PathBuf> = Vec::new();
    let mut out = Vec::new();
    for c in candidates {
        let Ok(canon) = c.canonicalize() else {
            continue;
        };
        if !is_executable_file(&canon) || Some(&canon) == current.as_ref() || seen.contains(&canon)
        {
            continue;
        }
        seen.push(canon);
        out.push(c);
    }
    out
}

fn is_executable_file(p: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    p.metadata()
        .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}

/// Runs `<path> -V` and returns its stdout, or `None` when it fails, prints nothing or does not
/// finish within `timeout` (the process is killed then).
pub fn query_version(path: &Path, timeout: Duration) -> Option<String> {
    let mut child = Command::new(path)
        .arg("-V")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                if !status.success() {
                    return None;
                }
                let mut text = String::new();
                std::io::Read::read_to_string(&mut child.stdout.take()?, &mut text).ok()?;
                let text = text.trim().to_string();
                return (!text.is_empty()).then_some(text);
            }
            Ok(None) if start.elapsed() < timeout => std::thread::sleep(Duration::from_millis(20)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
}

/// Turns the text of `spawn-at -V` into what is shown and a comparable version. A release build
/// prints `spawn-at <version>`; a development build prints `DEV BUILD ...` and has no version.
pub fn parse_version_output(out: &str) -> (String, Option<Version>) {
    let first = out.lines().next().unwrap_or("").trim();
    if first.starts_with("DEV BUILD") {
        return ("dev build".to_string(), None);
    }
    let ver = first.strip_prefix("spawn-at ").unwrap_or(first).trim();
    (ver.to_string(), crate::update::parse_semver(ver))
}

/// Finds the existing binaries and asks each for its version.
pub fn detect(
    user_bin: Option<&Path>,
    system_bin: &Path,
    path_env: Option<&OsStr>,
    current_exe: &Path,
) -> Vec<Found> {
    find_existing(user_bin, system_bin, path_env, current_exe)
        .into_iter()
        .map(|path| {
            let parsed = query_version(&path, VERSION_TIMEOUT).map(|o| parse_version_output(&o));
            Found {
                path,
                version: parsed.as_ref().map(|p| p.0.clone()),
                semver: parsed.and_then(|p| p.1),
            }
        })
        .collect()
}

/// What the user can pick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Choice {
    /// Leave everything as it is.
    NoChange,
    /// Install over the existing one (today's behaviour).
    Upgrade,
    /// Remove the existing install first.
    Clean,
}

/// A question for the user: the prompt, the items, and which one is the default.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Question {
    pub prompt: String,
    pub items: Vec<(String, Choice)>,
    pub default: usize,
}

/// What `spawn-at install` does next.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Plan {
    /// Install as usual.
    Proceed,
    /// Remove the existing binaries, extension and autostart entry, then install.
    Clean,
    /// Stop without changing anything.
    Stop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Relation {
    Same,
    Older,
    Newer,
    Unknown,
}

fn relation(found: &Found, new: &NewVersion) -> Relation {
    match (&found.semver, &new.semver) {
        (Some(a), Some(b)) if a == b => Relation::Same,
        (Some(a), Some(b)) if a < b => Relation::Older,
        (Some(a), Some(b)) if a > b => Relation::Newer,
        _ => Relation::Unknown,
    }
}

fn describe(found: &[Found], new: &NewVersion) -> Vec<String> {
    let shown = |f: &Found| {
        format!(
            "{} ({})",
            f.path.display(),
            f.version.as_deref().unwrap_or("version unknown")
        )
    };
    if let [only] = found {
        return vec![format!(
            "Existing: {}. Installing: {}.",
            shown(only),
            new.display
        )];
    }
    let mut lines = vec!["Existing:".to_string()];
    lines.extend(found.iter().map(|f| format!("  {}", shown(f))));
    lines.push(format!("Installing: {}.", new.display));
    lines
}

/// Decides what to do. `say` prints a line; `ask` shows a question and returns the index of the
/// picked item, or `None` when the prompt cannot be shown (then nothing is uninstalled).
///
/// - nothing found: proceed (with `clean_flag`, say that the flag had nothing to do);
/// - `clean_flag`: clean install, no question, interactive or not;
/// - not interactive, no flag: proceed, silently, nothing uninstalled;
/// - interactive: describe what was found and ask; the default keeps the install (for a
///   downgrade: stops, so the newer version is not replaced by accident).
pub fn plan(
    found: &[Found],
    new: &NewVersion,
    clean_flag: bool,
    interactive: bool,
    say: &mut dyn FnMut(&str),
    ask: &mut dyn FnMut(&Question) -> Option<usize>,
) -> Plan {
    if found.is_empty() {
        if clean_flag {
            say("--clean-install: no existing spawn-at install was found, so there is nothing to remove.");
        }
        return Plan::Proceed;
    }
    if clean_flag {
        for l in describe(found, new) {
            say(&l);
        }
        say("--clean-install: removing it first. Your configuration and settings are kept.");
        return Plan::Clean;
    }
    if !interactive {
        return Plan::Proceed;
    }
    for l in describe(found, new) {
        say(&l);
    }
    let clean_item = (
        "Clean install (remove the old one first, keep config)".to_string(),
        Choice::Clean,
    );
    let question = match relation(&found[0], new) {
        Relation::Same => {
            say("The same version is already installed.");
            Question {
                prompt: "What do you want to do?".into(),
                items: vec![
                    ("Keep it, change nothing [Default]".into(), Choice::NoChange),
                    ("Reinstall in place".into(), Choice::Upgrade),
                    clean_item,
                ],
                default: 0,
            }
        }
        Relation::Newer => {
            say("Warning: the installed version is NEWER than this one; installing would be a downgrade.");
            Question {
                prompt: "What do you want to do?".into(),
                items: vec![
                    (
                        "Keep the newer one, change nothing [Default]".into(),
                        Choice::NoChange,
                    ),
                    ("Downgrade in place".into(), Choice::Upgrade),
                    clean_item,
                ],
                default: 0,
            }
        }
        Relation::Older | Relation::Unknown => Question {
            prompt: "What do you want to do?".into(),
            items: vec![
                ("Upgrade in place [Default]".into(), Choice::Upgrade),
                clean_item,
            ],
            default: 0,
        },
    };
    match ask(&question).and_then(|i| question.items.get(i)) {
        Some((_, Choice::NoChange)) => Plan::Stop,
        Some((_, Choice::Clean)) => Plan::Clean,
        Some((_, Choice::Upgrade)) | None => Plan::Proceed,
    }
}

/// The steps of a clean install's removal, so tests can record them.
pub trait CleanOps {
    fn remove_binary(&mut self, path: &Path) -> Result<(), String>;
    fn remove_extension(&mut self) -> Result<(), String>;
    fn remove_autostart(&mut self);
}

/// Removes the existing install: every found binary, then the extension, then the autostart
/// entry. The config file and the settings are not touched (there is no step for them).
pub fn run_clean(
    found: &[Found],
    ops: &mut dyn CleanOps,
    say: &mut dyn FnMut(&str),
) -> Result<(), String> {
    for f in found {
        ops.remove_binary(&f.path)?;
    }
    ops.remove_extension()?;
    ops.remove_autostart();
    say("Your configuration and settings were kept.");
    Ok(())
}

/// The real removal: the same code `spawn-at uninstall` runs, minus its prompts and any
/// log-out or reload.
pub struct RealCleanOps<'a> {
    pub driver: &'a dyn CompositorBackend,
    pub user_bin: Option<PathBuf>,
    pub system_bin: PathBuf,
}

impl CleanOps for RealCleanOps<'_> {
    fn remove_binary(&mut self, path: &Path) -> Result<(), String> {
        let scope = if self
            .user_bin
            .as_deref()
            .map(|d| d.join("spawn-at"))
            .as_deref()
            == Some(path)
        {
            InstallScope::User
        } else if path == self.system_bin.join("spawn-at") {
            InstallScope::System
        } else {
            // Found on PATH outside the two install locations: not ours to remove.
            crate::ui::block_start();
            println!(
                "Not removed (outside the install locations): {}",
                path.display()
            );
            return Ok(());
        };
        installer::uninstall_binary(scope)
    }

    fn remove_extension(&mut self) -> Result<(), String> {
        let mut args = UninstallArgs {
            skip_bin: true,
            headless: true,
            ..Default::default()
        };
        #[cfg(target_os = "linux")]
        {
            args.gnome.gnome_delete_files = true;
            args.gnome.no_action = true;
        }
        self.driver.uninstall(&args).map_err(|e| e.to_string())
    }

    fn remove_autostart(&mut self) {
        crate::config::uninstall_login_autostart_entry();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    fn temp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("spawn-at-upg-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    fn script(path: &Path, body: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, format!("#!/bin/sh\n{body}\n")).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
    }

    fn v(s: &str) -> Option<Version> {
        crate::update::parse_semver(s)
    }

    fn found(version: Option<&str>) -> Found {
        Found {
            path: PathBuf::from("/home/u/.local/bin/spawn-at"),
            version: version.map(String::from),
            semver: version.and_then(v),
        }
    }

    fn new_ver() -> NewVersion {
        NewVersion {
            display: "v0.2.0-beta.2".into(),
            semver: v("0.2.0-beta.2"),
        }
    }

    /// Runs `plan` and returns the plan, the lines said and the question asked (if any).
    fn run(
        found: &[Found],
        flag: bool,
        interactive: bool,
        pick: Option<usize>,
    ) -> (Plan, Vec<String>, Option<Question>) {
        let mut said = Vec::new();
        let mut asked = None;
        let p = plan(
            found,
            &new_ver(),
            flag,
            interactive,
            &mut |l| said.push(l.to_string()),
            &mut |q| {
                asked = Some(q.clone());
                pick
            },
        );
        (p, said, asked)
    }

    #[derive(Default)]
    struct Rec {
        calls: Vec<String>,
        fail_binary: bool,
    }

    impl CleanOps for Rec {
        fn remove_binary(&mut self, path: &Path) -> Result<(), String> {
            self.calls.push(format!("binary {}", path.display()));
            if self.fail_binary {
                Err("denied".into())
            } else {
                Ok(())
            }
        }
        fn remove_extension(&mut self) -> Result<(), String> {
            self.calls.push("extension".into());
            Ok(())
        }
        fn remove_autostart(&mut self) {
            self.calls.push("autostart".into());
        }
    }

    // --- finding the existing binaries ---

    /// User location, system location and PATH are all searched; each file once; the running
    /// binary is never counted, also when it is reached through a symlink.
    #[test]
    fn test_find_existing_skips_the_running_binary_and_duplicates() {
        let root = temp("find");
        let user = root.join("user");
        let sys = root.join("sys");
        let other = root.join("other");
        script(&user.join("spawn-at"), "echo hi");
        script(&sys.join("spawn-at"), "echo hi");
        script(&other.join("spawn-at"), "echo hi");
        let running = root.join("run/spawn-at");
        script(&running, "echo hi");
        let link_dir = root.join("link");
        fs::create_dir_all(&link_dir).unwrap();
        std::os::unix::fs::symlink(&running, link_dir.join("spawn-at")).unwrap();
        let path_env = std::env::join_paths([&other, &user, &link_dir, &root.join("run")]).unwrap();

        let got = find_existing(Some(&user), &sys, Some(&path_env), &running);
        assert_eq!(
            got,
            vec![
                user.join("spawn-at"),
                sys.join("spawn-at"),
                other.join("spawn-at")
            ]
        );
        let _ = fs::remove_dir_all(&root);
    }

    /// Nothing there, a directory named spawn-at, or a file that is not executable: not found.
    #[test]
    fn test_find_existing_ignores_non_executables() {
        let root = temp("none");
        fs::create_dir_all(root.join("a/spawn-at")).unwrap();
        fs::create_dir_all(root.join("b")).unwrap();
        fs::write(root.join("b/spawn-at"), "x").unwrap();
        let path_env = std::env::join_paths([root.join("a"), root.join("b")]).unwrap();
        let got = find_existing(None, &root.join("nosys"), Some(&path_env), Path::new("/x"));
        assert!(got.is_empty(), "{got:?}");
        let _ = fs::remove_dir_all(&root);
    }

    // --- asking the existing binary for its version ---

    /// `-V` output is returned; a failing binary and a hanging one give None (the hanging one
    /// is killed after the timeout, well before its 30 s sleep ends).
    #[test]
    fn test_query_version_tolerates_failure_and_timeout() {
        let root = temp("ver");
        script(&root.join("ok"), "echo 'spawn-at 0.1.0'");
        script(&root.join("bad"), "exit 3");
        script(&root.join("empty"), "exit 0");
        script(&root.join("slow"), "exec sleep 30");
        let t = Duration::from_millis(500);
        assert_eq!(
            query_version(&root.join("ok"), t).as_deref(),
            Some("spawn-at 0.1.0")
        );
        assert_eq!(query_version(&root.join("bad"), t), None);
        assert_eq!(query_version(&root.join("empty"), t), None);
        let start = Instant::now();
        assert_eq!(query_version(&root.join("slow"), t), None);
        assert!(start.elapsed() < Duration::from_secs(5));
        assert_eq!(query_version(&root.join("missing"), t), None);
        let _ = fs::remove_dir_all(&root);
    }

    /// A release prints "spawn-at <version>"; a development build has no comparable version.
    #[test]
    fn test_parse_version_output() {
        assert_eq!(
            parse_version_output("spawn-at v0.2.0-beta.2"),
            ("v0.2.0-beta.2".to_string(), v("0.2.0-beta.2"))
        );
        let (shown, ver) = parse_version_output("DEV BUILD\nlast release was v0.2.0-beta.2\n---");
        assert_eq!(shown, "dev build");
        assert_eq!(ver, None);
        assert_eq!(parse_version_output("weird").1, None);
    }

    // --- the plan ---

    /// --clean-install with an existing install: clean, no question, interactive or not, and the
    /// message says the configuration is kept.
    #[test]
    fn test_flag_with_existing_install_cleans_without_asking() {
        for interactive in [false, true] {
            let (p, said, asked) = run(&[found(Some("0.1.0"))], true, interactive, Some(0));
            assert_eq!(p, Plan::Clean);
            assert!(asked.is_none());
            assert!(said
                .iter()
                .any(|l| l.contains("configuration and settings are kept")));
            assert!(said
                .iter()
                .any(|l| l.contains("/home/u/.local/bin/spawn-at (0.1.0)")));
        }
    }

    /// --clean-install without an existing install: proceeds, and says it had nothing to do.
    #[test]
    fn test_flag_without_existing_install_is_a_noop_with_a_message() {
        let (p, said, asked) = run(&[], true, true, None);
        assert_eq!(p, Plan::Proceed);
        assert!(asked.is_none());
        assert_eq!(said.len(), 1);
        assert!(said[0].contains("nothing to remove"), "{said:?}");
        // no flag, nothing found: silent
        let (p, said, _) = run(&[], false, true, None);
        assert_eq!((p, said.len()), (Plan::Proceed, 0));
    }

    /// No flag, not interactive: nothing is asked, nothing said, nothing removed.
    #[test]
    fn test_no_flag_non_interactive_proceeds_silently() {
        let (p, said, asked) = run(&[found(Some("0.1.0"))], false, false, Some(1));
        assert_eq!(p, Plan::Proceed);
        assert!(said.is_empty() && asked.is_none());
    }

    /// No flag, interactive, older version: the question is shown, the default (index 0) keeps
    /// and upgrades in place, the second item is the clean install.
    #[test]
    fn test_interactive_older_version_asks_and_defaults_to_keep() {
        let (p, said, asked) = run(&[found(Some("0.1.0"))], false, true, Some(0));
        assert_eq!(p, Plan::Proceed);
        let q = asked.expect("the prompt is shown");
        assert_eq!(q.default, 0);
        assert_eq!(q.items[0].1, Choice::Upgrade);
        assert_eq!(q.items[1].1, Choice::Clean);
        assert!(said
            .iter()
            .any(|l| l.contains("Installing: v0.2.0-beta.2.")));
        let (p, _, _) = run(&[found(Some("0.1.0"))], false, true, Some(1));
        assert_eq!(p, Plan::Clean);
    }

    /// A prompt that cannot be shown (None) never uninstalls: it behaves as today.
    #[test]
    fn test_prompt_failure_proceeds_without_uninstalling() {
        for ver in [Some("0.1.0"), Some("0.2.0-beta.2"), Some("9.0.0"), None] {
            let (p, _, asked) = run(&[found(ver)], false, true, None);
            assert!(asked.is_some());
            assert_eq!(p, Plan::Proceed, "{ver:?}");
        }
    }

    /// Same version: says so; the default changes nothing; "reinstall" proceeds.
    #[test]
    fn test_same_version_defaults_to_no_change() {
        let (p, said, asked) = run(&[found(Some("v0.2.0-beta.2"))], false, true, Some(0));
        assert_eq!(p, Plan::Stop);
        assert!(said
            .iter()
            .any(|l| l.contains("same version is already installed")));
        let q = asked.unwrap();
        assert_eq!(
            (q.default, q.items[0].1, q.items[1].1),
            (0, Choice::NoChange, Choice::Upgrade)
        );
        let (p, _, _) = run(&[found(Some("0.2.0-beta.2"))], false, true, Some(1));
        assert_eq!(p, Plan::Proceed);
    }

    /// A newer installed version: a downgrade warning, and the default keeps the newer one.
    #[test]
    fn test_newer_installed_version_warns_and_defaults_to_stop() {
        let (p, said, asked) = run(&[found(Some("0.3.0"))], false, true, Some(0));
        assert_eq!(p, Plan::Stop);
        assert!(said.iter().any(|l| l.contains("downgrade")), "{said:?}");
        assert_eq!(asked.unwrap().items[1].1, Choice::Upgrade);
    }

    /// A pre-release counts as older than the release, a development build as unknown (the
    /// ordinary upgrade question, no downgrade warning).
    #[test]
    fn test_prerelease_and_unknown_versions() {
        let (_, said, _) = run(&[found(Some("0.2.0-beta.1"))], false, true, Some(0));
        assert!(!said
            .iter()
            .any(|l| l.contains("downgrade") || l.contains("same version")));
        let (_, said, _) = run(&[found(None)], false, true, Some(0));
        assert!(said.iter().any(|l| l.contains("version unknown")));
        assert!(!said.iter().any(|l| l.contains("downgrade")));
    }

    /// The interactive question is one short line per item: a long item wraps on an 80-column
    /// terminal and dialoguer, which counts unwrapped lines when it redraws, then leaves a copy
    /// of the prompt behind (the "What do you want to do?" printed twice).
    #[test]
    fn test_question_lines_fit_an_80_column_terminal() {
        for ver in [Some("0.1.0"), Some("0.2.0-beta.2"), Some("9.0.0"), None] {
            let (_, _, asked) = run(&[found(ver)], false, true, None);
            let q = asked.unwrap();
            assert!(q.prompt.len() <= 60, "{}", q.prompt);
            for (label, _) in &q.items {
                assert!(label.len() <= 60, "{label}: {}", label.len());
            }
        }
    }

    /// One clear line for a single existing binary, including a development build; the old
    /// "(a development build)" / "This install is dev." wording is gone.
    #[test]
    fn test_describe_is_one_clear_line() {
        let dev = NewVersion {
            display: "dev build".into(),
            semver: None,
        };
        let existing = Found {
            path: PathBuf::from("/p/spawn-at"),
            version: Some("dev build".into()),
            semver: None,
        };
        assert_eq!(
            describe(std::slice::from_ref(&existing), &dev),
            vec!["Existing: /p/spawn-at (dev build). Installing: dev build."]
        );
        let two = describe(&[existing.clone(), found(Some("0.1.0"))], &new_ver());
        assert_eq!(two[0], "Existing:");
        assert_eq!(two.last().unwrap(), "Installing: v0.2.0-beta.2.");
    }

    // --- the removal ---

    /// The clean install removes every binary, then the extension, then the autostart entry, in
    /// that order, and says the configuration was kept.
    #[test]
    fn test_run_clean_order_and_message() {
        let mut ops = Rec::default();
        let mut said = Vec::new();
        let f = [
            found(Some("0.1.0")),
            Found {
                path: "/usr/local/bin/spawn-at".into(),
                ..found(None)
            },
        ];
        run_clean(&f, &mut ops, &mut |l| said.push(l.to_string())).unwrap();
        assert_eq!(
            ops.calls,
            vec![
                "binary /home/u/.local/bin/spawn-at",
                "binary /usr/local/bin/spawn-at",
                "extension",
                "autostart"
            ]
        );
        assert_eq!(said, vec!["Your configuration and settings were kept."]);
    }

    /// A failed binary removal stops the clean install before the extension is touched and
    /// says nothing about the configuration.
    #[test]
    fn test_run_clean_stops_on_the_first_failure() {
        let mut ops = Rec {
            fail_binary: true,
            ..Default::default()
        };
        let mut said = Vec::new();
        let r = run_clean(&[found(Some("0.1.0"))], &mut ops, &mut |l| {
            said.push(l.to_string())
        });
        assert_eq!(r, Err("denied".to_string()));
        assert_eq!(ops.calls.len(), 1);
        assert!(said.is_empty());
    }

    /// Files-on-disk version: the removal deletes the binary and leaves a config file next to
    /// it untouched. A stand-in for the real operations, in a temporary directory.
    #[test]
    fn test_clean_removal_leaves_the_config_file() {
        struct FsOps(PathBuf);
        impl CleanOps for FsOps {
            fn remove_binary(&mut self, p: &Path) -> Result<(), String> {
                fs::remove_file(p).map_err(|e| e.to_string())
            }
            fn remove_extension(&mut self) -> Result<(), String> {
                fs::remove_dir_all(self.0.join("ext")).map_err(|e| e.to_string())
            }
            fn remove_autostart(&mut self) {
                let _ = fs::remove_file(self.0.join("autostart.desktop"));
            }
        }
        let root = temp("fs");
        script(&root.join("bin/spawn-at"), "echo hi");
        fs::create_dir_all(root.join("ext")).unwrap();
        fs::write(root.join("ext/extension.js"), "x").unwrap();
        fs::write(root.join("autostart.desktop"), "x").unwrap();
        fs::write(root.join("config.toml"), "[update]\nchannel = \"beta\"\n").unwrap();
        let f = Found {
            path: root.join("bin/spawn-at"),
            version: None,
            semver: None,
        };
        run_clean(&[f], &mut FsOps(root.clone()), &mut |_| {}).unwrap();
        assert!(!root.join("bin/spawn-at").exists());
        assert!(!root.join("ext").exists());
        assert!(!root.join("autostart.desktop").exists());
        assert_eq!(
            fs::read_to_string(root.join("config.toml")).unwrap(),
            "[update]\nchannel = \"beta\"\n"
        );
        let _ = fs::remove_dir_all(&root);
    }
}
