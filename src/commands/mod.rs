//! # Command Handlers
//!
//! This module contains platform-agnostic implementations of the CLI commands.
//! It isolates CLI routing logic from the compositor-specific execution logic.

pub mod focus;
pub mod lifecycle;
pub mod query;
pub mod spawn;
pub mod transform;

pub use focus::{apply_focus_policy, run_defocus, run_focus};
pub use lifecycle::{run_maximize, run_minimize, run_restore, run_unminimize};
pub use query::run_query;
pub use spawn::run_spawn;
pub use transform::run_transform;

use crate::platform::{CompositorBackend, DriverError, WindowMetadata};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, PartialEq)]
pub enum ExpectedState {
    /// Window exists and is mapped.
    Mapped,
    /// Window position changed to approximately (x, y).
    Position { x: i32, y: i32 },
    /// Window dimensions changed to approximately (w, h).
    Size { w: u32, h: u32 },
    /// Window geometry changed to (x, y, w, h).
    Geometry {
        x: Option<i32>,
        y: Option<i32>,
        w: Option<u32>,
        h: Option<u32>,
    },
    /// Window input focus matches expected boolean.
    Focused(bool),
    /// Window maximized state matches expected boolean.
    Maximized(bool),
    /// Window minimized state matches expected boolean.
    Minimized(bool),
    /// Window is restored (neither maximized nor minimized).
    Restored,
}

/// Helper function to match a window in a metadata list against a target identifier.
/// Checks window ID, direct PID, child/descendant process tree PID, class name, or title.
pub fn matches_target(win: &WindowMetadata, target_id: &str) -> bool {
    if target_id.is_empty() {
        return false;
    }

    // Support composite target hints like "1234:gedit" (e.g. from spawn)
    if let Some((pid_part, class_part)) = target_id.split_once(':') {
        if matches_target(win, pid_part) {
            return true;
        }
        if matches_target(win, class_part) {
            return true;
        }
    }

    // Exact ID match
    if let Ok(id_val) = target_id.parse::<u64>() {
        if win.id == Some(id_val) {
            return true;
        }
    }

    // Direct PID or descendant PID match
    if let Ok(pid_val) = target_id.parse::<u32>() {
        if win.pid == Some(pid_val) {
            return true;
        }
        #[cfg(target_os = "linux")]
        if let Some(win_pid) = win.pid {
            if crate::platform::linux::is_process_descendant(win_pid, pid_val) {
                return true;
            }
        }
    }

    // Class / title match using app hint matching logic
    match_window_app_hint(target_id, &win.class, &win.title)
}

/// Matches an application hint against a window's class and title.
///
/// Designed to support launcher-style applications where the initial launcher process
/// hands off to a pre-existing daemon/server process (e.g., `gnome-terminal` -> `gnome-terminal-server`,
/// or reverse-DNS `org.gnome.Terminal` vs WM_CLASS `Gnome-terminal`), while maintaining strict
/// filtering so non-wildcard queries do not match arbitrary unrelated windows.
pub fn match_window_app_hint(app_hint: &str, win_class: &str, win_title: &str) -> bool {
    let hint = app_hint.trim();
    if hint.is_empty() || hint == "*" {
        return true;
    }

    let hint_lower = hint.to_lowercase();
    let class_lower = win_class.trim().to_lowercase();
    let title_lower = win_title.trim().to_lowercase();

    // 1. Direct class match
    if class_lower == hint_lower {
        return true;
    }

    // 2. Reverse-DNS last segment match (e.g. "org.gnome.Calculator" <-> "calculator")
    if let Some(last) = class_lower.split('.').next_back() {
        if !last.is_empty() && last == hint_lower {
            return true;
        }
    }
    if let Some(last) = hint_lower.split('.').next_back() {
        if !last.is_empty() && last == class_lower {
            return true;
        }
    }

    // 3. Title substring match
    if !hint_lower.is_empty() && title_lower.contains(&hint_lower) {
        return true;
    }

    // 4. Dot-separated multi-segment match for reverse-DNS vs hyphenated WM_CLASS
    // (e.g., "org.gnome.Terminal" dot segments ["org", "gnome", "terminal"]
    // matching last two segments joined as "gnome-terminal" or "gnometerminal")
    let hint_dots: Vec<&str> = hint_lower.split('.').collect();
    let class_dots: Vec<&str> = class_lower.split('.').collect();

    if hint_dots.len() > 1 {
        for start in 1..hint_dots.len() {
            let trailing_hyphen = hint_dots[start..].join("-");
            let trailing_concat = hint_dots[start..].join("");
            if trailing_hyphen == class_lower || trailing_concat == class_lower.replace('-', "") {
                return true;
            }
        }
    }

    if class_dots.len() > 1 {
        for start in 1..class_dots.len() {
            let trailing_hyphen = class_dots[start..].join("-");
            let trailing_concat = class_dots[start..].join("");
            if trailing_hyphen == hint_lower || trailing_concat == hint_lower.replace('-', "") {
                return true;
            }
        }
    }

    // 5. Launcher daemon suffix normalization (e.g., "gnome-terminal-server" vs "gnome-terminal")
    let base_hint = strip_daemon_suffix(&hint_lower);
    let base_class = strip_daemon_suffix(&class_lower);

    if base_hint != hint_lower || base_class != class_lower {
        if base_hint == class_lower || hint_lower == base_class || base_hint == base_class {
            return true;
        }
        if let Some(last) = base_class.split('.').next_back() {
            if !last.is_empty() && last == base_hint {
                return true;
            }
        }
        if let Some(last) = base_hint.split('.').next_back() {
            if !last.is_empty() && last == base_class {
                return true;
            }
        }
    }

    false
}

fn strip_daemon_suffix(s: &str) -> &str {
    s.strip_suffix("-server")
        .or_else(|| s.strip_suffix(".server"))
        .or_else(|| s.strip_suffix("-daemon"))
        .or_else(|| s.strip_suffix(".daemon"))
        .or_else(|| s.strip_suffix("-service"))
        .unwrap_or(s)
}

/// Blocks by polling `driver.get_windows()` until a window matching the spawned process
/// (by PID/process tree, startup notification token, or application class/title) appears
/// and is mapped (`w > 0 && h > 0`), or `timeout` expires.
///
/// Disregards pre-existing window IDs to ensure newly spawned instances are captured accurately,
/// including for D-Bus-activated applications.
pub async fn wait_for_spawn(
    driver: &dyn CompositorBackend,
    child_pid: u32,
    app_hint: &str,
    startup_id: &str,
    pre_existing_ids: &std::collections::HashSet<u64>,
    timeout: Duration,
) -> Result<WindowMetadata, DriverError> {
    let start = Instant::now();
    let poll_interval = Duration::from_millis(50);

    let app_hint_clean = app_hint.trim();
    let startup_id_clean = startup_id.trim();

    while start.elapsed() < timeout {
        let windows = driver.get_windows().await.unwrap_or_default();

        // 1. Look for a newly created window (ID not in pre_existing_ids)
        let new_win = windows.iter().find(|w| {
            let is_new = w.id.map_or(true, |id| !pre_existing_ids.contains(&id));
            if !is_new {
                return false;
            }

            // Direct PID or descendant PID match
            if let Some(win_pid) = w.pid {
                if win_pid == child_pid {
                    return true;
                }
                #[cfg(target_os = "linux")]
                if crate::platform::linux::is_process_descendant(win_pid, child_pid) {
                    return true;
                }
            }

            // Startup notification / entry key substring match in title or class
            if !startup_id_clean.is_empty() {
                if w.title.contains(startup_id_clean) || w.class.contains(startup_id_clean) {
                    return true;
                }
            }

            // Application hint match (class exact, reverse-DNS suffix, daemon suffix, app_id, or title substring)
            if !app_hint_clean.is_empty() && app_hint_clean != "*" {
                if match_window_app_hint(app_hint_clean, &w.class, &w.title) {
                    return true;
                }
                if let Some(ref app_id) = w.app_id {
                    if match_window_app_hint(app_hint_clean, app_id, &w.title) {
                        return true;
                    }
                }
            }

            // If wildcard or no app hint, any new mapped window is accepted
            app_hint_clean.is_empty() || app_hint_clean == "*"
        });

        // 2. Fallback: If no new window by ID was detected, check if any window matches direct PID
        let candidate = new_win.or_else(|| {
            windows.iter().find(|w| {
                if let Some(win_pid) = w.pid {
                    if win_pid == child_pid {
                        return true;
                    }
                    #[cfg(target_os = "linux")]
                    if crate::platform::linux::is_process_descendant(win_pid, child_pid) {
                        return true;
                    }
                }
                false
            })
        });

        if let Some(win) = candidate {
            if win.w > 0 && win.h > 0 {
                return Ok(win.clone());
            }
        }

        tokio::time::sleep(poll_interval).await;
    }

    // Check if an existing window was activated instead of creating a new window
    let windows = driver.get_windows().await.unwrap_or_default();
    if let Some(existing) = windows.iter().find(|w| {
        w.id.map_or(false, |id| pre_existing_ids.contains(&id))
            && (!app_hint_clean.is_empty() && (
                match_window_app_hint(app_hint_clean, &w.class, &w.title)
                || w.app_id.as_ref().map_or(false, |aid| match_window_app_hint(app_hint_clean, aid, &w.title))
            ))
    }) {
        let diag = crate::diagnostics::Diagnostic::ReusedExistingWindow {
            class: existing.class.clone(),
            title: existing.title.clone(),
            pid: existing.pid,
        };
        crate::diagnostics::render_diagnostic(&diag);
        return Err(DriverError::Execution(
            "Application reused an existing window; not repositioned.".into(),
        ));
    }

    Err(DriverError::TargetNotFound(format!(
        "Window matching app hint '{}' (PID: {}) did not appear within {:?}",
        app_hint, child_pid, timeout
    )))
}

/// Blocks by polling `driver.get_windows()` every 50ms until the target window's state
/// matches `expected_state` or `timeout` expires.
pub async fn wait_for_state_change(
    driver: &dyn CompositorBackend,
    target_id: &str,
    expected_state: ExpectedState,
    timeout: Duration,
) -> Result<WindowMetadata, DriverError> {
    let start = Instant::now();
    let poll_interval = Duration::from_millis(50);
    let mut last_seen_win: Option<WindowMetadata> = None;

    while start.elapsed() < timeout {
        let windows = driver.get_windows().await.unwrap_or_default();
        let found = windows.iter().find(|w| matches_target(w, target_id));

        if let Some(win) = found {
            last_seen_win = Some(win.clone());

            let satisfied = match expected_state {
                ExpectedState::Mapped => win.w > 0 && win.h > 0,
                ExpectedState::Position { x, y } => {
                    (win.x - x).abs() <= 4 && (win.y - y).abs() <= 4
                }
                ExpectedState::Size { w, h } => {
                    (win.w - w as i32).abs() <= 4 && (win.h - h as i32).abs() <= 4
                }
                ExpectedState::Geometry { x, y, w, h } => {
                    let x_ok = x.map_or(true, |tx| (win.x - tx).abs() <= 4);
                    let y_ok = y.map_or(true, |ty| (win.y - ty).abs() <= 4);
                    let w_ok = w.map_or(true, |tw| (win.w - tw as i32).abs() <= 4);
                    let h_ok = h.map_or(true, |th| (win.h - th as i32).abs() <= 4);
                    x_ok && y_ok && w_ok && h_ok
                }
                ExpectedState::Focused(exp) => win.focused == exp,
                ExpectedState::Maximized(exp) => win.maximized == exp,
                ExpectedState::Minimized(exp) => win.minimized == exp,
                ExpectedState::Restored => !win.maximized && !win.minimized,
            };

            if satisfied {
                return Ok(win.clone());
            }
        } else if expected_state == ExpectedState::Minimized(true) && last_seen_win.is_some() {
            // Window disappeared from active window list -> satisfied minimized
            if let Some(w) = last_seen_win {
                return Ok(w);
            }
        }

        tokio::time::sleep(poll_interval).await;
    }

    Err(DriverError::TargetNotFound(format!(
        "Window matching '{}' did not satisfy state {:?} within {:?}",
        target_id, expected_state, timeout
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::{MaximizeArgs, TransformArgs, WindowTargetArgs};
    use crate::platform::linux::x11::X11Driver;

    #[tokio::test]
    async fn test_transform_supported_on_x11() {
        let driver = X11Driver;
        let args = TransformArgs {
            class: Some("alacritty".into()),
            ..Default::default()
        };
        let err = run_transform(&driver, args, false).await.unwrap_err();
        let msg = err.to_string();
        assert!(!msg.contains("The active compositor does not support this feature"));
    }

    #[tokio::test]
    async fn test_focus_supported_on_x11() {
        let driver = X11Driver;
        let args = WindowTargetArgs {
            class: Some("alacritty".into()),
            ..Default::default()
        };
        let err = run_focus(&driver, args, false).await.unwrap_err();
        let msg = err.to_string();
        assert!(!msg.contains("The active compositor does not support this feature"));
    }

    #[tokio::test]
    async fn test_lifecycle_supported_on_x11() {
        let driver = X11Driver;
        let args = MaximizeArgs {
            target: WindowTargetArgs {
                class: Some("alacritty".into()),
                ..Default::default()
            },
            ..Default::default()
        };
        let err = run_maximize(&driver, args, false).await.unwrap_err();
        let msg = err.to_string();
        assert!(!msg.contains("The active compositor does not support this feature"));
    }

    #[test]
    fn test_matches_target_criteria() {
        let win = WindowMetadata {
            id: Some(101),
            pid: Some(5001),
            title: "Terminal - Alacritty".into(),
            class: "Alacritty".into(),
            app_id: None,
            x: 10,
            y: 20,
            w: 800,
            h: 600,
            focused: true,
            maximized: false,
            minimized: false,
        };

        // Match by numeric window ID
        assert!(matches_target(&win, "101"));
        // Match by numeric PID
        assert!(matches_target(&win, "5001"));
        // Match by exact class
        assert!(matches_target(&win, "alacritty"));
        // Match by composite PID:class
        assert!(matches_target(&win, "5001:alacritty"));
        assert!(matches_target(&win, "9999:alacritty"));
        // Match by title substring
        assert!(matches_target(&win, "Terminal"));

        // Non-matching queries
        assert!(!matches_target(&win, "gedit"));
        assert!(!matches_target(&win, "102"));
    }

    struct MockPollBackend {
        polls: std::sync::atomic::AtomicUsize,
    }

    #[async_trait::async_trait]
    impl crate::platform::Driver for MockPollBackend {
        async fn arm(&self, _batch: crate::platform::Batch) -> Result<crate::platform::Armed, DriverError> {
            Ok(crate::platform::Armed::default())
        }
    }

    #[async_trait::async_trait]
    impl CompositorBackend for MockPollBackend {
        fn name(&self) -> &'static str {
            "MockPoll"
        }
        fn resolve_id(&self, _cmd: &[String], _cls: Option<&str>) -> String {
            "mock".into()
        }
        async fn get_windows(&self) -> Result<Vec<WindowMetadata>, DriverError> {
            let count = self.polls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            // Simulate window state updating on the 2nd poll
            let is_max = count >= 1;
            Ok(vec![WindowMetadata {
                id: Some(200),
                pid: Some(6000),
                title: "Mock Window".into(),
                class: "mock-app".into(),
                app_id: None,
                x: if is_max { 0 } else { 100 },
                y: if is_max { 0 } else { 100 },
                w: if is_max { 1920 } else { 800 },
                h: if is_max { 1080 } else { 600 },
                focused: true,
                maximized: is_max,
                minimized: false,
            }])
        }
    }

    #[tokio::test]
    async fn test_wait_for_state_change_success() {
        let backend = MockPollBackend {
            polls: std::sync::atomic::AtomicUsize::new(0),
        };

        let res = wait_for_state_change(
            &backend,
            "mock-app",
            ExpectedState::Maximized(true),
            Duration::from_millis(500),
        )
        .await;

        assert!(res.is_ok());
        let win = res.unwrap();
        assert!(win.maximized);
        assert_eq!(win.w, 1920);
    }

    struct MockSpawnBackend {
        polls: std::sync::atomic::AtomicUsize,
    }

    #[async_trait::async_trait]
    impl crate::platform::Driver for MockSpawnBackend {
        async fn arm(&self, _batch: crate::platform::Batch) -> Result<crate::platform::Armed, DriverError> {
            Ok(crate::platform::Armed::default())
        }
    }

    #[async_trait::async_trait]
    impl CompositorBackend for MockSpawnBackend {
        fn name(&self) -> &'static str {
            "MockSpawn"
        }
        fn resolve_id(&self, _cmd: &[String], _cls: Option<&str>) -> String {
            "org.gnome.Calculator".into()
        }
        async fn get_windows(&self) -> Result<Vec<WindowMetadata>, DriverError> {
            let count = self.polls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            let mut wins = vec![
                // Pre-existing window
                WindowMetadata {
                    id: Some(10),
                    pid: Some(1000),
                    title: "Pre-existing App".into(),
                    class: "org.gnome.Calculator".into(),
                    app_id: None,
                    x: 0,
                    y: 0,
                    w: 300,
                    h: 400,
                    focused: false,
                    maximized: false,
                    minimized: false,
                },
            ];

            // Window appears on poll 2
            if count >= 2 {
                wins.push(WindowMetadata {
                    id: Some(20),
                    pid: Some(9999), // Different PID (D-Bus activated)
                    title: "Calculator".into(),
                    class: "org.gnome.Calculator".into(),
                    app_id: None,
                    x: 100,
                    y: 100,
                    w: 367,
                    h: 514,
                    focused: true,
                    maximized: false,
                    minimized: false,
                });
            }

            Ok(wins)
        }
    }

    #[tokio::test]
    async fn test_wait_for_spawn_ignores_preexisting_and_matches_dbus_app() {
        let backend = MockSpawnBackend {
            polls: std::sync::atomic::AtomicUsize::new(0),
        };

        let mut pre_existing = std::collections::HashSet::new();
        pre_existing.insert(10);

        let res = wait_for_spawn(
            &backend,
            5555, // child launcher PID
            "org.gnome.Calculator",
            "spawn-at-token_TIME123",
            &pre_existing,
            Duration::from_millis(500),
        )
        .await;

        assert!(res.is_ok());
        let win = res.unwrap();
        assert_eq!(win.id, Some(20));
        assert_eq!(win.w, 367);
        assert_eq!(win.h, 514);
    }

    #[tokio::test]
    async fn test_wait_for_spawn_timeout() {
        let backend = MockSpawnBackend {
            polls: std::sync::atomic::AtomicUsize::new(0),
        };

        let mut pre_existing = std::collections::HashSet::new();
        pre_existing.insert(10);

        let res = wait_for_spawn(
            &backend,
            5555,
            "nonexistent-app",
            "spawn-at-token",
            &pre_existing,
            Duration::from_millis(150),
        )
        .await;

        assert!(res.is_err());
    }

    #[test]
    fn test_match_window_app_hint_launcher_style_apps() {
        // gnome-terminal-server override vs Gnome-terminal class
        assert!(match_window_app_hint("gnome-terminal-server", "Gnome-terminal", ""));
        // org.gnome.Terminal vs Gnome-terminal class
        assert!(match_window_app_hint("org.gnome.Terminal", "Gnome-terminal", ""));
        // gnome-terminal vs org.gnome.Terminal
        assert!(match_window_app_hint("gnome-terminal", "org.gnome.Terminal", ""));
        // Calculator reverse-DNS vs hyphenated
        assert!(match_window_app_hint("org.gnome.Calculator", "gnome-calculator", ""));
        assert!(match_window_app_hint("calculator", "org.gnome.Calculator", ""));
        // Exact matches
        assert!(match_window_app_hint("Alacritty", "alacritty", ""));
        // Wildcards
        assert!(match_window_app_hint("*", "Alacritty", ""));
        assert!(match_window_app_hint("", "Alacritty", ""));
        // Negative matches: must not falsely match
        assert!(!match_window_app_hint("gnome-terminal-server", "Alacritty", ""));
        assert!(!match_window_app_hint("alacritty", "Google-chrome", ""));
        assert!(!match_window_app_hint("term", "Gnome-terminal", ""));
    }

    #[tokio::test]
    async fn test_wait_for_spawn_launcher_daemon_handoff() {
        struct LauncherMockBackend;

        #[async_trait::async_trait]
        impl crate::platform::Driver for LauncherMockBackend {
            async fn arm(&self, _batch: crate::platform::Batch) -> Result<crate::platform::Armed, DriverError> {
                Ok(crate::platform::Armed::default())
            }
        }

        #[async_trait::async_trait]
        impl CompositorBackend for LauncherMockBackend {
            fn name(&self) -> &'static str {
                "LauncherMock"
            }
            fn resolve_id(&self, _cmd: &[String], _cls: Option<&str>) -> String {
                "org.gnome.Terminal".into()
            }
            async fn get_windows(&self) -> Result<Vec<WindowMetadata>, DriverError> {
                Ok(vec![
                    WindowMetadata {
                        id: Some(55),
                        pid: Some(1250560), // Server daemon PID, NOT the launcher PID (1250555)
                        title: "harsh@harsh: ~".into(),
                        class: "Gnome-terminal".into(),
                        app_id: Some("org.gnome.Terminal".into()),
                        x: 200,
                        y: 200,
                        w: 800,
                        h: 500,
                        focused: true,
                        maximized: false,
                        minimized: false,
                    },
                ])
            }
        }

        let backend = LauncherMockBackend;
        let mut pre_existing = std::collections::HashSet::new();
        pre_existing.insert(10); // Window 55 is new

        // Test with app_hint = "org.gnome.Terminal" (launcher PID 1250555 != server PID 1250560)
        let res = wait_for_spawn(
            &backend,
            1250555,
            "org.gnome.Terminal",
            "",
            &pre_existing,
            Duration::from_millis(500),
        )
        .await;

        assert!(res.is_ok());
        let win = res.unwrap();
        assert_eq!(win.id, Some(55));
        assert_eq!(win.class, "Gnome-terminal");
    }
}
