//! Handler for `spawn-at-verify list` subcommand.

use super::args::ListArgs;
use super::check::resolve_verify_dir;
use crate::loader::{format_test_tree, TestFilter, TestRegistry};
use std::process::exit;

/// Lists discovered tests and suites in tree format.
pub fn handle_list(args: ListArgs) {
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
