//! Counting, summary text and process exit code for `spawn-at-verify run`.

use crate::runner::{FinalStatus, TestResult};

/// Exit code when at least one test failed, errored, was flaky or was blocked.
pub const EXIT_FAILED: i32 = 1;
/// Exit code when nothing failed but at least one REQUIRED test passed with part of its
/// evidence unchecked: its geometry was never verified, or its trace was never captured. It is
/// distinct from 0 so CI does not read it as a full pass.
pub const EXIT_UNCHECKED_REQUIRED: i32 = 2;

/// Help text appended to `run --help` documenting the exit codes.
pub const EXIT_CODE_HELP: &str = "\
Exit codes:
  0  no test failed; skipped, pending, and optional tests that passed with geometry or
     trace unchecked do not change the exit code
  1  at least one test failed, errored, was flaky, or was blocked
  2  nothing failed, but at least one REQUIRED test passed with part of its evidence unchecked:
     its geometry (no workarea was available, so the placement was never verified) or its
     trace (the extension's timing log held no events for the window): not a full pass
A failure (1) takes precedence over unchecked required passes (2).
A PASS (flash) (the placement is right but the window was shown before it was ready, or its size
or position changed after the reveal) does not change the exit code; with --strict-flash it is a
failure (1).";

/// Tallies of a run's final statuses.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct RunSummary {
    /// Plain passes.
    pub passed: usize,
    /// Failed, errored or flaky tests.
    pub failed: usize,
    /// Skipped tests.
    pub skipped: usize,
    /// Blocked tests.
    pub blocked: usize,
    /// Ids of required tests that passed with geometry unchecked.
    pub unchecked_required: Vec<String>,
    /// Ids of optional tests that passed with geometry unchecked.
    pub unchecked_optional: Vec<String>,
    /// Ids of required tests that passed with their trace unchecked.
    pub trace_unchecked_required: Vec<String>,
    /// Ids of optional tests that passed with their trace unchecked.
    pub trace_unchecked_optional: Vec<String>,
    /// Ids of tests that passed on placement and flashed: "PASS (flash)". They are counted in
    /// `passed` too; `--strict-flash` makes them failures instead, and they are then in `failed`.
    pub flashed: Vec<String>,
    /// How many of `flashed` have at least one flash whose lead is a full frame period or more
    /// on this screen; the rest are only possible (a sub-frame lead).
    pub flashed_certain: usize,
}

impl RunSummary {
    /// Tallies `results` by final status. An unchecked pass is counted separately, never as a
    /// plain pass.
    pub fn from_results(results: &[TestResult]) -> Self {
        let mut s = Self::default();
        for r in results {
            if r.is_flash_pass() {
                s.flashed.push(r.id.clone());
                if r.flash_info.flash_certain > 0 {
                    s.flashed_certain += 1;
                }
            }
            match r.final_status {
                FinalStatus::Pass => s.passed += 1,
                FinalStatus::PassUnchecked if r.required => s.unchecked_required.push(r.id.clone()),
                FinalStatus::PassUnchecked => s.unchecked_optional.push(r.id.clone()),
                FinalStatus::PassTraceUnchecked if r.required => {
                    s.trace_unchecked_required.push(r.id.clone())
                }
                FinalStatus::PassTraceUnchecked => s.trace_unchecked_optional.push(r.id.clone()),
                FinalStatus::Fail | FinalStatus::Error | FinalStatus::Flaky => s.failed += 1,
                FinalStatus::Skipped => s.skipped += 1,
                FinalStatus::Blocked => s.blocked += 1,
                FinalStatus::Pending => {}
            }
        }
        s
    }

    /// Process exit code: [`EXIT_FAILED`] for any failure or block, else
    /// [`EXIT_UNCHECKED_REQUIRED`] if a required test passed with its geometry or trace
    /// unchecked, else 0.
    pub fn exit_code(&self) -> i32 {
        if self.failed > 0 || self.blocked > 0 {
            EXIT_FAILED
        } else if !self.unchecked_required.is_empty() || !self.trace_unchecked_required.is_empty() {
            EXIT_UNCHECKED_REQUIRED
        } else {
            0
        }
    }

    /// Summary lines printed after the per-test results.
    pub fn lines(&self, total: usize) -> Vec<String> {
        let mut out = vec![format!(
            "\nTotal: {}, Passed: {}, Failed: {}, Skipped: {}, Blocked: {}",
            total, self.passed, self.failed, self.skipped, self.blocked
        )];
        if !self.flashed.is_empty() {
            out.push(format!(
                "Flash: {} of the {} passed test(s) flashed (PASS (flash); still counted as Passed, \
                 --strict-flash fails them): {}",
                self.flashed.len(),
                self.passed,
                self.flashed.join(", ")
                    + &format!(
                        " [{} certain, {} possible on this screen]",
                        self.flashed_certain,
                        self.flashed.len() - self.flashed_certain
                    )
            ));
        }
        for (what, required, optional) in [
            (
                "geometry",
                &self.unchecked_required,
                &self.unchecked_optional,
            ),
            (
                "trace",
                &self.trace_unchecked_required,
                &self.trace_unchecked_optional,
            ),
        ] {
            let unchecked = required.len() + optional.len();
            if unchecked > 0 {
                out.push(format!(
                    "Passed with {} unchecked (not counted as Passed): {} (required: {}, optional: {})",
                    what,
                    unchecked,
                    required.len(),
                    optional.len()
                ));
            }
            if !required.is_empty() {
                out.push(format!(
                    "\n⚠ {} required test(s) passed with {} UNCHECKED; this is not a full pass (exit code {}):",
                    required.len(),
                    what,
                    EXIT_UNCHECKED_REQUIRED
                ));
                for id in required {
                    out.push(format!("  - {}", id));
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runner::{AutoStatus, HumanVerdict};
    use crate::schema::TestKind;

    /// A result with the given id, required flag and final status.
    fn result(id: &str, required: bool, status: FinalStatus) -> TestResult {
        TestResult {
            id: id.into(),
            title: id.into(),
            kind: TestKind::Auto,
            required,
            auto_status: AutoStatus::AutoPass,
            human_verdict: HumanVerdict::default(),
            final_status: status,
            disagreement: false,
            duration_ms: 1,
            exit_code: Some(0),
            stdout: String::new(),
            stderr: String::new(),
            message: None,
            log_slice: None,
            trace_note: None,
            final_rect: None,
            flash_info: Default::default(),
            wait_info: Default::default(),
        }
    }

    /// All plain passes: exit 0, no unchecked lines.
    #[test]
    fn test_all_pass_exits_zero() {
        let s = RunSummary::from_results(&[result("a", true, FinalStatus::Pass)]);
        assert_eq!(s.exit_code(), 0);
        assert_eq!(s.lines(1).len(), 1);
    }

    /// A REQUIRED unchecked pass is not a full pass: exit 2, listed by id, counted apart from
    /// Passed, and the summary says so.
    #[test]
    fn test_required_unchecked_pass_exits_two() {
        let s = RunSummary::from_results(&[
            result("a", true, FinalStatus::Pass),
            result("b", true, FinalStatus::PassUnchecked),
        ]);
        assert_eq!(
            (s.passed, s.unchecked_required.clone()),
            (1, vec!["b".to_string()])
        );
        assert_eq!(s.exit_code(), EXIT_UNCHECKED_REQUIRED);
        let text = s.lines(2).join("\n");
        assert!(text.contains("Passed: 1"), "{}", text);
        assert!(text.contains("required: 1, optional: 0"), "{}", text);
        assert!(
            text.contains("1 required test(s) passed with geometry UNCHECKED"),
            "{}",
            text
        );
        assert!(
            text.contains("exit code 2") && text.contains("  - b"),
            "{}",
            text
        );
    }

    /// An OPTIONAL unchecked pass is reported but keeps exit 0.
    #[test]
    fn test_optional_unchecked_pass_exits_zero_but_is_reported() {
        let s = RunSummary::from_results(&[result("opt", false, FinalStatus::PassUnchecked)]);
        assert_eq!(s.exit_code(), 0);
        assert_eq!(s.passed, 0);
        let text = s.lines(1).join("\n");
        assert!(text.contains("required: 0, optional: 1"), "{}", text);
        assert!(!text.contains("not a full pass"), "{}", text);
    }

    /// A geometry failure is a `Fail`: exit 1, and failure outranks an unchecked required pass.
    #[test]
    fn test_geometry_fail_exits_one_and_outranks_unchecked() {
        let fail = result("g", true, FinalStatus::Fail);
        let s = RunSummary::from_results(&[fail]);
        assert_eq!((s.failed, s.exit_code()), (1, EXIT_FAILED));
        let s = RunSummary::from_results(&[
            result("g", true, FinalStatus::Fail),
            result("u", true, FinalStatus::PassUnchecked),
        ]);
        assert_eq!(s.exit_code(), EXIT_FAILED);
    }

    /// A REQUIRED pass with its trace unchecked follows the same rule as geometry unchecked:
    /// exit 2, listed by id, counted apart from Passed, and named as "trace" in the summary.
    #[test]
    fn test_required_trace_unchecked_pass_exits_two() {
        let s = RunSummary::from_results(&[
            result("a", true, FinalStatus::Pass),
            result("t", true, FinalStatus::PassTraceUnchecked),
        ]);
        assert_eq!(s.passed, 1);
        assert_eq!(s.trace_unchecked_required, vec!["t".to_string()]);
        assert!(s.unchecked_required.is_empty());
        assert_eq!(s.exit_code(), EXIT_UNCHECKED_REQUIRED);
        let text = s.lines(2).join("\n");
        assert!(
            text.contains(
                "Passed with trace unchecked (not counted as Passed): 1 (required: 1, optional: 0)"
            ),
            "{}",
            text
        );
        assert!(
            text.contains("1 required test(s) passed with trace UNCHECKED"),
            "{}",
            text
        );
        assert!(
            text.contains("exit code 2") && text.contains("  - t"),
            "{}",
            text
        );
    }

    /// An OPTIONAL trace-unchecked pass keeps exit 0 but is reported; a failure outranks a
    /// required trace-unchecked pass.
    #[test]
    fn test_optional_trace_unchecked_exits_zero_and_failure_outranks_it() {
        let s = RunSummary::from_results(&[result("o", false, FinalStatus::PassTraceUnchecked)]);
        assert_eq!((s.passed, s.exit_code()), (0, 0));
        let text = s.lines(1).join("\n");
        assert!(text.contains("required: 0, optional: 1"), "{}", text);
        assert!(!text.contains("not a full pass"), "{}", text);

        let s = RunSummary::from_results(&[
            result("f", true, FinalStatus::Fail),
            result("t", true, FinalStatus::PassTraceUnchecked),
        ]);
        assert_eq!(s.exit_code(), EXIT_FAILED);
    }

    /// Error, flaky and blocked also exit 1; skipped and pending do not.
    #[test]
    fn test_other_statuses_map_to_exit_codes() {
        for (st, code) in [
            (FinalStatus::Error, 1),
            (FinalStatus::Flaky, 1),
            (FinalStatus::Blocked, 1),
            (FinalStatus::Skipped, 0),
            (FinalStatus::Pending, 0),
        ] {
            let s = RunSummary::from_results(&[result("x", true, st)]);
            assert_eq!(s.exit_code(), code, "{:?}", st);
        }
    }

    /// A flashing pass is still a pass (counted in Passed, exit 0) and is named in its own
    /// summary line with the count of flashes; an unflashed run adds no such line. A pass whose
    /// geometry is unchecked is not a "PASS (flash)" even if it flashed.
    #[test]
    fn test_flash_pass_is_counted_apart_and_keeps_exit_zero() {
        let mut flashed = result("f", true, FinalStatus::Pass);
        flashed.flash_info.flash = true;
        let mut unchecked = result("u", false, FinalStatus::PassUnchecked);
        unchecked.flash_info.flash = true;
        let s =
            RunSummary::from_results(&[flashed, result("ok", true, FinalStatus::Pass), unchecked]);
        assert_eq!(
            (s.passed, s.flashed.clone(), s.exit_code()),
            (2, vec!["f".to_string()], 0)
        );
        let text = s.lines(3).join("\n");
        assert!(
            text.contains("Flash: 1 of the 2 passed test(s) flashed (PASS (flash)"),
            "{}",
            text
        );
        assert!(text.contains("--strict-flash fails them): f"), "{}", text);
        let clean = RunSummary::from_results(&[result("ok", true, FinalStatus::Pass)]);
        assert!(!clean.lines(1).join("\n").contains("Flash:"));
    }

    /// With `--strict-flash` the runner turns the flashing pass into a Fail, so the summary counts
    /// it as failed and the exit code is 1; it is no longer listed as a flash pass.
    #[test]
    fn test_strict_flash_failure_exits_one() {
        let mut failed = result("f", true, FinalStatus::Fail);
        failed.auto_status = AutoStatus::AutoFail;
        failed.flash_info.flash = true;
        let s = RunSummary::from_results(&[failed]);
        assert_eq!(
            (s.failed, s.flashed.len(), s.exit_code()),
            (1, 0, EXIT_FAILED)
        );
    }
}
