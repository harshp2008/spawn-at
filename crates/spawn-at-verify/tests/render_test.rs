//! `spawn-at-verify render`: recorded real cases only, correct case selected, nothing wider than
//! the requested width, and the window-scoped timeline of the real delay-500 recording.

use spawn_at_verify::tui::render::{find_case, recorded_cases, render_case};
use std::path::PathBuf;
use std::process::Command;
use unicode_width::UnicodeWidthStr;

/// The repository's `verify/` directory, found relative to this crate.
fn verify_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../verify")
}

/// Every embedded recording is listed, in fixture order.
#[test]
fn test_recorded_cases_list_the_real_recordings() {
    let names: Vec<String> = recorded_cases().into_iter().map(|c| c.name).collect();
    assert_eq!(
        names,
        vec![
            "placement/bottom-right",
            "cloak/delayed-paint/delay-500",
            "cloak/delayed-paint/delay-500@probed",
            "cloak/delayed-paint/delay-0@no-trace",
            "placement/top-left",
            "placement/center-200x150"
        ]
    );
}

/// Exact ids and unique suffixes resolve; unknown names list the available cases.
#[test]
fn test_find_case_by_id_suffix_and_error() {
    assert_eq!(
        find_case("delay-500").unwrap().id,
        "cloak/delayed-paint/delay-500"
    );
    assert_eq!(
        find_case("placement/bottom-right").unwrap().id,
        "placement/bottom-right"
    );
    let err = find_case("nope").err().expect("unknown case must error");
    assert!(
        err.contains("placement/bottom-right") && err.contains("delay-500"),
        "{}",
        err
    );
}

/// At every width no rendered row is wider than the terminal, and the drawer shows the case that
/// was asked for (not another test from the registry).
#[test]
fn test_render_never_exceeds_width_and_shows_requested_case() {
    for case_name in ["placement/bottom-right", "delay-500"] {
        let case = find_case(case_name).unwrap();
        for &w in &[60u16, 70, 80, 100, 140] {
            let text = render_case(&verify_dir(), &case, w, 150, 0)
                .unwrap_or_else(|e| panic!("{} at width {}: {}", case_name, w, e));
            assert_eq!(text.lines().count(), 150, "one line per terminal row");
            for (i, line) in text.lines().enumerate() {
                assert!(
                    line.width() <= w as usize,
                    "row {} is {} columns at width {}: {:?}",
                    i,
                    line.width(),
                    w,
                    line
                );
            }
            assert!(
                text.contains(&case.id),
                "{} must show its id at width {}",
                case.id,
                w
            );
        }
    }
}

/// The delay-500 recording's log slice mentions a leftover window (870906468) next to the window
/// named in stdout (870906469); the drawer must be about 869 only, with that window's own cloak.
#[test]
fn test_delay500_drawer_measures_only_the_stdout_window() {
    let case = find_case("delay-500").unwrap();
    assert_eq!(case.result.claimed_window_id(), Some(870906469));
    let text = render_case(&verify_dir(), &case, 140, 200, 0).unwrap();
    assert!(
        text.contains("window 870906469"),
        "timeline header names the stdout window"
    );
    assert!(
        !text.contains("870906468"),
        "leftover window must not appear anywhere"
    );
    let cloak = text
        .lines()
        .find(|l| l.contains("Cloak "))
        .expect("Cloak row");
    assert!(
        cloak.contains("39.9 ms"),
        "cloak of window ...469 is 39.89 ms, row: {:?}",
        cloak
    );
}

/// The real binary: `--list` and a render at width 60 succeed and print no row wider than 60.
#[test]
fn test_render_subcommand_binary_lists_and_renders() {
    let exe = env!("CARGO_BIN_EXE_spawn-at-verify");
    let list = Command::new(exe)
        .args(["render", "--list"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .unwrap();
    assert!(list.status.success());
    let list_out = String::from_utf8_lossy(&list.stdout);
    assert!(
        list_out.contains("cloak/delayed-paint/delay-500"),
        "{}",
        list_out
    );

    let out = Command::new(exe)
        .args([
            "render",
            "--width",
            "60",
            "--height",
            "150",
            "--case",
            "delay-500",
        ])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("App paints slowly (500 ms)"));
    for line in stdout.lines() {
        assert!(line.width() <= 60, "row wider than 60: {:?}", line);
    }

    let bad = Command::new(exe)
        .args(["render", "--case", "nope"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .unwrap();
    assert!(!bad.status.success(), "unknown case must fail");
}

/// The compact TIMELINE rows themselves (not just the header id and the cloak value) must come
/// from the stdout window only. The leftover window ...468 contributes its own ACTOR_MAP, CLOAK
/// and UNCLOAK to the raw slice; the drawer must show each of those exactly once, starting with
/// the target's own ARM_SPAWN.
#[test]
fn test_delay500_timeline_rows_are_one_window_only() {
    let case = find_case("delay-500").unwrap();
    let text = render_case(&verify_dir(), &case, 140, 200, 0).unwrap();
    let timeline: Vec<&str> = text
        .lines()
        .skip_while(|l| !l.contains("── TIMELINE"))
        .skip(1)
        .collect();
    let tags: Vec<&str> = timeline
        .iter()
        .filter_map(|l| {
            let t = l.trim_matches(['│', ' ']);
            t.strip_prefix('+')
                .and_then(|r| r.split_once(" ms"))
                .map(|(_, tag)| tag.trim())
        })
        .collect();
    assert_eq!(
        tags.first(),
        Some(&"ARM_SPAWN"),
        "timeline must start at the target's own ARM_SPAWN: {:?}",
        tags
    );
    for once in ["ACTOR_MAP", "CLOAK", "UNCLOAK"] {
        assert_eq!(
            tags.iter().filter(|t| **t == once).count(),
            1,
            "{} must appear exactly once in {:?}",
            once,
            tags
        );
    }
}

/// Recordings made before the workarea fields existed still load: the fields are simply absent,
/// serialize back out only when present, and the case renders with the extension-derived marker.
#[test]
fn test_old_recording_without_workarea_fields_loads_and_is_marked_extension_derived() {
    let old = find_case("placement/bottom-right").unwrap();
    assert_eq!(old.environment.primary_workarea, None);
    assert_eq!(old.environment.independent_workarea, None);
    let json = serde_json::to_string(&old.environment).unwrap();
    assert!(
        !json.contains("workarea"),
        "absent fields are not written: {}",
        json
    );
    let text = render_case(&verify_dir(), &old, 100, 100, 0).unwrap();
    assert!(
        text.contains("workarea from the extension, not independently probed"),
        "{}",
        text
    );
}

/// A report environment without the fields deserializes; with them it round-trips.
#[test]
fn test_environment_fingerprint_workarea_fields_are_optional_and_round_trip() {
    use spawn_at_verify::oracle::Rect;
    use spawn_at_verify::report::EnvironmentFingerprint;
    let bare = r#"{"hostname":"h","session_type":"wayland","desktop":"gnome","gnome_version":46,
        "monitors_count":1,"binary_path":"b","binary_commit":null,"is_dirty_build":false,
        "generated_at":1}"#;
    let e: EnvironmentFingerprint = serde_json::from_str(bare).unwrap();
    assert_eq!((e.primary_workarea, e.independent_workarea), (None, None));
    let mut e2 = e.clone();
    e2.independent_workarea = Some(Rect::new(0, 40, 1920, 1040));
    let back: EnvironmentFingerprint =
        serde_json::from_str(&serde_json::to_string(&e2).unwrap()).unwrap();
    assert_eq!(
        back.independent_workarea,
        Some(Rect::new(0, 40, 1920, 1040))
    );
    assert_eq!(back.primary_workarea, None);
}

/// `delay-500` still means the older recording; the labelled one is addressed explicitly.
#[test]
fn test_labelled_recording_is_addressed_by_its_own_name() {
    assert_eq!(
        find_case("delay-500").unwrap().fixture,
        "real_delay500_run.json"
    );
    for name in ["delay-500@probed", "cloak/delayed-paint/delay-500@probed"] {
        let c = find_case(name).unwrap();
        assert_eq!(c.fixture, "real_delay500_probed_run.json");
        assert_eq!(c.id, "cloak/delayed-paint/delay-500");
    }
}

/// The fresh recording carries both workarea measurements, and `render` shows the independent
/// result: a plain green-style pass with no extension-derived marker and no warning, taken from
/// what the run recorded (never from a live probe of this machine).
#[test]
fn test_probed_recording_renders_the_independent_result() {
    use spawn_at_verify::oracle::Rect;
    let case = find_case("delay-500@probed").unwrap();
    assert_eq!(
        case.environment.independent_workarea,
        Some(Rect::new(0, 40, 1920, 1040))
    );
    assert_eq!(
        case.environment.primary_workarea,
        Some(Rect::new(0, 40, 1920, 1040))
    );
    assert_eq!(case.environment.hostname, "redacted");

    let text = render_case(&verify_dir(), &case, 100, 100, 0).unwrap();
    let geometry = text
        .lines()
        .find(|l| l.contains("  Geometry"))
        .expect("Geometry row");
    assert!(
        geometry.contains("✔ within ±1 px of expected"),
        "{}",
        geometry
    );
    assert!(!text.contains("not independently probed"), "{}", text);
    assert!(!text.contains("workarea differs"), "{}", text);
}

/// The two recordings of the same test differ exactly in the independent evidence: the old one is
/// marked extension-derived, the new one is not.
#[test]
fn test_old_and_probed_recordings_differ_in_independence_marker() {
    let old = render_case(&verify_dir(), &find_case("delay-500").unwrap(), 100, 100, 0).unwrap();
    let new = render_case(
        &verify_dir(),
        &find_case("delay-500@probed").unwrap(),
        100,
        100,
        0,
    )
    .unwrap();
    assert!(old.contains("workarea from the extension, not independently probed"));
    assert!(!new.contains("workarea from the extension, not independently probed"));
}

/// The `delay-0@no-trace` recording is a real run of the current binary whose timing log could
/// not be enabled (`gdbus` was not on PATH) while the extension's logging was off. Its log slice
/// is empty, so the drawer must say nothing was analysed or measured, give the reason, and show
/// the unchecked verdict (never a clean trace or a cloak time).
#[test]
fn test_no_trace_recording_renders_not_analysed_and_unchecked() {
    let case = find_case("delay-0@no-trace").unwrap();
    assert_eq!(case.result.log_slice.as_deref(), Some(""));
    assert_eq!(case.result.claimed_window_id(), Some(870906703));
    let text = render_case(&verify_dir(), &case, 100, 150, 0).unwrap();
    // The drawer row for `label`, without the box border on either side.
    let row = |label: &str| {
        text.lines()
            .map(|l| l.trim_matches(|c: char| c == '│' || c.is_whitespace()))
            .find(|l| l.starts_with(label))
            .unwrap_or_else(|| panic!("no {} row in:\n{}", label, text))
            .to_string()
    };
    assert!(
        row("Verdict").contains("Pass (trace unchecked)"),
        "{}",
        text
    );
    assert!(
        row("Trace").contains("not analysed (no trace captured)"),
        "{}",
        text
    );
    assert!(row("Cloak").ends_with("not measured"), "{}", text);
    assert!(row("Budget").ends_with("not measured"), "{}", text);
    assert!(row("Reason").contains("log slice is empty"), "{}", text);
    assert!(
        text.contains("extension timing log could not be enabled"),
        "{}",
        text
    );
    assert!(!text.contains("clean, no violations"), "{}", text);
    assert!(
        !text.contains("122.0 ms"),
        "run duration is not a cloak time:\n{}",
        text
    );
}

/// The recorded `both`-mode run of `placement/center-200x150` whose automatic check failed (spawn-at
/// warned that the window was not at the target) renders `✖ Fail` in the Verdict row and never
/// `Pending`; only the "Your verdict" row, which is the human's, stays Pending.
#[test]
fn test_failed_automatic_check_of_a_both_test_shows_fail_not_pending() {
    let case = find_case("placement/center-200x150").unwrap();
    assert_eq!(
        case.result.final_status,
        spawn_at_verify::runner::FinalStatus::Fail
    );
    let text = render_case(&verify_dir(), &case, 80, 60, 0).unwrap();
    let row = |label: &str| {
        text.lines()
            .find(|l| l.contains(label))
            .unwrap_or_else(|| panic!("no {} row in:\n{}", label, text))
            .to_string()
    };
    let verdict = row("Verdict");
    assert!(verdict.contains("✖ Fail"), "{}", verdict);
    assert!(!verdict.contains("Pending"), "{}", verdict);
    assert!(row("Your verdict").contains("Pending"), "{}", text);
}
