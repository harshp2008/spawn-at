//! Handler for `spawn-at-verify check` subcommand.

use super::args::CheckArgs;
use crate::loader::TestRegistry;
use std::path::PathBuf;
use std::process::exit;

/// Resolves the filesystem path to the verify test definitions directory.
pub fn resolve_verify_dir(explicit: Option<PathBuf>) -> Result<PathBuf, String> {
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

/// Resolves the filesystem path to the target spawn-at CLI binary.
pub fn resolve_bin_path(explicit: Option<PathBuf>) -> Result<PathBuf, String> {
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

/// Executes test and suite verification checks.
pub fn handle_check(args: CheckArgs) {
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
                reg.tests
                    .iter()
                    .map(|t| t.category())
                    .collect::<std::collections::HashSet<_>>()
                    .len()
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
