//! Unit tests for report structures, redaction, and formatting.

use super::*;
use std::time::SystemTime;

use crate::runner::{AutoStatus, FinalStatus, HumanVerdict, TestResult};
use crate::schema::TestKind;

/// Tests qualification rule that skipped required tests prevent platform certification.
#[test]
fn test_skipped_is_never_a_pass() {
    let results = vec![
        TestResult {
            id: "preflight/binary-head".into(),
            title: "Preflight".into(),
            kind: TestKind::Auto,
            required: true,
            auto_status: AutoStatus::AutoPass,
            human_verdict: HumanVerdict::default(),
            final_status: FinalStatus::Pass,
            disagreement: false,
            duration_ms: 5,
            exit_code: Some(0),
            stdout: "".into(),
            stderr: "".into(),
            message: None,
            log_slice: None,
            trace_note: None,
            final_rect: None,
            flash_info: Default::default(),
            wait_info: Default::default(),
        },
        TestResult {
            id: "cloak/delayed-paint".into(),
            title: "Cloak test".into(),
            kind: TestKind::Both,
            required: true,
            auto_status: AutoStatus::Skipped,
            human_verdict: HumanVerdict::default(),
            final_status: FinalStatus::Skipped,
            disagreement: false,
            duration_ms: 0,
            exit_code: None,
            stdout: "".into(),
            stderr: "".into(),
            message: Some("Wayland only".into()),
            log_slice: None,
            trace_note: None,
            final_rect: None,
            flash_info: Default::default(),
            wait_info: Default::default(),
        },
    ];

    let qual = evaluate_qualification(&results);
    assert_eq!(qual.total_required, 2);
    assert_eq!(qual.passed_required, 1);
    assert_eq!(qual.skipped_required.len(), 1);
    // RULE: Skipped test prevents platform support qualification
    assert!(!qual.supported);
}

/// Tests command argument title redaction when include_titles is false.
#[test]
fn test_title_redaction() {
    let unredacted = "args: --title MyPrivateDocumentWindow --app-id org.test";
    let redacted = redact_titles(unredacted, false);
    assert!(redacted.contains("[REDACTED]"));
    assert!(!redacted.contains("MyPrivateDocumentWindow"));

    let preserved = redact_titles(unredacted, true);
    assert!(preserved.contains("MyPrivateDocumentWindow"));
}

/// Tests title redaction isolates only the title argument without altering other flags.
#[test]
fn test_title_redaction_does_not_alter_other_command_arguments() {
    let cmd = "spawn-at spawn --title \"Sensitive Document Title\" --anchor top-right --size 800 600 -m 20 --class org.gnome.Terminal --clamp true";
    let redacted = redact_titles(cmd, false);

    // Title itself must be redacted
    assert!(redacted.contains("--title \"[REDACTED]\""));
    assert!(!redacted.contains("Sensitive Document Title"));

    // All other arguments MUST be preserved exactly
    assert!(redacted.contains("--anchor top-right"));
    assert!(redacted.contains("--size 800 600"));
    assert!(redacted.contains("-m 20"));
    assert!(redacted.contains("--class org.gnome.Terminal"));
    assert!(redacted.contains("--clamp true"));

    // Check -t short flag with single-quoted title
    let short_cmd = "spawn-at spawn -t 'Private Budget' --monitor primary";
    let short_redacted = redact_titles(short_cmd, false);
    assert!(short_redacted.contains("-t \"[REDACTED]\""));
    assert!(!short_redacted.contains("Private Budget"));
    assert!(short_redacted.contains("--monitor primary"));

    // Check that flags containing 'title' in their name are NOT altered
    let title_flag_cmd = "spawn-at spawn --title-format standard --class title-editor";
    let title_flag_redacted = redact_titles(title_flag_cmd, false);
    assert_eq!(title_flag_cmd, title_flag_redacted);

    // Check JSON title redaction
    let json_text = r#"{"id": 42, "title": "Secret Bank Statement", "class": "calc", "w": 300}"#;
    let json_redacted = redact_titles(json_text, false);
    assert!(json_redacted.contains(r#""title": "[REDACTED]""#));
    assert!(!json_redacted.contains("Secret Bank Statement"));
    assert!(json_redacted.contains(r#""id": 42"#));
    assert!(json_redacted.contains(r#""class": "calc""#));
    assert!(json_redacted.contains(r#""w": 300"#));
}

/// Tests that real system hostname is queried successfully.
#[test]
fn test_get_real_hostname_returns_valid_name() {
    let host = get_real_hostname();
    assert!(!host.is_empty());
    assert!(!host.contains('\n'));
    assert!(!host.contains(' '));
}

/// Tests format and lexicographical sorting order of sortable timestamps.
#[test]
fn test_format_sortable_timestamp() {
    let t1 = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1700000000);
    let t2 = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1700000060);

    let s1 = format_sortable_timestamp(t1);
    let s2 = format_sortable_timestamp(t2);

    // Regex check for YYYYMMDD-HHMMSS (8 digits, hyphen, 6 digits)
    let re = regex::Regex::new(r"^\d{8}-\d{6}$").unwrap();
    assert!(re.is_match(&s1), "s1 '{}' should match YYYYMMDD-HHMMSS", s1);
    assert!(re.is_match(&s2), "s2 '{}' should match YYYYMMDD-HHMMSS", s2);

    // Sort order check: later time must sort lexicographically after earlier time
    assert!(
        s2 > s1,
        "Timestamp must preserve chronological sort order: {} > {}",
        s2,
        s1
    );
}

/// Tests sortable timestamp format across specific historical and leap-year test vectors.
#[test]
fn test_format_epoch_secs_sortable_test_vectors() {
    // Test vectors for sortable UTC timestamp format YYYYMMDD-HHMMSS:
    // 1. User test vector: epoch 1791559108 -> 20261009-151828 UTC
    assert_eq!(format_epoch_secs_sortable(1791559108), "20261009-151828");

    // 2. Epoch 0 -> 19700101-000000 UTC
    assert_eq!(format_epoch_secs_sortable(0), "19700101-000000");

    // 3. Leap-day test vector: epoch 1709208000 -> 20240229-120000 UTC
    // (2024 is a leap year; 2024-02-29 12:00:00 UTC = 1709208000)
    assert_eq!(format_epoch_secs_sortable(1709208000), "20240229-120000");
}

/// A result with the given id, stdout and duration, used by the repeat-run tests.
fn run_result(id: &str, stdout: &str, duration_ms: u64) -> TestResult {
    TestResult {
        id: id.into(),
        title: id.into(),
        kind: TestKind::Auto,
        required: false,
        auto_status: AutoStatus::AutoPass,
        human_verdict: HumanVerdict::default(),
        final_status: FinalStatus::Pass,
        disagreement: false,
        duration_ms,
        exit_code: Some(0),
        stdout: stdout.into(),
        stderr: String::new(),
        message: None,
        log_slice: None,
        trace_note: None,
        final_rect: None,
        flash_info: Default::default(),
        wait_info: Default::default(),
    }
}

/// Builds a report for `results` and per-test `runs` with an empty environment.
fn report_with_runs(results: &[TestResult], runs: &[Vec<TestResult>]) -> SessionReport {
    let preflight = crate::preflight::PreflightReport {
        repo_head: None,
        detected_commit: None,
        repo_dirty: false,
        is_dirty_build: false,
        verdict: crate::preflight::PreflightVerdict::Pass,
        binary_version_output: String::new(),
    };
    let env = crate::probe::EnvironmentInfo {
        session_type: "wayland".into(),
        desktop: "gnome".into(),
        gnome_version: None,
        monitors_count: 1,
        primary_workarea: None,
        independent_workarea: None,
    };
    build_session_report_with_runs(
        std::path::Path::new("bin"),
        &preflight,
        &env,
        results,
        runs,
        false,
    )
}

/// `--repeat 3`: all three runs are stored under `repeat_runs` in order, `results` keeps the
/// first, and titles are redacted in every stored run.
#[test]
fn test_repeat_runs_are_all_stored_and_redacted() {
    let runs: Vec<TestResult> = (1..=3)
        .map(|i| run_result("t/a", "--title SecretWindow", i * 10))
        .collect();
    let report = report_with_runs(&runs[..1], &[runs.clone()]);
    assert_eq!(report.results.len(), 1);
    assert_eq!(report.repeat_runs.len(), 1);
    assert_eq!(report.repeat_runs[0].id, "t/a");
    let durations: Vec<u64> = report.repeat_runs[0]
        .runs
        .iter()
        .map(|r| r.duration_ms)
        .collect();
    assert_eq!(durations, vec![10, 20, 30]);
    for r in &report.repeat_runs[0].runs {
        assert!(!r.stdout.contains("SecretWindow"), "{}", r.stdout);
    }
    let md = render_markdown_report(&report);
    assert!(
        md.contains("## 4. Repeat Runs") && md.contains("`t/a` (3 runs)"),
        "{}",
        md
    );
}

/// A single run adds nothing: no `repeat_runs` in the struct or in the JSON, no Markdown section.
#[test]
fn test_single_run_has_no_repeat_runs_field() {
    let r = run_result("t/a", "", 5);
    let report = report_with_runs(&[r.clone()], &[vec![r]]);
    assert!(report.repeat_runs.is_empty());
    let json = serde_json::to_string(&report).unwrap();
    assert!(!json.contains("repeat_runs"), "{}", json);
    assert!(!render_markdown_report(&report).contains("Repeat Runs"));
}

/// A report written before `repeat_runs` existed still deserializes (field defaults to empty),
/// and one with the field round-trips.
#[test]
fn test_old_report_without_repeat_runs_loads_and_new_round_trips() {
    let runs = vec![run_result("t/a", "", 1), run_result("t/a", "", 2)];
    let report = report_with_runs(&runs[..1], &[runs.clone()]);
    let mut value = serde_json::to_value(&report).unwrap();
    let back: SessionReport = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(back.repeat_runs[0].runs.len(), 2);
    value.as_object_mut().unwrap().remove("repeat_runs");
    let old: SessionReport = serde_json::from_value(value).unwrap();
    assert!(old.repeat_runs.is_empty());
}

/// A required test that passed with its trace unchecked does not qualify the platform: it is
/// counted as failed-required, like a geometry-unchecked pass.
#[test]
fn test_required_trace_unchecked_pass_does_not_qualify() {
    let result = |status: FinalStatus, auto: AutoStatus| TestResult {
        id: "cloak/delayed-paint/delay-0".into(),
        title: "t".into(),
        kind: TestKind::Auto,
        required: true,
        auto_status: auto,
        human_verdict: HumanVerdict::default(),
        final_status: status,
        disagreement: false,
        duration_ms: 1,
        exit_code: Some(0),
        stdout: String::new(),
        stderr: String::new(),
        message: None,
        log_slice: None,
        trace_note: None,
        final_rect: None,
        flash_info: Default::default(),
        wait_info: Default::default(),
    };
    let q = evaluate_qualification(&[result(FinalStatus::Pass, AutoStatus::AutoPass)]);
    assert!(q.supported);
    let q = evaluate_qualification(&[result(
        FinalStatus::PassTraceUnchecked,
        AutoStatus::AutoPassTraceUnchecked,
    )]);
    assert_eq!(
        (q.supported, q.passed_required, q.failed_required),
        (false, 0, 1)
    );
}

/// Three runs written in the same second (same `generated_at`, same host) get three separate
/// directories that sort in start order, each with its own `report.json`. Fails if a later run
/// overwrote an earlier one's report: the three results here have different ids.
#[test]
fn test_same_second_runs_keep_separate_report_directories() {
    let base = std::env::temp_dir().join(format!(
        "spawn-at-verify-same-second-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let mut dirs = Vec::new();
    for id in ["a/one", "a/two", "a/three"] {
        let result = run_result(id, "", 1);
        let mut report = report_with_runs(&[result], &[]);
        report.environment.generated_at = 1_800_000_000;
        report.environment.hostname = "host".into();
        dirs.push(write_report_artifacts(&base, &report).unwrap());
    }
    let names: Vec<String> = dirs
        .iter()
        .map(|d| d.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    assert!(names[0].ends_with("-host"), "{:?}", names);
    assert!(names[1].ends_with("-host-02"), "{:?}", names);
    assert!(names[2].ends_with("-host-03"), "{:?}", names);
    let mut sorted = names.clone();
    sorted.sort();
    assert_eq!(sorted, names, "names must sort in start order");
    for (d, id) in dirs.iter().zip(["a/one", "a/two", "a/three"]) {
        let json = std::fs::read_to_string(d.join("report.json")).unwrap();
        assert!(
            json.contains(&format!("\"id\": \"{}\"", id)),
            "{}",
            d.display()
        );
    }
    std::fs::remove_dir_all(&base).unwrap();
}

/// With the plain name and `-02` to `-99` all taken (100 runs in one second), the next write is
/// an error that names the cause ("100 runs in the same second"), not a bare "File exists", and
/// it creates no `-100` directory and no report file.
#[test]
fn test_hundred_same_second_runs_is_an_explicit_error() {
    let base = std::env::temp_dir().join(format!(
        "spawn-at-verify-hundred-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let mut report = report_with_runs(&[run_result("a/one", "", 1)], &[]);
    report.environment.generated_at = 1_800_000_000;
    report.environment.hostname = "host".into();
    let first = write_report_artifacts(&base, &report).unwrap();
    let plain = first.file_name().unwrap().to_string_lossy().into_owned();
    for n in 2..=99 {
        std::fs::create_dir(base.join(format!("{}-{:02}", plain, n))).unwrap();
    }
    let err = write_report_artifacts(&base, &report).unwrap_err();
    assert!(err.contains("100 runs in the same second"), "{}", err);
    assert!(err.contains(&plain), "{}", err);
    assert!(!base.join(format!("{}-100", plain)).exists());
    assert_eq!(
        std::fs::read_dir(&base).unwrap().count(),
        99,
        "plain name + -02..-99"
    );
    std::fs::remove_dir_all(&base).unwrap();
}

/// `set_load` puts both load averages and the CPU count into the serialized environment, and a
/// report without them (an older recording) still loads with all three absent.
#[test]
fn test_report_records_load_average_and_old_reports_still_load() {
    use crate::loadavg::LoadAvg;
    let start = LoadAvg {
        one: 0.52,
        five: 0.58,
        fifteen: 0.59,
    };
    let end = LoadAvg {
        one: 12.5,
        five: 3.0,
        fifteen: 1.0,
    };
    let mut report = report_with_runs(&[run_result("a/one", "", 1)], &[]);
    assert_eq!(report.environment.load_avg_start, None);
    report.environment.set_load(Some(start), Some(end));
    let value = serde_json::to_value(&report).unwrap();
    let env = &value["environment"];
    assert_eq!(env["load_avg_start"]["one"], 0.52);
    assert_eq!(env["load_avg_end"]["one"], 12.5);
    assert!(env["cpu_count"].as_u64().unwrap() >= 1);

    let mut old = value.clone();
    for key in ["load_avg_start", "load_avg_end", "cpu_count"] {
        old["environment"].as_object_mut().unwrap().remove(key);
    }
    let back: SessionReport = serde_json::from_value(old).unwrap();
    assert_eq!(back.environment.load_avg_start, None);
    assert_eq!(back.environment.cpu_count, None);
}

/// Slice for window 77 with an `ANCHOR_CORRECTION` 43.9 ms after the `UNCLOAK`.
const POST_REVEAL_SLICE: &str = "\
[spawn-at-time] +10.00ms | CLOAK win=[id=77 pid=2 class=\"verify.x\" app=\"\" type=NORMAL proto=Wayland] opacity=0 frame=(0,0,400x300)\n\
[spawn-at-time] +40.00ms | UNCLOAK win=[id=77 pid=2 class=\"verify.x\" app=\"\" type=NORMAL proto=Wayland] opacity=255 frame=(760,764,400x300)\n\
[spawn-at-time] +83.90ms | ANCHOR_CORRECTION win=[id=77 pid=2 class=\"verify.x\" app=\"\" type=NORMAL proto=Wayland] size=(300x200) from=(760,764) to=(810,864)\n";

/// report.md lists a post-reveal correction and a stale printed rectangle under "5. Warnings",
/// one row each, and a clean run adds no such section.
#[test]
fn test_markdown_lists_run_warnings_and_omits_the_section_when_clean() {
    let mut warned = run_result(
        "t/warned",
        r#"{"window_id":77,"x":760,"y":764,"w":400,"h":300}"#,
        5,
    );
    warned.log_slice = Some(POST_REVEAL_SLICE.into());
    warned.final_rect = Some(crate::oracle::Rect::new(810, 864, 300, 200));
    let clean = run_result("t/clean", r#"{"window_id":78,"x":1,"y":2,"w":3,"h":4}"#, 5);

    let md = render_markdown_report(&report_with_runs(&[warned.clone(), clean.clone()], &[]));
    assert!(md.contains("## 5. Warnings"), "{}", md);
    assert!(
        md.contains("| `t/warned` | placement corrected after reveal (+43.9 ms) |"),
        "{}",
        md
    );
    assert!(
        md.contains("| `t/warned` | spawn-at printed a stale rectangle (printed 760,764 400×300, final 810,864 300×200) |"),
        "{}",
        md
    );
    assert!(!md.contains("`t/clean` |  "), "{}", md);
    assert_eq!(
        md.matches("| `t/clean` |").count(),
        1,
        "only its table row: {}",
        md
    );

    let md = render_markdown_report(&report_with_runs(&[clean], &[]));
    assert!(!md.contains("Warnings"), "{}", md);
}

/// A result that passed on placement and flashed: one certain client event 25 ms after the reveal
/// and one uncertain event.
fn flashing_result(id: &str) -> TestResult {
    use crate::trace::flash::{FlashBasis, FlashEvent, FlashInfo, FlashKind, FlashSource};
    let mut r = run_result(id, r#"{"window_id":77,"x":1,"y":2,"w":3,"h":4}"#, 5);
    r.flash_info = FlashInfo {
        flash: true,
        flash_certain: 1,
        refresh_hz: Some(60.0),
        frame_ms: Some(1000.0 / 60.0),
        flash_events: vec![
            FlashEvent {
                kind: FlashKind::PaintAfterReveal,
                source: FlashSource::Client,
                lead_ms: 25.0,
                numeric: true,
                visibility: Some(crate::trace::flash::Visibility::Certain),
                visible_probability: 1.0,
                detail: "first paint at 800x600 came 25.0 ms after the reveal".into(),
            },
            FlashEvent {
                kind: FlashKind::SizeChangeAfterReveal,
                source: FlashSource::Client,
                lead_ms: 3.0,
                numeric: false,
                visibility: None,
                visible_probability: 0.0,
                detail: "size 400x300 -> 800x600 3.0 ms after the reveal".into(),
            },
        ],
        flash_basis: Some(FlashBasis::Client),
        flash_notes: vec![],
        flash_clock_uncertainty_ms: Some(1.2),
        ..Default::default()
    };
    r
}

/// report.json carries `flash` and `flash_events` on every result, next to the other result
/// fields (not nested), `flash` false and `flash_events` empty for a run that did not flash, and
/// a report written before the check existed reads back as no flash.
#[test]
fn test_json_has_flash_and_flash_events_at_the_result_level() {
    let flashed = flashing_result("t/flash");
    let clean = run_result("t/clean", "", 5);
    let report = report_with_runs(&[flashed, clean], &[]);
    let value = serde_json::to_value(&report).unwrap();
    let results = value["results"].as_array().unwrap();
    assert_eq!(results[0]["flash"], true);
    assert_eq!(results[0]["flash_basis"], "client");
    assert_eq!(results[0]["flash_clock_uncertainty_ms"], 1.2);
    assert_eq!(results[0]["flash_certain"], 1);
    assert_eq!(results[0]["flash_possible"], 0);
    assert_eq!(results[0]["refresh_hz"], 60.0);
    assert!((results[0]["frame_ms"].as_f64().unwrap() - 16.6667).abs() < 1e-3);
    assert_eq!(results[0]["flash_events"][0]["lead_ms"], 25.0);
    assert_eq!(results[0]["flash_events"][0]["visible_probability"], 1.0);
    assert_eq!(results[0]["flash_events"][0]["visibility"], "certain");
    let events = results[0]["flash_events"].as_array().unwrap();
    assert_eq!(events.len(), 2);
    assert_eq!(events[0]["kind"], "paint-after-reveal");
    assert_eq!(events[0]["source"], "client");
    assert_eq!(events[0]["numeric"], true);
    assert_eq!(events[1]["numeric"], false);
    assert_eq!(results[1]["flash"], false);
    assert_eq!(results[1]["flash_events"], serde_json::json!([]));
    assert!(results[1].get("flash_basis").is_none());

    let mut old = value.clone();
    for key in [
        "flash",
        "flash_events",
        "flash_basis",
        "flash_clock_uncertainty_ms",
    ] {
        old["results"][0].as_object_mut().unwrap().remove(key);
    }
    let back: SessionReport = serde_json::from_value(old).unwrap();
    assert!(!back.results[0].flash_info.flash);
    assert!(back.results[0].flash_info.flash_events.is_empty());
    let round: SessionReport = serde_json::from_value(value).unwrap();
    assert_eq!(round.results[0].flash_info, report.results[0].flash_info);
}

/// report.md labels the run `Pass (flash)`, counts it in the summary, lists every event (the
/// uncertain one marked as not counted) under "6. Flash", and a run without events adds no row.
#[test]
fn test_markdown_lists_flash_events_and_counts_the_pass_with_flash() {
    let flashed = flashing_result("t/flash");
    let clean = run_result("t/clean", "", 5);
    let md = render_markdown_report(&report_with_runs(&[flashed, clean], &[]));
    assert!(md.contains("**`Pass (flash)`**"), "{}", md);
    assert!(md.contains("**Tests Passed With Flash:** 1 (1 with a certain flash, 0 with only possible ones; display "), "{}", md);
    assert!(md.contains("## 6. Flash"), "{}", md);
    assert!(
        md.contains("| `t/flash` | PaintAfterReveal | Client | 25.0 | yes | certain | first paint at 800x600 came 25.0 ms after the reveal |"),
        "{}",
        md
    );
    assert!(
        md.contains(
            "| `t/flash` | SizeChangeAfterReveal | Client | 3.0 | no (not above the uncertainty or threshold) | - |"
        ),
        "{}",
        md
    );
    assert!(!md.contains("| `t/clean` | PaintAfterReveal"), "{}", md);
    let none = render_markdown_report(&report_with_runs(&[run_result("t/clean", "", 5)], &[]));
    assert!(!none.contains("## 6. Flash"), "{}", none);
    assert!(none.contains("**Tests Passed With Flash:** 0"), "{}", none);
}

/// report.json carries the wait rule of each run at the result level: `wait_strategy` and
/// `wait_cap_ms` from the extension's log for every traced run, and `wait_requested` only when
/// `run` was given the options. A run with nothing recorded has none of the keys, and a report
/// written before the fields existed reads back as nothing recorded.
#[test]
fn test_json_wait_fields_with_and_without_an_override() {
    use crate::trace::wait::{WaitInfo, WaitRequest};
    let mut default_run = run_result("t/default", "", 5);
    default_run.wait_info = WaitInfo {
        wait_strategy: Some("ready-wait".into()),
        wait_cap_ms: Some(150),
        ..Default::default()
    };
    let mut legacy_run = run_result("t/legacy", "", 5);
    legacy_run.wait_info = WaitInfo {
        wait_strategy: Some("legacy".into()),
        wait_requested: Some(WaitRequest {
            strategy: Some("legacy".into()),
            cap_ms: Some("90".into()),
        }),
        wait_notes: vec!["override had no effect: cap_ms(legacy-has-no-cap)".into()],
        ..Default::default()
    };
    let untraced = run_result("t/untraced", "", 5);
    let report = report_with_runs(&[default_run, legacy_run, untraced], &[]);
    let value = serde_json::to_value(&report).unwrap();
    let results = value["results"].as_array().unwrap();

    // Without the options: the effective rule, no request, no notes.
    assert_eq!(results[0]["wait_strategy"], "ready-wait");
    assert_eq!(results[0]["wait_cap_ms"], 150);
    assert!(results[0].get("wait_requested").is_none());
    assert!(results[0].get("wait_notes").is_none());

    // With the options: the request as typed, the effective rule, no cap for legacy, the note.
    assert_eq!(results[1]["wait_strategy"], "legacy");
    assert!(results[1].get("wait_cap_ms").is_none());
    assert_eq!(
        results[1]["wait_requested"],
        serde_json::json!({"strategy": "legacy", "cap_ms": "90"})
    );
    assert_eq!(
        results[1]["wait_notes"],
        serde_json::json!(["override had no effect: cap_ms(legacy-has-no-cap)"])
    );

    // Nothing recorded: none of the keys.
    for key in [
        "wait_strategy",
        "wait_cap_ms",
        "wait_requested",
        "wait_notes",
    ] {
        assert!(results[2].get(key).is_none(), "{key}");
    }

    // The fields sit next to the other result fields, not nested.
    assert!(results[0].get("wait_info").is_none());

    let mut old = value.clone();
    for key in ["wait_strategy", "wait_cap_ms"] {
        old["results"][0].as_object_mut().unwrap().remove(key);
    }
    let back: SessionReport = serde_json::from_value(old).unwrap();
    assert_eq!(back.results[0].wait_info, WaitInfo::default());
}

/// The paint fields are written next to the other flash fields as `paint_source` ("wire" or
/// "stamp"), `paint_size` as `[w, h]`, `paint_wire_ms`, `paint_stamp_ms`, `paint_stamp_lag_ms`
/// and `flash_by_stamp`, round-trip, and are absent for a run without paints.
#[test]
fn test_json_carries_the_paint_source_and_both_paint_times() {
    use crate::trace::flash::PaintSource;
    let mut r = flashing_result("t/wire");
    r.flash_info.paint_source = Some(PaintSource::Wire);
    r.flash_info.paint_size = Some((800, 600));
    r.flash_info.paint_wire_ms = Some(-10.0);
    r.flash_info.paint_stamp_ms = Some(25.0);
    r.flash_info.paint_stamp_lag_ms = Some(35.0);
    r.flash_info.flash_by_stamp = Some(true);
    let clean = run_result("t/clean", "", 5);
    let report = report_with_runs(&[r, clean], &[]);
    let value = serde_json::to_value(&report).unwrap();
    let results = value["results"].as_array().unwrap();
    assert_eq!(results[0]["paint_source"], "wire");
    assert_eq!(results[0]["paint_size"], serde_json::json!([800, 600]));
    assert_eq!(results[0]["paint_wire_ms"], -10.0);
    assert_eq!(results[0]["paint_stamp_ms"], 25.0);
    assert_eq!(results[0]["paint_stamp_lag_ms"], 35.0);
    assert_eq!(results[0]["flash_by_stamp"], true);
    for key in [
        "paint_source",
        "paint_size",
        "paint_wire_ms",
        "flash_by_stamp",
    ] {
        assert!(
            results[1].get(key).is_none(),
            "{key} written for a run without paints"
        );
    }
    let round: SessionReport = serde_json::from_value(value).unwrap();
    assert_eq!(round.results[0].flash_info, report.results[0].flash_info);
}
