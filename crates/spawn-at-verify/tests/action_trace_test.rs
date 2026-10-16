//! The per-action trace of a version 2 test: the owner's real journal lines for a transform and a
//! focus of an existing window, the filter that keeps only runner-owned windows, the window of an
//! action, and the reason shown when there is no timeline.
//!
//! `tests/fixtures/real_transform_existing_window_trace.log` and `real_focus_existing_window_trace.log`
//! are the lines from the owner's GNOME journal; `tests/fixtures/README.md` says which fields were
//! not in the sample and how they were filled.

use spawn_at_verify::trace::action::{
    build_action_trace, command_trace, joined_raw, latest_elapsed_ms, owned_events, within_action,
    ActionSpec, OwnedWindow, TraceStatus, TraceSupport,
};

const TRANSFORM: &str = include_str!("fixtures/real_transform_existing_window_trace.log");
const FOCUS: &str = include_str!("fixtures/real_focus_existing_window_trace.log");

const W: u64 = 3210662173;
const A: u64 = 3210662165;

fn owned() -> Vec<OwnedWindow> {
    vec![
        OwnedWindow {
            id: W,
            name: "w".into(),
        },
        OwnedWindow {
            id: A,
            name: "a".into(),
        },
    ]
}

fn spec<'a>(command: &'a str, target: &'a str, id: u64) -> ActionSpec<'a> {
    ActionSpec {
        index: 0,
        command,
        target: Some(target),
        window_id: Some(id),
    }
}

fn approx(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-6
}

/// The owner's transform lines become one timeline of 16 events in log order, offsets counted from
/// the first line, and the cloak-to-reveal time is `CLOAK` (+2579307.43) to `UNCLOAK_4D_REVEAL`
/// (+2579314.46) = 7.03 ms. A parser that took the wrong pair (`UNCLOAK` at +314.44, or the snapshot
/// at +306.86) would give 7.01 or 7.60.
#[test]
fn test_real_transform_lines_make_a_timeline_with_cloak_to_reveal() {
    let events = owned_events(TRANSFORM, &owned());
    assert_eq!(events.len(), 16);
    let t = build_action_trace(&spec("transform", "w", W), &events, &owned(), None, None);
    assert_eq!(t.status, TraceStatus::Captured);
    assert_eq!(t.events.len(), 16);
    assert_eq!(t.events[0].tag, "BATCH_STEP");
    assert_eq!(t.events[0].detail, "inst=Snapshot");
    assert!(approx(t.events[0].offset_ms, 0.0));
    let cloak = t.events.iter().find(|e| e.tag == "CLOAK").unwrap();
    assert!(approx(cloak.offset_ms, 2579307.43 - 2579306.86));
    assert_eq!(cloak.detail, "opacity=0 frame=(100,140,400x300)");
    assert_eq!(cloak.window, "w");
    assert!(t.has_cloak_phase());
    let c2r = t
        .cloak_to_reveal_ms
        .expect("CLOAK and UNCLOAK_4D_REVEAL are in the lines");
    assert!(approx(c2r, 2579314.46 - 2579307.43), "{c2r}");
    assert_eq!(t.snapshot.as_deref(), Some("taken"));
    let last = t.events.last().unwrap();
    assert_eq!(last.tag, "ANCHOR_RELEASED");
    assert_eq!(last.detail, "reason=EXTERNAL_CLOSE");
}

/// The owner's focus lines: RAISE then ACTIVATE of window a, 0.09 ms apart, no cloak phase, so no
/// cloak-to-reveal and no snapshot state.
#[test]
fn test_real_focus_lines_make_a_two_event_timeline_without_cloak() {
    let events = owned_events(FOCUS, &owned());
    let t = build_action_trace(&spec("focus", "a", A), &events, &owned(), None, None);
    assert_eq!(t.status, TraceStatus::Captured);
    let tags: Vec<&str> = t.events.iter().map(|e| e.tag.as_str()).collect();
    assert_eq!(tags, ["RAISE", "ACTIVATE"]);
    assert!(approx(t.events[1].offset_ms, 2267771.38 - 2267771.29));
    assert_eq!(t.events[0].detail, "opacity=242 frame=(100,140,400x300)");
    assert!(!t.has_cloak_phase());
    assert_eq!(t.cloak_to_reveal_ms, None);
    assert_eq!(t.snapshot, None);
}

/// Another window's lines never survive the filter: the owner's browser (id 777) shares the slice
/// with the runner's window and a `SET_LOGGING` echo, and only the runner's two lines come out,
/// in a timeline that does not contain the other id or its class anywhere (not in `raw` either).
#[test]
fn test_other_windows_lines_are_excluded_everywhere() {
    let other = r#"[spawn-at-time] +2267771.10ms | RAISE win=[id=777 pid=1 class="firefox" app="org.mozilla.firefox" type=NORMAL proto=Wayland] opacity=255 frame=(0,0,1920x1080)"#;
    let echo = "[spawn-at-time] +2267770.00ms | SET_LOGGING enabled=true";
    let other_after = r#"[spawn-at-time] +2267771.50ms | ACTIVATE win=[id=777 pid=1 class="firefox" app="org.mozilla.firefox" type=NORMAL proto=Wayland] opacity=255 frame=(0,0,1920x1080)"#;
    let slice = format!("{echo}\n{other}\n{FOCUS}{other_after}\nunrelated gnome-shell line");
    let events = owned_events(&slice, &owned());
    assert_eq!(events.len(), 2);
    let t = build_action_trace(&spec("focus", "a", A), &events, &owned(), None, None);
    let all = serde_json::to_string(&t).unwrap();
    assert!(!all.contains("id=777"), "{all}");
    assert!(!all.contains("\"window_id\":777"), "{all}");
    assert!(!all.contains("firefox"), "{all}");
    assert!(!all.contains("SET_LOGGING"), "{all}");
    assert_eq!(joined_raw(&[t]).lines().count(), 2);
}

/// With no owned window in the list nothing is kept, however many lines name windows.
#[test]
fn test_empty_owned_list_keeps_nothing() {
    assert!(owned_events(TRANSFORM, &[]).is_empty());
    assert!(owned_events(FOCUS, &[]).is_empty());
}

/// The window of an action: lines at or before the floor (the log's state before the action) and
/// after the ceiling (the next action) are dropped; a line exactly at the floor is dropped too
/// (it was already in the baseline).
#[test]
fn test_within_action_applies_floor_and_ceiling() {
    let events = owned_events(TRANSFORM, &owned());
    // Floor at CLOAK's own stamp: CLOAK (+307.43) is excluded, SET_SIZE_REQUEST (same stamp) too,
    // READY_WAIT_START (+307.66) is the first line left.
    let kept = within_action(events.clone(), "transform", Some(2579307.43), None);
    assert_eq!(kept[0].tag, "READY_WAIT_START");
    // Ceiling at UNCLOAK_4B: it is kept (<=), UNCLOAK (+314.44) is not.
    let kept = within_action(events.clone(), "transform", None, Some(2579311.15));
    assert_eq!(kept.last().unwrap().tag, "UNCLOAK_4B");
    // No bounds: everything.
    assert_eq!(within_action(events, "transform", None, None).len(), 16);
}

/// Window-creation lines belong to the setup of an existing window, so they are dropped for every
/// command but `spawn`, whose action is the window's creation.
#[test]
fn test_window_creation_lines_belong_to_setup_not_to_the_action() {
    let created = r#"[spawn-at-time] +10.00ms | WINDOW_CREATED win=[id=3210662165 pid=1 class="" app="" type=NORMAL proto=Wayland] actorFound=true
[spawn-at-time] +11.00ms | ACTOR_MAP win=[id=3210662165 pid=1 class="verify.owned.a" app="" type=NORMAL proto=Wayland]
[spawn-at-time] +12.00ms | RAISE win=[id=3210662165 pid=1 class="verify.owned.a" app="" type=NORMAL proto=Wayland] opacity=255 frame=(0,0,400x300)"#;
    let events = owned_events(created, &owned());
    assert_eq!(events.len(), 3);
    let focus = within_action(events.clone(), "focus", None, None);
    assert_eq!(focus.len(), 1);
    assert_eq!(focus[0].tag, "RAISE");
    assert_eq!(within_action(events, "spawn", None, None).len(), 3);
}

/// `latest_elapsed_ms` is the newest line of any window, including lines without a window block.
#[test]
fn test_latest_elapsed_counts_every_line() {
    let slice = format!("{FOCUS}[spawn-at-time] +2267999.00ms | SET_LOGGING enabled=true\n");
    assert_eq!(latest_elapsed_ms(&slice), Some(2267999.00));
    assert_eq!(latest_elapsed_ms(""), None);
}

/// Each reason is the exact text the owner asked for; a captured trace has none.
#[test]
fn test_each_reason_string() {
    assert_eq!(TraceStatus::Captured.reason(), None);
    assert_eq!(
        TraceStatus::NotApplicable.reason().as_deref(),
        Some("not applicable: the extension logs nothing for this command")
    );
    assert_eq!(
        TraceStatus::NoExtension {
            backend: "sway".into()
        }
        .reason()
        .as_deref(),
        Some("no extension on this backend (sway)")
    );
    assert_eq!(
        TraceStatus::CaptureFailed {
            why: "journalctl exited with error: x".into()
        }
        .reason()
        .as_deref(),
        Some("capture failed: journalctl exited with error: x")
    );
    assert_eq!(
        TraceStatus::NoLines.reason().as_deref(),
        Some("no lines for this window in the slice")
    );
}

/// Empty events: a command that logs nothing is "not applicable", one that logs something is "no
/// lines"; an unknown command is the latter. A capture failure beats both, because without a
/// capture an empty timeline says nothing about the command.
#[test]
fn test_empty_timeline_status_follows_the_inventory_and_failure_wins() {
    let status = |cmd: &str, failure: Option<TraceStatus>| {
        build_action_trace(&spec(cmd, "w", W), &[], &owned(), failure, None).status
    };
    assert_eq!(status("close", None), TraceStatus::NotApplicable);
    assert_eq!(status("defocus", None), TraceStatus::NotApplicable);
    assert_eq!(status("maximize", None), TraceStatus::NotApplicable);
    assert_eq!(status("query", None), TraceStatus::NotApplicable);
    assert_eq!(status("focus", None), TraceStatus::NoLines);
    assert_eq!(status("transform", None), TraceStatus::NoLines);
    assert_eq!(status("minimize", None), TraceStatus::NoLines);
    assert_eq!(status("something-new", None), TraceStatus::NoLines);
    let failed = TraceStatus::CaptureFailed { why: "boom".into() };
    assert_eq!(status("close", Some(failed.clone())), failed);
    let none = TraceStatus::NoExtension {
        backend: "sway".into(),
    };
    assert_eq!(status("focus", Some(none.clone())), none);
}

/// The inventory classes, one per command, as read from the extension source.
#[test]
fn test_inventory_classes() {
    use TraceSupport::*;
    let class = |c: &str| command_trace(c).support;
    assert_eq!(class("transform"), Usable);
    assert_eq!(class("spawn"), Usable);
    assert_eq!(class("focus"), Usable);
    assert_eq!(class("minimize"), Partial);
    assert_eq!(class("unminimize"), Partial);
    assert_eq!(class("restore"), Partial);
    assert_eq!(class("maximize"), Nothing);
    assert_eq!(class("unmaximize"), Nothing);
    assert_eq!(class("defocus"), Nothing);
    assert_eq!(class("close"), Nothing);
    assert_eq!(class("query"), Nothing);
    assert_eq!(class("mystery"), Partial);
    assert_eq!(
        command_trace("transform").completes_with,
        ["UNCLOAK_4D_REVEAL"]
    );
    assert!(command_trace("focus").completes_with.contains(&"ACTIVATE"));
    assert!(command_trace("close").completes_with.is_empty());
}

/// A skipped snapshot is reported with the extension's reason, and a transform with no snapshot
/// step logs neither state.
#[test]
fn test_snapshot_skipped_reason_is_read_from_the_line() {
    let lines = r#"[spawn-at-time] +1.00ms | BATCH_STEP win=[id=3210662173 pid=1 class="verify.owned.w" app="" type=NORMAL proto=Wayland] inst=Snapshot
[spawn-at-time] +1.10ms | SNAPSHOT_SKIPPED win=[id=3210662173 pid=1 class="verify.owned.w" app="" type=NORMAL proto=Wayland] reason="minimized"
[spawn-at-time] +2.00ms | CLOAK win=[id=3210662173 pid=1 class="verify.owned.w" app="" type=NORMAL proto=Wayland] opacity=0 frame=(0,0,400x300)"#;
    let events = owned_events(lines, &owned());
    let t = build_action_trace(&spec("transform", "w", W), &events, &owned(), None, None);
    assert_eq!(t.snapshot.as_deref(), Some("skipped: minimized"));
    assert_eq!(
        t.cloak_to_reveal_ms, None,
        "no UNCLOAK_4D_REVEAL was logged"
    );
    assert!(t.has_cloak_phase());
}

/// A report written by this version reads back (the trace is part of the result's JSON), and a
/// trace without the optional fields reads with them absent.
#[test]
fn test_action_trace_round_trips_through_json() {
    let events = owned_events(TRANSFORM, &owned());
    let t = build_action_trace(
        &spec("transform", "w", W),
        &events,
        &owned(),
        None,
        Some(12.5),
    );
    let json = serde_json::to_string(&t).unwrap();
    let back: spawn_at_verify::trace::action::ActionTrace = serde_json::from_str(&json).unwrap();
    assert_eq!(back, t);
    assert_eq!(back.lead_in_ms, Some(12.5));
    let none = build_action_trace(&spec("close", "w", W), &[], &owned(), None, None);
    let json = serde_json::to_string(&none).unwrap();
    assert!(!json.contains("lead_in_ms") && !json.contains("cloak_to_reveal_ms"));
    assert!(json.contains(r#""state":"not-applicable""#), "{json}");
}
