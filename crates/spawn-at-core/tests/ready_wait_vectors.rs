//! Runs every golden script in `tests/ready_wait_vectors/scripts/` through the Rust ready-wait
//! machine with the virtual-clock driver described in that directory's README, and compares the
//! outcome with the script's `expect`. The JS port runs the same files (`tests/ready_wait_test.js`).

use serde_json::Value;
use spawn_at_core::ready_wait::{Action, Policy, ReadyWait, Request};
use std::path::PathBuf;

/// Directory of the shared vectors, relative to this crate.
fn vectors_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/ready_wait_vectors")
}

/// Runs one script and returns the outcome as JSON.
fn run_script(script: &Value) -> Value {
    let policy: Policy = match script.get("policy") {
        Some(p) => serde_json::from_value(p.clone()).expect("policy parses"),
        None => Policy::default(),
    };
    let request: Request =
        serde_json::from_value(script["request"].clone()).expect("request parses");
    let (mut machine, mut action) = ReadyWait::start(policy, request, 0);
    for event in script["events"].as_array().expect("events array") {
        let at = event["at"].as_u64().expect("event time");
        let size = (
            event["size"][0].as_u64().expect("w") as u32,
            event["size"][1].as_u64().expect("h") as u32,
        );
        while let Action::Wait { deadline_ms } = action {
            if deadline_ms >= at {
                break;
            }
            action = machine.on_tick(deadline_ms);
        }
        if matches!(action, Action::Done(_)) {
            break;
        }
        action = machine.on_sample(size, at);
    }
    loop {
        match action {
            Action::Wait { deadline_ms } => action = machine.on_tick(deadline_ms),
            Action::Done(outcome) => return serde_json::to_value(outcome).unwrap(),
        }
    }
}

/// Every script in the directory ends in exactly the outcome it states, and the scenarios the
/// design lists are all present (so a deleted script cannot go unnoticed).
#[test]
fn test_all_golden_scripts_give_the_expected_outcome() {
    let mut files: Vec<PathBuf> = std::fs::read_dir(vectors_dir().join("scripts"))
        .expect("scripts dir")
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .collect();
    files.sort();
    assert!(
        files.len() >= 20,
        "expected at least 20 scripts, found {}",
        files.len()
    );
    for prefix in [
        "01_", "02_", "03_", "04_", "05_", "06_", "07_", "08_", "09_", "09b_", "09c_", "10_",
        "10b_", "10c_", "10d_", "11_", "12_", "13_", "14_", "15_",
    ] {
        assert!(
            files
                .iter()
                .any(|f| f.file_name().unwrap().to_string_lossy().starts_with(prefix)),
            "missing scenario {}",
            prefix
        );
    }
    for file in &files {
        let script: Value = serde_json::from_str(&std::fs::read_to_string(file).unwrap())
            .unwrap_or_else(|e| panic!("{}: {}", file.display(), e));
        assert_eq!(
            run_script(&script),
            script["expect"],
            "{} ({})",
            file.file_name().unwrap().to_string_lossy(),
            script["name"]
        );
    }
}

/// The built-in default policy equals the shared `policy.json`, which the JS port is checked
/// against too: the numbers have one source.
#[test]
fn test_default_policy_matches_the_shared_file() {
    let shared: Policy =
        serde_json::from_str(&std::fs::read_to_string(vectors_dir().join("policy.json")).unwrap())
            .unwrap();
    assert_eq!(shared, Policy::default());
    assert_eq!(
        (shared.cap_ms, shared.quiet_ms, shared.hard_ms),
        (250, 30, 1500)
    );
}
