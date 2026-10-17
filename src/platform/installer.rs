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

/// Copies the running executable (`current_exe`) to `<bin dir>/spawn-at` and returns that path.
///
/// Creates the directory if missing; skips the copy when source and destination are the same
/// file (compared after canonicalizing); sets mode 0755 after a plain copy (a failure to set it
/// is ignored). Only prints a warning with `export PATH` advice, never edits a shell file, when
/// the directory is not in `PATH`.
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
        println!("To run 'spawn-at' globally, add it to your shell startup file:");
        println!(
            "  echo 'export PATH=\"{}:$PATH\"' >> ~/.bashrc",
            target_dir.display()
        );
        println!("  (or ~/.zshrc / ~/.profile depending on your shell)");
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
