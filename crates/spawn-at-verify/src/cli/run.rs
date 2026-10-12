//! Handler for `spawn-at-verify run` subcommand.

use super::args::RunArgs;
use super::check::{resolve_bin_path, resolve_verify_dir};
use super::run_summary::RunSummary;
use crate::ext_logging::echo_timeout_note;
use crate::loader::{TestFilter, TestRegistry};
use crate::preflight::{missing_optional_tools, optional_tools_hint};
use crate::report::{build_session_report_with_runs, write_report_artifacts};
use crate::runner::{FinalStatus, RunOptions, TestResult, TestRunner};
use std::path::PathBuf;
use std::process::exit;

/// Stands in for the echo-timeout note on a repeat, so the long sentence is printed once.
const REPEAT_ECHO_NOTE: &str = "timing log not confirmed (echo timeout, see the first note)";

/// `r` as it should be printed. The first result that carries `note` (in its message or as its
/// `trace_note`) is returned unchanged and sets `shown`; later ones get the note replaced by a
/// short pointer in the message and no `trace_note`, so the sentence appears once per run.
fn collapse_repeat_note(r: &TestResult, note: &str, shown: &mut bool) -> TestResult {
    let in_message = r.message.as_deref().is_some_and(|m| m.contains(note));
    let in_note = r.trace_note.as_deref() == Some(note);
    if !in_message && !in_note {
        return r.clone();
    }
    let repeat = *shown;
    *shown = true;
    let mut out = r.clone();
    if repeat {
        out.message = r
            .message
            .as_ref()
            .map(|m| m.replace(note, REPEAT_ECHO_NOTE));
        if in_note {
            out.trace_note = None;
        }
    }
    out
}

/// Executes automated verification test suites.
pub fn handle_run(args: RunArgs) {
    // Printed to stderr so --json output on stdout stays clean.
    if let Some(hint) = optional_tools_hint(&missing_optional_tools()) {
        eprintln!("Note: {}", hint);
    }

    let verify_dir = match resolve_verify_dir(args.dir) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Error: {}", e);
            exit(1);
        }
    };

    let bin_path = match resolve_bin_path(args.bin) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("Error: {}", e);
            exit(1);
        }
    };

    let reg = match TestRegistry::load_from_dir(&verify_dir) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Error loading test registry: {}", e);
            exit(1);
        }
    };

    let glob_or_filter = args.glob.or(args.filter);

    let filter = TestFilter {
        suite: args.suite,
        category_path: args.category,
        tags: args.tags,
        kind: args.kind.map(Into::into),
        required: if args.required { Some(true) } else { None },
        id_glob: glob_or_filter,
    };

    let runner = TestRunner::new(
        reg,
        RunOptions {
            bin_path: bin_path.clone(),
            allow_dirty: args.allow_dirty,
            dry_run: args.dry_run,
            repeat: args.repeat,
            filter,
            include_titles: args.include_titles,
            ..Default::default()
        },
    );

    let load_start = crate::loadavg::read_loadavg();
    let (preflight, env_info, results, all_runs) = match runner.run_detailed() {
        Ok(res) => res,
        Err(e) => {
            eprintln!("Runner execution failed: {}", e);
            exit(1);
        }
    };
    let load_end = crate::loadavg::read_loadavg();

    // Build full session report
    let mut session_report = build_session_report_with_runs(
        &bin_path,
        &preflight,
        &env_info,
        &results,
        &all_runs,
        args.include_titles,
    );
    session_report.environment.set_load(load_start, load_end);

    // Write report artifacts
    let results_base = PathBuf::from("verify-results");
    let session_dir = write_report_artifacts(&results_base, &session_report).ok();

    let summary = RunSummary::from_results(&results);

    if args.json {
        let out = serde_json::to_string_pretty(&session_report).unwrap();
        println!("{}", out);
    } else {
        println!("\n=== Verification Run Results ===");
        println!("Timing log source: {}", runner.log_source.name());

        let echo_note = echo_timeout_note();
        let mut echo_note_shown = false;
        for r in &results {
            let r = &collapse_repeat_note(r, &echo_note, &mut echo_note_shown);
            match r.final_status {
                FinalStatus::Pass => {
                    println!("  ✓ PASS: {} ({}) [{}ms]", r.id, r.title, r.duration_ms);
                }
                FinalStatus::PassUnchecked => {
                    println!(
                        "  ◐ PASS (geometry unchecked): {} ({}) [{}ms]: {}",
                        r.id,
                        r.title,
                        r.duration_ms,
                        r.message.as_deref().unwrap_or("")
                    );
                }
                FinalStatus::PassTraceUnchecked => {
                    println!(
                        "  ◐ PASS (trace unchecked): {} ({}) [{}ms]: {}",
                        r.id,
                        r.title,
                        r.duration_ms,
                        r.message.as_deref().unwrap_or("")
                    );
                }
                FinalStatus::Fail => {
                    let msg = r.message.as_deref().unwrap_or("Failed");
                    println!(
                        "  ✗ FAIL: {} ({}) [{}ms]: {}",
                        r.id, r.title, r.duration_ms, msg
                    );
                }
                FinalStatus::Skipped => {
                    let msg = r.message.as_deref().unwrap_or("Unmet requirement");
                    println!("  ⊘ SKIP: {} ({}): {}", r.id, r.title, msg);
                }
                FinalStatus::Blocked => {
                    let msg = r.message.as_deref().unwrap_or("Blocked by preflight");
                    println!("  ⛔ BLOCKED: {} ({}): {}", r.id, r.title, msg);
                }
                FinalStatus::Error => {
                    let msg = r.message.as_deref().unwrap_or("Harness error");
                    println!("  💥 ERROR: {} ({}): {}", r.id, r.title, msg);
                }
                FinalStatus::Flaky => {
                    println!("  ⚠️ FLAKY: {} ({})", r.id, r.title);
                }
                FinalStatus::Pending => {
                    println!("  ● PENDING: {} ({})", r.id, r.title);
                }
            }
            // A trace-unchecked message already carries the note as its reason.
            if let Some(note) = r
                .trace_note
                .as_ref()
                .filter(|n| !r.message.as_deref().unwrap_or("").contains(n.as_str()))
            {
                println!("      trace: {}", note);
            }
        }

        for line in summary.lines(results.len()) {
            println!("{}", line);
        }

        if !session_report.qualification.skipped_required.is_empty() {
            println!("\n⚠️ Required-But-Skipped Tests (Platform Not Qualified):");
            for sk in &session_report.qualification.skipped_required {
                println!("  - {}: {}", sk.id, sk.reason);
            }
        }

        if let Some(dir) = session_dir {
            println!("\nSession artifacts written to: {}", dir.display());
        }
    }

    // Same exit code in text and JSON mode (JSON mode used to always exit 0).
    let code = summary.exit_code();
    if code != 0 {
        exit(code);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runner::{AutoStatus, HumanVerdict};
    use crate::schema::TestKind;

    /// A passed result with the given message and `trace_note`.
    fn result(message: Option<&str>, trace_note: Option<&str>) -> TestResult {
        TestResult {
            id: "t".into(),
            title: "t".into(),
            kind: TestKind::Auto,
            required: false,
            auto_status: AutoStatus::AutoPass,
            human_verdict: HumanVerdict::default(),
            final_status: FinalStatus::Pass,
            disagreement: false,
            duration_ms: 1,
            exit_code: Some(0),
            stdout: String::new(),
            stderr: String::new(),
            message: message.map(String::from),
            log_slice: None,
            trace_note: trace_note.map(String::from),
            final_rect: None,
        }
    }

    /// Three results all carrying the echo-timeout note: the first keeps the full sentence in
    /// both places, the 2nd and 3rd keep neither (message points back, `trace_note` cleared).
    /// A result with an unrelated note is untouched. Fails if the sentence prints per test.
    #[test]
    fn test_echo_timeout_note_is_kept_once_per_run() {
        let note = echo_timeout_note();
        let with_msg = format!(
            "Assertions passed, but the trace was not analysed: {}",
            note
        );
        let mut shown = false;

        let first = collapse_repeat_note(&result(Some(&with_msg), Some(&note)), &note, &mut shown);
        assert_eq!(first.message.as_deref(), Some(with_msg.as_str()));
        assert_eq!(first.trace_note.as_deref(), Some(note.as_str()));

        let other = collapse_repeat_note(&result(None, Some("no log source")), &note, &mut shown);
        assert_eq!(other.trace_note.as_deref(), Some("no log source"));

        for _ in 0..2 {
            let again =
                collapse_repeat_note(&result(Some(&with_msg), Some(&note)), &note, &mut shown);
            let msg = again.message.unwrap();
            assert!(!msg.contains(&note), "{}", msg);
            assert!(msg.contains(REPEAT_ECHO_NOTE), "{}", msg);
            assert_eq!(again.trace_note, None);
        }
    }

    /// A passing result that only has the note in `trace_note` loses it on a repeat (no
    /// `trace:` line), while its message stays `None`.
    #[test]
    fn test_echo_timeout_trace_note_only_result_drops_note_on_repeat() {
        let note = echo_timeout_note();
        let mut shown = true;
        let again = collapse_repeat_note(&result(None, Some(&note)), &note, &mut shown);
        assert_eq!(again.trace_note, None);
        assert_eq!(again.message, None);
    }
}
