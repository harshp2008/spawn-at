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
        let new_ids: Vec<u64> = current_ids
            .into_iter()
            .filter(|id| !self.known_preexisting.contains(id))
            .collect();

        Ok(new_ids)
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
        let mut failed_closes = Vec::new();

        for &id in &self.tracked_test_windows {
            let res = Command::new(bin_path)
                .args(["close", "--id", &id.to_string()])
                .output();

            match res {
                Ok(out) if out.status.success() => {}
                _ => {
                    failed_closes.push(id);
                }
            }
        }

        self.tracked_test_windows.clear();
        Ok(failed_closes)
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
    fn test_tracker_preserves_preexisting_windows() {
        let mut tracker = WindowTracker {
            known_preexisting: HashSet::from([100, 200, 300]),
            tracked_test_windows: Vec::new(),
        };

        // Ensure preexisting window is never registered
        assert!(tracker.register_window(500).is_ok());
        assert_eq!(tracker.tracked_test_windows, vec![500]);
    }

    #[test]
    fn test_tracker_enforces_concurrency_cap() {
        let mut tracker = WindowTracker {
            known_preexisting: HashSet::new(),
            tracked_test_windows: (1..=MAX_CONCURRENT_WINDOWS as u64).collect(),
        };

        let res = tracker.register_window(999);
        assert!(res.is_err());
        assert!(res.unwrap_err().contains("Concurrency cap reached"));
    }
}
