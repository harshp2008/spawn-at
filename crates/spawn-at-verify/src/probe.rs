//! Environment probe: checks desktop environment, session type, and app availability.

use crate::oracle::Rect;
use crate::schema::TestRequires;
use std::env;
use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone)]
pub struct EnvironmentInfo {
    pub session_type: String,
    pub desktop: String,
    pub gnome_version: Option<u32>,
    pub monitors_count: usize,
    pub primary_workarea: Option<Rect>,
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

    #[test]
    fn test_check_requirements_session_match() {
        let env = EnvironmentInfo {
            session_type: "wayland".into(),
            desktop: "gnome".into(),
            gnome_version: Some(46),
            monitors_count: 1,
            primary_workarea: None,
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

    #[test]
    fn test_check_requirements_session_mismatch() {
        let env = EnvironmentInfo {
            session_type: "x11".into(),
            desktop: "gnome".into(),
            gnome_version: Some(46),
            monitors_count: 1,
            primary_workarea: None,
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
