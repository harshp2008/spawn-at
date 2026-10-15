//! Detection of the GNOME Shell version and of a Wayland session.

use std::process::Command;

// ============================================================================
// 2. Session & Desktop Detection
// ============================================================================

/// Detects the GNOME Shell major version (e.g. 42, 43, 44, 45, 46, etc.).
pub fn detect_gnome_shell_major_version() -> Option<u32> {
    let output = Command::new("gnome-shell").arg("--version").output().ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_gnome_shell_major_version(&stdout)
}

pub fn parse_gnome_shell_major_version(version_str: &str) -> Option<u32> {
    for part in version_str.split_whitespace() {
        if let Some(first_num) = part.split('.').next() {
            if let Ok(ver) = first_num.parse::<u32>() {
                if (40..=60).contains(&ver) {
                    return Some(ver);
                }
            }
        }
    }
    None
}

/// Resolves session identifier to pass into `loginctl show-session <id> -p Type --value`.
fn get_session_id() -> Option<String> {
    if let Ok(id) = std::env::var("XDG_SESSION_ID") {
        let trimmed = id.trim().to_string();
        if !trimmed.is_empty() {
            return Some(trimmed);
        }
    }

    if let Ok(uid_out) = Command::new("id").arg("-u").output() {
        let uid = String::from_utf8_lossy(&uid_out.stdout).trim().to_string();
        if !uid.is_empty() {
            if let Ok(display_out) = Command::new("loginctl")
                .args(["show-user", &uid, "-p", "Display", "--value"])
                .output()
            {
                let sess = String::from_utf8_lossy(&display_out.stdout)
                    .trim()
                    .to_string();
                if !sess.is_empty() {
                    return Some(sess);
                }
            }
        }
    }

    if let Ok(out) = Command::new("loginctl")
        .args(["list-sessions", "--no-legend"])
        .output()
    {
        let text = String::from_utf8_lossy(&out.stdout);
        for line in text.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if let Some(first) = parts.first() {
                if !first.is_empty() {
                    return Some((*first).to_string());
                }
            }
        }
    }

    None
}

/// Returns true if the active desktop session is running on Wayland.
///
/// Decides X11 vs Wayland with `loginctl show-session <id> -p Type --value`,
/// NOT `$XDG_SESSION_TYPE` (which has been unreliable).
pub fn is_wayland_session() -> bool {
    if let Some(id) = get_session_id() {
        if let Ok(output) = Command::new("loginctl")
            .args(["show-session", &id, "-p", "Type", "--value"])
            .output()
        {
            let sess_type = String::from_utf8_lossy(&output.stdout)
                .trim()
                .to_lowercase();
            if sess_type == "wayland" {
                return true;
            } else if sess_type == "x11" {
                return false;
            }
        }
    }

    if let Ok(output) = Command::new("loginctl")
        .args(["show-session", "auto", "-p", "Type", "--value"])
        .output()
    {
        let sess_type = String::from_utf8_lossy(&output.stdout)
            .trim()
            .to_lowercase();
        if sess_type == "wayland" {
            return true;
        } else if sess_type == "x11" {
            return false;
        }
    }

    std::env::var("WAYLAND_DISPLAY").is_ok()
}
