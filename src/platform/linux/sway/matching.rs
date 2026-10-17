//! Which new window belongs to the process spawn-at launched. Pure: the process-tree test is a
//! parameter and time is passed in, so every rule runs in a unit test without Sway.
//!
//! The rules, in order of strength (`docs/DESIGN_SWAY.md` section 6), applied only to windows
//! that were not in the tree when the driver was armed:
//! 1. the window's pid is the launched process,
//! 2. the window's pid is a descendant of it (wrapper scripts, `env`, launchers that exec),
//! 3. the window's app id (or X11 class) matches the entry's application hint, for launchers
//!    that hand the work to a server that is already running and so never match by pid.
//!
//! A rule 1 or 2 match is taken at once. A rule 3 match is remembered and taken only if no
//! stronger match arrives within [`APP_ID_GRACE`], so a stray window of the same application
//! (a second `gnome-terminal` window, say) does not win over the real one.
//!
//! Rule 3 uses `commands::match_window_app_hint` with an empty title, so its title rule never
//! applies here: only the class or app id is compared. The caller is `place::wait_for_window`.
//! This module reads no clock, no `/proc` and no socket. The rules are checked only by unit
//! tests on recorded JSON and, for the Sway backend as a whole, on a private headless Sway 1.9;
//! the backend is experimental.

use super::types::{is_window, node_class, Node};
use crate::commands::match_window_app_hint;
use std::collections::HashSet;
use std::time::Duration;

/// How long a rule 3 match waits for a pid match before it is accepted: 300 ms, counted from the
/// time the candidate was first offered.
pub const APP_ID_GRACE: Duration = Duration::from_millis(300);

/// Which rule matched. The variants are declared strongest first, so the derived ordering puts
/// `Pid` before `Descendant` before `AppId`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Rule {
    Pid,
    Descendant,
    AppId,
}

/// Applies the rules to one window. `is_descendant(window_pid, launched_pid)` answers rule 2.
/// An empty or `*` hint never matches by rule 3 (it would match every window). `None` for a node
/// that is not a window (`types::is_window`). A window with no pid can still match by rule 3.
pub fn classify(
    node: &Node,
    launched_pid: u32,
    app_hint: &str,
    is_descendant: &dyn Fn(u32, u32) -> bool,
) -> Option<Rule> {
    if !is_window(node) {
        return None;
    }
    if let Some(pid) = node.pid {
        if pid == launched_pid {
            return Some(Rule::Pid);
        }
        if is_descendant(pid, launched_pid) {
            return Some(Rule::Descendant);
        }
    }
    let hint = app_hint.trim();
    if !hint.is_empty() && hint != "*" && match_window_app_hint(hint, &node_class(node), "") {
        return Some(Rule::AppId);
    }
    None
}

/// Collects the offers of window events and says when a window has been chosen.
pub struct Matcher {
    snapshot: HashSet<u64>,
    launched_pid: u32,
    app_hint: String,
    /// The first rule 3 match: the window id and the time (since the start of the wait) it was seen.
    app_id_candidate: Option<(u64, Duration)>,
}

impl Matcher {
    /// `snapshot` holds the ids of the windows that existed when the driver was armed.
    pub fn new(snapshot: HashSet<u64>, launched_pid: u32, app_hint: &str) -> Matcher {
        Matcher {
            snapshot,
            launched_pid,
            app_hint: app_hint.to_string(),
            app_id_candidate: None,
        }
    }

    /// Looks at a window (from a `new` or `title` event, or a tree scan) seen `at` after the
    /// start. Returns its id if it is the window to place right now: a rule 1 or 2 match that
    /// is not in the snapshot. A rule 3 match returns `None` and is remembered, but only the
    /// FIRST one; later rule 3 windows are ignored.
    pub fn offer(
        &mut self,
        node: &Node,
        at: Duration,
        is_descendant: &dyn Fn(u32, u32) -> bool,
    ) -> Option<u64> {
        if self.snapshot.contains(&node.id) {
            return None;
        }
        match classify(node, self.launched_pid, &self.app_hint, is_descendant)? {
            Rule::Pid | Rule::Descendant => Some(node.id),
            Rule::AppId => {
                if self.app_id_candidate.is_none() {
                    self.app_id_candidate = Some((node.id, at));
                }
                None
            }
        }
    }

    /// The remembered rule 3 window, once it has waited `APP_ID_GRACE` by time `now`; with
    /// `last_chance` (the wait is ending) it is returned without waiting.
    pub fn app_id_fallback(&self, now: Duration, last_chance: bool) -> Option<u64> {
        let (id, seen) = self.app_id_candidate?;
        if last_chance || now.saturating_sub(seen) >= APP_ID_GRACE {
            Some(id)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::linux::sway::types::WindowProperties;

    fn win(id: u64, pid: u32, app_id: Option<&str>) -> Node {
        Node {
            id,
            kind: "con".into(),
            pid: Some(pid),
            app_id: app_id.map(String::from),
            ..Node::default()
        }
    }

    fn never(_: u32, _: u32) -> bool {
        false
    }

    /// Rule 1: the launched process owns the window.
    #[test]
    fn test_pid_match() {
        assert_eq!(
            classify(&win(5, 100, Some("x")), 100, "", &never),
            Some(Rule::Pid)
        );
    }

    /// Rule 2: the window's pid is a child of the launched one; the function given decides.
    #[test]
    fn test_descendant_match() {
        let desc = |w: u32, p: u32| w == 200 && p == 100;
        assert_eq!(
            classify(&win(5, 200, None), 100, "", &desc),
            Some(Rule::Descendant)
        );
        assert_eq!(classify(&win(5, 201, None), 100, "", &desc), None);
    }

    /// Rule 3: app id against the hint with the shared hint matching (exact, reverse-DNS last
    /// segment, case-insensitive); a title is never consulted (the hint is not matched against
    /// the window title here).
    #[test]
    fn test_app_id_match_uses_the_shared_hint_rules() {
        let n = win(5, 999, Some("org.gnome.Calculator"));
        assert_eq!(classify(&n, 100, "calculator", &never), Some(Rule::AppId));
        assert_eq!(
            classify(&n, 100, "org.gnome.calculator", &never),
            Some(Rule::AppId)
        );
        assert_eq!(classify(&n, 100, "gedit", &never), None);
        let mut titled = win(6, 999, Some("other"));
        titled.name = Some("calculator".into());
        assert_eq!(classify(&titled, 100, "calculator", &never), None);
    }

    /// An XWayland window has no app id; its X11 class is used.
    #[test]
    fn test_xwayland_class_match() {
        let mut n = win(5, 999, None);
        n.window_properties = Some(WindowProperties {
            class: Some("XTerm".into()),
            instance: None,
            title: None,
        });
        assert_eq!(classify(&n, 100, "xterm", &never), Some(Rule::AppId));
    }

    /// An empty or `*` hint never matches by app id: it would match every window on screen.
    #[test]
    fn test_empty_and_wildcard_hints_never_match() {
        let n = win(5, 999, Some("anything"));
        assert_eq!(classify(&n, 100, "", &never), None);
        assert_eq!(classify(&n, 100, "  ", &never), None);
        assert_eq!(classify(&n, 100, "*", &never), None);
    }

    /// A split container or a workspace is not a window, whatever its pid.
    #[test]
    fn test_non_windows_never_match() {
        let mut n = win(5, 100, Some("x"));
        n.kind = "workspace".into();
        assert_eq!(classify(&n, 100, "x", &never), None);
        let mut split = win(6, 100, Some("x"));
        split.nodes = vec![win(7, 1, None)];
        assert_eq!(classify(&split, 100, "x", &never), None);
    }

    /// A window that was in the tree when the driver was armed is never chosen, even when its
    /// pid is the launched one (a reused pid, or an old window of the same application).
    #[test]
    fn test_snapshot_windows_are_never_chosen() {
        let mut m = Matcher::new(HashSet::from([5]), 100, "x");
        assert_eq!(
            m.offer(&win(5, 100, Some("x")), Duration::ZERO, &never),
            None
        );
        assert_eq!(m.app_id_fallback(Duration::from_secs(10), true), None);
        assert_eq!(
            m.offer(&win(6, 100, Some("x")), Duration::ZERO, &never),
            Some(6)
        );
    }

    /// A rule 3 window waits for a better one for 300 ms: a pid match that arrives in time
    /// wins, and without one the app id window is taken after the grace.
    #[test]
    fn test_app_id_waits_for_a_pid_match() {
        let mut m = Matcher::new(HashSet::new(), 100, "calc");
        assert_eq!(
            m.offer(
                &win(8, 999, Some("calc")),
                Duration::from_millis(50),
                &never
            ),
            None
        );
        assert_eq!(m.app_id_fallback(Duration::from_millis(349), false), None);
        assert_eq!(
            m.app_id_fallback(Duration::from_millis(350), false),
            Some(8)
        );
        // the real one shows up in time
        assert_eq!(
            m.offer(
                &win(9, 100, Some("calc")),
                Duration::from_millis(200),
                &never
            ),
            Some(9)
        );
    }

    /// The first rule 3 window is the one kept; a second does not replace it. When the wait is
    /// about to end the candidate is returned without the grace.
    #[test]
    fn test_first_app_id_candidate_is_kept_and_last_chance_returns_it() {
        let mut m = Matcher::new(HashSet::new(), 100, "calc");
        m.offer(
            &win(8, 999, Some("calc")),
            Duration::from_millis(10),
            &never,
        );
        m.offer(
            &win(9, 998, Some("calc")),
            Duration::from_millis(20),
            &never,
        );
        assert_eq!(m.app_id_fallback(Duration::from_millis(11), true), Some(8));
    }
}
