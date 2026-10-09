//! CLI binary entrypoint for spawn-at-verify.

use clap::{Args, Parser, Subcommand};
use spawn_at_verify::loader::TestRegistry;
use spawn_at_verify::runner::{RunOptions, TestRunner, TestStatus};
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
    /// List available tests and suites
    List(ListArgs),
    /// Validate test definitions and suite schemas
    Check(CheckArgs),
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

    /// Filter tests by ID or title regex
    #[arg(long)]
    filter: Option<String>,

    /// Allow verification against a dirty build
    #[arg(long)]
    allow_dirty: bool,

    /// Dry run: list planned tests without executing actions
    #[arg(long)]
    dry_run: bool,

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

    /// Filter by tag name
    #[arg(long)]
    tag: Option<String>,

    /// Filter by category name
    #[arg(long)]
    category: Option<String>,

    /// Output list in JSON format
    #[arg(long)]
    json: bool,
}

#[derive(Args, Debug)]
struct CheckArgs {
    /// Path to verify test library directory [default: auto-detected verify/]
    #[arg(long)]
    dir: Option<PathBuf>,
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

    // Check ./verify
    let local = PathBuf::from("verify");
    if local.is_dir() {
        return Ok(local);
    }

    // Check ../verify or ../../verify
    let parent = PathBuf::from("../verify");
    if parent.is_dir() {
        return Ok(parent);
    }
    let grandparent = PathBuf::from("../../verify");
    if grandparent.is_dir() {
        return Ok(grandparent);
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

    // Check ./target/debug/spawn-at
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

            let tests = if let Some(suite) = &args.suite {
                match reg.filter_by_suite(suite) {
                    Ok(t) => t,
                    Err(e) => {
                        eprintln!("Error: {}", e);
                        exit(1);
                    }
                }
            } else if let Some(tag) = &args.tag {
                reg.filter_by_tag(tag)
            } else if let Some(cat) = &args.category {
                reg.filter_by_category(cat)
            } else {
                reg.tests
            };

            if args.json {
                let out = serde_json::to_string_pretty(&tests).unwrap();
                println!("{}", out);
            } else {
                println!("Discovered {} tests:", tests.len());
                for t in &tests {
                    let req_str = if t.required { " [REQUIRED]" } else { "" };
                    println!("  - {}: {} ({:?}){}", t.id, t.title, t.kind, req_str);
                }
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

            let runner = TestRunner::new(
                reg,
                RunOptions {
                    bin_path,
                    allow_dirty: args.allow_dirty,
                    dry_run: args.dry_run,
                    suite: args.suite,
                    filter: args.filter,
                },
            );

            let results = match runner.run() {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("Runner execution failed: {}", e);
                    exit(1);
                }
            };

            if args.json {
                let out = serde_json::to_string_pretty(&results).unwrap();
                println!("{}", out);
            } else {
                println!("\n=== Verification Run Results ===");
                let mut passed = 0;
                let mut failed = 0;
                let mut skipped = 0;
                let mut blocked = 0;

                for r in &results {
                    match &r.status {
                        TestStatus::AutoPass => {
                            passed += 1;
                            println!("  ✓ PASS: {} ({}) [{}ms]", r.id, r.title, r.duration_ms);
                        }
                        TestStatus::AutoFail(err) => {
                            failed += 1;
                            println!(
                                "  ✗ FAIL: {} ({}) [{}ms]: {}",
                                r.id, r.title, r.duration_ms, err
                            );
                        }
                        TestStatus::Skipped(reason) => {
                            skipped += 1;
                            println!("  ⊘ SKIP: {} ({}): {}", r.id, r.title, reason);
                        }
                        TestStatus::Blocked(reason) => {
                            blocked += 1;
                            println!("  ⛔ BLOCKED: {} ({}): {}", r.id, r.title, reason);
                        }
                        TestStatus::Error(reason) => {
                            failed += 1;
                            println!("  💥 ERROR: {} ({}): {}", r.id, r.title, reason);
                        }
                        TestStatus::Pending => {
                            println!("  ● PENDING: {} ({})", r.id, r.title);
                        }
                        TestStatus::Running => {}
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

                if failed > 0 || blocked > 0 {
                    exit(1);
                }
            }
        }
    }
}
