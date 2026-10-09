//! # OS-Level Binary Installer & Uninstaller
//!
//! Handles copying and removing the `spawn-at` binary to/from directories in `$PATH`
//! for user and system installation scopes across operating systems.

use crate::platform::InstallScope;
use std::fs;
use std::io;
use std::path::PathBuf;

/// Stubs and documentation for macOS binary installation:
/// - User Scope: `~/.local/bin/spawn-at`
/// - System Scope: `/usr/local/bin/spawn-at`
#[allow(dead_code)]
fn macos_destination_info(scope: InstallScope) -> &'static str {
    match scope {
        InstallScope::User => "~/.local/bin/spawn-at",
        InstallScope::System => "/usr/local/bin/spawn-at",
    }
}

/// Stubs and documentation for Windows binary installation:
/// - User Scope: `%LOCALAPPDATA%\Programs\spawn-at\spawn-at.exe` (requires modifying HKCU\Environment registry key for PATH)
/// - System Scope: `%ProgramFiles%\spawn-at\spawn-at.exe` (requires modifying HKLM\SYSTEM\CurrentControlSet\Control\Session Manager\Environment for PATH)
#[allow(dead_code)]
fn windows_destination_info(scope: InstallScope) -> &'static str {
    match scope {
        InstallScope::User => r"%LOCALAPPDATA%\Programs\spawn-at\spawn-at.exe",
        InstallScope::System => r"%ProgramFiles%\spawn-at\spawn-at.exe",
    }
}

/// Returns target installation directory for Linux:
/// - User: `$HOME/.local/bin`
/// - System: `/usr/local/bin`
fn get_target_dir(scope: InstallScope) -> Result<PathBuf, String> {
    match scope {
        InstallScope::User => {
            let home = std::env::var("HOME")
                .map_err(|_| "Failed to determine $HOME directory for binary installation.".to_string())?;
            Ok(PathBuf::from(home).join(".local/bin"))
        }
        InstallScope::System => Ok(PathBuf::from("/usr/local/bin")),
    }
}

/// Checks if `dir` is present in the current process's `$PATH`.
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

/// Installs the currently running executable to the target scope directory (`~/.local/bin` or `/usr/local/bin`).
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

    println!("\x1b[1;36m=== OS-Level Binary Installation ===\x1b[0m");
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
        println!("Source and destination are identical ({}), skipping file copy.", dest_file.display());
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
                println!("Successfully copied binary via elevated privileges to: {}", dest_file.display());
            }
            Err(e) => return Err(format!("Failed to copy binary to '{}': {}", dest_file.display(), e)),
        }
    }

    if !is_in_path(&target_dir) {
        crate::diagnostics::render_warning(&format!("'{}' is not currently in your $PATH.", target_dir.display()));
        println!("To run 'spawn-at' globally, add it to your shell startup file:");
        println!("  echo 'export PATH=\"{}:$PATH\"' >> ~/.bashrc", target_dir.display());
        println!("  (or ~/.zshrc / ~/.profile depending on your shell)");
    } else {
        println!("Verified: '{}' is in $PATH.", target_dir.display());
    }

    Ok(dest_file)
}

/// Removes the `spawn-at` binary from the specified target scope directory.
pub fn uninstall_binary(scope: InstallScope) -> Result<(), String> {
    if !cfg!(target_os = "linux") {
        return Err(format!(
            "Binary uninstallation is currently only supported on Linux (Current OS: {}).",
            std::env::consts::OS
        ));
    }

    let target_dir = get_target_dir(scope)?;
    let dest_file = target_dir.join("spawn-at");

    println!("\x1b[1;36m=== OS-Level Binary Uninstallation ===\x1b[0m");
    println!("Scope: {:?}", scope);
    println!("Target path: {}", dest_file.display());

    if dest_file.exists() {
        match fs::remove_file(&dest_file) {
            Ok(_) => println!("Successfully removed binary: {}", dest_file.display()),
            Err(e) if e.kind() == io::ErrorKind::PermissionDenied => {
                crate::platform::escalate::remove_elevated(&dest_file)?;
                println!("Successfully removed binary via elevated privileges: {}", dest_file.display());
            }
            Err(e) => return Err(format!("Failed to remove binary '{}': {}", dest_file.display(), e)),
        }
    } else {
        println!("Binary not found at '{}', skipping removal.", dest_file.display());
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
        assert_eq!(macos_destination_info(InstallScope::User), "~/.local/bin/spawn-at");
        assert_eq!(macos_destination_info(InstallScope::System), "/usr/local/bin/spawn-at");
        assert!(windows_destination_info(InstallScope::User).contains("LOCALAPPDATA"));
        assert!(windows_destination_info(InstallScope::System).contains("ProgramFiles"));
    }
}
