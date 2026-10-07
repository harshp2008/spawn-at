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

impl WindowSelector {
    pub fn matches(&self, window: &WindowMetadata) -> bool {
        // 1. PID match: Exact
        if let Some(pid) = self.pid {
            if window.pid != Some(pid) {
                return false;
            }
        }
        // 2. Focused match: Exact
        if self.focused && !window.focused {
            return false;
        }

        // 3. Class match: Exact OR reverse-DNS segment match (case-insensitive)
        if let Some(ref target_class) = self.class {
            let win_class = window.class.to_lowercase();
            let query = target_class.to_lowercase();
            let exact = win_class == query;
            let suffix_match = win_class.split('.').last() == Some(query.as_str());

            if !exact && !suffix_match {
                return false;
            }
        }

        // 4. Title match: Substring match (case-insensitive)
        if let Some(ref target_title) = self.title {
            if !window.title.to_lowercase().contains(&target_title.to_lowercase()) {
                return false;
            }
        }
        true
    }
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
/// - If `class` is specified, matches exact class or reverse-DNS suffix (case-insensitive).
/// - If `title` is specified, performs case-insensitive substring search in `win.title`.
/// - If `focused` is true, matches `win.focused == true`.
///
/// Tie-breaking:
/// - If multiple windows match, prefers the one with `focused == true`, otherwise returns the first match.
///
/// Error:
/// - Returns an informative error message if no window satisfies the criteria, offering smart suggestions if a similar class exists.
pub fn resolve_target<'a>(
    windows: &'a [WindowMetadata],
    selector: &WindowSelector,
) -> Result<&'a WindowMetadata, String> {
    let mut matching: Vec<&'a WindowMetadata> = windows
        .iter()
        .filter(|win| selector.matches(win))
        .collect();

    if matching.is_empty() {
        if let Some(ref target) = selector.class {
            let target_lower = target.to_lowercase();
            let mut suggestions: Vec<(f64, &WindowMetadata)> = windows
                .iter()
                .map(|w| {
                    let class_lower = w.class.to_lowercase();

                    // 1. Score against full class string
                    let mut best_score = strsim::jaro_winkler(&target_lower, &class_lower);

                    // 2. Score against individual dot-separated segments
                    for segment in class_lower.split('.') {
                        if segment.len() <= 3 {
                            continue;
                        }
                        let seg_score = strsim::jaro_winkler(&target_lower, segment);
                        if seg_score > best_score {
                            best_score = seg_score;
                        }
                    }

                    // 3. Substring boost
                    if class_lower.contains(&target_lower) {
                        best_score = best_score.max(0.85);
                    }

                    (best_score, w)
                })
                .filter(|(score, _)| *score > 0.70)
                .collect();

            suggestions.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());

            if let Some((_, best)) = suggestions.first() {
                let pid_str = best
                    .pid
                    .map(|p| p.to_string())
                    .unwrap_or_else(|| "unknown".to_string());
                return Err(format!(
                    "No active window matched class \"{target}\".\n\nDid you mean:\n  -c {} (title: \"{}\", pid: {})\n\nTip: You can also match by title using `-t \"{}\"`",
                    best.class, best.title, pid_str, best.title
                ));
            }
        }

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

    if matching.len() > 1 {
        let mut candidate_lines = Vec::new();
        for (i, win) in matching.iter().enumerate() {
            let pid_str = win
                .pid
                .map(|p| p.to_string())
                .unwrap_or_else(|| "unknown".to_string());
            candidate_lines.push(format!(
                "  {}. -c {} (PID: {}, Title: \"{}\")",
                i + 1,
                win.class,
                pid_str,
                win.title
            ));
        }
        return Err(format!(
            "Ambiguous window selector matched {} active windows.\nMatched candidates:\n{}\nPlease disambiguate by using the full reverse-DNS class, PID (--pid), or Title (-t).",
            matching.len(),
            candidate_lines.join("\n")
        ));
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
            class: Some("google-chrome".to_string()),
            ..Default::default()
        };
        let res = resolve_target(&windows, &selector).unwrap();
        assert_eq!(res.id, Some(3));
        assert_eq!(res.pid, Some(1003));
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
        let res = resolve_target(&windows, &selector).unwrap();
        assert!(res.focused);
        assert_eq!(res.id, Some(2));
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

    #[test]
    fn test_match_by_reverse_dns_class() {
        let mut windows = sample_windows();
        windows.push(WindowMetadata {
            id: Some(5),
            pid: Some(1005),
            title: "Calculator".to_string(),
            class: "org.gnome.Calculator".to_string(),
            x: 0,
            y: 0,
            w: 400,
            h: 500,
            focused: false,
        });

        // Query by last segment "calculator"
        let selector = WindowSelector {
            class: Some("calculator".to_string()),
            ..Default::default()
        };
        let res = resolve_target(&windows, &selector).unwrap();
        assert_eq!(res.id, Some(5));
        assert_eq!(res.class, "org.gnome.Calculator");
    }

    #[test]
    fn test_typo_suggestion_calcultor() {
        let mut windows = sample_windows();
        windows.push(WindowMetadata {
            id: Some(5),
            pid: Some(76342),
            title: "Calculator".to_string(),
            class: "org.gnome.Calculator".to_string(),
            x: 0,
            y: 0,
            w: 400,
            h: 500,
            focused: false,
        });

        let selector = WindowSelector {
            class: Some("calcultor".to_string()),
            ..Default::default()
        };
        let res = resolve_target(&windows, &selector);
        assert!(res.is_err());
        let err = res.unwrap_err();
        assert!(err.contains("No active window matched class \"calcultor\"."));
        assert!(err.contains("Did you mean:"));
        assert!(err.contains("-c org.gnome.Calculator (title: \"Calculator\", pid: 76342)"));
    }

    #[test]
    fn test_ambiguity_error_multiple_matches() {
        let windows = vec![
            WindowMetadata {
                id: Some(1),
                pid: Some(76342),
                title: "Calculator".to_string(),
                class: "org.gnome.Calculator".to_string(),
                x: 0,
                y: 0,
                w: 400,
                h: 500,
                focused: false,
            },
            WindowMetadata {
                id: Some(2),
                pid: Some(81204),
                title: "Qalculate!".to_string(),
                class: "io.github.qalculate.calculator".to_string(),
                x: 0,
                y: 0,
                w: 400,
                h: 500,
                focused: false,
            },
        ];

        let selector = WindowSelector {
            class: Some("calculator".to_string()),
            ..Default::default()
        };
        let res = resolve_target(&windows, &selector);
        assert!(res.is_err());
        let err = res.unwrap_err();
        assert!(err.contains("Ambiguous window selector matched 2 active windows."));
        assert!(err.contains("1. -c org.gnome.Calculator (PID: 76342, Title: \"Calculator\")"));
        assert!(err.contains("2. -c io.github.qalculate.calculator (PID: 81204, Title: \"Qalculate!\")"));
        assert!(err.contains("Please disambiguate by using the full reverse-DNS class, PID (--pid), or Title (-t)."));
    }
}
