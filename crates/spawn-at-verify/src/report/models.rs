//! Session report and qualification summary data structures.

use crate::oracle::Rect;
use crate::runner::TestResult;
use serde::{Deserialize, Serialize};

/// System environment and binary fingerprint recorded during test execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentFingerprint {
    /// Hostname of the system where tests were executed.
    pub hostname: String,
    /// Desktop session type ("wayland", "x11", or "unknown").
    pub session_type: String,
    /// Active desktop environment ("gnome", etc.).
    pub desktop: String,
    /// Major version of GNOME Shell if detected.
    pub gnome_version: Option<u32>,
    /// Number of connected displays.
    pub monitors_count: usize,
    /// Primary workarea from `<bin> query layout` (on GNOME, the extension under test). Absent in
    /// recordings made before this field existed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub primary_workarea: Option<Rect>,
    /// Primary workarea read from the window system (mutter's `_NET_WORKAREA`), independent of
    /// the extension. Absent in older recordings and when it could not be read.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub independent_workarea: Option<Rect>,
    /// Absolute path to the verified spawn-at binary.
    pub binary_path: String,
    /// Git commit recorded in the binary build metadata.
    pub binary_commit: Option<String>,
    /// Whether the binary was built from a dirty working tree.
    pub is_dirty_build: bool,
    /// Unix timestamp in seconds when report was generated.
    pub generated_at: u64,
    /// `/proc/loadavg` when the run started. Absent in older recordings and where unreadable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub load_avg_start: Option<crate::loadavg::LoadAvg>,
    /// `/proc/loadavg` when the run ended.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub load_avg_end: Option<crate::loadavg::LoadAvg>,
    /// CPUs available to the run, the basis of the busy-machine warning.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cpu_count: Option<usize>,
    /// Display refresh rate the flash visibility labels were computed for (60 when it could not
    /// be read; see `refresh_source`). Absent in older recordings.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refresh_hz: Option<f64>,
    /// One frame period in milliseconds at `refresh_hz`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frame_ms: Option<f64>,
    /// Where the refresh rate came from, or why 60 Hz was assumed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refresh_source: Option<String>,
}

impl EnvironmentFingerprint {
    /// Records the load averages at the start and end of the run and the CPU count.
    pub fn set_load(
        &mut self,
        start: Option<crate::loadavg::LoadAvg>,
        end: Option<crate::loadavg::LoadAvg>,
    ) {
        self.load_avg_start = start;
        self.load_avg_end = end;
        self.cpu_count = Some(crate::loadavg::cpu_count());
    }
}

/// Diagnostic record for a required test that was skipped during qualification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequiredSkippedItem {
    /// Identifier of the skipped test.
    pub id: String,
    /// Human-readable title of the test specification.
    pub title: String,
    /// Reason explaining why the test was skipped.
    pub reason: String,
}

/// Overall platform qualification verdict summarizing required test outcomes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualificationReport {
    /// True if all mandatory tests passed and platform is fully qualified.
    pub supported: bool,
    /// Total count of required test cases in the test suite.
    pub total_required: usize,
    /// Number of required test cases that passed.
    pub passed_required: usize,
    /// Number of required test cases that failed (fail, error, flaky, blocked, or a pass whose
    /// geometry or trace was not verified). Pending cases are not counted here.
    pub failed_required: usize,
    /// Number of required test cases still pending (not run, or waiting for a human verdict).
    /// They keep `supported` false but are not failures. Absent in older reports (read as 0).
    #[serde(default)]
    pub pending_required: usize,
    /// Required tests that were skipped due to environment filters.
    pub skipped_required: Vec<RequiredSkippedItem>,
}

/// Complete root verification session report persisted to .
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionReport {
    /// Host environment and binary build fingerprint.
    pub environment: EnvironmentFingerprint,
    /// Overall platform qualification evaluation.
    pub qualification: QualificationReport,
    /// Detailed individual test execution results.
    pub results: Vec<TestResult>,
    /// Every repetition of each test that was run more than once (`--repeat N`), in execution
    /// order. `results` keeps the first run of such a test (marked flaky if outcomes differed);
    /// this field holds all N. Absent for single runs and in older reports.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub repeat_runs: Vec<RepeatRuns>,
}

/// All repetitions of one test from a `--repeat N` run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepeatRuns {
    /// Test id the runs belong to.
    pub id: String,
    /// One result per repetition, in execution order.
    pub runs: Vec<TestResult>,
}
