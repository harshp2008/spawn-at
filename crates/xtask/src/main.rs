//! xtask: Internal developer and CI tooling for spawn-at.
//!
//! Provides `check-comment-only` and `check-doc-coverage` without external dependencies.
//! `check-doc-coverage` with no paths gates `crates/spawn-at-verify/src` and only reports a count
//! for `tests/`.
//! `check-doc-coverage` combines a text scan with the compiler's `missing_docs` lint.

mod comment_only;
mod compiler_docs;
mod doc_coverage;

use std::env;
use std::process::exit;

fn main() {
    let mut args = env::args().skip(1);
    let subcommand = match args.next() {
        Some(cmd) => cmd,
        None => {
            eprintln!("Usage: cargo xtask <subcommand> [args...]");
            eprintln!("Subcommands:");
            eprintln!("  check-comment-only <git-rev> [path]");
            eprintln!("  check-doc-coverage [path...]");
            exit(1);
        }
    };

    match subcommand.as_str() {
        "check-comment-only" => {
            let base_commit = match args.next() {
                Some(rev) => rev,
                None => {
                    eprintln!("Usage: cargo xtask check-comment-only <git-rev> [path]");
                    exit(1);
                }
            };
            let target_path = args
                .next()
                .unwrap_or_else(|| "crates/spawn-at-verify".to_string());
            comment_only::run_check_comment_only(&base_commit, &target_path);
        }
        "check-doc-coverage" => {
            let paths: Vec<String> = args.collect();
            // Default: src/ is the gate; tests/ gets an informational count only.
            let (target_paths, informational) = if paths.is_empty() {
                (
                    vec!["crates/spawn-at-verify/src".to_string()],
                    vec!["crates/spawn-at-verify/tests".to_string()],
                )
            } else {
                (paths, Vec::new())
            };
            doc_coverage::run_check_doc_coverage(&target_paths, &informational);
        }
        other => {
            eprintln!("Unknown subcommand: {}", other);
            eprintln!("Available subcommands: check-comment-only, check-doc-coverage");
            exit(1);
        }
    }
}
