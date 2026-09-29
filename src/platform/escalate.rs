//! # Platform Privilege Escalation
//!
//! Provides reactive elevation helpers (such as `sudo` on Linux/macOS or UAC on Windows)
//! invoked only when standard filesystem operations fail with `PermissionDenied`.

use std::path::Path;
use std::process::Command;

/// Attempts to execute a file copy with elevated privileges appropriate for the host OS.
pub fn copy_elevated(src: &Path, dest: &Path) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        println!("\x1b[1;33mPermission denied.\x1b[0m Requesting administrative privileges via sudo...");
        let status = Command::new("sudo")
            .args(["install", "-D", "-m", "755", src.to_str().unwrap(), dest.to_str().unwrap()])
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
            .args(["install", "-m", "755", src.to_str().unwrap(), dest.to_str().unwrap()])
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

/// Attempts to remove a file with elevated privileges.
pub fn remove_elevated(dest: &Path) -> Result<(), String> {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        println!("\x1b[1;33mPermission denied.\x1b[0m Requesting administrative privileges via sudo...");
        let status = Command::new("sudo")
            .args(["rm", "-f", dest.to_str().unwrap()])
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
