//! Unit tests for test runner execution, parsing, and verdict resolution.

use super::engine::TestRunner;
use super::models::{
    resolve_final_status, AutoStatus, FinalStatus, HumanVerdict, HumanVerdictKind, RunOptions,
};
use super::validation::{
    build_action_command_args, check_pattern_match, extract_claimed_window_id, pass_assessment,
    settle_final_rect, validate_geometry, validate_geometry_final, Settle,
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
            min_width: None,
            min_height: None,
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
            min_width: None,
            min_height: None,
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

    let args = build_action_command_args(&test_explicit, &fixture_bin, &Default::default());
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
            min_width: None,
            min_height: None,
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

    let args_inf = build_action_command_args(&test_inferred, &fixture_bin, &Default::default());
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

/// The `placement/bottom-right` spec (bottom-right, 400x300, margin 16, tolerance 1).
fn bottom_right_spec() -> TestDefinition {
    toml::from_str(
        r#"
id = "placement/bottom-right"
category = "placement"
title = "t"
description = "d"
kind = "auto"
required = true
tags = []
[requires]
session = ["any"]
backend = ["any"]
gnome = "any"
monitors_min = 1
scale = "any"
apps = []
[action]
command = "spawn"
repeatable = false
args = []
[expect]
exit_code = 0
stderr_absent = []
stderr_present = []
diagnostics = []
never_visible_off_target = false
[expect.rect]
anchor = "bottom-right"
size = [400, 300]
margin = 16
tolerance_px = 1
"#,
    )
    .expect("spec parses")
}

/// Environment with the given independent and extension workareas.
fn env_with(
    independent: Option<crate::oracle::Rect>,
    ext: Option<crate::oracle::Rect>,
) -> EnvironmentInfo {
    EnvironmentInfo {
        session_type: "wayland".into(),
        desktop: "gnome".into(),
        gnome_version: Some(46),
        monitors_count: 1,
        primary_workarea: ext,
        independent_workarea: independent,
    }
}

/// Stdout of a window at 1504, 764, 400x300 (where bottom-right of 0,40 1920x1040 puts it).
const AT_TARGET: &str = r#"{"window_id":7,"x":1504,"y":764,"w":400,"h":300}"#;
/// Log line whose `APPLY_ANCHOR` records the extension's workarea bounds.
const LOG_BOUNDS: &str = "APPLY_ANCHOR bounds=(0,40,1920x1040) final=(1504,764)";
/// The workarea 0,40 1920x1040.
fn wa() -> crate::oracle::Rect {
    crate::oracle::Rect::new(0, 40, 1920, 1040)
}

/// Pass: an independent workarea agrees with the window; plain `AutoPass`, plain message, and
/// the final status for an automatic test is `Pass`.
#[test]
fn test_assessment_pass_with_independent_workarea() {
    let env = env_with(Some(wa()), Some(wa()));
    let check = validate_geometry(&bottom_right_spec(), AT_TARGET, Some(LOG_BOUNDS), &env)
        .unwrap()
        .unwrap();
    let (status, message) = pass_assessment(Some(&check));
    assert_eq!(status, AutoStatus::AutoPass);
    assert_eq!(message, "Assertions passed");
    let (fin, _) = resolve_final_status(TestKind::Auto, status, &HumanVerdict::default());
    assert_eq!(fin, FinalStatus::Pass);
    // A test without a rectangle expectation is also a plain pass.
    assert_eq!(
        pass_assessment(None),
        (AutoStatus::AutoPass, "Assertions passed".to_string())
    );
}

/// PassUnchecked: a rectangle was observed but there is no workarea from any source; the status
/// is `AutoPassUnchecked` (final `PassUnchecked`) and the message says geometry was not verified.
#[test]
fn test_assessment_pass_unchecked_without_any_workarea() {
    let env = env_with(None, None);
    let check = validate_geometry(&bottom_right_spec(), AT_TARGET, None, &env)
        .unwrap()
        .unwrap();
    let (status, message) = pass_assessment(Some(&check));
    assert_eq!(status, AutoStatus::AutoPassUnchecked);
    assert!(message.contains("geometry was not verified"), "{}", message);
    let (fin, _) = resolve_final_status(TestKind::Auto, status, &HumanVerdict::default());
    assert_eq!(fin, FinalStatus::PassUnchecked);
}

/// Log-only workarea: still `AutoPass` (the window is judged against the log bounds) but the
/// message notes the workarea came from the extension and was not independently probed.
#[test]
fn test_assessment_log_only_pass_carries_extension_note() {
    let env = env_with(None, None);
    let check = validate_geometry(&bottom_right_spec(), AT_TARGET, Some(LOG_BOUNDS), &env)
        .unwrap()
        .unwrap();
    let (status, message) = pass_assessment(Some(&check));
    assert_eq!(status, AutoStatus::AutoPass);
    assert_eq!(
        message,
        "Assertions passed (workarea taken from the extension, not independently probed)"
    );
}

/// An independent workarea that differs from the extension's adds a warning to the message and
/// the pass is judged on the independent one.
#[test]
fn test_assessment_conflict_adds_warning() {
    let env = env_with(Some(wa()), Some(crate::oracle::Rect::new(0, 0, 1920, 1000)));
    let check = validate_geometry(&bottom_right_spec(), AT_TARGET, None, &env)
        .unwrap()
        .unwrap();
    let (status, message) = pass_assessment(Some(&check));
    assert_eq!(status, AutoStatus::AutoPass);
    assert!(
        message
            .contains("warning: workarea differs: probe 0,40 1920×1040 vs extension 0,0 1920×1000"),
        "{}",
        message
    );
}

/// Geometry fail: the window is 80 px too high for the independent workarea, so
/// `validate_geometry` returns `Err` naming the deltas (the executor turns that into `AutoFail`,
/// final `Fail`), even though the extension's own log bounds would have accepted it.
#[test]
fn test_geometry_fail_is_an_error_with_deltas() {
    let env = env_with(Some(crate::oracle::Rect::new(0, 0, 1920, 1000)), None);
    let err =
        validate_geometry(&bottom_right_spec(), AT_TARGET, Some(LOG_BOUNDS), &env).unwrap_err();
    assert!(
        err.starts_with("Geometry mismatch (tolerance 1px)"),
        "{}",
        err
    );
    assert!(err.contains("dy=+80"), "{}", err);
    let (fin, _) = resolve_final_status(
        TestKind::Auto,
        AutoStatus::AutoFail,
        &HumanVerdict::default(),
    );
    assert_eq!(fin, FinalStatus::Fail);
}

/// Every automatic/human combination for a `both` test: (auto, human, final status, disagreement).
/// Auto Fail with human Pass is Fail (and a disagreement); auto Pass with human Fail is Fail (and
/// a disagreement); auto Pass with human unset is Pending; both Pass is Pass; an automatic Fail
/// stays Fail whatever the human has not said or said.
#[test]
fn test_both_final_status_table() {
    let human = |v: HumanVerdictKind| HumanVerdict {
        verdict: v,
        note: None,
    };
    let table = [
        (
            AutoStatus::AutoPass,
            HumanVerdictKind::Pass,
            FinalStatus::Pass,
            false,
        ),
        (
            AutoStatus::AutoPass,
            HumanVerdictKind::Fail,
            FinalStatus::Fail,
            true,
        ),
        (
            AutoStatus::AutoPass,
            HumanVerdictKind::Unset,
            FinalStatus::Pending,
            false,
        ),
        (
            AutoStatus::AutoFail,
            HumanVerdictKind::Pass,
            FinalStatus::Fail,
            true,
        ),
        (
            AutoStatus::AutoFail,
            HumanVerdictKind::Fail,
            FinalStatus::Fail,
            false,
        ),
        (
            AutoStatus::AutoFail,
            HumanVerdictKind::Unset,
            FinalStatus::Fail,
            false,
        ),
    ];
    for (auto, h, want, want_dis) in table {
        let (got, dis) = resolve_final_status(TestKind::Both, auto, &human(h));
        assert_eq!(
            (got, dis),
            (want, want_dis),
            "auto {:?}, human {:?}",
            auto,
            h
        );
    }
}

/// A spec with `min_width = 300` and `min_height = 200` under `[subject]` parses, and the runner
/// adds `--min-width 300 --min-height 200` to the fixture command, before `--exit-after-ms`. A
/// minimum already spelled out in `subject.args` is not added a second time, and a spec without
/// the fields adds neither flag. Fails if the fields are dropped on parse or duplicated on assembly.
#[test]
fn test_subject_min_size_fields_parse_and_reach_the_fixture_command() {
    let spec = |extra_subject: &str, subject_args: &str| -> TestDefinition {
        toml::from_str(&format!(
            r#"
id = "placement/x"
category = "placement"
title = "t"
description = "d"
kind = "auto"
required = true

[subject]
app = "fixture"
profile = "plain"
args = [{subject_args}]
{extra_subject}

[action]
command = "spawn"
args = ["--anchor", "center", "--size", "200", "150"]

[expect]
exit_code = 0
"#
        ))
        .unwrap()
    };
    let fixture_bin = PathBuf::from("/path/to/target/debug/fixture");
    let value_after = |args: &[String], flag: &str| -> Option<String> {
        args.iter()
            .position(|a| a == flag)
            .map(|i| args[i + 1].clone())
    };

    let with_min = spec("min_width = 300\nmin_height = 200", r#""--title", "T""#);
    assert_eq!(with_min.subject.as_ref().unwrap().min_width, Some(300));
    assert_eq!(with_min.subject.as_ref().unwrap().min_height, Some(200));
    let args = build_action_command_args(&with_min, &fixture_bin, &Default::default());
    assert_eq!(value_after(&args, "--min-width").as_deref(), Some("300"));
    assert_eq!(value_after(&args, "--min-height").as_deref(), Some("200"));
    let min_idx = args.iter().position(|a| a == "--min-width").unwrap();
    let exit_idx = args.iter().position(|a| a == "--exit-after-ms").unwrap();
    assert!(min_idx < exit_idx, "{:?}", args);

    let spelled = spec("min_width = 300", r#""--min-width", "250", "--title", "T""#);
    let args = build_action_command_args(&spelled, &fixture_bin, &Default::default());
    assert_eq!(args.iter().filter(|a| *a == "--min-width").count(), 1);
    assert_eq!(value_after(&args, "--min-width").as_deref(), Some("250"));

    let none = spec("", r#""--title", "T""#);
    let args = build_action_command_args(&none, &fixture_bin, &Default::default());
    assert!(!args
        .iter()
        .any(|a| a == "--min-width" || a == "--min-height"));
}

/// `expects_no_window` is true only when the spec expects a non-zero exit code and the run
/// returned exactly that code: exit 2 expected and 2 seen skips attribution; exit 2 expected but
/// 0 seen, and the usual expectation 0, do not. Fails if an expected-failure spawn is still sent
/// through window attribution ("could not attribute; windows left open: []").
#[test]
fn test_expects_no_window_only_for_a_matched_nonzero_exit() {
    let spec = |code: i32| -> TestDefinition {
        toml::from_str(&format!(
            r#"
id = "placement/x"
category = "placement"
title = "t"
description = "d"
kind = "auto"

[action]
command = "spawn"
args = []

[expect]
exit_code = {code}
"#
        ))
        .unwrap()
    };
    assert!(super::validation::expects_no_window(&spec(2), 2));
    assert!(!super::validation::expects_no_window(&spec(2), 0));
    assert!(!super::validation::expects_no_window(&spec(2), 1));
    assert!(!super::validation::expects_no_window(&spec(0), 0));
}

/// What `spawn-at` printed in the loaded gate-(b) failures: the anchor's position for 200x150.
const STALE_PRINT: &str = r#"{"window_id":7,"x":1704,"y":914,"w":200,"h":150}"#;

/// Printed rectangle already at the target: the wait-and-read closure is never called, so the run
/// takes no extra time, and the result is `NotNeeded`.
#[test]
fn test_no_settle_wait_when_the_printed_rectangle_matches() {
    let env = env_with(Some(wa()), Some(wa()));
    let calls = std::cell::Cell::new(0);
    let settle = settle_final_rect(&bottom_right_spec(), AT_TARGET, None, &env, || {
        calls.set(calls.get() + 1);
        None
    });
    assert_eq!(settle, Settle::NotNeeded);
    assert_eq!(calls.get(), 0, "a matching print must not wait");
}

/// Printed rectangle stale: the closure runs exactly once and its rectangle is returned; with the
/// window at the target the run passes and carries the stale-print warning in its message.
#[test]
fn test_stale_print_settles_once_and_passes_with_warning_when_final_is_right() {
    let env = env_with(Some(wa()), Some(wa()));
    let calls = std::cell::Cell::new(0);
    let fin = crate::oracle::Rect::new(1504, 764, 400, 300);
    let settle = settle_final_rect(&bottom_right_spec(), STALE_PRINT, None, &env, || {
        calls.set(calls.get() + 1);
        Some(fin)
    });
    assert_eq!(settle, Settle::Read(fin));
    assert_eq!(calls.get(), 1);
    let check = validate_geometry_final(&bottom_right_spec(), STALE_PRINT, Some(fin), None, &env)
        .unwrap()
        .unwrap();
    let (status, message) = pass_assessment(Some(&check));
    assert_eq!(status, AutoStatus::AutoPass);
    assert_eq!(
        message,
        "Assertions passed (warning: spawn-at printed a stale rectangle (printed 1704,914 200×150, final 1504,764 400×300))"
    );
}

/// Negative control: stale print and a wrong final rectangle (1704,914 200x150 again) still fail.
#[test]
fn test_stale_print_with_wrong_final_still_fails() {
    let env = env_with(Some(wa()), Some(wa()));
    let wrong = crate::oracle::Rect::new(1604, 864, 300, 200);
    let err = validate_geometry_final(&bottom_right_spec(), STALE_PRINT, Some(wrong), None, &env)
        .unwrap_err();
    assert!(err.contains("actual [1604, 864, 300x200]"), "{}", err);
}

/// A wait that cannot read the window gives `Unreadable`, and judging with no final rectangle
/// keeps the printed (failing) verdict.
#[test]
fn test_unreadable_final_keeps_the_printed_verdict() {
    let env = env_with(Some(wa()), Some(wa()));
    let settle = settle_final_rect(&bottom_right_spec(), STALE_PRINT, None, &env, || None);
    assert_eq!(settle, Settle::Unreadable);
    assert!(validate_geometry_final(&bottom_right_spec(), STALE_PRINT, None, None, &env).is_err());
}

/// A spawn spec for the wait-override tests: a fixture spawn at 800x600, `command` as its action.
fn wait_spec(command: &str, action_args: &str) -> TestDefinition {
    toml::from_str(&format!(
        r#"
id = "placement/w"
category = "placement"
title = "t"
description = "d"
kind = "auto"
required = true

[subject]
app = "fixture"
profile = "plain"
args = ["--title", "T"]

[action]
command = "{command}"
args = [{action_args}]

[expect]
exit_code = 0
"#
    ))
    .unwrap()
}

fn wait_request(strategy: Option<&str>, cap_ms: Option<&str>) -> crate::trace::wait::WaitRequest {
    crate::trace::wait::WaitRequest {
        strategy: strategy.map(String::from),
        cap_ms: cap_ms.map(String::from),
    }
}

/// The two options reach the `spawn` command line, after the spec's own arguments and before the
/// `--` that starts the fixture (after it they would be the fixture's arguments). Fails if they
/// land behind the `--`, are dropped, or are not passed as typed.
#[test]
fn test_run_wait_options_reach_the_spawn_command_before_the_double_dash() {
    let spec = wait_spec("spawn", r#""--anchor", "center", "--size", "800", "600""#);
    let fixture_bin = PathBuf::from("/path/to/target/debug/fixture");
    let args = build_action_command_args(
        &spec,
        &fixture_bin,
        &wait_request(Some("legacy"), Some("90")),
    );
    let at = |flag: &str| args.iter().position(|a| a == flag).unwrap();
    let dashes = args.iter().position(|a| a == "--").unwrap();
    assert_eq!(args[at("--wait-strategy") + 1], "legacy");
    assert_eq!(args[at("--wait-cap-ms") + 1], "90");
    assert!(at("--size") < at("--wait-strategy"), "{args:?}");
    assert!(at("--wait-cap-ms") < dashes, "{args:?}");
    assert!(
        args[dashes..].iter().all(|a| !a.starts_with("--wait-")),
        "{args:?}"
    );
    // A typo is passed as typed: spawn-at is the one that warns about it.
    let typo = build_action_command_args(&spec, &fixture_bin, &wait_request(Some("legcy"), None));
    assert_eq!(
        typo[typo.iter().position(|a| a == "--wait-strategy").unwrap() + 1],
        "legcy"
    );
}

/// Without the options the argument list is the one built before they existed: nothing added,
/// whether the request is empty or absent. Fails if a default run gets any `--wait-` argument.
#[test]
fn test_run_without_wait_options_adds_nothing_to_the_spawn_command() {
    let spec = wait_spec("spawn", r#""--anchor", "center", "--size", "800", "600""#);
    let fixture_bin = PathBuf::from("/path/to/target/debug/fixture");
    let args = build_action_command_args(&spec, &fixture_bin, &Default::default());
    assert!(!args.iter().any(|a| a.starts_with("--wait-")), "{args:?}");
    // `auto` alone is still passed (spawn-at treats it as the default); the harness does not
    // second-guess the value.
    let auto = build_action_command_args(&spec, &fixture_bin, &wait_request(Some("auto"), None));
    let without: Vec<_> = auto
        .iter()
        .filter(|a| !["--wait-strategy", "auto"].contains(&a.as_str()))
        .cloned()
        .collect();
    assert_eq!(without, args);
}

/// Only a `spawn` action takes the options: a `transform` or `query` action is left alone (the
/// flags do not exist there and would fail the command).
#[test]
fn test_run_wait_options_are_not_added_to_other_commands() {
    let fixture_bin = PathBuf::from("/path/to/target/debug/fixture");
    let transform = wait_spec("transform", r#""--class", "x", "--size", "800", "600""#);
    let args = build_action_command_args(
        &transform,
        &fixture_bin,
        &wait_request(Some("legacy"), Some("90")),
    );
    assert!(!args.iter().any(|a| a.starts_with("--wait-")), "{args:?}");
}

/// A spec that spells out one of the options keeps its own value, the other one is still added,
/// and no flag appears twice (a repeated flag is a parse error in `spawn-at`).
#[test]
fn test_run_wait_options_do_not_duplicate_spec_flags() {
    let spec = wait_spec("spawn", r#""--size", "800", "600", "--wait-cap-ms", "40""#);
    let fixture_bin = PathBuf::from("/path/to/target/debug/fixture");
    let args = build_action_command_args(
        &spec,
        &fixture_bin,
        &wait_request(Some("legacy"), Some("90")),
    );
    assert_eq!(args.iter().filter(|a| *a == "--wait-cap-ms").count(), 1);
    assert_eq!(
        args[args.iter().position(|a| a == "--wait-cap-ms").unwrap() + 1],
        "40"
    );
    assert_eq!(args.iter().filter(|a| *a == "--wait-strategy").count(), 1);
}
