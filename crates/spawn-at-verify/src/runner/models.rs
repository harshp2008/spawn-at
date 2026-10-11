//! Outcome models, execution options, and verdict resolution rules for test runs.

use crate::loader::TestFilter;
use crate::schema::TestKind;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const MAX_REPEAT_CAP: usize = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AutoStatus {
    Pending,
    Running,
    AutoPass,
    AutoFail,
    Error,
    Skipped,
    Blocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum HumanVerdictKind {
    #[default]
    Unset,
    Pass,
    Fail,
    Skip,
    Flaky,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct HumanVerdict {
    pub verdict: HumanVerdictKind,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FinalStatus {
    Pass,
    Fail,
    Skipped,
    Blocked,
    Error,
    Flaky,
    Pending,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestResult {
    pub id: String,
    pub title: String,
    pub kind: TestKind,
    pub required: bool,
    pub auto_status: AutoStatus,
    pub human_verdict: HumanVerdict,
    pub final_status: FinalStatus,
    pub disagreement: bool,
    pub duration_ms: u64,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub message: Option<String>,
    pub log_slice: Option<String>,
}

impl TestResult {
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

#[derive(Debug, Clone)]
pub struct RunOptions {
    pub bin_path: PathBuf,
    pub allow_dirty: bool,
    pub dry_run: bool,
    pub repeat: usize,
    pub filter: TestFilter,
    pub include_titles: bool,
    pub step_timeout: std::time::Duration,
    pub test_timeout: std::time::Duration,
}

impl Default for RunOptions {
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
