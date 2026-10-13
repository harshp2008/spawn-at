//! Wiring of the flash check: the journal slice, the fixture's event text and the anchor brackets
//! become a result; the event directory is private to a run; a flash passes or fails the run
//! depending on `--strict-flash`.

use super::flash::{
    apply_flash_policy, evaluate, reveal_mono_ns, wants_flash_capture, FlashCapture, Reveal,
    EVENT_DIR_ENV,
};
use super::models::AutoStatus;
use crate::ext_logging::FakeExtension;
use crate::schema::TestDefinition;
use crate::trace::flash::{FlashBasis, FlashEvent, FlashInfo, FlashKind, FlashSource, MonoBracket};
use std::process::Command;

/// Where the extension's clock starts on the monotonic clock in these slices: 1000 s.
const T0_NS: u64 = 1_000_000_000_000;

/// A journal slice for window 77 with two `SetLogging` echoes (at +5 ms and +9 ms) and a reveal at +70 ms.
fn slice() -> String {
    let win = "win=[id=77 pid=4242 class=\"verify.x\" app=\"\" type=NORMAL proto=X11]";
    [
        "[spawn-at-time] +5.00ms | SET_LOGGING enabled=true".to_string(),
        "[spawn-at-time] +9.00ms | SET_LOGGING enabled=true".to_string(),
        format!("[spawn-at-time] +30.00ms | CLOAK {win} opacity=0 frame=(0,0,400x300)"),
        format!("[spawn-at-time] +70.00ms | UNCLOAK {win} opacity=255 frame=(0,0,800x600)"),
    ]
    .join("\n")
}

/// Brackets 1 ms wide around the two echoes of [`slice`] for an origin at `T0_NS`.
fn brackets() -> Vec<MonoBracket> {
    [5_u64, 9]
        .iter()
        .map(|ms| MonoBracket {
            before_ns: T0_NS + ms * 1_000_000 - 500_000,
            after_ns: T0_NS + ms * 1_000_000 + 500_000,
        })
        .collect()
}

/// The fixture's event text with the first paint at 800x600 at `paint_ms` on the extension's clock.
fn client_text(paint_ms: u64) -> String {
    format!(
        "{} FIXTURE FIRST_PAINT 400x300\n{} FIXTURE FIRST_PAINT 800x600\n",
        T0_NS + 8_000_000,
        T0_NS + paint_ms * 1_000_000
    )
}

/// Slice, fixture text and brackets together: a paint at +95 ms against a reveal at +70 ms is a
/// flash on the client's evidence; at +40 ms it is not.
#[test]
fn test_evaluate_ties_slice_fixture_text_and_brackets_together() {
    let late = evaluate(
        Some(&slice()),
        Some(77),
        &client_text(95),
        None,
        &brackets(),
        Default::default(),
    );
    assert!(late.flash, "{:?}", late.flash_events);
    assert_eq!(late.flash_basis, Some(FlashBasis::Client));
    assert!((late.flash_events[0].lead_ms - 25.0).abs() < 0.1);
    let early = evaluate(
        Some(&slice()),
        Some(77),
        &client_text(40),
        None,
        &brackets(),
        Default::default(),
    );
    assert!(!early.flash, "{:?}", early.flash_events);
    assert_eq!(early.flash_basis, Some(FlashBasis::Client));
}

/// Brackets that disagree with the echoes give no anchor; the result says so and falls back to
/// the extension's own log instead of guessing an order.
#[test]
fn test_evaluate_without_a_usable_anchor_says_why() {
    let far = [MonoBracket {
        before_ns: T0_NS + 900_000_000,
        after_ns: T0_NS + 901_000_000,
    }];
    let two = [far[0], far[0]];
    let info = evaluate(
        Some(&slice()),
        Some(77),
        &client_text(95),
        None,
        &two,
        Default::default(),
    );
    assert!(!info.flash);
    assert_eq!(info.flash_basis, Some(FlashBasis::ExtensionOnly));
    assert!(
        info.flash_notes
            .iter()
            .any(|n| n.contains("no clock anchor")),
        "{:?}",
        info.flash_notes
    );
}

/// No window, no slice: nothing is attributed, so the check does not run (default result).
#[test]
fn test_evaluate_needs_a_window_and_a_slice() {
    assert_eq!(
        evaluate(Some(&slice()), None, "", None, &[], Default::default()),
        FlashInfo::default()
    );
    assert_eq!(
        evaluate(None, Some(77), "", None, &[], Default::default()),
        FlashInfo::default()
    );
}

/// The reveal is placed at the middle of the anchor interval plus its +70 ms; a missing UNCLOAK
/// and a missing anchor are told apart, because only the first is worth waiting for.
#[test]
fn test_reveal_mono_ns_distinguishes_missing_reveal_from_missing_anchor() {
    match reveal_mono_ns(&slice(), 77, &brackets()) {
        Reveal::At(ns) => assert!((ns - (T0_NS as f64 + 70e6)).abs() < 1e6, "{}", ns),
        other => panic!("expected a reveal, got {:?}", other),
    }
    let no_reveal = slice().replace("UNCLOAK", "ANCHOR_ARMED");
    assert_eq!(
        reveal_mono_ns(&no_reveal, 77, &brackets()),
        Reveal::NotLogged
    );
    assert_eq!(reveal_mono_ns(&slice(), 77, &[]), Reveal::Unanchored);
}

/// A flash event for the policy tests.
fn flashing() -> FlashInfo {
    FlashInfo {
        flash: true,
        flash_events: vec![FlashEvent {
            kind: FlashKind::PaintAfterReveal,
            source: FlashSource::Client,
            lead_ms: 25.0,
            numeric: true,
            visibility: None,
            visible_probability: 0.0,
            detail: "first paint at 800x600 came 25.0 ms after the reveal".into(),
        }],
        flash_basis: Some(FlashBasis::Client),
        ..Default::default()
    }
}

/// Default: a flash on a passing run keeps `AutoPass` and appends what flashed to the message.
#[test]
fn test_flash_keeps_the_pass_by_default() {
    let (s, m) = apply_flash_policy(
        AutoStatus::AutoPass,
        "Assertions passed".into(),
        &flashing(),
        false,
    );
    assert_eq!(s, AutoStatus::AutoPass);
    assert_eq!(
        m,
        "Assertions passed (flash: first paint at 800x600 came 25.0 ms after the reveal)"
    );
}

/// `--strict-flash`: the same run fails, for each kind of pass; an unflashed pass and a run that
/// already failed are left as they were.
#[test]
fn test_strict_flash_fails_a_flashing_pass_only() {
    for pass in [
        AutoStatus::AutoPass,
        AutoStatus::AutoPassUnchecked,
        AutoStatus::AutoPassTraceUnchecked,
    ] {
        let (s, m) = apply_flash_policy(pass, "ok".into(), &flashing(), true);
        assert_eq!(s, AutoStatus::AutoFail);
        assert!(
            m.starts_with("Flash (--strict-flash): first paint at 800x600"),
            "{}",
            m
        );
    }
    let (s, m) = apply_flash_policy(
        AutoStatus::AutoPass,
        "ok".into(),
        &FlashInfo::default(),
        true,
    );
    assert_eq!((s, m.as_str()), (AutoStatus::AutoPass, "ok"));
    let (s, m) = apply_flash_policy(AutoStatus::AutoFail, "bad".into(), &flashing(), true);
    assert_eq!((s, m.as_str()), (AutoStatus::AutoFail, "bad"));
}

/// The event directory is made for the run, handed to the spawned command through the
/// environment, read back by the fixture's pid (or the only file), and removed on drop.
#[test]
fn test_capture_directory_is_private_read_by_pid_and_removed() {
    let ext = FakeExtension::new(true, &[]);
    ext.set_brackets(brackets());
    let cap = FlashCapture::start(&ext, false).expect("brackets were provided");
    let mut cmd = Command::new("/bin/true");
    cap.apply(&mut cmd);
    let dir = cmd
        .get_envs()
        .find(|(k, _)| *k == EVENT_DIR_ENV)
        .and_then(|(_, v)| v)
        .map(std::path::PathBuf::from)
        .expect("the directory is passed in the environment");
    assert!(dir.is_dir());
    assert_eq!(cap.read_events(Some(1)), "", "no file yet");

    std::fs::write(dir.join("fixture-4242.events"), "1 FIXTURE MOVED 1,2\n").unwrap();
    assert_eq!(cap.read_events(Some(4242)), "1 FIXTURE MOVED 1,2\n");
    assert_eq!(
        cap.read_events(Some(1)),
        "1 FIXTURE MOVED 1,2\n",
        "the only file is used"
    );
    assert_eq!(cap.read_events(None), "1 FIXTURE MOVED 1,2\n");

    std::fs::write(dir.join("fixture-9.events"), "2 FIXTURE MOVED 3,4\n").unwrap();
    assert_eq!(cap.read_events(Some(9)), "2 FIXTURE MOVED 3,4\n");
    assert_eq!(
        cap.read_events(Some(1)),
        "",
        "two files and no pid match: nothing is guessed"
    );

    drop(cap);
    assert!(!dir.exists(), "the directory is removed with the capture");
}

/// A logging control that cannot bracket calls (no extension) gives no capture, so the run is not
/// delayed for a check that could not compare clocks.
#[test]
fn test_no_brackets_means_no_capture() {
    let ext = FakeExtension::new(true, &[]);
    assert!(FlashCapture::start(&ext, false).is_none());
}

/// Only a fixture spawn that is expected to start gets a capture.
#[test]
fn test_capture_is_wanted_for_fixture_spawns_only() {
    let spec = |app: &str, command: &str, exit_code: i32| -> TestDefinition {
        toml::from_str(&format!(
            r#"
id = "placement/t"
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
[subject]
app = "{app}"
profile = "plain"
args = []
[action]
command = "{command}"
repeatable = false
args = []
[expect]
exit_code = {exit_code}
stderr_absent = []
stderr_present = []
diagnostics = []
never_visible_off_target = false
"#
        ))
        .expect("spec parses")
    };
    assert!(wants_flash_capture(&spec("fixture", "spawn", 0)));
    assert!(!wants_flash_capture(&spec("gedit", "spawn", 0)));
    assert!(!wants_flash_capture(&spec("fixture", "query", 0)));
    assert!(!wants_flash_capture(&spec("fixture", "spawn", 2)));
}

/// `--strict-flash` acts on the numeric flash, not on visibility: a run whose only event is a
/// sub-frame (possible) flash fails, and a run with no numeric flash passes even though an
/// uncounted event is listed.
#[test]
fn test_strict_flash_follows_the_numeric_flash_not_visibility() {
    let mut possible = flashing();
    possible.flash_possible = 1;
    possible.flash_events[0].lead_ms = 3.0;
    possible.flash_events[0].visibility = Some(crate::trace::flash::Visibility::Possible);
    possible.flash_events[0].visible_probability = 0.18;
    let (s, _) = apply_flash_policy(AutoStatus::AutoPass, "ok".into(), &possible, true);
    assert_eq!(s, AutoStatus::AutoFail);

    let mut listed_only = possible.clone();
    listed_only.flash = false;
    listed_only.flash_possible = 0;
    listed_only.flash_events[0].numeric = false;
    let (s, m) = apply_flash_policy(AutoStatus::AutoPass, "ok".into(), &listed_only, true);
    assert_eq!((s, m.as_str()), (AutoStatus::AutoPass, "ok"));
}
