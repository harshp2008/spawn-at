//! Copying or removing a file through `sudo` after a plain attempt was refused.
//!
//! Linux and macOS run `sudo` as a child process with the terminal attached, so it may prompt
//! for a password; on Windows both helpers return an error saying it is not implemented, and on
//! any other target one saying it is unsupported. It must not be used as a first attempt: the
//! helpers print a "Permission denied" notice themselves and do not try the plain operation.
// Callers at this commit: src/platform/installer.rs, only in the `PermissionDenied` arm after
// `fs::copy` or `fs::remove_file` failed.

use std::path::Path;
use std::process::Command;

/// Copies `src` to `dest` through `sudo`.
///
/// Linux: `sudo install -D -m 755 src dest` (creates missing parent directories of `dest`).
/// macOS: `sudo install -m 755 src dest` (no `-D`, so `dest`'s directory must exist).
/// Prints a "Permission denied" notice to stdout first.
///
/// Errors: a message string if `sudo` cannot be started, if it exits non-zero ("Elevation was
/// rejected or failed."), and always on Windows and on targets other than Linux, macOS and
/// Windows.
pub fn copy_elevated(src: &Path, dest: &Path) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        println!(
            "\x1b[1;33mPermission denied.\x1b[0m Requesting administrative privileges via sudo..."
        );
        let status = Command::new("sudo")
            .args(["install", "-D", "-m", "755"])
            .arg(src)
            .arg(dest)
            .status()
            .map_err(|e| format!("Failed to invoke sudo: {e}"))?;

        if status.success() {
            Ok(())
        } else {
            Err("Elevation was rejected or failed.".to_string())
        }
    }

    #[cfg(target_os = "macos")]
    {
        // macOS: try sudo in terminal, or osascript fallback for GUI
        println!("\x1b[1;33mPermission denied.\x1b[0m Requesting administrative privileges...");
        let status = Command::new("sudo")
            .args(["install", "-m", "755"])
            .arg(src)
            .arg(dest)
            .status()
            .map_err(|e| format!("Failed to invoke sudo: {e}"))?;

        if status.success() {
            Ok(())
        } else {
            Err("Elevation was rejected or failed.".to_string())
        }
    }

    #[cfg(target_os = "windows")]
    {
        // Windows: PowerShell Start-Process -Verb RunAs (triggers UAC)
        let _ = (src, dest);
        Err("Automatic elevation for Windows is not yet implemented.".to_string())
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        let _ = (src, dest);
        Err("Elevation is not supported on this platform.".to_string())
    }
}

/// Removes `dest` through `sudo rm -f`, after printing the same "Permission denied" notice.
///
/// Errors: as [`copy_elevated`]. `rm -f` succeeds when the file is already gone.
pub fn remove_elevated(dest: &Path) -> Result<(), String> {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        println!(
            "\x1b[1;33mPermission denied.\x1b[0m Requesting administrative privileges via sudo..."
        );
        let status = Command::new("sudo")
            .args(["rm", "-f"])
            .arg(dest)
            .status()
            .map_err(|e| format!("Failed to invoke sudo: {e}"))?;

        if status.success() {
            Ok(())
        } else {
            Err("Elevation was rejected or failed.".to_string())
        }
    }

    #[cfg(target_os = "windows")]
    {
        let _ = dest;
        Err("Automatic elevation for Windows is not yet implemented.".to_string())
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        let _ = dest;
        Err("Elevation is not supported on this platform.".to_string())
    }
}
