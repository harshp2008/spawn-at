//! Test runner execution engine, repeat loops, and process orchestration.

use super::models::{FinalStatus, RunOptions, TestResult, MAX_REPEAT_CAP};
use super::owned::{ActionRunner, Control};
use crate::ext_logging::{GdbusLogging, LoggingControl, NoLogging};
use crate::loader::TestRegistry;
use crate::log_source::{GnomeJournalSource, LogSource, NoOpLogSource};
use crate::preflight::{inspect_binary, PreflightReport};
use crate::probe::EnvironmentInfo;
use crate::tracker::WindowTracker;
use std::sync::Arc;

/// Orchestrates test suite execution, preflight checks, and window lifecycle attribution.
pub struct TestRunner {
    /// Registry containing all loaded verification test specifications.
    pub registry: TestRegistry,
    /// Execution options configuring repetitions, timeouts, and filters.
    pub options: RunOptions,
    /// Provider source for capturing system and compositor journal logs.
    pub log_source: Box<dyn LogSource>,
    /// Switch that turns the extension's timing log on before each run that needs a trace.
    pub logging: Box<dyn LoggingControl>,
    /// How version 2 tests list, close and refocus windows; `None` means the `spawn-at` binary
    /// of the options. Tests inject a fake.
    pub(crate) control: Option<Arc<dyn Control>>,
    /// How version 2 tests run their actions; `None` means the `spawn-at` binary of the options.
    pub(crate) actions: Option<Arc<dyn ActionRunner>>,
    /// Waits and grace periods of version 2 tests (shortened by tests).
    pub(crate) v2_timings: super::owned::Timings,
}

/// Why the no-op log source is in use: names `XDG_CURRENT_DESKTOP` and what it holds.
fn no_log_source_reason() -> String {
    match std::env::var("XDG_CURRENT_DESKTOP") {
        Ok(d) => format!(
            "no log source: XDG_CURRENT_DESKTOP is '{}', which does not contain 'gnome'",
            d
        ),
        Err(_) => "no log source: XDG_CURRENT_DESKTOP is unset".to_string(),
    }
}

/// The logging control that goes with `source`: D-Bus for the extension's journal, otherwise
/// none. A no-op source carries `noop_reason` so the lack of a trace is explained.
fn logging_for(source: &dyn LogSource, noop_reason: Option<String>) -> Box<dyn LoggingControl> {
    match source.name() {
        "journalctl" => Box::new(GdbusLogging::new()),
        "noop" => Box::new(NoLogging {
            reason: noop_reason,
        }),
        _ => Box::new(NoLogging { reason: None }),
    }
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
        let logging = logging_for(&*log_source, Some(no_log_source_reason()));

        Self {
            registry,
            options,
            log_source,
            logging,
            control: None,
            actions: None,
            v2_timings: Default::default(),
        }
    }

    /// Shortens the waits and grace periods of version 2 tests (for tests with fakes).
    pub fn with_v2_timings(mut self, timings: super::owned::Timings) -> Self {
        self.v2_timings = timings;
        self
    }

    /// Makes version 2 tests list, close and refocus windows through `control` instead of the
    /// `spawn-at` binary (for tests with a fake compositor).
    pub fn with_control(mut self, control: Arc<dyn Control>) -> Self {
        self.control = Some(control);
        self
    }

    /// Makes version 2 tests run their actions through `actions` instead of the `spawn-at` binary.
    pub fn with_actions(mut self, actions: Arc<dyn ActionRunner>) -> Self {
        self.actions = Some(actions);
        self
    }

    /// Attaches an explicit log source provider to the test runner, with the logging control
    /// that goes with it (none for a source that is not the extension's journal).
    pub fn with_log_source(mut self, log_source: Box<dyn LogSource>) -> Self {
        self.logging = logging_for(
            &*log_source,
            Some("no log source: a no-op log source was attached".to_string()),
        );
        self.log_source = log_source;
        self
    }

    /// Attaches an explicit logging control (for example a fake extension in tests).
    pub fn with_logging_control(mut self, logging: Box<dyn LoggingControl>) -> Self {
        self.logging = logging;
        self
    }

    /// Runs all selected tests and returns the execution report.
    ///
    /// With `--repeat N` only the first run of each test is kept in the returned results;
    /// use [`TestRunner::run_detailed`] to also get every individual run.
    pub fn run(&self) -> Result<(PreflightReport, EnvironmentInfo, Vec<TestResult>), String> {
        self.run_detailed().map(|(p, e, r, _)| (p, e, r))
    }

    /// Like [`TestRunner::run`], but also returns, per selected test, the results of every
    /// repetition in execution order (a single entry when `repeat` is 1).
    #[allow(clippy::type_complexity)]
    pub fn run_detailed(
        &self,
    ) -> Result<
        (
            PreflightReport,
            EnvironmentInfo,
            Vec<TestResult>,
            Vec<Vec<TestResult>>,
        ),
        String,
    > {
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
            owned: Default::default(),
        });

        let repeat_count = self.options.repeat.clamp(1, MAX_REPEAT_CAP);
        let mut results = Vec::new();
        let mut all_runs: Vec<Vec<TestResult>> = Vec::new();

        for test in &tests_to_run {
            // Ctrl-C / SIGTERM / SIGHUP in `run`: start nothing new; the tracker cleanup below
            // closes whatever is still open.
            if self
                .options
                .interrupted
                .load(std::sync::atomic::Ordering::SeqCst)
            {
                break;
            }
            if repeat_count == 1 {
                let res = self.execute_single_test(test, &preflight, &env_info, &mut tracker);
                all_runs.push(vec![res.clone()]);
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

                all_runs.push(run_results.clone());
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

        Ok((preflight, env_info, results, all_runs))
    }
}
