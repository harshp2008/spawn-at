//! Unit tests for test runner execution, parsing, and verdict resolution.

use super::engine::TestRunner;
use super::models::{
    resolve_final_status, AutoStatus, FinalStatus, HumanVerdict, HumanVerdictKind, RunOptions,
};
use super::validation::{
    build_action_command_args, check_pattern_match, extract_claimed_window_id,
};
use crate::loader::TestRegistry;
use crate::preflight::{PreflightReport, PreflightVerdict};
use crate::probe::EnvironmentInfo;
use crate::schema::{TestDefinition, TestKind};
use crate::tracker::WindowTracker;
use std::path::PathBuf;

/// Tests extraction of window ID from JSON output and fallback heuristic.
#[test]
fn test_extract_claimed_window_id_from_json_and_fallback() {
    // 1. JSON with window_id (success/fallback)
    let json_success = r#"{"window_id":12345,"x":16,"y":56,"w":400,"h":300,"size_raised":false,"reused_existing_window":false,"source":"claim"}"#;
    assert_eq!(extract_claimed_window_id(json_success, ""), Some(12345));

    // 2. JSON with null window_id (reused window)
    let json_reused = r#"{"window_id":null,"x":16,"y":56,"w":400,"h":300,"size_raised":false,"reused_existing_window":true,"source":"poll"}"#;
    assert_eq!(extract_claimed_window_id(json_reused, ""), None);

    // 3. JSON with null window_id (timeout)
    let json_timeout = r#"{"window_id":null,"x":0,"y":0,"w":0,"h":0,"size_raised":false,"reused_existing_window":false,"source":"poll"}"#;
    assert_eq!(extract_claimed_window_id(json_timeout, ""), None);

    // 4. Non-JSON fallback regex match
    let text_fallback = "Compositor claimed window: 99999 successfully";
    assert_eq!(extract_claimed_window_id(text_fallback, ""), Some(99999));
    assert_eq!(extract_claimed_window_id("", text_fallback), Some(99999));
}

/// Tests pattern matching against literal strings and regex expressions.
#[test]
fn test_pattern_match_literal_and_regex() {
    let text = "[spawn-at] ERROR: Failed to claim window 101";

    // Literal match
    assert!(check_pattern_match("[spawn-at] ERROR:", text).unwrap());
    assert!(!check_pattern_match("[spawn-at] WARNING:", text).unwrap());

    // Regex match
    assert!(check_pattern_match("re:Failed to claim.*101", text).unwrap());
    assert!(!check_pattern_match("re:Failed to claim.*999", text).unwrap());
}

/// Tests final status resolution rules combining automated results and human verdicts.
#[test]
fn test_results_model_resolution_rules() {
    // Auto test: AutoPass -> Pass
    let (st, dis) = resolve_final_status(
        TestKind::Auto,
        AutoStatus::AutoPass,
        &HumanVerdict::default(),
    );
    assert_eq!(st, FinalStatus::Pass);
    assert!(!dis);

    // Human test: Pass -> Pass
    let human_pass = HumanVerdict {
        verdict: HumanVerdictKind::Pass,
        note: None,
    };
    let (st, dis) = resolve_final_status(TestKind::Human, AutoStatus::AutoFail, &human_pass);
    assert_eq!(st, FinalStatus::Pass);
    assert!(!dis);

    // Both test: Both pass -> Pass
    let (st, dis) = resolve_final_status(TestKind::Both, AutoStatus::AutoPass, &human_pass);
    assert_eq!(st, FinalStatus::Pass);
    assert!(!dis);

    // Both test: AutoPass but Human Fail -> Fail with disagreement flag true!
    let human_fail = HumanVerdict {
        verdict: HumanVerdictKind::Fail,
        note: Some("Window flashed at top-left".into()),
    };
    let (st, dis) = resolve_final_status(TestKind::Both, AutoStatus::AutoPass, &human_fail);
    assert_eq!(st, FinalStatus::Fail);
    assert!(dis);
}

/// Tests clear diagnostic reporting when fixture binary cannot be found.
#[test]
fn test_missing_fixture_binary_fails_with_clear_error_message() {
    let runner = TestRunner::new(
        TestRegistry {
            tests: vec![],
            suites: std::collections::HashMap::new(),
        },
        RunOptions {
            bin_path: PathBuf::from("/nonexistent/bin/spawn-at"),
            ..Default::default()
        },
    );

    let test = TestDefinition {
        id: "test/missing-fixture".into(),
        category: "basics".into(),
        subcategory: None,
        title: "Missing Fixture Test".into(),
        description: "Verifies early failure on missing fixture".into(),
        kind: TestKind::Auto,
        required: true,
        tags: vec!["needs-fixture".into()],
        requires: crate::schema::TestRequires::default(),
        subject: Some(crate::schema::TestSubject {
            app: "fixture".into(),
            profile: "plain".into(),
            args: vec![],
        }),
        action: crate::schema::TestAction {
            command: "spawn".into(),
            repeatable: false,
            args: vec!["--anchor".into(), "center".into()],
        },
        expect: crate::schema::TestExpect::default(),
        human: None,
        matrix: None,
        regression: None,
    };

    let mut tracker = WindowTracker::new_with_preexisting(&[]);
    let preflight = PreflightReport {
        repo_head: None,
        detected_commit: None,
        repo_dirty: false,
        is_dirty_build: false,
        verdict: PreflightVerdict::Pass,
        binary_version_output: String::new(),
    };

    let env_info = EnvironmentInfo {
        session_type: "wayland".into(),
        desktop: "gnome".into(),
        gnome_version: Some(46),
        monitors_count: 1,
        primary_workarea: None,
        independent_workarea: None,
    };

    let result = runner.execute_single_test(&test, &preflight, &env_info, &mut tracker);
    assert_eq!(result.auto_status, AutoStatus::Error);
    assert_eq!(result.final_status, FinalStatus::Error);
    let msg = result.message.expect("expected error message");
    assert!(msg.contains("Fixture binary not found at"));
    assert!(msg.contains("cargo build -p spawn-at-verify --bin fixture --features fixture"));
}

/// Tests command-line argument construction for fixture execution.
#[test]
fn test_build_action_command_args_fixture_and_class() {
    let fixture_bin = PathBuf::from("/path/to/target/debug/fixture");

    // 1. Explicit class in action.args: preserved and not duplicated
    let test_explicit = TestDefinition {
        id: "test/explicit-class".into(),
        category: "basics".into(),
        subcategory: None,
        title: "Explicit Class Test".into(),
        description: "Tests explicit class passing".into(),
        kind: TestKind::Auto,
        required: true,
        tags: vec![],
        requires: crate::schema::TestRequires::default(),
        subject: Some(crate::schema::TestSubject {
            app: "fixture".into(),
            profile: "plain".into(),
            args: vec!["--title".into(), "CustomTitle".into()],
        }),
        action: crate::schema::TestAction {
            command: "spawn".into(),
            repeatable: false,
            args: vec![
                "--class".into(),
                "custom.app.explicit".into(),
                "--anchor".into(),
                "bottom-right".into(),
            ],
        },
        expect: crate::schema::TestExpect::default(),
        human: None,
        matrix: None,
        regression: None,
    };

    let args = build_action_command_args(&test_explicit, &fixture_bin);
    assert!(args.contains(&"--json".to_string()));
    let class_idx = args.iter().position(|a| a == "--class").unwrap();
    assert_eq!(args[class_idx + 1], "custom.app.explicit");

    let dash_dash_idx = args.iter().position(|a| a == "--").unwrap();
    assert_eq!(args[dash_dash_idx + 1], "/path/to/target/debug/fixture");
    assert_eq!(args[dash_dash_idx + 2], "--title");
    assert_eq!(args[dash_dash_idx + 3], "CustomTitle");

    // Auto test timeout backstop is 10000ms
    let exit_idx = args.iter().position(|a| a == "--exit-after-ms").unwrap();
    assert_eq!(args[exit_idx + 1], "10000");

    // 2. Inferred class from --app-id in subject.args: forwarded as --class
    let test_inferred = TestDefinition {
        id: "test/inferred-class".into(),
        category: "basics".into(),
        subcategory: None,
        title: "Inferred Class Test".into(),
        description: "Tests inferred class forwarding".into(),
        kind: TestKind::Both,
        required: true,
        tags: vec![],
        requires: crate::schema::TestRequires::default(),
        subject: Some(crate::schema::TestSubject {
            app: "fixture".into(),
            profile: "plain".into(),
            args: vec!["--app-id".into(), "verify.inferred.id".into()],
        }),
        action: crate::schema::TestAction {
            command: "spawn".into(),
            repeatable: false,
            args: vec!["--anchor".into(), "center".into()],
        },
        expect: crate::schema::TestExpect::default(),
        human: None,
        matrix: None,
        regression: None,
    };

    let args_inf = build_action_command_args(&test_inferred, &fixture_bin);
    let inf_class_idx = args_inf.iter().position(|a| a == "--class").unwrap();
    assert_eq!(args_inf[inf_class_idx + 1], "verify.inferred.id");

    // Human/Both test timeout backstop is 300000ms (5 minutes)
    let inf_exit_idx = args_inf
        .iter()
        .position(|a| a == "--exit-after-ms")
        .unwrap();
    assert_eq!(args_inf[inf_exit_idx + 1], "300000");
}

/// An automatic test whose geometry was unchecked resolves to `PassUnchecked`, not `Pass`; with a
/// human pass on a "both" test the unchecked mark is kept.
#[test]
fn test_unchecked_auto_pass_resolves_to_pass_unchecked() {
    let none = HumanVerdict::default();
    let (f, _) = resolve_final_status(TestKind::Auto, AutoStatus::AutoPassUnchecked, &none);
    assert_eq!(f, FinalStatus::PassUnchecked);
    let (f, _) = resolve_final_status(TestKind::Auto, AutoStatus::AutoPass, &none);
    assert_eq!(f, FinalStatus::Pass);
    let human_pass = HumanVerdict {
        verdict: HumanVerdictKind::Pass,
        note: None,
    };
    let (f, disagree) =
        resolve_final_status(TestKind::Both, AutoStatus::AutoPassUnchecked, &human_pass);
    assert_eq!(f, FinalStatus::PassUnchecked);
    assert!(!disagree);
}
