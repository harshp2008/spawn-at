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

    /// Queries the currently open windows and returns any newly spawned window IDs
    /// not present in the initial pre-existing snapshot.
    pub fn detect_new_windows(&self, bin_path: &Path) -> Result<Vec<u64>, String> {
        let current_ids = query_window_ids(bin_path)?;
        let new_ids = self.filter_new_windows(&current_ids);
        Ok(new_ids)
    }

    /// Pure in-memory filtering: returns only windows that were not in pre-existing snapshot.
    pub fn filter_new_windows(&self, current_ids: &[u64]) -> Vec<u64> {
        current_ids
            .iter()
            .copied()
            .filter(|id| !self.known_preexisting.contains(id))
            .collect()
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

/// Helper to query current window IDs from `spawn-at query windows --json`.
pub fn query_window_ids(bin_path: &Path) -> Result<Vec<u64>, String> {
    let output = Command::new(bin_path)
        .args(["query", "windows", "--json"])
        .output()
        .map_err(|e| format!("Failed to run 'query windows --json': {}", e))?;

    if !output.status.success() {
        return Ok(Vec::new());
    }

    let parsed: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("Failed to parse windows JSON: {}", e))?;

    let mut ids = Vec::new();
    if let Some(arr) = parsed.as_array() {
        for w in arr {
            if let Some(id) = w.get("id").and_then(|v| v.as_u64()) {
                ids.push(id);
            }
        }
    }

    Ok(ids)
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
    fn test_tracker_never_closes_by_class_or_pid() {
        // WindowTracker interface enforces strictly numeric window IDs (u64).
        // It provides zero methods accepting class names or PIDs for closing.
        let mut tracker = WindowTracker::new_with_preexisting(&[10]);

        // Attempting to register and close requires strict u64
        assert!(tracker.register_window(42).is_ok());

        let mut invocations = Vec::new();
        tracker.cleanup_with_closer(|id| {
            invocations.push(id);
            true
        });

        assert_eq!(invocations, vec![42]);
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
        let current_windows = vec![1, 2, 3, 50, 99]; // 50 is test-launched, 99 is external unlaunched
        let detected = tracker.filter_new_windows(&current_windows);
        assert_eq!(detected, vec![50, 99]);

        // Harness only launched and registered 50
        assert!(tracker.register_window(50).is_ok());

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
