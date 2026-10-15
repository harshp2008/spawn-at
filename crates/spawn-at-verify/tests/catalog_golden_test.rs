//! Golden of the original catalog: the 45 version 1 specs of `verify/` after matrix expansion,
//! serialized, and which of them each suite selects. `docs/DESIGN_RUNNER_SETUP.md` section 2.4
//! requires that the optional keys of schema version 2 leave every existing spec exactly as it
//! was; this test fails if any of the 45, or any suite's selection among them, changes.
//!
//! The golden file `tests/golden/catalog_v1.json` was taken from the code before the first schema
//! change and has never been regenerated. Specs added later (the version 2 catalog specs) are not
//! in it: the test compares only the entries whose ids are in the golden, in load order, so new
//! specs may be added but none of the 45 may change, move out of a suite or disappear.
//! A deliberate change to one of the 45 needs `UPDATE_CATALOG_GOLDEN=1 cargo test -p
//! spawn-at-verify --test catalog_golden_test` and a reviewed diff of the golden file.

use serde_json::Value;
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

/// The whole current catalog as one JSON document: `tests` in load order, and `suites` mapping
/// each suite name to the ids it selects, in load order.
fn catalog_json() -> Value {
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
    serde_json::json!({ "tests": reg.tests, "suites": suites })
}

/// The 45 original specs serialize and are selected by the suites exactly as in the golden.
#[test]
fn test_catalog_matches_the_golden() {
    let now = catalog_json();
    if std::env::var_os("UPDATE_CATALOG_GOLDEN").is_some() {
        let mut text = serde_json::to_string_pretty(&now).unwrap();
        text.push('\n');
        std::fs::write(golden_path(), text).unwrap();
        return;
    }
    let golden: Value =
        serde_json::from_str(&std::fs::read_to_string(golden_path()).expect("missing golden"))
            .unwrap();
    let golden_tests = golden["tests"].as_array().unwrap();
    let golden_ids: Vec<&str> = golden_tests
        .iter()
        .map(|t| t["id"].as_str().unwrap())
        .collect();
    assert_eq!(
        golden_ids.len(),
        45,
        "the golden holds the 45 original specs"
    );

    // 1. Every original spec is still there, byte for byte (as serialized), in the same order.
    let now_tests: Vec<&Value> = now["tests"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|t| golden_ids.contains(&t["id"].as_str().unwrap()))
        .collect();
    assert_eq!(
        now_tests.len(),
        golden_tests.len(),
        "an original spec disappeared or was renamed"
    );
    for (now, was) in now_tests.iter().zip(golden_tests) {
        assert_eq!(*now, was, "original spec {} changed", was["id"]);
    }

    // 2. Each suite selects the same original specs as before (new specs are not counted).
    for (suite, was) in golden["suites"].as_object().unwrap() {
        let was: Vec<&str> = was
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        let now: Vec<&str> = now["suites"][suite]
            .as_array()
            .unwrap_or_else(|| panic!("suite {suite} disappeared"))
            .iter()
            .map(|v| v.as_str().unwrap())
            .filter(|id| golden_ids.contains(id))
            .collect();
        assert_eq!(now, was, "suite {suite} selects different original specs");
    }
}
