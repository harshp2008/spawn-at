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

/// Every embedded recording is listed, and each id is one of the two real recordings.
#[test]
fn test_recorded_cases_list_the_real_recordings() {
    let ids: Vec<String> = recorded_cases().into_iter().map(|c| c.id).collect();
    assert_eq!(
        ids,
        vec!["placement/bottom-right", "cloak/delayed-paint/delay-500"]
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
