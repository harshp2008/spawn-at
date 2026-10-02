//! # Window Target Resolution Engine
//!
//! Provides deterministic window target selection matching windows by PID, class,
//! title substring, or focus state.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default)]
pub struct WindowSelector {
    pub class: Option<String>,
    pub title: Option<String>,
    pub pid: Option<u32>,
    pub focused: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WindowMetadata {
    #[serde(default)]
    pub id: Option<u64>,
    #[serde(default)]
    pub pid: Option<u32>,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub class: String,
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    #[serde(default)]
    pub focused: bool,
}

/// Resolves a window target from a list of window metadata entries based on `WindowSelector` criteria.
///
/// Filter criteria:
/// - If `pid` is specified, matches `win.pid == Some(pid)`.
/// - If `class` is specified, performs case-insensitive exact match with `win.class`.
/// - If `title` is specified, performs case-insensitive substring search in `win.title`.
/// - If `focused` is true, matches `win.focused == true`.
///
/// Tie-breaking:
/// - If multiple windows match, prefers the one with `focused == true`, otherwise returns the first match.
///
/// Error:
/// - Returns an informative error message if no window satisfies the criteria.
pub fn resolve_target<'a>(
    windows: &'a [WindowMetadata],
    selector: &WindowSelector,
) -> Result<&'a WindowMetadata, String> {
    let mut matching: Vec<&'a WindowMetadata> = windows
        .iter()
        .filter(|win| {
            if let Some(target_pid) = selector.pid {
                if win.pid != Some(target_pid) {
                    return false;
                }
            }

            if let Some(ref target_class) = selector.class {
                if !win.class.eq_ignore_ascii_case(target_class) {
                    return false;
                }
            }

            if let Some(ref target_title) = selector.title {
                if !win.title.to_lowercase().contains(&target_title.to_lowercase()) {
                    return false;
                }
            }

            if selector.focused && !win.focused {
                return false;
            }

            true
        })
        .collect();

    if matching.is_empty() {
        let mut criteria = Vec::new();
        if let Some(ref c) = selector.class {
            criteria.push(format!("class: {:?}", c));
        }
        if let Some(ref t) = selector.title {
            criteria.push(format!("title: {:?}", t));
        }
        if let Some(p) = selector.pid {
            criteria.push(format!("pid: {}", p));
        }
        if selector.focused {
            criteria.push("focused: true".to_string());
        }
        let crit_str = if criteria.is_empty() {
            "none".to_string()
        } else {
            criteria.join(", ")
        };

        return Err(format!(
            "No active window matched criteria ({})\nTip: Run `spawn-at query windows` to view all active window identifiers.",
            crit_str
        ));
    }

    // If multiple matches, prioritize focused window
    if let Some(focused_win) = matching.iter().find(|w| w.focused) {
        return Ok(*focused_win);
    }

    Ok(matching.remove(0))
}

/// Determines the optimal string identifier for a resolved window to send to the compositor.
/// Prioritizes unique compositor window ID, then PID, then window class / application ID.
pub fn resolve_target_id(win: &WindowMetadata) -> String {
    if let Some(id) = win.id {
        if id > 0 {
            return id.to_string();
        }
    }
    if let Some(pid) = win.pid {
        if pid > 0 {
            return pid.to_string();
        }
    }
    win.class.clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_windows() -> Vec<WindowMetadata> {
        vec![
            WindowMetadata {
                id: Some(1),
                pid: Some(1001),
                title: "Harsh - Alacritty".to_string(),
                class: "Alacritty".to_string(),
                x: 100,
                y: 100,
                w: 800,
                h: 600,
                focused: false,
            },
            WindowMetadata {
                id: Some(2),
                pid: Some(1002),
                title: "Editor - main.rs - spawn-at".to_string(),
                class: "code".to_string(),
                x: 200,
                y: 200,
                w: 1200,
                h: 800,
                focused: true,
            },
            WindowMetadata {
                id: Some(3),
                pid: Some(1003),
                title: "Google Chrome".to_string(),
                class: "google-chrome".to_string(),
                x: 0,
                y: 0,
                w: 1920,
                h: 1080,
                focused: false,
            },
            WindowMetadata {
                id: Some(4),
                pid: Some(1004),
                title: "Terminal 2".to_string(),
                class: "Alacritty".to_string(),
                x: 300,
                y: 300,
                w: 800,
                h: 600,
                focused: true,
            },
        ]
    }

    #[test]
    fn test_match_by_pid() {
        let windows = sample_windows();
        let selector = WindowSelector {
            pid: Some(1002),
            ..Default::default()
        };
        let res = resolve_target(&windows, &selector).unwrap();
        assert_eq!(res.id, Some(2));
        assert_eq!(res.class, "code");
    }

    #[test]
    fn test_match_by_class_case_insensitive() {
        let windows = sample_windows();
        let selector = WindowSelector {
            class: Some("alacritty".to_string()),
            ..Default::default()
        };
        // Among matching Alacritty windows (pid 1001 unfocused, pid 1004 focused), focused should be picked
        let res = resolve_target(&windows, &selector).unwrap();
        assert_eq!(res.id, Some(4));
        assert_eq!(res.pid, Some(1004));
    }

    #[test]
    fn test_match_by_title_substring() {
        let windows = sample_windows();
        let selector = WindowSelector {
            title: Some("main.rs".to_string()),
            ..Default::default()
        };
        let res = resolve_target(&windows, &selector).unwrap();
        assert_eq!(res.id, Some(2));
    }

    #[test]
    fn test_match_by_focused() {
        let windows = sample_windows();
        let selector = WindowSelector {
            focused: true,
            ..Default::default()
        };
        // Returns one of the focused windows
        let res = resolve_target(&windows, &selector).unwrap();
        assert!(res.focused);
    }

    #[test]
    fn test_no_match_returns_error() {
        let windows = sample_windows();
        let selector = WindowSelector {
            class: Some("NonExistentApp".to_string()),
            ..Default::default()
        };
        let res = resolve_target(&windows, &selector);
        assert!(res.is_err());
        assert!(res.unwrap_err().contains("No active window matched criteria"));
    }

    #[test]
    fn test_resolve_target_id() {
        let win_with_id = WindowMetadata {
            id: Some(42),
            pid: Some(1234),
            title: "Test".to_string(),
            class: "test-app".to_string(),
            x: 0,
            y: 0,
            w: 100,
            h: 100,
            focused: false,
        };
        assert_eq!(resolve_target_id(&win_with_id), "42");

        let win_with_pid = WindowMetadata {
            id: None,
            pid: Some(1234),
            title: "Test".to_string(),
            class: "test-app".to_string(),
            x: 0,
            y: 0,
            w: 100,
            h: 100,
            focused: false,
        };
        assert_eq!(resolve_target_id(&win_with_pid), "1234");

        let win_fallback_class = WindowMetadata {
            id: None,
            pid: None,
            title: "Test".to_string(),
            class: "test-app".to_string(),
            x: 0,
            y: 0,
            w: 100,
            h: 100,
            focused: false,
        };
        assert_eq!(resolve_target_id(&win_fallback_class), "test-app");
    }
}
