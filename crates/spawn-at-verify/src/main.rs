//! CLI binary entrypoint for spawn-at-verify.

use clap::{Args, Parser, Subcommand, ValueEnum};
use spawn_at_verify::loader::{format_test_tree, TestFilter, TestRegistry};
use spawn_at_verify::report::{
    build_session_report, render_markdown_report, write_report_artifacts, SessionReport,
};
use spawn_at_verify::runner::{FinalStatus, RunOptions, TestRunner};
use spawn_at_verify::schema::TestKind;
use std::fs;
use std::path::PathBuf;
use std::process::exit;

#[derive(Parser, Debug)]
#[command(
    name = "spawn-at-verify",
    about = "Verification Engine & Test Harness for spawn-at",
    version = "0.1.0"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Execute automated verification tests
    Run(RunArgs),
    /// List available tests and suites formatted as a tree with counts
    List(ListArgs),
    /// Validate test definitions and suite schemas
    Check(CheckArgs),
    /// Re-render or inspect past verification reports
    Report(ReportArgs),
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum KindArg {
    Auto,
    Human,
    Both,
}

impl From<KindArg> for TestKind {
    fn from(k: KindArg) -> Self {
        match k {
            KindArg::Auto => TestKind::Auto,
            KindArg::Human => TestKind::Human,
            KindArg::Both => TestKind::Both,
        }
    }
}

#[derive(Args, Debug)]
struct RunArgs {
    /// Path to spawn-at binary under test [default: target/debug/spawn-at]
    #[arg(long)]
    bin: Option<PathBuf>,

    /// Path to verify test library directory [default: auto-detected verify/]
    #[arg(long)]
    dir: Option<PathBuf>,

    /// Run a predefined test suite (e.g. quick, release-gate, visual)
    #[arg(long)]
    suite: Option<String>,

    /// Select by category path prefix (e.g. placement/, cloak/no-flash)
    #[arg(long)]
    category: Option<String>,

    /// Select by tag (can be specified multiple times)
    #[arg(long = "tag")]
    tags: Vec<String>,

    /// Select by test kind (auto, human, both)
    #[arg(long)]
    kind: Option<KindArg>,

    /// Select required tests only
    #[arg(long)]
    required: bool,

    /// Filter tests by ID glob pattern (e.g. *bottom*, placement/*)
    #[arg(long)]
    glob: Option<String>,

    /// Legacy filter argument (alias for glob)
    #[arg(long)]
    filter: Option<String>,

    /// Repeat each test N times (cap 100) for flakiness detection
    #[arg(long, default_value = "1")]
    repeat: usize,

    /// Allow verification against a dirty build
    #[arg(long)]
    allow_dirty: bool,

    /// Dry run: list planned tests without executing actions
    #[arg(long)]
    dry_run: bool,

    /// Include real window titles in reports (opt out of default redaction)
    #[arg(long)]
    include_titles: bool,

    /// Output results in JSON format
    #[arg(long)]
    json: bool,
}

#[derive(Args, Debug)]
struct ListArgs {
    /// Path to verify test library directory [default: auto-detected verify/]
    #[arg(long)]
    dir: Option<PathBuf>,

    /// Filter by suite name
    #[arg(long)]
    suite: Option<String>,

    /// Select by category path prefix (e.g. placement/, cloak/no-flash)
    #[arg(long)]
    category: Option<String>,

    /// Filter by tag (can be specified multiple times)
    #[arg(long = "tag")]
    tags: Vec<String>,

    /// Filter by test kind (auto, human, both)
    #[arg(long)]
    kind: Option<KindArg>,

    /// Filter required tests only
    #[arg(long)]
    required: bool,

    /// Filter by ID glob pattern
    #[arg(long)]
    glob: Option<String>,

    /// Output list in raw JSON format
    #[arg(long)]
    json: bool,
}

#[derive(Args, Debug)]
struct CheckArgs {
    /// Path to verify test library directory [default: auto-detected verify/]
    #[arg(long)]
    dir: Option<PathBuf>,
}

#[derive(Args, Debug)]
struct ReportArgs {
    /// Path to session report directory or report.json [default: latest in verify-results/]
    path: Option<PathBuf>,
}

fn resolve_verify_dir(explicit: Option<PathBuf>) -> Result<PathBuf, String> {
    if let Some(p) = explicit {
        if p.is_dir() {
            return Ok(p);
        }
        return Err(format!(
            "Specified verify directory '{}' not found",
            p.display()
        ));
    }

    let candidates = [
        PathBuf::from("verify"),
        PathBuf::from("../verify"),
        PathBuf::from("../../verify"),
    ];

    for c in &candidates {
        if c.is_dir() {
            return Ok(c.clone());
        }
    }

    Err("Could not find 'verify/' test directory. Use --dir <PATH>.".into())
}

fn resolve_bin_path(explicit: Option<PathBuf>) -> Result<PathBuf, String> {
    if let Some(p) = explicit {
        if p.exists() {
            return Ok(p);
        }
        return Err(format!("Specified binary '{}' not found", p.display()));
    }

    let candidates = [
        PathBuf::from("target/debug/spawn-at"),
        PathBuf::from("../../target/debug/spawn-at"),
        PathBuf::from("../target/debug/spawn-at"),
        PathBuf::from("target/release/spawn-at"),
    ];

    for c in &candidates {
        if c.exists() {
            return Ok(c.clone());
        }
    }

    Err(
        "Could not find 'spawn-at' executable. Build it with 'cargo build' or pass --bin <PATH>."
            .into(),
    )
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Check(args) => {
            let verify_dir = match resolve_verify_dir(args.dir) {
                Ok(d) => d,
                Err(e) => {
                    eprintln!("Error: {}", e);
                    exit(1);
                }
            };

            println!("Validating test definitions in {}...", verify_dir.display());
            match TestRegistry::load_from_dir(&verify_dir) {
                Ok(reg) => {
                    println!(
                        "✓ Loaded {} test definitions (including matrix expansions) across {} categories",
                        reg.tests.len(),
                        reg.tests.iter().map(|t| t.category()).collect::<std::collections::HashSet<_>>().len()
                    );
                    println!(
                        "✓ Loaded {} suite definitions: {:?}",
                        reg.suites.len(),
                        reg.suites.keys().collect::<Vec<_>>()
                    );
                    println!("All verification schemas are valid!");
                }
                Err(e) => {
                    eprintln!("Validation failed: {}", e);
                    exit(1);
                }
            }
        }
        Commands::List(args) => {
            let verify_dir = match resolve_verify_dir(args.dir) {
                Ok(d) => d,
                Err(e) => {
                    eprintln!("Error: {}", e);
                    exit(1);
                }
            };

            let reg = match TestRegistry::load_from_dir(&verify_dir) {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("Error loading registry: {}", e);
                    exit(1);
                }
            };

            let filter = TestFilter {
                suite: args.suite,
                category_path: args.category,
                tags: args.tags,
                kind: args.kind.map(Into::into),
                required: if args.required { Some(true) } else { None },
                id_glob: args.glob,
            };

            let tests = match reg.filter(&filter) {
                Ok(t) => t,
                Err(e) => {
                    eprintln!("Error: {}", e);
                    exit(1);
                }
            };

            if args.json {
                let out = serde_json::to_string_pretty(&tests).unwrap();
                println!("{}", out);
            } else {
                println!("{}", format_test_tree(&tests));
            }
        }
        Commands::Run(args) => {
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
        Commands::Report(args) => {
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
    }
}
