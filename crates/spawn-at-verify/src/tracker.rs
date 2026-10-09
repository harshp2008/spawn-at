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

    /// Attributes and registers windows spawned strictly by the test action.
    ///
    /// Attribution requires the window to:
    /// 1. NOT be in the pre-existing snapshot.
    /// 2. Strictly originate from this test's own spawn:
    ///    - Explicit claim window ID (from SpawnClaimed signal or claim result), OR
    ///    - The fixture's own app_id / class (e.g. "fixture", "spawn-at-fixture"), OR
    ///    - The test subject's target app_id / class.
    ///
    /// Any external window that appears mid-test from another source is NEVER registered.
    pub fn attribute_and_register_test_windows(
        &mut self,
        current_windows: &[WindowRecord],
        claimed_id: Option<u64>,
        subject_app: Option<&str>,
    ) -> Result<Vec<u64>, String> {
        let matched_ids = self.filter_test_windows(current_windows, claimed_id, subject_app);
        let mut newly_registered = Vec::new();
        for id in matched_ids {
            if !self.tracked_test_windows.contains(&id) {
                self.register_window(id)?;
                newly_registered.push(id);
            }
        }
        Ok(newly_registered)
    }

    /// Pure in-memory filtering: returns only windows that match this test's own spawn
    /// and were not present in the pre-existing snapshot.
    pub fn filter_test_windows(
        &self,
        current_windows: &[WindowRecord],
        claimed_id: Option<u64>,
        subject_app: Option<&str>,
    ) -> Vec<u64> {
        let mut matched = Vec::new();
        for win in current_windows {
            // Must not be pre-existing
            if self.known_preexisting.contains(&win.id) {
                continue;
            }

            // Attribution check:
            let is_attributed = if let Some(cid) = claimed_id {
                win.id == cid
            } else if let Some(app) = subject_app {
                let target = app.strip_prefix("system:").unwrap_or(app).to_lowercase();
                let matches_app_id = win
                    .app_id
                    .as_deref()
                    .map(|a| {
                        let al = a.to_lowercase();
                        al.contains(&target)
                            || target.contains(&al)
                            || (target == "fixture" && al.contains("fixture"))
                    })
                    .unwrap_or(false);
                let matches_class = win
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
            } else {
                false
            };

            if is_attributed {
                matched.push(win.id);
            }
        }
        matched
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
}
