//! Environment probe: checks desktop environment, session type, and app availability.

use crate::oracle::Rect;
use crate::schema::TestRequires;
use std::env;
use std::path::Path;
use std::process::Command;

/// Host environment metadata probed from desktop session and display server.
#[derive(Debug, Clone)]
pub struct EnvironmentInfo {
    /// Display session type ("wayland", "x11", or "unknown").
    pub session_type: String,
    /// Active desktop environment ("gnome", etc.).
    pub desktop: String,
    /// Detected GNOME Shell major version number, if running GNOME.
    pub gnome_version: Option<u32>,
    /// Number of detected connected monitors.
    pub monitors_count: usize,
    /// Primary monitor workarea as reported by `<bin> query layout`. On GNOME that is the
    /// extension under test (`workspace.get_work_area_for_monitor`), so it is NOT independent.
    pub primary_workarea: Option<Rect>,
    /// Primary workarea read from mutter's `_NET_WORKAREA` hint, independent of the extension;
    /// `None` when it cannot be read (see [`probe_independent_workarea`]).
    pub independent_workarea: Option<Rect>,
}

impl EnvironmentInfo {
    /// Probes the local session environment using standard environment variables,
    /// gnome-shell version, and optionally running `<bin> query layout --json`.
    pub fn probe(bin_path: Option<&Path>) -> Self {
        let session_type = env::var("XDG_SESSION_TYPE")
            .unwrap_or_else(|_| "unknown".to_string())
            .to_lowercase();

        let desktop = env::var("XDG_CURRENT_DESKTOP")
            .or_else(|_| env::var("DESKTOP_SESSION"))
            .unwrap_or_else(|_| "unknown".to_string())
            .to_lowercase();

        let gnome_version = probe_gnome_shell_version();

        let (monitors_count, primary_workarea) = if let Some(bin) = bin_path {
            probe_layout(bin).unwrap_or((1, None))
        } else {
            (1, None)
        };

        Self {
            session_type,
            desktop,
            gnome_version,
            monitors_count,
            primary_workarea,
            independent_workarea: probe_independent_workarea(),
        }
    }

    /// The workarea measurements to judge geometry with: the independent value (primary) and the
    /// extension's `query layout` value (cross-check / fallback).
    pub fn workarea_probe(&self) -> crate::oracle::WorkareaProbe {
        crate::oracle::WorkareaProbe {
            independent: self.independent_workarea,
            extension: self.primary_workarea,
        }
    }

    /// Checks if this environment satisfies the specified test requirements.
    /// Returns `Ok(())` if all conditions are satisfied, or `Err(reason)` if skipped.
    pub fn check_requirements(&self, req: &TestRequires) -> Result<(), String> {
        // 1. Session check
        if !req.session.iter().any(|s| s == "any") {
            let matches_session = req
                .session
                .iter()
                .any(|s| s.eq_ignore_ascii_case(&self.session_type));
            if !matches_session {
                return Err(format!(
                    "Current session '{}' does not match requirement {:?}",
                    self.session_type, req.session
                ));
            }
        }

        // 2. Backend check
        if !req.backend.iter().any(|b| b == "any") {
            let is_gnome = self.desktop.contains("gnome");
            let is_x11 = self.session_type == "x11";

            let mut satisfied = false;
            for b in &req.backend {
                if (b == "gnome" && is_gnome) || (b == "x11" && is_x11) {
                    satisfied = true;
                }
            }
            if !satisfied {
                return Err(format!(
                    "Desktop '{}' (session '{}') does not match backend requirement {:?}",
                    self.desktop, self.session_type, req.backend
                ));
            }
        }

        // 3. GNOME version range check (e.g. "42-48")
        if req.gnome != "any" {
            let current_v = self.gnome_version.ok_or_else(|| {
                "GNOME Shell version required, but GNOME Shell is not detected".to_string()
            })?;

            if let Some((start_s, end_s)) = req.gnome.split_once('-') {
                let start: u32 = start_s.trim().parse().unwrap_or(0);
                let end: u32 = end_s.trim().parse().unwrap_or(999);
                if current_v < start || current_v > end {
                    return Err(format!(
                        "GNOME Shell version {} out of required range '{}-{}'",
                        current_v, start, end
                    ));
                }
            } else if let Ok(exact) = req.gnome.parse::<u32>() {
                if current_v != exact {
                    return Err(format!(
                        "GNOME Shell version {} does not match required exact version {}",
                        current_v, exact
                    ));
                }
            }
        }

        // 4. Minimum monitors check
        if (self.monitors_count as u32) < req.monitors_min {
            return Err(format!(
                "Detected {} monitor(s), test requires at least {}",
                self.monitors_count, req.monitors_min
            ));
        }

        // 5. Apps check
        for app in &req.apps {
            if !command_exists(app) {
                return Err(format!("Required application '{}' not found in PATH", app));
            }
        }

        Ok(())
    }
}

/// Probes the installed and running GNOME Shell major version.
fn probe_gnome_shell_version() -> Option<u32> {
    let output = Command::new("gnome-shell").arg("--version").output().ok()?;

    if !output.status.success() {
        return None;
    }

    let out_str = String::from_utf8_lossy(&output.stdout);
    // e.g. "GNOME Shell 46.0"
    for part in out_str.split_whitespace() {
        if let Some(major_str) = part.split('.').next() {
            if let Ok(v) = major_str.parse::<u32>() {
                return Some(v);
            }
        }
    }
    None
}

/// Parses `xrandr --listactivemonitors` output and returns the active monitor count from its
/// `Monitors: N` header line.
pub fn parse_xrandr_active_monitors(text: &str) -> Option<usize> {
    text.lines()
        .find_map(|l| l.trim().strip_prefix("Monitors:"))
        .and_then(|n| n.trim().parse().ok())
}

/// Parses `xprop -root _NET_WORKAREA _NET_CURRENT_DESKTOP` output and returns the work area of
/// the current desktop (desktop 0 when the current desktop is not reported).
///
/// `_NET_WORKAREA` is a list of `x, y, w, h` quadruples, one per desktop.
pub fn parse_net_workarea(text: &str) -> Option<Rect> {
    let numbers_after_eq = |prefix: &str| -> Option<Vec<i64>> {
        let line = text.lines().find(|l| l.starts_with(prefix))?;
        let (_, rhs) = line.split_once('=')?;
        rhs.split(',').map(|n| n.trim().parse().ok()).collect()
    };
    let area = numbers_after_eq("_NET_WORKAREA")?;
    let desktop = numbers_after_eq("_NET_CURRENT_DESKTOP")
        .and_then(|d| d.first().copied())
        .unwrap_or(0)
        .max(0) as usize;
    let q = area.get(desktop * 4..desktop * 4 + 4)?;
    let (w, h) = (u32::try_from(q[2]).ok()?, u32::try_from(q[3]).ok()?);
    Some(Rect::new(
        i32::try_from(q[0]).ok()?,
        i32::try_from(q[1]).ok()?,
        w,
        h,
    ))
}

/// Reads the primary workarea from the window system itself, without asking spawn-at or its
/// GNOME extension: mutter publishes `_NET_WORKAREA` on the XWayland (or X11) root window, read
/// here with `xprop`. It is a single rectangle for the whole desktop, so it is only used when
/// `xrandr` reports exactly one active monitor; otherwise (or when either tool or `DISPLAY` is
/// missing) there is no independent value.
///
/// Limitation: under fractional scaling the hint may not be in the logical pixels spawn-at uses;
/// such a mismatch surfaces as a "workarea differs" warning against the extension's value.
fn probe_independent_workarea() -> Option<Rect> {
    let xrandr = Command::new("xrandr")
        .arg("--listactivemonitors")
        .output()
        .ok()
        .filter(|o| o.status.success())?;
    if parse_xrandr_active_monitors(&String::from_utf8_lossy(&xrandr.stdout))? != 1 {
        return None;
    }
    let xprop = Command::new("xprop")
        .args(["-root", "_NET_WORKAREA", "_NET_CURRENT_DESKTOP"])
        .output()
        .ok()
        .filter(|o| o.status.success())?;
    parse_net_workarea(&String::from_utf8_lossy(&xprop.stdout))
}

/// Invokes CLI layout query to determine monitor counts and primary workarea geometry.
fn probe_layout(bin_path: &Path) -> Result<(usize, Option<Rect>), String> {
    let output = Command::new(bin_path)
        .args(["query", "layout", "--json"])
        .output()
        .map_err(|e| format!("Failed to query layout: {}", e))?;

    if !output.status.success() {
        return Ok((1, None));
    }

    let parsed: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("Failed to parse layout json: {}", e))?;

    let monitors_arr = parsed.get("monitors").and_then(|m| m.as_array());
    let monitors_count = monitors_arr.map(|a| a.len()).unwrap_or(1);

    let primary_workarea = if let Some(monitors) = monitors_arr {
        let m = monitors
            .iter()
            .find(|m| m.get("primary").and_then(|p| p.as_bool()).unwrap_or(false))
            .or_else(|| monitors.first());
        m.and_then(|m| m.get("workarea")).and_then(|val| {
            let x = val.get("x")?.as_i64()? as i32;
            let y = val.get("y")?.as_i64()? as i32;
            let w = val.get("w")?.as_u64()? as u32;
            let h = val.get("h")?.as_u64()? as u32;
            Some(Rect::new(x, y, w, h))
        })
    } else {
        parsed
            .get("workareas")
            .and_then(|w| w.as_array())
            .and_then(|a| a.first())
            .and_then(|val| {
                let x = val.get("x")?.as_i64()? as i32;
                let y = val.get("y")?.as_i64()? as i32;
                let w = val.get("w")?.as_u64()? as u32;
                let h = val.get("h")?.as_u64()? as u32;
                Some(Rect::new(x, y, w, h))
            })
    };

    Ok((monitors_count, primary_workarea))
}

/// Checks whether an executable command binary exists on the system PATH.
fn command_exists(cmd: &str) -> bool {
    if let Ok(path_var) = env::var("PATH") {
        for dir in env::split_paths(&path_var) {
            let full = dir.join(cmd);
            if full.is_file() {
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Real `xprop -root _NET_WORKAREA _NET_CURRENT_DESKTOP` output (two workspaces, current 0).
    const XPROP_REAL: &str = "_NET_WORKAREA(CARDINAL) = 0, 40, 1920, 1040, 0, 40, 1920, 1040\n_NET_CURRENT_DESKTOP(CARDINAL) = 0\n";

    /// The current desktop's quadruple is returned, not just the first one.
    #[test]
    fn test_parse_net_workarea_real_output_and_current_desktop() {
        assert_eq!(
            parse_net_workarea(XPROP_REAL),
            Some(Rect::new(0, 40, 1920, 1040))
        );
        let two = "_NET_WORKAREA(CARDINAL) = 0, 40, 1920, 1040, 5, 6, 700, 800\n_NET_CURRENT_DESKTOP(CARDINAL) = 1\n";
        assert_eq!(parse_net_workarea(two), Some(Rect::new(5, 6, 700, 800)));
    }

    /// Missing current-desktop line means desktop 0; a missing, short or non-numeric property
    /// yields `None`, never a guessed rectangle.
    #[test]
    fn test_parse_net_workarea_rejects_bad_input() {
        let no_desktop = "_NET_WORKAREA(CARDINAL) = 1, 2, 3, 4\n";
        assert_eq!(parse_net_workarea(no_desktop), Some(Rect::new(1, 2, 3, 4)));
        assert_eq!(parse_net_workarea("_NET_WORKAREA:  not found.\n"), None);
        assert_eq!(
            parse_net_workarea("_NET_WORKAREA(CARDINAL) = 1, 2, 3\n"),
            None
        );
        assert_eq!(
            parse_net_workarea("_NET_WORKAREA(CARDINAL) = 0, 0, -5, 10\n"),
            None
        );
        assert_eq!(parse_net_workarea(""), None);
    }

    /// Real `xrandr --listactivemonitors` output counts one monitor; two monitors count two.
    #[test]
    fn test_parse_xrandr_active_monitors() {
        let one = "Monitors: 1\n 0: +*eDP-1 1920/340x1080/190+0+0  eDP-1\n";
        assert_eq!(parse_xrandr_active_monitors(one), Some(1));
        let two = "Monitors: 2\n 0: +*DP-1 1920/1x1080/1+0+0 DP-1\n 1: +HDMI-1 1920/1x1080/1+1920+0 HDMI-1\n";
        assert_eq!(parse_xrandr_active_monitors(two), Some(2));
        assert_eq!(parse_xrandr_active_monitors("can't open display"), None);
    }

    /// Tests requirement validation when session matches requested environment.
    #[test]
    fn test_check_requirements_session_match() {
        let env = EnvironmentInfo {
            session_type: "wayland".into(),
            desktop: "gnome".into(),
            gnome_version: Some(46),
            monitors_count: 1,
            primary_workarea: None,
            independent_workarea: None,
        };

        let req = TestRequires {
            session: vec!["wayland".into()],
            backend: vec!["gnome".into()],
            gnome: "42-48".into(),
            monitors_min: 1,
            scale: "any".into(),
            apps: vec![],
        };

        assert!(env.check_requirements(&req).is_ok());
    }

    /// Tests requirement validation rejection when session differs from specification.
    #[test]
    fn test_check_requirements_session_mismatch() {
        let env = EnvironmentInfo {
            session_type: "x11".into(),
            desktop: "gnome".into(),
            gnome_version: Some(46),
            monitors_count: 1,
            primary_workarea: None,
            independent_workarea: None,
        };

        let req = TestRequires {
            session: vec!["wayland".into()],
            backend: vec!["any".into()],
            gnome: "any".into(),
            monitors_min: 1,
            scale: "any".into(),
            apps: vec![],
        };

        let res = env.check_requirements(&req);
        assert!(res.is_err());
        assert!(res.unwrap_err().contains("session 'x11' does not match"));
    }
}
