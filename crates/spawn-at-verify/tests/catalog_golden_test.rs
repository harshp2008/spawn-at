//! Golden of the whole loaded catalog: every spec of `verify/` after matrix expansion, serialized,
//! and which tests each suite selects. The runner-setup work (`docs/DESIGN_RUNNER_SETUP.md`
//! section 2.4) adds optional keys to the spec schema; this test fails if any of the existing
//! specs, or any suite's selection, changes meaning or serialization.
//!
//! The golden file was taken from the code before the first schema change. To regenerate it on
//! purpose (a spec was added or edited): `UPDATE_CATALOG_GOLDEN=1 cargo test -p spawn-at-verify
//! --test catalog_golden_test`, and review the diff.

use spawn_at_verify::loader::TestRegistry;
use std::path::{Path, PathBuf};

fn verify_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("verify")
}

fn golden_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/catalog_v1.json")
}

/// The catalog as one JSON document: `tests` in load order, and `suites` mapping each suite name
/// (sorted) to the ids it selects, in load order.
fn catalog_json() -> String {
    let reg = TestRegistry::load_from_dir(verify_dir()).expect("failed to load verify/");
    let mut suites = serde_json::Map::new();
    let mut names: Vec<&String> = reg.suites.keys().collect();
    names.sort();
    for name in names {
        let suite = &reg.suites[name];
        let ids: Vec<&str> = reg
            .tests
            .iter()
            .filter(|t| suite.matches(t))
            .map(|t| t.id.as_str())
            .collect();
        suites.insert(name.clone(), serde_json::json!(ids));
    }
    let doc = serde_json::json!({ "tests": reg.tests, "suites": suites });
    let mut text = serde_json::to_string_pretty(&doc).unwrap();
    text.push('\n');
    text
}

/// The 45 existing specs and the five suites serialize and select exactly as in the golden.
#[test]
fn test_catalog_matches_the_golden() {
    let now = catalog_json();
    if std::env::var_os("UPDATE_CATALOG_GOLDEN").is_some() {
        std::fs::write(golden_path(), &now).unwrap();
        return;
    }
    let golden = std::fs::read_to_string(golden_path()).expect("missing catalog golden");
    assert!(
        now == golden,
        "the loaded catalog differs from tests/golden/catalog_v1.json; \
         existing specs must keep their exact meaning"
    );
}
