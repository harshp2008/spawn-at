//! xtask: Internal developer and CI tooling for spawn-at.
//!
//! Provides `check-comment-only`, `check-doc-coverage`, `burn-cpu` and the documentation-site
//! commands (`docs-site`, `docs-cli-reference`, `docs-check-src`, `docs-check-html`) without
//! external dependencies.
//! `check-doc-coverage` with no paths gates `crates/spawn-at-verify/src` and only reports a count
//! for `tests/`.
//! `check-doc-coverage` combines a text scan with the compiler's `missing_docs` lint.

mod burn_cpu;
mod comment_only;
mod compiler_docs;
mod doc_coverage;
mod docs_check;
mod docs_cli;
mod docs_html;
mod docs_site;

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
            eprintln!("  burn-cpu --threads N --seconds S");
            eprintln!("  docs-site [--mdbook P] [--bin P] [--out DIR]");
            eprintln!("  docs-cli-reference [--bin P] [--out DIR] [--summary FILE]");
            eprintln!("  docs-check-src [DIR]");
            eprintln!("  docs-check-html <site-dir> [--site-prefix P] [--lenient-fragments DIR]");
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
        "burn-cpu" => {
            let rest: Vec<String> = args.collect();
            burn_cpu::run_burn_cpu(&rest);
        }
        "docs-site" | "docs-cli-reference" | "docs-check-src" | "docs-check-html" => {
            let rest: Vec<String> = args.collect();
            let result = match subcommand.as_str() {
                "docs-site" => docs_site::run(&rest),
                "docs-cli-reference" => docs_cli::run(&rest),
                "docs-check-src" => docs_check::run(&rest),
                _ => docs_html::run(&rest),
            };
            if let Err(message) = result {
                eprintln!("{} failed:\n{}", subcommand, message);
                exit(1);
            }
        }
        other => {
            eprintln!("Unknown subcommand: {}", other);
            eprintln!(
                "Available subcommands: check-comment-only, check-doc-coverage, burn-cpu, \
                 docs-site, docs-cli-reference, docs-check-src, docs-check-html"
            );
            exit(1);
        }
    }
}
