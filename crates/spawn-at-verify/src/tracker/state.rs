//! Window tracker state, attribution algorithms, and window leak cleanup.

use crate::runner::owned::{SetupSession, TeardownReport};
use std::collections::HashSet;
use std::path::Path;
use std::process::Command;
use std::sync::{Arc, Mutex};

/// Maximum number of concurrent test windows tracked to prevent runaway leakage.
pub const MAX_CONCURRENT_WINDOWS: usize = 20;

/// Snapshot record of an open window queried from the compositor or X11 server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowRecord {
    /// Compositor or X11 window identifier.
    pub id: u64,
    /// Process ID owning the window, if known.
    pub pid: Option<u64>,
    /// Wayland application ID.
    pub app_id: Option<String>,
    /// X11 WM_CLASS or window class name.
    pub class: Option<String>,
    /// Window title text.
    pub title: Option<String>,
}

/// Version 2 setup sessions that outlive their test (a Both or Human test keeps its window until
/// the verdict). Held by the tracker so every cleanup path (end of `run`, the TUI's Ctrl-C, quit
/// and signals, `close_test_windows`) reaches them. Clones share the same sessions.
#[derive(Clone, Default)]
pub struct OwnedSessions(Arc<Mutex<Vec<SetupSession>>>);

impl std::fmt::Debug for OwnedSessions {
    /// Prints how many sessions are held.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "OwnedSessions({})", self.len())
    }
}

impl OwnedSessions {
    /// Takes over a session whose test is not finished (its windows stay open).
    pub fn push(&self, session: SetupSession) {
        self.0.lock().unwrap().push(session);
    }

    /// Number of sessions held.
    pub fn len(&self) -> usize {
        self.0.lock().unwrap().len()
    }

    /// True when no session is held.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Tears down every held session, newest first, and forgets them.
    pub fn teardown_all(&self) -> Vec<TeardownReport> {
        let mut sessions = std::mem::take(&mut *self.0.lock().unwrap());
        let mut reports = Vec::new();
        while let Some(mut s) = sessions.pop() {
            reports.push(s.teardown());
        }
        reports
    }
}

/// Short lines about a test that are neither its verdict nor a failure: that focus could not be
/// returned to the terminal, that teardown left a window behind. Kept by test id; the TUI shows
/// them next to the result and never lets them change a verdict. Clones share the same notes.
#[derive(Clone, Default)]
pub struct SessionNotes(Arc<Mutex<std::collections::HashMap<String, Vec<String>>>>);

impl std::fmt::Debug for SessionNotes {
    /// Prints how many tests have notes.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SessionNotes({})", self.0.lock().unwrap().len())
    }
}

impl SessionNotes {
    /// Adds a line for `test_id`.
    pub fn add(&self, test_id: &str, line: impl Into<String>) {
        self.0
            .lock()
            .unwrap()
            .entry(test_id.to_string())
            .or_default()
            .push(line.into());
    }

    /// The lines noted for `test_id`, oldest first.
    pub fn get(&self, test_id: &str) -> Vec<String> {
        self.0
            .lock()
            .unwrap()
            .get(test_id)
            .cloned()
            .unwrap_or_default()
    }

    /// Forgets the lines of `test_id` (a re-run starts clean).
    pub fn clear(&self, test_id: &str) {
        self.0.lock().unwrap().remove(test_id);
    }
}

/// Tracks windows launched during verification runs to isolate test windows and clean up leaks.
#[derive(Debug, Clone)]
pub struct WindowTracker {
    /// Set of pre-existing window IDs present prior to session start.
    pub known_preexisting: HashSet<u64>,
    /// IDs of windows launched by and attributed to active test runs.
    pub tracked_test_windows: Vec<u64>,
    /// Version 2 setup sessions still open (closed by id and stopped through their `Child`).
    pub owned: OwnedSessions,
    /// Lines about a test that are not its verdict (focus not returned, a window left behind).
    pub notes: SessionNotes,
}

/// Result of window attribution matching an action launch with candidate windows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttributionOutcome {
    /// Successfully attributed window ID registered for tracking.
    pub registered_id: Option<u64>,
    /// All candidate window IDs observed after launch.
    pub candidate_ids: Vec<u64>,
    /// Diagnostic error message if attribution failed or was ambiguous.
    pub error_message: Option<String>,
}

impl WindowTracker {
    /// Creates a tracker initialized with a specified set of pre-existing window IDs.
    pub fn new_with_preexisting(preexisting: &[u64]) -> Self {
        Self {
            known_preexisting: preexisting.iter().copied().collect(),
            tracked_test_windows: Vec::new(),
            owned: OwnedSessions::default(),
            notes: SessionNotes::default(),
        }
    }

    /// Snapshots all open windows at startup.
    pub fn snapshot_startup(bin_path: &Path) -> Result<Self, String> {
        let windows = query_window_ids(bin_path)?;
        let known_preexisting: HashSet<u64> = windows.into_iter().collect();

        Ok(Self {
            known_preexisting,
            tracked_test_windows: Vec::new(),
            owned: OwnedSessions::default(),
            notes: SessionNotes::default(),
        })
    }

    /// Attributes and registers windows spawned strictly by this test's action.
    ///
    /// Safety requirements:
    /// 1. Fresh window snapshot immediately before each test's action (`pre_action_snapshot`).
    /// 2. Windows in `known_preexisting` (session start) or `pre_action_snapshot` are NEVER candidates.
    /// 3. Fixtures: attribute by the process the harness itself spawned (`pid -> window id`).
    /// 4. Real apps: register ONLY if exactly ONE new window appeared during that action AND
    ///    it matches the subject.
    /// 5. If there are zero or several candidates, register nothing, close nothing,
    ///    and set `error_message` containing "could not attribute; windows left open" with candidate list.
    /// 6. The subject.app class fallback is removed as a sole basis for registration.
    pub fn attribute_action_windows(
        &mut self,
        pre_action_snapshot: &HashSet<u64>,
        post_action_windows: &[WindowRecord],
        fixture_pid: Option<u64>,
        subject_app: Option<&str>,
        claimed_id: Option<u64>,
    ) -> Result<AttributionOutcome, String> {
        // Windows that are newly created strictly during this action:
        let new_windows: Vec<&WindowRecord> = post_action_windows
            .iter()
            .filter(|w| {
                !self.known_preexisting.contains(&w.id) && !pre_action_snapshot.contains(&w.id)
            })
            .collect();

        // 1. Explicit claim ID (if available from signal or CLI)
        if let Some(cid) = claimed_id {
            if let Some(w) = new_windows.iter().find(|w| w.id == cid) {
                self.register_window(w.id)?;
                return Ok(AttributionOutcome {
                    registered_id: Some(w.id),
                    candidate_ids: vec![w.id],
                    error_message: None,
                });
            }
        }

        // 2. Fixtures: attribute by process PID spawned by harness (pid -> window id)
        if let Some(f_pid) = fixture_pid {
            let pid_matches: Vec<u64> = new_windows
                .iter()
                .filter(|w| w.pid == Some(f_pid))
                .map(|w| w.id)
                .collect();

            if pid_matches.len() == 1 {
                let id = pid_matches[0];
                self.register_window(id)?;
                return Ok(AttributionOutcome {
                    registered_id: Some(id),
                    candidate_ids: pid_matches,
                    error_message: None,
                });
            } else if pid_matches.len() > 1 {
                return Ok(AttributionOutcome {
                    registered_id: None,
                    candidate_ids: pid_matches.clone(),
                    error_message: Some(format!(
                        "could not attribute; windows left open: {:?}",
                        pid_matches
                    )),
                });
            }
        }

        // 3. Candidate filtering: match subject app if present
        let candidates: Vec<u64> = if let Some(app) = subject_app {
            let target = app.strip_prefix("system:").unwrap_or(app).to_lowercase();
            new_windows
                .iter()
                .filter(|w| {
                    let matches_app_id = w
                        .app_id
                        .as_deref()
                        .map(|a| {
                            let al = a.to_lowercase();
                            al.contains(&target)
                                || target.contains(&al)
                                || (target == "fixture" && al.contains("fixture"))
                        })
                        .unwrap_or(false);
                    let matches_class = w
                        .class
                        .as_deref()
                        .map(|c| {
                            let cl = c.to_lowercase();
                            cl.contains(&target)
                                || target.contains(&cl)
                                || (target == "fixture" && cl.contains("fixture"))
                        })
                        .unwrap_or(false);
                    matches_app_id || matches_class
                })
                .map(|w| w.id)
                .collect()
        } else {
            new_windows.iter().map(|w| w.id).collect()
        };

        // Real apps: register only if exactly ONE candidate appeared
        if candidates.len() == 1 {
            let id = candidates[0];
            self.register_window(id)?;
            Ok(AttributionOutcome {
                registered_id: Some(id),
                candidate_ids: candidates,
                error_message: None,
            })
        } else {
            // Zero or several candidates -> register nothing, close nothing
            Ok(AttributionOutcome {
                registered_id: None,
                candidate_ids: candidates.clone(),
                error_message: Some(format!(
                    "could not attribute; windows left open: {:?}",
                    candidates
                )),
            })
        }
    }

    /// Legacy convenience wrapper: calls `attribute_action_windows` with empty pre-action snapshot.
    pub fn attribute_and_register_test_windows(
        &mut self,
        current_windows: &[WindowRecord],
        claimed_id: Option<u64>,
        subject_app: Option<&str>,
    ) -> Result<Vec<u64>, String> {
        let empty_pre = HashSet::new();
        let outcome = self.attribute_action_windows(
            &empty_pre,
            current_windows,
            None,
            subject_app,
            claimed_id,
        )?;
        Ok(outcome.registered_id.into_iter().collect())
    }

    /// Registers a newly created test window. Refuses if the 20-window cap is reached.
    pub fn register_window(&mut self, window_id: u64) -> Result<(), String> {
        if self.tracked_test_windows.contains(&window_id) {
            return Ok(());
        }

        if self.tracked_test_windows.len() >= MAX_CONCURRENT_WINDOWS {
            return Err(format!(
                "Concurrency cap reached: harness is tracking {} test windows (maximum allowed is {}). Clean up windows before launching more.",
                self.tracked_test_windows.len(),
                MAX_CONCURRENT_WINDOWS
            ));
        }

        self.tracked_test_windows.push(window_id);
        Ok(())
    }

    /// Closes all test-spawned windows using `spawn-at close --id <ID>`.
    /// Never touches any pre-existing window.
    pub fn cleanup_test_windows(&mut self, bin_path: &Path) -> Result<Vec<u64>, String> {
        self.cleanup_test_windows_reported(bin_path).map(|(f, _)| f)
    }

    /// [`WindowTracker::cleanup_test_windows`] that also returns what the teardown of the
    /// version 2 sessions had to say (warnings, windows left behind), for the TUI to show apart
    /// from any verdict.
    pub fn cleanup_test_windows_reported(
        &mut self,
        bin_path: &Path,
    ) -> Result<(Vec<u64>, Vec<TeardownReport>), String> {
        // Version 2 sessions first: each closes its own recorded windows by id and stops its own
        // fixture processes through their `Child` handles.
        let reports = self.owned.teardown_all();
        let failed = self.cleanup_with_closer(|id| {
            let mut cmd = Command::new(bin_path);
            cmd.args(["close", "--id", &id.to_string()]);
            match crate::process::execute_with_timeout(cmd, std::time::Duration::from_secs(5)) {
                Ok(out) => out.exit_code == Some(0),
                Err(_) => false,
            }
        });
        Ok((failed, reports))
    }

    /// Closes test windows using an injected closure (useful for testing and deterministic mocking).
    /// Returns any IDs that failed to close.
    pub fn cleanup_with_closer<F>(&mut self, mut close_fn: F) -> Vec<u64>
    where
        F: FnMut(u64) -> bool,
    {
        let mut failed = Vec::new();
        for &id in &self.tracked_test_windows {
            if !close_fn(id) {
                failed.push(id);
            }
        }
        self.tracked_test_windows.clear();
        failed
    }
}

/// Helper to query structured window records from `spawn-at query windows --json`.
pub fn query_window_records(bin_path: &Path) -> Result<Vec<WindowRecord>, String> {
    let mut cmd = Command::new(bin_path);
    cmd.args(["query", "windows", "--json"]);
    let output = crate::process::execute_with_timeout(cmd, std::time::Duration::from_secs(5))
        .map_err(|e| format!("Failed to run 'query windows --json': {}", e))?;

    if output.exit_code != Some(0) {
        return Ok(Vec::new());
    }

    let parsed: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("Failed to parse windows JSON: {}", e))?;

    let mut records = Vec::new();
    if let Some(arr) = parsed.as_array() {
        for w in arr {
            if let Some(id) = w.get("id").and_then(|v| v.as_u64()) {
                records.push(WindowRecord {
                    id,
                    pid: w.get("pid").and_then(|v| v.as_u64()),
                    app_id: w
                        .get("app_id")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string()),
                    class: w
                        .get("class")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string()),
                    title: w
                        .get("title")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string()),
                });
            }
        }
    }

    Ok(records)
}

/// Helper to query current window IDs from `spawn-at query windows --json`.
pub fn query_window_ids(bin_path: &Path) -> Result<Vec<u64>, String> {
    let records = query_window_records(bin_path)?;
    Ok(records.into_iter().map(|r| r.id).collect())
}

/// Reads window `id`'s current rectangle (`x`, `y`, `w`, `h`) from `spawn-at query windows
/// --json`, the same read the zenity gate used to find where a window ended up after
/// `spawn-at` had exited. `None` when the query fails, the window is gone, or an entry lacks
/// one of the four numbers.
pub fn query_window_rect(bin_path: &Path, id: u64) -> Option<crate::oracle::Rect> {
    let mut cmd = Command::new(bin_path);
    cmd.args(["query", "windows", "--json"]);
    let output =
        crate::process::execute_with_timeout(cmd, std::time::Duration::from_secs(5)).ok()?;
    if output.exit_code != Some(0) {
        return None;
    }
    window_rect_from_json(&output.stdout, id)
}

/// Rectangle of window `id` in the JSON array printed by `query windows --json`.
pub fn window_rect_from_json(json: &[u8], id: u64) -> Option<crate::oracle::Rect> {
    let parsed: serde_json::Value = serde_json::from_slice(json).ok()?;
    let w = parsed
        .as_array()?
        .iter()
        .find(|w| w.get("id").and_then(|v| v.as_u64()) == Some(id))?;
    Some(crate::oracle::Rect::new(
        i32::try_from(w.get("x")?.as_i64()?).ok()?,
        i32::try_from(w.get("y")?.as_i64()?).ok()?,
        u32::try_from(w.get("w")?.as_u64()?).ok()?,
        u32::try_from(w.get("h")?.as_u64()?).ok()?,
    ))
}

#[cfg(test)]
mod rect_tests {
    use super::*;

    /// The entry with the requested id supplies the rectangle; a different id's numbers are
    /// not used, and an unknown id or a missing field gives `None`.
    #[test]
    fn test_window_rect_from_json_picks_the_requested_id() {
        let json = br#"[{"id":5,"x":0,"y":0,"w":10,"h":10},{"id":7,"x":1504,"y":764,"w":400,"h":300},{"id":9,"x":1,"y":2,"w":3}]"#;
        assert_eq!(
            window_rect_from_json(json, 7),
            Some(crate::oracle::Rect::new(1504, 764, 400, 300))
        );
        assert_eq!(window_rect_from_json(json, 8), None);
        assert_eq!(window_rect_from_json(json, 9), None, "no h");
        assert_eq!(window_rect_from_json(b"not json", 7), None);
    }
}
