//! Control of the GNOME extension's timing log over session D-Bus.

use std::process::Command;

/// Invokes `SetLogging` on the GNOME Shell extension via session D-Bus.
pub fn set_extension_logging(enabled: bool) -> Result<(), String> {
    let output = Command::new("gdbus")
        .args([
            "call",
            "--session",
            "--dest",
            "org.gnome.Shell",
            "--object-path",
            "/org/gnome/Shell/Extensions/SpawnAt",
            "--method",
            "org.gnome.Shell.Extensions.SpawnAt.SetLogging",
            if enabled { "true" } else { "false" },
        ])
        .output();

    match output {
        Ok(out) if out.status.success() => Ok(()),
        Ok(out) => Err(format!(
            "gdbus returned non-zero exit code: {:?}, stderr: {}",
            out.status.code(),
            String::from_utf8_lossy(&out.stderr)
        )),
        Err(e) => Err(format!("Failed to execute gdbus: {}", e)),
    }
}
