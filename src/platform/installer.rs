//! Copies the running `spawn-at` executable into the bin directory of an [`InstallScope`] and
//! removes it again. Only the binary: the extension and the config are other modules' business.
//!
//! The directory comes from `platform::host().bin_dir(scope)`; this module hardcodes none. Both
//! functions refuse with an error string on any OS but Linux (`cfg!(target_os = "linux")`), print
//! their progress to stdout, and fall back to `sudo` (see `platform::escalate`) only when the
//! plain copy or removal fails with `PermissionDenied`.

use crate::platform::InstallScope;
use std::fs;
use std::io;
use std::path::PathBuf;

/// The macOS install path as text: `~/.local/bin/spawn-at` for user scope, `/usr/local/bin/spawn-at`
/// for system scope. Not used by the install code (that is Linux only); only the test below calls
/// it, which is why it has `allow(dead_code)`.
#[allow(dead_code)]
fn macos_destination_info(scope: InstallScope) -> &'static str {
    match scope {
        InstallScope::User => "~/.local/bin/spawn-at",
        InstallScope::System => "/usr/local/bin/spawn-at",
    }
}

/// The Windows install path as text: `%LOCALAPPDATA%\Programs\spawn-at\spawn-at.exe` for user
/// scope, `%ProgramFiles%\spawn-at\spawn-at.exe` for system scope. Not used by the install code
/// (that is Linux only); only the test below calls it. Nothing in this file edits PATH or the
/// registry.
#[allow(dead_code)]
fn windows_destination_info(scope: InstallScope) -> &'static str {
    match scope {
        InstallScope::User => r"%LOCALAPPDATA%\Programs\spawn-at\spawn-at.exe",
        InstallScope::System => r"%ProgramFiles%\spawn-at\spawn-at.exe",
    }
}

/// Returns target installation directory (Linux: `$HOME/.local/bin` or `/usr/local/bin`); the
/// values belong to the OS, see `platform::host`.
fn get_target_dir(scope: InstallScope) -> Result<PathBuf, String> {
    crate::platform::host().bin_dir(scope)
}

/// The path `install_binary(scope)` puts the binary at: the scope's bin directory plus
/// `spawn-at`. Absolute when the host's directory is (Linux: yes).
///
/// Errors: the host's message when the directory cannot be determined (Linux, user scope: `HOME`
/// unset or not UTF-8). Touches nothing on disk.
pub fn installed_binary_path(scope: InstallScope) -> Result<PathBuf, String> {
    Ok(get_target_dir(scope)?.join("spawn-at"))
}

/// The binary the login update-check entry must run.
///
/// Without `--skip-bin` that is the binary this run installed (`installed(scope)`). With
/// `--skip-bin` the running binary is not necessarily the one that stays: `spawn-at update` runs
/// the new binary from a temp directory as `install --skip-bin --headless` and deletes that
/// directory afterwards. So the installed copy is used if there is one: the `scope` path first,
/// then the other scope's. Only when neither is a file does it fall back to `current_exe`.
///
/// `installed`, `is_file` and `current_exe` are passed in so a test needs no real home directory.
pub fn login_entry_exe(
    skip_bin: bool,
    scope: InstallScope,
    installed: impl Fn(InstallScope) -> Result<PathBuf, String>,
    is_file: impl Fn(&std::path::Path) -> bool,
    current_exe: impl FnOnce() -> Result<PathBuf, String>,
) -> Result<PathBuf, String> {
    if !skip_bin {
        return installed(scope);
    }
    let other = match scope {
        InstallScope::User => InstallScope::System,
        InstallScope::System => InstallScope::User,
    };
    [scope, other]
        .into_iter()
        .filter_map(|s| installed(s).ok())
        .find(|p| is_file(p))
        .map_or_else(current_exe, Ok)
}

/// True if `dir` equals an entry of this process's `PATH` as written, or after both are
/// canonicalized. False when `PATH` is unset.
fn is_in_path(dir: &PathBuf) -> bool {
    let path_env = match std::env::var_os("PATH") {
        Some(val) => val,
        None => return false,
    };

    let dir_canon = dir.canonicalize().ok();

    std::env::split_paths(&path_env).any(|p| {
        if &p == dir {
            return true;
        }
        if let (Some(ref target_canon), Ok(p_canon)) = (&dir_canon, p.canonicalize()) {
            if &p_canon == target_canon {
                return true;
            }
        }
        false
    })
}

/// Quotes `dir` for POSIX sh. Returns `(text, plain)`: when every character is one of
/// `A-Z a-z 0-9 _ . / - + @ % = , :` the text is `dir` unchanged and `plain` is true (safe inside
/// double quotes); otherwise it is `dir` in single quotes with each `'` written as `'\''` and
/// `plain` is false.
fn sh_quote_dir(dir: &str) -> (String, bool) {
    let plain = !dir.is_empty()
        && dir
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "_./-+@%=,:".contains(c));
    if plain {
        (dir.to_string(), true)
    } else {
        (format!("'{}'", dir.replace('\'', "'\\''")), false)
    }
}

/// The text printed when `dir` is not in `PATH`: how to use it in this terminal and a guarded
/// line to put in a startup file by hand. Pure; nothing is written anywhere.
///
/// When `dir` is `<home>/.local/bin` it first says that `~/.profile` adds that directory once it
/// exists on Debian and Ubuntu, so logging in again is usually enough. The guarded line is
/// `case ":$PATH:" in ... esac` (POSIX sh, also valid in bash and zsh): it adds `dir` only when
/// `PATH` does not already hold it, so running it twice or in nested shells adds it once. A `dir`
/// with characters outside the plain set (see `sh_quote_dir`) is single-quoted. The text has no
/// `>>` append command.
fn path_advice(dir: &std::path::Path, home: Option<&std::path::Path>) -> String {
    let shown = dir.to_string_lossy();
    let (q, plain) = sh_quote_dir(&shown);
    let (export, guard) = if plain {
        (
            format!("export PATH=\"{q}:$PATH\""),
            format!("case \":$PATH:\" in *\":{q}:\"*) ;; *) export PATH=\"{q}:$PATH\" ;; esac"),
        )
    } else {
        (
            format!("export PATH={q}:\"$PATH\""),
            format!("case \":$PATH:\" in *\":\"{q}\":\"*) ;; *) export PATH={q}:\"$PATH\" ;; esac"),
        )
    };
    let mut out = String::new();
    if home.is_some_and(|h| dir == h.join(".local").join("bin")) {
        out.push_str(
            "On Debian and Ubuntu, ~/.profile adds this directory once it exists, so logging out\n\
             and back in (or running '. ~/.profile') is usually enough.\n",
        );
    }
    out.push_str(&format!("For this terminal only:\n  {export}\n"));
    out.push_str("To make it permanent in any shell, add this line to ~/.profile, ~/.bashrc or\n");
    out.push_str(&format!(
        "~/.zshrc with an editor (safe to run more than once):\n  {guard}"
    ));
    out
}

/// Copies the running executable (`current_exe`) to `<bin dir>/spawn-at` and returns that path.
///
/// Creates the directory if missing; skips the copy when source and destination are the same
/// file (compared after canonicalizing); sets mode 0755 after a plain copy (a failure to set it
/// is ignored). When the directory is not in `PATH` it only prints a warning and the `path_advice`
/// text; it never edits a shell file.
///
/// Errors: a message string on a non-Linux OS, when the executable or the directory cannot be
/// found or created, when the copy fails for a reason other than `PermissionDenied`, or when
/// the `sudo` fallback fails. A `PermissionDenied` while creating the directory is not an error
/// by itself (the copy then fails or falls back to `sudo`).
pub fn install_binary(scope: InstallScope) -> Result<PathBuf, String> {
    if !cfg!(target_os = "linux") {
        return Err(format!(
            "Binary installation is currently only supported on Linux (Current OS: {}).",
            std::env::consts::OS
        ));
    }

    let src = std::env::current_exe()
        .map_err(|e| format!("Failed to locate current running executable: {}", e))?;

    let target_dir = get_target_dir(scope)?;
    let dest_file = target_dir.join("spawn-at");

    crate::ui::section("OS-Level Binary Installation");
    println!("Scope: {:?}", scope);
    println!("Source: {}", src.display());
    println!("Destination: {}", dest_file.display());

    if let Err(e) = fs::create_dir_all(&target_dir) {
        if e.kind() != io::ErrorKind::PermissionDenied {
            return Err(format!(
                "Failed to create target directory '{}': {}",
                target_dir.display(),
                e
            ));
        }
    }

    let same_file = match (src.canonicalize(), dest_file.canonicalize()) {
        (Ok(s), Ok(d)) => s == d,
        _ => false,
    };

    if same_file {
        println!(
            "Source and destination are identical ({}), skipping file copy.",
            dest_file.display()
        );
    } else {
        match fs::copy(&src, &dest_file) {
            Ok(_) => {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    if let Ok(metadata) = fs::metadata(&dest_file) {
                        let mut perms = metadata.permissions();
                        perms.set_mode(0o755);
                        let _ = fs::set_permissions(&dest_file, perms);
                    }
                }
                println!("Successfully copied binary to: {}", dest_file.display());
            }
            Err(e) if e.kind() == io::ErrorKind::PermissionDenied => {
                crate::platform::escalate::copy_elevated(&src, &dest_file)?;
                println!(
                    "Successfully copied binary via elevated privileges to: {}",
                    dest_file.display()
                );
            }
            Err(e) => {
                return Err(format!(
                    "Failed to copy binary to '{}': {}",
                    dest_file.display(),
                    e
                ))
            }
        }
    }

    if !is_in_path(&target_dir) {
        crate::diagnostics::render_warning(&format!(
            "'{}' is not currently in your $PATH.",
            target_dir.display()
        ));
        let home = std::env::var_os("HOME").map(PathBuf::from);
        println!("{}", path_advice(&target_dir, home.as_deref()));
    } else {
        println!("Verified: '{}' is in $PATH.", target_dir.display());
    }

    Ok(dest_file)
}

/// Removes `<bin dir>/spawn-at` for `scope`. A missing file is not an error: it prints that the
/// binary was not found and returns `Ok(())`.
///
/// Errors: a message string on a non-Linux OS, when the directory cannot be determined, or when
/// the removal fails (after the `sudo` fallback for `PermissionDenied`).
pub fn uninstall_binary(scope: InstallScope) -> Result<(), String> {
    if !cfg!(target_os = "linux") {
        return Err(format!(
            "Binary uninstallation is currently only supported on Linux (Current OS: {}).",
            std::env::consts::OS
        ));
    }

    let target_dir = get_target_dir(scope)?;
    let dest_file = target_dir.join("spawn-at");

    crate::ui::section("OS-Level Binary Uninstallation");
    println!("Scope: {:?}", scope);
    println!("Target path: {}", dest_file.display());

    if dest_file.exists() {
        match fs::remove_file(&dest_file) {
            Ok(_) => println!("Successfully removed binary: {}", dest_file.display()),
            Err(e) if e.kind() == io::ErrorKind::PermissionDenied => {
                crate::platform::escalate::remove_elevated(&dest_file)?;
                println!(
                    "Successfully removed binary via elevated privileges: {}",
                    dest_file.display()
                );
            }
            Err(e) => {
                return Err(format!(
                    "Failed to remove binary '{}': {}",
                    dest_file.display(),
                    e
                ))
            }
        }
    } else {
        println!(
            "Binary not found at '{}', skipping removal.",
            dest_file.display()
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_linux_target_dirs() {
        let user_dir = get_target_dir(InstallScope::User);
        assert!(user_dir.is_ok());
        assert!(user_dir.unwrap().ends_with(".local/bin"));

        let sys_dir = get_target_dir(InstallScope::System).unwrap();
        assert_eq!(sys_dir, PathBuf::from("/usr/local/bin"));
    }

    use std::path::Path;

    #[test]
    fn advice_has_guarded_line_with_exact_dir() {
        let a = path_advice(Path::new("/home/x/.local/bin"), Some(Path::new("/home/x")));
        assert!(a.contains(
            "case \":$PATH:\" in *\":/home/x/.local/bin:\"*) ;; *) export PATH=\"/home/x/.local/bin:$PATH\" ;; esac"
        ));
        assert!(a.contains("  export PATH=\"/home/x/.local/bin:$PATH\"\n"));
    }

    #[test]
    fn advice_never_appends_to_a_file() {
        for dir in ["/home/x/.local/bin", "/opt/my tools/bin", "/usr/local/bin"] {
            let a = path_advice(Path::new(dir), Some(Path::new("/home/x")));
            assert!(!a.contains(">>"), "{a}");
        }
    }

    #[test]
    fn advice_logout_hint_only_for_home_local_bin() {
        let home = Some(Path::new("/home/x"));
        assert!(path_advice(Path::new("/home/x/.local/bin"), home).contains("~/.profile adds"));
        assert!(!path_advice(Path::new("/usr/local/bin"), home).contains("~/.profile adds"));
        assert!(!path_advice(Path::new("/home/y/.local/bin"), home).contains("~/.profile adds"));
        assert!(!path_advice(Path::new("/home/x/.local/bin"), None).contains("~/.profile adds"));
    }

    #[test]
    fn advice_quotes_dirs_with_spaces_and_quotes() {
        let a = path_advice(Path::new("/opt/my tools/bin"), None);
        assert!(
            a.contains("export PATH='/opt/my tools/bin':\"$PATH\""),
            "{a}"
        );
        assert!(a.contains("*\":\"'/opt/my tools/bin'\":\"*"), "{a}");
        let b = path_advice(Path::new("/o'p/$x"), None);
        assert!(b.contains("'/o'\\''p/$x'"), "{b}");
    }

    /// Stand-ins for the two scopes' install paths and the updater's temp directory.
    fn fake_installed(s: InstallScope) -> Result<PathBuf, String> {
        Ok(match s {
            InstallScope::User => PathBuf::from("/home/u/.local/bin/spawn-at"),
            InstallScope::System => PathBuf::from("/usr/local/bin/spawn-at"),
        })
    }
    const TMP_EXE: &str = "/tmp/spawn-at-update-v0.2.1-4242/spawn-at";

    /// `spawn-at update` on GNOME runs `install --skip-bin --headless` from the temp directory.
    /// With an installed copy the entry's `Exec=` names that copy, never the temp path (the
    /// updater deletes the directory right after), in either scope.
    #[test]
    fn test_login_entry_skip_bin_uses_installed_copy_not_temp_dir() {
        for (on_disk, want) in [
            ("/home/u/.local/bin/spawn-at", "/home/u/.local/bin/spawn-at"),
            ("/usr/local/bin/spawn-at", "/usr/local/bin/spawn-at"),
        ] {
            let exe = login_entry_exe(
                true,
                InstallScope::User,
                fake_installed,
                |p| p == Path::new(on_disk),
                || Ok(PathBuf::from(TMP_EXE)),
            )
            .unwrap();
            assert_eq!(exe, PathBuf::from(want));
            let entry = crate::config::autostart_entry_content(&exe);
            assert!(!entry.contains("spawn-at-update-"), "{entry}");
            assert!(!entry.contains("/tmp/"), "{entry}");
            assert!(entry.contains(&format!("\nExec={want} update --check --headless\n")));
        }
    }

    /// With both copies present the requested scope wins; with none, `current_exe` is the
    /// fallback (what `install --skip-bin` did before); without `--skip-bin` the installed path
    /// is used whether or not the file exists yet, and an error from it is passed on.
    #[test]
    fn test_login_entry_exe_order_and_fallbacks() {
        let cur = || Ok(PathBuf::from(TMP_EXE));
        let both = login_entry_exe(true, InstallScope::System, fake_installed, |_| true, cur);
        assert_eq!(both.unwrap(), PathBuf::from("/usr/local/bin/spawn-at"));
        let none = login_entry_exe(true, InstallScope::User, fake_installed, |_| false, cur);
        assert_eq!(none.unwrap(), PathBuf::from(TMP_EXE));
        let noskip = login_entry_exe(false, InstallScope::User, fake_installed, |_| false, cur);
        assert_eq!(
            noskip.unwrap(),
            PathBuf::from("/home/u/.local/bin/spawn-at")
        );
        let err = login_entry_exe(
            false,
            InstallScope::User,
            |_| Err("no HOME".to_string()),
            |_| true,
            cur,
        );
        assert_eq!(err.unwrap_err(), "no HOME");
        // --skip-bin where the installed paths cannot even be worked out: current_exe.
        let unknown = login_entry_exe(
            true,
            InstallScope::User,
            |_| Err("no HOME".to_string()),
            |_| true,
            cur,
        );
        assert_eq!(unknown.unwrap(), PathBuf::from(TMP_EXE));
    }

    #[test]
    fn test_macos_windows_stubs() {
        assert_eq!(
            macos_destination_info(InstallScope::User),
            "~/.local/bin/spawn-at"
        );
        assert_eq!(
            macos_destination_info(InstallScope::System),
            "/usr/local/bin/spawn-at"
        );
        assert!(windows_destination_info(InstallScope::User).contains("LOCALAPPDATA"));
        assert!(windows_destination_info(InstallScope::System).contains("ProgramFiles"));
    }
}
