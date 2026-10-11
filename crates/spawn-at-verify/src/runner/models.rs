//! Outcome models, execution options, and verdict resolution rules for test runs.

use crate::loader::TestFilter;
use crate::schema::TestKind;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Maximum number of repeat iterations permitted for stress and flakiness verification.
pub const MAX_REPEAT_CAP: usize = 100;

/// Automated result status computed by the test runner prior to human review.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AutoStatus {
    /// Test execution is queued or pending.
    Pending,
    /// Test action is actively executing.
    Running,
    /// Automated checks and assertions passed completely.
    AutoPass,
    /// Automated checks or trace lifecycle assertions failed.
    AutoFail,
    /// Internal execution error or unexpected exception occurred.
    Error,
    /// Test was skipped due to platform or environment filter.
    Skipped,
    /// Test execution is blocked by unmet dependencies.
    Blocked,
}

/// Evaluation verdict provided by a human reviewer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum HumanVerdictKind {
    /// No verdict has been recorded by the reviewer yet.
    #[default]
    Unset,
    /// Reviewer confirmed visual behavior passed specification.
    Pass,
    /// Reviewer identified visual defect or improper behavior.
    Fail,
    /// Reviewer skipped manual evaluation for this case.
    Skip,
    /// Reviewer flagged intermittent or flaky visual behavior.
    Flaky,
}

/// Structured record of human visual evaluation and rationale.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct HumanVerdict {
    /// Recorded human evaluation verdict.
    pub verdict: HumanVerdictKind,
    /// Optional explanatory note or defect description.
    pub note: Option<String>,
}

/// Consolidated final test status integrating automated results and human verdicts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FinalStatus {
    /// Case satisfies all automated and required human criteria.
    Pass,
    /// Case failed automated verification or human review.
    Fail,
    /// Case was skipped and not executed.
    Skipped,
    /// Case execution was blocked by preconditions.
    Blocked,
    /// Case encountered an unrecoverable runtime error.
    Error,
    /// Case exhibited flaky or nondeterministic behavior.
    Flaky,
    /// Case is pending completion or awaiting human review.
    Pending,
}

/// Complete result and diagnostic payload for a verification test execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestResult {
    /// Unique test identifier.
    pub id: String,
    /// Human-readable title of the test specification.
    pub title: String,
    /// Test type classification.
    pub kind: TestKind,
    /// Whether this test is mandatory for suite success.
    pub required: bool,
    /// Automated test execution status.
    pub auto_status: AutoStatus,
    /// Human evaluation verdict and notes.
    pub human_verdict: HumanVerdict,
    /// Effective final resolution status.
    pub final_status: FinalStatus,
    /// True if human verdict contradicted automated evaluation.
    pub disagreement: bool,
    /// Total execution duration in milliseconds.
    pub duration_ms: u64,
    /// Process exit code if action executed.
    pub exit_code: Option<i32>,
    /// Captured standard output.
    pub stdout: String,
    /// Captured standard error.
    pub stderr: String,
    /// Failure reason or diagnostic message.
    pub message: Option<String>,
    /// Relevant slice of compositor log or system journal.
    pub log_slice: Option<String>,
}

impl TestResult {
    /// Cloak duration in milliseconds measured from this result's log slice, or `None` when
    /// there is no slice or the slice holds no complete CLOAK..UNCLOAK pair.
    pub fn cloak_duration_ms(&self) -> Option<f64> {
        let slice = self.log_slice.as_ref()?;
        crate::trace::TraceAnalyser::new()
            .analyse(slice)
            .cloak_duration_ms
    }
    /// Returns the category of this test result based on its ID.
    pub fn category(&self) -> &str {
        if self.id.starts_with("preflight") || self.id.starts_with("basics") {
            "basics"
        } else if self.id.starts_with("placement") {
            "placement"
        } else if self.id.starts_with("cloak") || self.id.starts_with("animation") {
            "animation-and-cloak"
        } else if self.id.starts_with("transform") {
            "transforms"
        } else if self.id.starts_with("focus") {
            "focus"
        } else if self.id.starts_with("query") {
            "queries"
        } else if self.id.starts_with("robust") || self.id.starts_with("resilience") {
            "robustness"
        } else if self.id.starts_with("platform") {
            "platform"
        } else {
            self.id.split('/').next().unwrap_or("basics")
        }
    }
}

/// Resolves the final test status and checks for disagreement between automated and human verdicts.
pub fn resolve_final_status(
    kind: TestKind,
    auto_status: AutoStatus,
    human: &HumanVerdict,
) -> (FinalStatus, bool) {
    let mut disagreement = false;

    let final_status = match kind {
        TestKind::Auto => match auto_status {
            AutoStatus::AutoPass => FinalStatus::Pass,
            AutoStatus::AutoFail => FinalStatus::Fail,
            AutoStatus::Skipped => FinalStatus::Skipped,
            AutoStatus::Blocked => FinalStatus::Blocked,
            AutoStatus::Error => FinalStatus::Error,
            AutoStatus::Pending => FinalStatus::Pending,
            AutoStatus::Running => FinalStatus::Pending,
        },
        TestKind::Human => match human.verdict {
            HumanVerdictKind::Pass => FinalStatus::Pass,
            HumanVerdictKind::Fail => FinalStatus::Fail,
            HumanVerdictKind::Skip => FinalStatus::Skipped,
            HumanVerdictKind::Flaky => FinalStatus::Flaky,
            HumanVerdictKind::Unset => FinalStatus::Pending,
        },
        TestKind::Both => {
            // Rule: Passes ONLY if BOTH auto-pass and human pass are achieved.
            let auto_passed = auto_status == AutoStatus::AutoPass;
            let human_passed = human.verdict == HumanVerdictKind::Pass;

            if (auto_passed && human.verdict == HumanVerdictKind::Fail)
                || (auto_status == AutoStatus::AutoFail && human_passed)
            {
                disagreement = true;
            }

            if auto_status == AutoStatus::Skipped {
                FinalStatus::Skipped
            } else if auto_status == AutoStatus::Blocked {
                FinalStatus::Blocked
            } else if auto_status == AutoStatus::Error {
                FinalStatus::Error
            } else if human.verdict == HumanVerdictKind::Flaky {
                FinalStatus::Flaky
            } else if auto_passed && human_passed {
                FinalStatus::Pass
            } else if auto_status == AutoStatus::AutoFail || human.verdict == HumanVerdictKind::Fail
            {
                FinalStatus::Fail
            } else {
                FinalStatus::Pending
            }
        }
    };

    (final_status, disagreement)
}

/// Configuration options governing test runner execution behavior.
#[derive(Debug, Clone)]
pub struct RunOptions {
    /// Path to the spawn-at CLI binary under test.
    pub bin_path: PathBuf,
    /// Allow running verification despite dirty working tree.
    pub allow_dirty: bool,
    /// Run preflight and discovery without executing actions.
    pub dry_run: bool,
    /// Number of repeat cycles to run each test.
    pub repeat: usize,
    /// Test filter selecting categories or IDs.
    pub filter: TestFilter,
    /// Whether to include redacted window titles in reports.
    pub include_titles: bool,
    /// Maximum duration permitted for individual action steps.
    pub step_timeout: std::time::Duration,
    /// Maximum duration permitted for an entire test case.
    pub test_timeout: std::time::Duration,
}

impl Default for RunOptions {
    /// Returns standard default run options (single iteration, full suite).
    fn default() -> Self {
        Self {
            bin_path: PathBuf::from("target/debug/spawn-at"),
            allow_dirty: false,
            dry_run: false,
            repeat: 1,
            filter: TestFilter::default(),
            include_titles: false,
            step_timeout: std::time::Duration::from_secs(5),
            test_timeout: std::time::Duration::from_secs(10),
        }
    }
}
