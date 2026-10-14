//! Handler for `spawn-at-verify report` subcommand.

use super::args::ReportArgs;
use crate::report::{render_markdown_report, SessionReport};
use std::fs;
use std::path::PathBuf;
use std::process::exit;

/// Displays or re-renders previous session reports.
pub fn handle_report(args: ReportArgs) {
    let target_path = if let Some(p) = args.path {
        p
    } else {
        // Find latest session in verify-results/
        let results_dir = PathBuf::from("verify-results");
        if !results_dir.is_dir() {
            eprintln!("No verify-results directory found.");
            exit(1);
        }

        let mut sessions: Vec<PathBuf> = fs::read_dir(&results_dir)
            .map(|rd| {
                rd.filter_map(|e| e.ok().map(|ent| ent.path()))
                    .filter(|p| p.is_dir())
                    .collect()
            })
            .unwrap_or_default();

        sessions.sort();
        match sessions.pop() {
            Some(latest) => latest,
            None => {
                eprintln!("No past verification sessions found in verify-results/");
                exit(1);
            }
        }
    };

    let report_json_path = if target_path.is_file() {
        target_path
    } else {
        target_path.join("report.json")
    };

    if !report_json_path.exists() {
        eprintln!("Report file '{}' not found", report_json_path.display());
        exit(1);
    }

    let content = match fs::read_to_string(&report_json_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to read report: {}", e);
            exit(1);
        }
    };

    let report: SessionReport = match serde_json::from_str(&content) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Failed to parse report JSON: {}", e);
            exit(1);
        }
    };

    println!("{}", render_markdown_report(&report));
}
