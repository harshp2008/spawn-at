//! Detection of the GNOME Shell version and of a Wayland session.
//!
//! Both functions start child processes (`gnome-shell`, `id`, `loginctl`) and read the
//! environment. They are called by the install and uninstall in `deploy.rs`. A failing child is
//! treated as "no answer" and the next source is tried, never reported as an error. This
//! module does not talk to the extension and does not change anything on the machine.

use std::process::Command;

// ============================================================================
// 2. Session & Desktop Detection
// ============================================================================

/// The major version printed by `gnome-shell --version`, for example 46 for "GNOME Shell 46.0".
///
/// `None` when `gnome-shell` cannot be started, exits non-zero, or prints no number in the
/// range accepted by [`parse_gnome_shell_major_version`]. Runs a child process and waits for it.
pub fn detect_gnome_shell_major_version() -> Option<u32> {
    let output = Command::new("gnome-shell").arg("--version").output().ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_gnome_shell_major_version(&stdout)
}

/// The first whitespace-separated word of `version_str` whose text before the first `.` is an
/// integer from 40 to 60 inclusive; that integer is returned. "GNOME Shell 46.0" gives 46,
/// "unknown" gives `None`. Pure.
///
/// The 40..=60 bounds reject other numbers in the text; they are not a statement about which
/// GNOME versions work (not verified).
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

/// The logind session id for `loginctl show-session <id> -p Type --value`, from the first of
/// three sources that gives a non-empty answer: `$XDG_SESSION_ID`; the `Display` property of
/// `loginctl show-user <uid>` (uid from `id -u`); the first column of the first line of
/// `loginctl list-sessions --no-legend`. The exit status of `id` and `loginctl` is not checked,
/// only whether stdout is empty. The third source takes the first listed session, which need
/// not be this user's graphical one.
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

/// True when the session is Wayland. Order of the checks:
///
/// 1. `loginctl show-session <id> -p Type --value` for the id from `get_session_id`;
/// 2. the same with the literal id `auto`;
/// 3. `$WAYLAND_DISPLAY` being set at all (even empty).
///
/// A check that prints "wayland" or "x11" decides (true / false); any other output, including
/// "tty" or a failed `loginctl`, falls through to the next check. `$XDG_SESSION_TYPE` is not
/// read. Runs up to five child processes (three in `get_session_id`, two `show-session`).
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
