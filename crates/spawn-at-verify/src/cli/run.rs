//! Handler for `spawn-at-verify run` subcommand.

use super::args::RunArgs;
use super::check::{resolve_bin_path, resolve_verify_dir};
use crate::loader::{TestFilter, TestRegistry};
use crate::report::{build_session_report, write_report_artifacts};
use crate::runner::{FinalStatus, RunOptions, TestRunner};
use std::path::PathBuf;
use std::process::exit;

/// Executes automated verification test suites.
pub fn handle_run(args: RunArgs) {
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

    let (preflight, env_info, results) = match runner.run() {
        Ok(res) => res,
        Err(e) => {
            eprintln!("Runner execution failed: {}", e);
            exit(1);
        }
    };

    // Build full session report
    let session_report = build_session_report(
        &bin_path,
        &preflight,
        &env_info,
        &results,
        args.include_titles,
    );

    // Write report artifacts
    let results_base = PathBuf::from("verify-results");
    let session_dir = write_report_artifacts(&results_base, &session_report).ok();

    if args.json {
        let out = serde_json::to_string_pretty(&session_report).unwrap();
        println!("{}", out);
    } else {
        println!("\n=== Verification Run Results ===");
        let mut passed = 0;
        let mut failed = 0;
        let mut skipped = 0;
        let mut blocked = 0;

        for r in &results {
            match r.final_status {
                FinalStatus::Pass => {
                    passed += 1;
                    println!("  ✓ PASS: {} ({}) [{}ms]", r.id, r.title, r.duration_ms);
                }
                FinalStatus::Fail => {
                    failed += 1;
                    let msg = r.message.as_deref().unwrap_or("Failed");
                    println!(
                        "  ✗ FAIL: {} ({}) [{}ms]: {}",
                        r.id, r.title, r.duration_ms, msg
                    );
                }
                FinalStatus::Skipped => {
                    skipped += 1;
                    let msg = r.message.as_deref().unwrap_or("Unmet requirement");
                    println!("  ⊘ SKIP: {} ({}): {}", r.id, r.title, msg);
                }
                FinalStatus::Blocked => {
                    blocked += 1;
                    let msg = r.message.as_deref().unwrap_or("Blocked by preflight");
                    println!("  ⛔ BLOCKED: {} ({}): {}", r.id, r.title, msg);
                }
                FinalStatus::Error => {
                    failed += 1;
                    let msg = r.message.as_deref().unwrap_or("Harness error");
                    println!("  💥 ERROR: {} ({}): {}", r.id, r.title, msg);
                }
                FinalStatus::Flaky => {
                    failed += 1;
                    println!("  ⚠️ FLAKY: {} ({})", r.id, r.title);
                }
                FinalStatus::Pending => {
                    println!("  ● PENDING: {} ({})", r.id, r.title);
                }
            }
        }

        println!(
            "\nTotal: {}, Passed: {}, Failed: {}, Skipped: {}, Blocked: {}",
            results.len(),
            passed,
            failed,
            skipped,
            blocked
        );

        if !session_report.qualification.skipped_required.is_empty() {
            println!("\n⚠️ Required-But-Skipped Tests (Platform Not Qualified):");
            for sk in &session_report.qualification.skipped_required {
                println!("  - {}: {}", sk.id, sk.reason);
            }
        }

        if let Some(dir) = session_dir {
            println!("\nSession artifacts written to: {}", dir.display());
        }

        if failed > 0 || blocked > 0 {
            exit(1);
        }
    }
}
