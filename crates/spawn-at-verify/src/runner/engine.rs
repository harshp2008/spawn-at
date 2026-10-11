//! Test runner execution engine, repeat loops, and process orchestration.

use super::models::{FinalStatus, RunOptions, TestResult, MAX_REPEAT_CAP};
use crate::loader::TestRegistry;
use crate::log_source::{GnomeJournalSource, LogSource, NoOpLogSource};
use crate::preflight::{inspect_binary, PreflightReport};
use crate::probe::EnvironmentInfo;
use crate::tracker::WindowTracker;

/// Orchestrates test suite execution, preflight checks, and window lifecycle attribution.
pub struct TestRunner {
    /// Registry containing all loaded verification test specifications.
    pub registry: TestRegistry,
    /// Execution options configuring repetitions, timeouts, and filters.
    pub options: RunOptions,
    /// Provider source for capturing system and compositor journal logs.
    pub log_source: Box<dyn LogSource>,
}

impl TestRunner {
    /// Constructs a new `TestRunner` with the given test registry and execution options.
    pub fn new(registry: TestRegistry, options: RunOptions) -> Self {
        let is_gnome = std::env::var("XDG_CURRENT_DESKTOP")
            .map(|d| d.to_lowercase().contains("gnome"))
            .unwrap_or(false);

        let log_source: Box<dyn LogSource> = if is_gnome {
            Box::new(GnomeJournalSource)
        } else {
            Box::new(NoOpLogSource)
        };

        Self {
            registry,
            options,
            log_source,
        }
    }

    /// Attaches an explicit log source provider to the test runner.
    pub fn with_log_source(mut self, log_source: Box<dyn LogSource>) -> Self {
        self.log_source = log_source;
        self
    }

    /// Runs all selected tests and returns the execution report.
    pub fn run(&self) -> Result<(PreflightReport, EnvironmentInfo, Vec<TestResult>), String> {
        let bin = &self.options.bin_path;
        if !bin.exists() {
            return Err(format!("Target binary '{}' not found", bin.display()));
        }

        // 1. Run Preflight Inspection
        let preflight = inspect_binary(bin)?;

        let is_release_gate = self.options.filter.suite.as_deref() == Some("release-gate");
        if is_release_gate && preflight.is_dirty_build && !self.options.allow_dirty {
            return Err(
                "release-gate suite strictly refuses to run against dirty build without --allow-dirty"
                    .into(),
            );
        }

        // 2. Filter tests
        let tests_to_run = self.registry.filter(&self.options.filter)?;

        // 3. Probe environment
        let env_info = EnvironmentInfo::probe(Some(bin));

        // 4. Snapshot initial windows
        let mut tracker = WindowTracker::snapshot_startup(bin).unwrap_or_else(|_| WindowTracker {
            known_preexisting: Default::default(),
            tracked_test_windows: Default::default(),
        });

        let repeat_count = self.options.repeat.clamp(1, MAX_REPEAT_CAP);
        let mut results = Vec::new();

        for test in &tests_to_run {
            if repeat_count == 1 {
                let res = self.execute_single_test(test, &preflight, &env_info, &mut tracker);
                results.push(res);
            } else {
                // Multi-run flakiness detection across N repetitions
                let mut run_results = Vec::new();
                for _ in 0..repeat_count {
                    let r = self.execute_single_test(test, &preflight, &env_info, &mut tracker);
                    run_results.push(r);
                }

                // Check for mixed results
                let all_same = run_results
                    .windows(2)
                    .all(|w| w[0].final_status == w[1].final_status);

                let mut combined = run_results.into_iter().next().unwrap();
                if !all_same {
                    combined.final_status = FinalStatus::Flaky;
                    combined.message = Some(format!(
                        "Multi-run inconsistency: detected varying outcomes across {} repetitions",
                        repeat_count
                    ));
                }
                results.push(combined);
            }
        }

        // 5. Final cleanup of any lingering windows
        let _ = tracker.cleanup_test_windows(bin);

        Ok((preflight, env_info, results))
    }
}
