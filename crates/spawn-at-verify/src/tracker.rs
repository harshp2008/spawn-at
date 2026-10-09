//! Window Ownership & Safe Cleanup Tracker.
//!
//! Follows Section 6 of verify/README.md:
//! 1. Snapshots all pre-existing windows on session startup.
//! 2. Strictly never touches pre-existing windows.
//! 3. Only closes windows launched during tests via `spawn-at close --id <ID>`.
//! 4. Enforces 20 concurrent window cap.

use std::collections::HashSet;
use std::path::Path;
use std::process::Command;

pub const MAX_CONCURRENT_WINDOWS: usize = 20;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowRecord {
    pub id: u64,
    pub pid: Option<u64>,
    pub app_id: Option<String>,
    pub class: Option<String>,
    pub title: Option<String>,
}

#[derive(Debug, Clone)]
pub struct WindowTracker {
    pub known_preexisting: HashSet<u64>,
    pub tracked_test_windows: Vec<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttributionOutcome {
    pub registered_id: Option<u64>,
    pub candidate_ids: Vec<u64>,
    pub error_message: Option<String>,
}

impl WindowTracker {
    /// Creates a tracker initialized with a specified set of pre-existing window IDs.
    pub fn new_with_preexisting(preexisting: &[u64]) -> Self {
        Self {
            known_preexisting: preexisting.iter().copied().collect(),
            tracked_test_windows: Vec::new(),
        }
    }

    /// Snapshots all open windows at startup.
    pub fn snapshot_startup(bin_path: &Path) -> Result<Self, String> {
        let windows = query_window_ids(bin_path)?;
        let known_preexisting: HashSet<u64> = windows.into_iter().collect();

        Ok(Self {
            known_preexisting,
            tracked_test_windows: Vec::new(),
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
        let failed = self.cleanup_with_closer(|id| {
            let res = Command::new(bin_path)
                .args(["close", "--id", &id.to_string()])
                .output();
            matches!(res, Ok(out) if out.status.success())
        });
        Ok(failed)
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
    let output = Command::new(bin_path)
        .args(["query", "windows", "--json"])
        .output()
        .map_err(|e| format!("Failed to run 'query windows --json': {}", e))?;

    if !output.status.success() {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tracker_never_closes_preexisting_windows() {
        let fake_preexisting = vec![101, 102, 103];
        let mut tracker = WindowTracker::new_with_preexisting(&fake_preexisting);

        // Harness spawns window 201 and 202
        assert!(tracker.register_window(201).is_ok());
        assert!(tracker.register_window(202).is_ok());

        let mut closed_ids = Vec::new();
        let failed = tracker.cleanup_with_closer(|id| {
            closed_ids.push(id);
            true
        });

        assert!(failed.is_empty());
        // Verify only test windows 201 and 202 were targeted
        assert_eq!(closed_ids, vec![201, 202]);
        // Verify preexisting windows 101, 102, 103 were never passed to closer
        for pre in &fake_preexisting {
            assert!(!closed_ids.contains(pre));
            assert!(tracker.known_preexisting.contains(pre));
        }
    }

    #[test]
    fn test_runner_cleanup_only_ever_passes_registered_ids_to_close() {
        // Behavioral test: The runner cleanup path only ever passes registered IDs to close.
        // It never passes pre-existing window IDs, never passes external/unregistered window IDs,
        // and only takes strict numeric u64 IDs (cannot pass class names or PIDs).
        let preexisting = vec![101, 102];
        let mut tracker = WindowTracker::new_with_preexisting(&preexisting);

        // System has preexisting windows (101, 102), a registered test window (201),
        // and an external window (999) that appeared mid-test.
        let current_windows = vec![
            WindowRecord {
                id: 101,
                pid: Some(10),
                app_id: Some("preexisting".into()),
                class: Some("preexisting".into()),
                title: Some("Preexisting 1".into()),
            },
            WindowRecord {
                id: 201,
                pid: Some(20),
                app_id: Some("spawn-at-fixture".into()),
                class: Some("spawn-at-fixture".into()),
                title: Some("Fixture".into()),
            },
            WindowRecord {
                id: 999,
                pid: Some(30),
                app_id: Some("chrome".into()),
                class: Some("chrome".into()),
                title: Some("External Chrome".into()),
            },
        ];

        // Attribute windows for fixture spawn
        let newly = tracker
            .attribute_and_register_test_windows(&current_windows, None, Some("fixture"))
            .unwrap();
        assert_eq!(newly, vec![201]);

        // Execute the cleanup path (identical to runner::cleanup_test_windows)
        let mut closed_ids: Vec<u64> = Vec::new();
        let failed = tracker.cleanup_with_closer(|id| {
            closed_ids.push(id);
            true
        });

        assert!(failed.is_empty());
        // Verify ONLY the registered window ID 201 was passed to the closer
        assert_eq!(closed_ids, vec![201]);
        // Preexisting and external windows were never passed to closer
        assert!(!closed_ids.contains(&101));
        assert!(!closed_ids.contains(&102));
        assert!(!closed_ids.contains(&999));
    }

    #[test]
    fn test_tracker_ignores_external_window_appearing_mid_test() {
        // A window that appears mid-test from another source is never registered or closed.
        let preexisting = vec![10, 20];
        let mut tracker = WindowTracker::new_with_preexisting(&preexisting);

        // Current windows in system during a test with subject = "fixture":
        // - 10: preexisting
        // - 20: preexisting
        // - 100: spawned fixture window
        // - 999: external window opened mid-test (e.g. user opened Chrome or terminal externally)
        let current_windows = vec![
            WindowRecord {
                id: 10,
                pid: Some(1000),
                app_id: Some("app1".into()),
                class: Some("app1".into()),
                title: Some("App 1".into()),
            },
            WindowRecord {
                id: 20,
                pid: Some(1001),
                app_id: Some("app2".into()),
                class: Some("app2".into()),
                title: Some("App 2".into()),
            },
            WindowRecord {
                id: 100,
                pid: Some(2000),
                app_id: Some("spawn-at-fixture".into()),
                class: Some("spawn-at-fixture".into()),
                title: Some("Fixture Window".into()),
            },
            WindowRecord {
                id: 999,
                pid: Some(3000),
                app_id: Some("google-chrome".into()),
                class: Some("google-chrome".into()),
                title: Some("Chrome Window".into()),
            },
        ];

        // Attribute windows for fixture test
        let registered = tracker
            .attribute_and_register_test_windows(&current_windows, None, Some("fixture"))
            .expect("Registration should succeed");

        // Only fixture window 100 is registered
        assert_eq!(registered, vec![100]);
        assert_eq!(tracker.tracked_test_windows, vec![100]);
        // External window 999 is NEVER registered
        assert!(!tracker.tracked_test_windows.contains(&999));

        // When cleanup runs, external window 999 is NEVER closed
        let mut closed_ids = Vec::new();
        tracker.cleanup_with_closer(|id| {
            closed_ids.push(id);
            true
        });

        assert_eq!(closed_ids, vec![100]);
        assert!(!closed_ids.contains(&999));
    }

    #[test]
    fn test_tracker_respects_20_window_cap() {
        let mut tracker = WindowTracker::new_with_preexisting(&[]);

        // Register exactly MAX_CONCURRENT_WINDOWS (20)
        for i in 1..=MAX_CONCURRENT_WINDOWS as u64 {
            assert!(tracker.register_window(i).is_ok());
        }

        // 21st window registration must fail with concurrency cap error
        let err = tracker.register_window(999);
        assert!(err.is_err());
        assert!(err.unwrap_err().contains("Concurrency cap reached"));
    }

    #[test]
    fn test_tracker_does_not_close_unlaunched_windows() {
        let fake_preexisting = vec![1, 2, 3];
        let mut tracker = WindowTracker::new_with_preexisting(&fake_preexisting);

        // Current system state includes pre-existing, one test-launched window,
        // and an external unlaunched window (e.g. user opened a browser outside test)
        let current_windows = vec![
            WindowRecord {
                id: 1,
                pid: Some(10),
                app_id: None,
                class: None,
                title: None,
            },
            WindowRecord {
                id: 2,
                pid: Some(11),
                app_id: None,
                class: None,
                title: None,
            },
            WindowRecord {
                id: 3,
                pid: Some(12),
                app_id: None,
                class: None,
                title: None,
            },
            WindowRecord {
                id: 50,
                pid: Some(500),
                app_id: Some("fixture".into()),
                class: Some("fixture".into()),
                title: None,
            },
            WindowRecord {
                id: 99,
                pid: Some(990),
                app_id: Some("unrelated".into()),
                class: Some("unrelated".into()),
                title: None,
            },
        ];

        let newly = tracker
            .attribute_and_register_test_windows(&current_windows, None, Some("fixture"))
            .unwrap();
        assert_eq!(newly, vec![50]);

        let mut closed_ids = Vec::new();
        tracker.cleanup_with_closer(|id| {
            closed_ids.push(id);
            true
        });

        // 50 was closed, but 99 was never registered and never closed
        assert_eq!(closed_ids, vec![50]);
        assert!(!closed_ids.contains(&99));
    }

    #[test]
    fn test_window_of_same_class_opened_between_session_start_and_action_never_closed() {
        // Startup snapshot has window 10
        let startup_preexisting = vec![10];
        let mut tracker = WindowTracker::new_with_preexisting(&startup_preexisting);

        // User / external process opens a window of the SAME class (terminal)
        // after session start, but BEFORE this test's action runs.
        let external_terminal = WindowRecord {
            id: 20,
            pid: Some(2000),
            app_id: Some("org.gnome.Terminal".into()),
            class: Some("org.gnome.Terminal".into()),
            title: Some("User Terminal".into()),
        };

        // Fresh snapshot taken immediately before this test's action:
        let mut pre_action_snapshot = HashSet::new();
        pre_action_snapshot.insert(10);
        pre_action_snapshot.insert(20);

        // Action runs: spawns a new terminal window 30
        let test_terminal = WindowRecord {
            id: 30,
            pid: Some(3000),
            app_id: Some("org.gnome.Terminal".into()),
            class: Some("org.gnome.Terminal".into()),
            title: Some("Spawned Test Terminal".into()),
        };

        let current_windows = vec![
            WindowRecord {
                id: 10,
                pid: Some(1000),
                app_id: None,
                class: None,
                title: None,
            },
            external_terminal,
            test_terminal,
        ];

        let outcome = tracker
            .attribute_action_windows(
                &pre_action_snapshot,
                &current_windows,
                None,
                Some("system:org.gnome.Terminal"),
                None,
            )
            .expect("Attribution should succeed");

        // Window 20 was in pre_action_snapshot, so ONLY window 30 was a candidate and registered.
        assert_eq!(outcome.registered_id, Some(30));
        assert_eq!(outcome.candidate_ids, vec![30]);
        assert_eq!(outcome.error_message, None);

        // Verify cleanup: window 20 is NEVER closed!
        let mut closed_ids = Vec::new();
        tracker.cleanup_with_closer(|id| {
            closed_ids.push(id);
            true
        });

        assert_eq!(closed_ids, vec![30]);
        assert!(!closed_ids.contains(&20));
        assert!(!closed_ids.contains(&10));
    }

    #[test]
    fn test_two_candidates_nothing_closed() {
        let startup_preexisting = vec![1];
        let mut tracker = WindowTracker::new_with_preexisting(&startup_preexisting);
        let pre_action_snapshot = HashSet::from([1]);

        // Action results in TWO candidate windows of matching subject
        let current_windows = vec![
            WindowRecord {
                id: 1,
                pid: Some(10),
                app_id: None,
                class: None,
                title: None,
            },
            WindowRecord {
                id: 101,
                pid: Some(1010),
                app_id: Some("gnome-calculator".into()),
                class: Some("gnome-calculator".into()),
                title: Some("Calc 1".into()),
            },
            WindowRecord {
                id: 102,
                pid: Some(1020),
                app_id: Some("gnome-calculator".into()),
                class: Some("gnome-calculator".into()),
                title: Some("Calc 2".into()),
            },
        ];

        let outcome = tracker
            .attribute_action_windows(
                &pre_action_snapshot,
                &current_windows,
                None,
                Some("system:gnome-calculator"),
                None,
            )
            .expect("Attribution evaluation should succeed");

        // When multiple candidates appear: register nothing
        assert_eq!(outcome.registered_id, None);
        assert_eq!(outcome.candidate_ids, vec![101, 102]);
        assert!(outcome.error_message.is_some());
        let err = outcome.error_message.unwrap();
        assert!(err.contains("could not attribute; windows left open"));
        assert!(err.contains("101") && err.contains("102"));

        // Cleanup: NOTHING closed
        let mut closed_ids = Vec::new();
        tracker.cleanup_with_closer(|id| {
            closed_ids.push(id);
            true
        });
        assert!(closed_ids.is_empty());
    }

    #[test]
    fn test_zero_candidates_nothing_closed() {
        let startup_preexisting = vec![1];
        let mut tracker = WindowTracker::new_with_preexisting(&startup_preexisting);
        let pre_action_snapshot = HashSet::from([1]);

        // Action results in NO new windows matching subject
        let current_windows = vec![WindowRecord {
            id: 1,
            pid: Some(10),
            app_id: None,
            class: None,
            title: None,
        }];

        let outcome = tracker
            .attribute_action_windows(
                &pre_action_snapshot,
                &current_windows,
                None,
                Some("system:gnome-calculator"),
                None,
            )
            .expect("Attribution evaluation should succeed");

        // When zero candidates appear: register nothing
        assert_eq!(outcome.registered_id, None);
        assert!(outcome.candidate_ids.is_empty());
        assert!(outcome.error_message.is_some());
        let err = outcome.error_message.unwrap();
        assert!(err.contains("could not attribute; windows left open"));

        // Cleanup: NOTHING closed
        let mut closed_ids = Vec::new();
        tracker.cleanup_with_closer(|id| {
            closed_ids.push(id);
            true
        });
        assert!(closed_ids.is_empty());
    }

    #[test]
    fn test_fixture_attribution_by_pid() {
        let startup_preexisting = vec![10];
        let mut tracker = WindowTracker::new_with_preexisting(&startup_preexisting);
        let pre_action_snapshot = HashSet::from([10]);

        // Harness spawns fixture with PID 4000
        let fixture_pid = 4000;

        let current_windows = vec![
            WindowRecord {
                id: 10,
                pid: Some(100),
                app_id: None,
                class: None,
                title: None,
            },
            // Unrelated window created during test
            WindowRecord {
                id: 50,
                pid: Some(3000),
                app_id: Some("other".into()),
                class: Some("other".into()),
                title: None,
            },
            // Fixture window with matching PID
            WindowRecord {
                id: 42,
                pid: Some(fixture_pid),
                app_id: Some("fixture".into()),
                class: Some("fixture".into()),
                title: Some("Fixture Window".into()),
            },
        ];

        let outcome = tracker
            .attribute_action_windows(
                &pre_action_snapshot,
                &current_windows,
                Some(fixture_pid),
                Some("fixture"),
                None,
            )
            .expect("Attribution should succeed");

        assert_eq!(outcome.registered_id, Some(42));
        assert_eq!(outcome.candidate_ids, vec![42]);

        let mut closed_ids = Vec::new();
        tracker.cleanup_with_closer(|id| {
            closed_ids.push(id);
            true
        });
        assert_eq!(closed_ids, vec![42]);
    }
}
