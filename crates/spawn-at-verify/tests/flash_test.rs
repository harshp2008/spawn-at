//! Flash detection on synthetic traces: the extension's log lines and the fixture's event file
//! are written by hand so every timing in the assertions is a number in the test.

use spawn_at_verify::trace::flash::{
    detect_flash, echo_times_ms, parse_client_events, ClockAnchor, FlashBasis, FlashInputs,
    FlashKind, FlashSource, MonoBracket,
};
use spawn_at_verify::trace::{TraceAnalyser, TraceEvent};

/// Where the extension's clock starts on the monotonic clock in these traces: 1000 s.
const T0_NS: u64 = 1_000_000_000_000;

/// One extension log line for window 77.
fn line(ms: f64, tag: &str, extra: &str) -> String {
    format!(
        "+{:.2}ms | {} win=[id=77 pid=9 class=\"verify.x\" app=\"\" type=NORMAL proto=X11] {}",
        ms, tag, extra
    )
}

/// The extension's events for window 77 parsed from `lines`.
fn events(lines: &[String]) -> Vec<TraceEvent> {
    TraceAnalyser::new()
        .with_target_window_id(77)
        .parse_events(&lines.join("\n"))
}

/// One fixture event line stamped `ms` after the extension's clock origin.
fn client_line(ms: f64, tag: &str, detail: &str) -> String {
    format!("{} FIXTURE {} {}", T0_NS + (ms * 1e6) as u64, tag, detail)
}

/// An anchor whose interval is `half_ms` wide on each side of `T0_NS`: one echo at 5 ms
/// (`+5.00ms | SET_LOGGING enabled=true`) in a call bracketed `half_ms` before and after it.
fn anchor(half_ms: f64) -> ClockAnchor {
    let echo_at = T0_NS + 5_000_000;
    let b = MonoBracket {
        before_ns: echo_at - (half_ms * 1e6) as u64,
        after_ns: echo_at + (half_ms * 1e6) as u64,
    };
    ClockAnchor::from_samples(&[b], &[5.0]).unwrap()
}

/// The extension side of `placement/bottom-right-larger` on X11, as in the screen recording:
/// the frame rect reads 800x600 0.2 ms after SetSize and COMMIT_RESOLVED says AT_TARGET 0.1 ms
/// later, whatever the client has painted. The reveal is at +70 ms.
fn x11_lying_trace() -> Vec<TraceEvent> {
    events(&[
        line(30.0, "CLOAK", "opacity=0 frame=(1104,464,400x300)"),
        line(34.3, "SET_SIZE_REQUEST", "requestedSize=(800x600)"),
        line(34.5, "AFTER_SET_SIZE", "opacity=0 frame=(1104,464,800x600)"),
        line(
            34.7,
            "COMMIT_RESOLVED",
            "reason=AT_TARGET finalFrame=(800x600) target=(800x600) safe=(800x600)",
        ),
        line(60.5, "BATCH_STEP", "inst=SetPositionAnchored"),
        line(
            62.0,
            "AFTER_SET_POSITION_ANCHORED",
            "opacity=0 frame=(1104,464,800x600)",
        ),
        line(70.0, "UNCLOAK", "opacity=255 frame=(1104,464,800x600)"),
    ])
}

/// A fixture that was told to resize at +35 ms (the X server applied it at once) but painted the
/// new size only at +95 ms, 25 ms after the reveal: the recorded flash.
fn x11_late_paint_client() -> String {
    [
        client_line(10.0, "FIRST_PAINT", "400x300"),
        client_line(10.4, "RESIZED", "400x300"),
        client_line(35.0, "RESIZED", "800x600"),
        client_line(95.0, "FIRST_PAINT", "800x600"),
    ]
    .join("\n")
}

/// The X11 recording: the extension's frame says 800x600 at +34.5 and the trace has no warning of
/// any kind, but the fixture's first paint at 800x600 came 25 ms after the reveal. The check
/// must call it a flash, count it, and name the 25 ms.
#[test]
fn test_x11_lying_frame_rect_late_paint_is_a_flash() {
    let ev = x11_lying_trace();
    let client = parse_client_events(&x11_late_paint_client());
    let a = anchor(1.0);
    let info = detect_flash(&FlashInputs {
        trace_events: &ev,
        client_events: &client,
        anchor: Some(&a),
        params: Default::default(),
    });
    assert!(info.flash, "events: {:?}", info.flash_events);
    assert_eq!(info.flash_basis, Some(FlashBasis::Client));
    assert_eq!(info.flash_events.len(), 1, "{:?}", info.flash_events);
    let e = &info.flash_events[0];
    assert_eq!(e.kind, FlashKind::SizeChangeAfterReveal);
    assert_eq!(e.source, FlashSource::Client);
    assert!(e.numeric);
    assert!(
        (e.lead_ms - 25.0).abs() < 0.01,
        "paint at +95 ms, reveal at +70 ms: {}",
        e.lead_ms
    );
    assert!(
        e.detail.contains("first paint at 800x600") && e.detail.contains("content was 400x300"),
        "{}",
        e.detail
    );
    assert!((info.flash_clock_uncertainty_ms.unwrap() - 1.0).abs() < 0.02);
}

/// The same extension trace judged without the fixture's events: the old signals see nothing.
/// This is the blind spot: no ANCHOR_CORRECTION, the frame rect equals the target, so the run is
/// "clean" and `flash` stays false, with a note that only the extension was consulted.
#[test]
fn test_x11_lying_frame_rect_is_invisible_to_the_extension_only_check() {
    let ev = x11_lying_trace();
    let info = detect_flash(&FlashInputs {
        trace_events: &ev,
        client_events: &[],
        anchor: None,
        params: Default::default(),
    });
    assert!(!info.flash);
    assert_eq!(info.flash_basis, Some(FlashBasis::ExtensionOnly));
    assert!(info
        .flash_notes
        .iter()
        .any(|n| n.contains("no fixture events")));
    let report = TraceAnalyser::new()
        .with_target_window_id(77)
        .analyse(&x11_raw());
    assert!(report.is_pass());
    assert!(
        report.warnings.is_empty(),
        "the existing warnings are blind to it: {:?}",
        report.warnings
    );
}

/// The raw text of [`x11_lying_trace`], for the analyser.
fn x11_raw() -> String {
    [
        line(30.0, "CLOAK", "opacity=0 frame=(1104,464,400x300)"),
        line(34.5, "AFTER_SET_SIZE", "opacity=0 frame=(1104,464,800x600)"),
        line(
            34.7,
            "COMMIT_RESOLVED",
            "reason=AT_TARGET finalFrame=(800x600) target=(800x600) safe=(800x600)",
        ),
        line(
            62.0,
            "AFTER_SET_POSITION_ANCHORED",
            "opacity=0 frame=(1104,464,800x600)",
        ),
        line(70.0, "UNCLOAK", "opacity=255 frame=(1104,464,800x600)"),
    ]
    .join("\n")
}

/// Control: the fixture repainted at 800x600 1.2 ms after the resize, 33 ms before the reveal.
/// Nothing changes after the reveal, so there is no flash and no event.
#[test]
fn test_paint_before_reveal_is_not_a_flash() {
    let ev = x11_lying_trace();
    let client = parse_client_events(
        &[
            client_line(10.0, "FIRST_PAINT", "400x300"),
            client_line(35.0, "RESIZED", "800x600"),
            client_line(36.2, "FIRST_PAINT", "800x600"),
        ]
        .join("\n"),
    );
    let a = anchor(1.0);
    let info = detect_flash(&FlashInputs {
        trace_events: &ev,
        client_events: &client,
        anchor: Some(&a),
        params: Default::default(),
    });
    assert!(!info.flash);
    assert!(info.flash_events.is_empty(), "{:?}", info.flash_events);
    assert_eq!(info.flash_basis, Some(FlashBasis::Client));
}

/// Top-left anchor, window shrinks in place from 400x300 to 300x200 after the reveal. The
/// anchor corner does not move, so the extension logs no ANCHOR_CORRECTION and the old warning
/// stays silent; the fixture's RESIZED + FIRST_PAINT 30 ms after the reveal is a size flash.
#[test]
fn test_top_left_in_place_shrink_after_reveal_is_a_size_flash() {
    let lines = [
        line(20.0, "CLOAK", "opacity=0 frame=(16,56,400x300)"),
        line(24.0, "AFTER_SET_SIZE", "opacity=0 frame=(16,56,400x300)"),
        line(
            54.0,
            "COMMIT_RESOLVED",
            "reason=SIZE_SETTLED finalFrame=(400x300) target=(300x200) safe=(300x200)",
        ),
        line(
            56.0,
            "AFTER_SET_POSITION_ANCHORED",
            "opacity=0 frame=(16,56,400x300)",
        ),
        line(60.0, "UNCLOAK", "opacity=255 frame=(16,56,400x300)"),
        line(61.0, "ANCHOR_ARMED", ""),
    ];
    let ev = events(&lines);
    let report = TraceAnalyser::new()
        .with_target_window_id(77)
        .analyse(&lines.join("\n"));
    assert!(report.warnings.is_empty(), "{:?}", report.warnings);

    let client = parse_client_events(
        &[
            client_line(8.0, "FIRST_PAINT", "400x300"),
            client_line(90.0, "RESIZED", "300x200"),
            client_line(91.0, "FIRST_PAINT", "300x200"),
        ]
        .join("\n"),
    );
    let a = anchor(1.0);
    let info = detect_flash(&FlashInputs {
        trace_events: &ev,
        client_events: &client,
        anchor: Some(&a),
        params: Default::default(),
    });
    assert!(info.flash, "{:?}", info.flash_events);
    let kinds: Vec<FlashKind> = info.flash_events.iter().map(|e| e.kind).collect();
    assert_eq!(kinds, vec![FlashKind::SizeChangeAfterReveal]);
    let e = &info.flash_events[0];
    assert!(
        (e.lead_ms - 31.0).abs() < 0.01,
        "paint at +91 ms, reveal at +60 ms: {}",
        e.lead_ms
    );
    assert!(
        e.detail.contains("first paint at 300x200") && e.detail.contains("content was 400x300"),
        "{}",
        e.detail
    );
}

/// A window revealed before its first paint with no size change at all (the fixture's
/// `--paint-delay-ms`): the first and only paint comes 240 ms after the reveal.
#[test]
fn test_first_paint_after_reveal_without_a_size_change_is_a_flash() {
    let ev = x11_lying_trace();
    let client = parse_client_events(&client_line(310.0, "FIRST_PAINT", "800x600"));
    let a = anchor(1.0);
    let info = detect_flash(&FlashInputs {
        trace_events: &ev,
        client_events: &client,
        anchor: Some(&a),
        params: Default::default(),
    });
    assert!(info.flash);
    assert_eq!(info.flash_events.len(), 1);
    assert_eq!(info.flash_events[0].kind, FlashKind::PaintAfterReveal);
    assert!((info.flash_events[0].lead_ms - 240.0).abs() < 0.01);
}

/// A paint 3 ms after the reveal with a clock anchor that is 5 ms wide on each side cannot be
/// ordered against the reveal: the event is listed as not certain and the run is not a flash.
#[test]
fn test_order_inside_the_clock_uncertainty_is_listed_but_not_counted() {
    let ev = x11_lying_trace();
    let client = parse_client_events(&client_line(73.0, "FIRST_PAINT", "800x600"));
    let a = anchor(5.0);
    let info = detect_flash(&FlashInputs {
        trace_events: &ev,
        client_events: &client,
        anchor: Some(&a),
        params: Default::default(),
    });
    assert!(!info.flash);
    assert_eq!(info.flash_events.len(), 1);
    assert!(!info.flash_events[0].numeric);
}

/// A size change of 2 px (within the tolerance) after the reveal is not a flash; one of 3 px is.
/// A change 1600 ms after the reveal is outside the 1500 ms watch and is not reported.
#[test]
fn test_tolerance_and_watch_window() {
    let ev = x11_lying_trace();
    let a = anchor(1.0);
    let run = |client: &str| {
        let c = parse_client_events(client);
        detect_flash(&FlashInputs {
            trace_events: &ev,
            client_events: &c,
            anchor: Some(&a),
            params: Default::default(),
        })
    };
    // The fixture has painted 800x600 before the reveal; `extra` is one more paint.
    let with = |extra: String| {
        run(&[
            client_line(10.0, "FIRST_PAINT", "800x600"),
            client_line(35.0, "RESIZED", "800x600"),
            extra,
        ]
        .join("\n"))
    };
    let within = with(client_line(100.0, "FIRST_PAINT", "802x598"));
    assert!(!within.flash, "{:?}", within.flash_events);
    let beyond = with(client_line(100.0, "FIRST_PAINT", "803x600"));
    assert!(beyond.flash, "{:?}", beyond.flash_events);
    let late = with(client_line(1670.0, "FIRST_PAINT", "400x300"));
    assert!(!late.flash, "{:?}", late.flash_events);
}

/// Without the fixture (a real app) the extension's own ANCHOR_CORRECTION after UNCLOAK is the
/// only signal: it makes a flash with source Extension, exact on the extension's clock.
#[test]
fn test_extension_only_position_correction_after_reveal() {
    let ev = events(&[
        line(30.0, "CLOAK", "opacity=0 frame=(760,764,400x300)"),
        line(50.0, "UNCLOAK", "opacity=255 frame=(760,764,400x300)"),
        line(
            83.9,
            "ANCHOR_CORRECTION",
            "size=(300x200) from=(760,764) to=(810,864)",
        ),
    ]);
    let info = detect_flash(&FlashInputs {
        trace_events: &ev,
        client_events: &[],
        anchor: None,
        params: Default::default(),
    });
    assert!(info.flash);
    assert_eq!(info.flash_basis, Some(FlashBasis::ExtensionOnly));
    let e = &info.flash_events[0];
    assert_eq!(e.kind, FlashKind::PositionChangeAfterReveal);
    assert_eq!(e.source, FlashSource::Extension);
    assert!((e.lead_ms - 33.9).abs() < 0.01);
}

/// The extension's ANCHOR_CORRECTION and the fixture's MOVED report one jump: it is listed once,
/// from the extension (exact on its own clock); the client's position is not consulted then.
#[test]
fn test_extension_correction_is_listed_once_not_doubled_by_the_client_move() {
    let ev = events(&[
        line(30.0, "CLOAK", "opacity=0 frame=(760,764,400x300)"),
        line(50.0, "UNCLOAK", "opacity=255 frame=(760,764,400x300)"),
        line(
            83.9,
            "ANCHOR_CORRECTION",
            "size=(300x200) from=(760,764) to=(810,864)",
        ),
    ]);
    let client = parse_client_events(
        &[
            client_line(5.0, "FIRST_PAINT", "300x200"),
            client_line(20.0, "MOVED", "760,764"),
            client_line(84.5, "MOVED", "810,864"),
        ]
        .join("\n"),
    );
    let a = anchor(1.0);
    let info = detect_flash(&FlashInputs {
        trace_events: &ev,
        client_events: &client,
        anchor: Some(&a),
        params: Default::default(),
    });
    assert!(info.flash);
    assert_eq!(info.flash_events.len(), 1, "{:?}", info.flash_events);
    assert_eq!(info.flash_events[0].source, FlashSource::Extension);
    assert_eq!(
        info.flash_events[0].kind,
        FlashKind::PositionChangeAfterReveal
    );
}

/// The first live run on X11: the window manager moved the window to 1104,464 before the reveal
/// (the extension logged that frame at UNCLOAK), but the fixture was told 11 ms after the reveal.
/// A late *report* of a move is not a move after the reveal: no flash, no event.
#[test]
fn test_late_moved_report_of_a_move_before_the_reveal_is_not_a_flash() {
    let ev = x11_lying_trace();
    let client = parse_client_events(
        &[
            client_line(10.0, "FIRST_PAINT", "800x600"),
            client_line(20.0, "MOVED", "50,90"),
            client_line(81.4, "MOVED", "1104,464"),
        ]
        .join("\n"),
    );
    let a = anchor(1.0);
    let info = detect_flash(&FlashInputs {
        trace_events: &ev,
        client_events: &client,
        anchor: Some(&a),
        params: Default::default(),
    });
    assert!(!info.flash, "{:?}", info.flash_events);
    assert!(info.flash_events.is_empty());
}

/// The client may deliver several old positions after the reveal; only where it ends up counts.
/// Ending at the revealed position is lag (no flash); ending 30 px away is a move after the
/// reveal, certain and attributed to the client.
#[test]
fn test_only_the_final_client_position_decides_a_move_after_the_reveal() {
    let ev = x11_lying_trace();
    let a = anchor(1.0);
    let run = |moves: &[String]| {
        let mut lines = vec![client_line(10.0, "FIRST_PAINT", "800x600")];
        lines.extend(moves.iter().cloned());
        let client = parse_client_events(&lines.join("\n"));
        detect_flash(&FlashInputs {
            trace_events: &ev,
            client_events: &client,
            anchor: Some(&a),
            params: Default::default(),
        })
    };
    let lagging = run(&[
        client_line(72.0, "MOVED", "50,90"),
        client_line(76.0, "MOVED", "1104,464"),
    ]);
    assert!(!lagging.flash, "{:?}", lagging.flash_events);
    let moved = run(&[
        client_line(72.0, "MOVED", "1104,464"),
        client_line(100.0, "MOVED", "1134,464"),
    ]);
    assert!(moved.flash, "{:?}", moved.flash_events);
    assert_eq!(moved.flash_events.len(), 1);
    let e = &moved.flash_events[0];
    assert_eq!(
        (e.kind, e.source, e.numeric),
        (
            FlashKind::PositionChangeAfterReveal,
            FlashSource::Client,
            true
        )
    );
    assert!((e.lead_ms - 30.0).abs() < 0.01);
}

/// A trace with no UNCLOAK has no reveal to compare with: nothing is checked and the basis says so.
#[test]
fn test_no_uncloak_means_nothing_was_checked() {
    let ev = events(&[line(30.0, "CLOAK", "opacity=0 frame=(0,0,400x300)")]);
    let info = detect_flash(&FlashInputs {
        trace_events: &ev,
        client_events: &[],
        anchor: None,
        params: Default::default(),
    });
    assert!(!info.flash);
    assert_eq!(info.flash_basis, Some(FlashBasis::None));
}

/// The fixture file is parsed by tag: FIRST_PAINT, RESIZED and MOVED are kept, TRACE_START,
/// NEW_EVENTS and a half-written line are dropped, and the result is ordered by time.
#[test]
fn test_parse_client_events_keeps_known_tags_in_time_order() {
    let text = "300 FIXTURE MOVED -5,40\n\
                100 FIXTURE TRACE_START realtime_ns=1\n\
                200 FIXTURE FIRST_PAINT 400x300\n\
                250 FIXTURE NEW_EVENTS Init\n\
                oops\n\
                150 FIXTURE RESIZED 400x";
    let ev = parse_client_events(text);
    assert_eq!(ev.len(), 2);
    assert_eq!(ev[0].t_ns, 200);
    assert_eq!(ev[1].t_ns, 300);
}

/// Echo lines are read from journal text, each value once even when the line appears twice.
#[test]
fn test_echo_times_are_read_once_and_sorted() {
    let slice = "[spawn-at-time] +120.50ms | SET_LOGGING enabled=true\n\
                 [spawn-at-time] +120.50ms | SET_LOGGING enabled=true\n\
                 [spawn-at-time] +95.25ms | SET_LOGGING enabled=true\n\
                 [spawn-at-time] +100.00ms | CLOAK win=[id=1]";
    assert_eq!(echo_times_ms(slice), vec![95.25, 120.5]);
}

/// Two brackets of 6 ms and 4 ms around the same origin intersect to a 4 ms interval; two that
/// cannot both contain one origin are refused instead of averaged.
#[test]
fn test_anchor_intersects_brackets_and_refuses_disagreement() {
    let echo_a = 10.0;
    let echo_b = 40.0;
    let a = MonoBracket {
        before_ns: T0_NS + 8_000_000,
        after_ns: T0_NS + 14_000_000,
    };
    let b = MonoBracket {
        before_ns: T0_NS + 38_000_000,
        after_ns: T0_NS + 42_000_000,
    };
    let anchor = ClockAnchor::from_samples(&[a, b], &[echo_a, echo_b]).unwrap();
    assert_eq!(anchor.samples, 2);
    // Interval from b: [-2, +2] ms; from a: [-2, +4] ms; intersection [-2, +2] ms, +-5 us slack.
    assert!(
        (anchor.half_width_ms() - 2.005).abs() < 0.01,
        "{}",
        anchor.half_width_ms()
    );
    let off = MonoBracket {
        before_ns: T0_NS + 100_000_000,
        after_ns: T0_NS + 102_000_000,
    };
    let err = ClockAnchor::from_samples(&[a, off], &[echo_a, echo_b]).unwrap_err();
    assert!(err.contains("disagree"), "{}", err);
    assert!(ClockAnchor::from_samples(&[], &[1.0]).is_err());
}

/// The numeric flash check at `refresh_hz` and `threshold_ms` for a fixture that painted 400x300
/// at +10 ms and 800x600 `lead` ms after the reveal (+70 ms); the clock anchor is +-1.005 ms.
fn lead_run(
    lead: f64,
    refresh_hz: f64,
    threshold_ms: f64,
) -> spawn_at_verify::trace::flash::FlashInfo {
    use spawn_at_verify::trace::flash::FlashParams;
    let ev = x11_lying_trace();
    let c = parse_client_events(
        &[
            client_line(10.0, "FIRST_PAINT", "400x300"),
            client_line(70.0 + lead, "FIRST_PAINT", "800x600"),
        ]
        .join("\n"),
    );
    let a = anchor(1.0);
    detect_flash(&FlashInputs {
        trace_events: &ev,
        client_events: &c,
        anchor: Some(&a),
        params: FlashParams {
            refresh_hz,
            threshold_ms,
        },
    })
}

/// Lead above, at and below the frame period (50 Hz = exactly 20 ms): the headline flash is the
/// same numeric yes for all three; only the label differs. Above: certain, probability 1. At the
/// period: certain (>=), probability 1. Below: possible with probability lead / frame.
#[test]
fn test_visibility_label_above_at_and_below_the_frame_period() {
    use spawn_at_verify::trace::flash::Visibility;
    for (lead, label, prob, certain, possible) in [
        (30.0, Visibility::Certain, 1.0, 1, 0),
        (20.0, Visibility::Certain, 1.0, 1, 0),
        (5.0, Visibility::Possible, 0.25, 0, 1),
    ] {
        let info = lead_run(lead, 50.0, 0.0);
        assert!(info.flash, "lead {}: {:?}", lead, info.flash_events);
        assert_eq!(
            (info.flash_certain, info.flash_possible),
            (certain, possible),
            "lead {}",
            lead
        );
        let e = &info.flash_events[0];
        assert!(e.numeric);
        assert_eq!(e.visibility, Some(label), "lead {}", lead);
        assert!(
            (e.visible_probability - prob).abs() < 1e-6,
            "lead {}: {}",
            lead,
            e.visible_probability
        );
        assert_eq!((info.refresh_hz, info.frame_ms), (Some(50.0), Some(20.0)));
        assert!((info.visible_probability.unwrap() - prob).abs() < 1e-6);
    }
}

/// Leads at or below the clock uncertainty (half-width 1.005 ms) are listed but not counted: 0.5
/// ms is inside it, 1.5 ms is outside. The headline is false, both counts are 0, and the event
/// carries its lead and a label but is not numeric.
#[test]
fn test_lead_inside_the_clock_uncertainty_is_listed_not_counted() {
    let inside = lead_run(0.5, 60.0, 0.0);
    assert!(!inside.flash);
    assert_eq!((inside.flash_certain, inside.flash_possible), (0, 0));
    assert_eq!(inside.flash_events.len(), 1);
    assert!(!inside.flash_events[0].numeric);
    assert_eq!(inside.visible_probability, None);
    let outside = lead_run(1.5, 60.0, 0.0);
    assert!(outside.flash);
    assert_eq!(outside.flash_possible, 1);
}

/// `--flash-threshold-ms` raises the floor: a 5 ms lead is a flash at a floor of 4.9 and is not at
/// 5.0 (the lead must be above it) or 8.0; it stays listed.
#[test]
fn test_flash_threshold_raises_the_floor() {
    assert!(lead_run(5.0, 60.0, 4.9).flash);
    for floor in [5.0, 8.0] {
        let info = lead_run(5.0, 60.0, floor);
        assert!(!info.flash, "floor {}", floor);
        assert_eq!(info.flash_events.len(), 1);
        assert!(!info.flash_events[0].numeric);
    }
}

/// The same 10 ms late paint on a 60 Hz and a 144 Hz screen: the headline is identical (one
/// numeric flash, lead 10 ms); the label is "possible, 60%" at 60 Hz (frame 16.7 ms) and
/// "certain" at 144 Hz (frame 6.9 ms).
#[test]
fn test_same_lead_on_60_and_144_hz() {
    use spawn_at_verify::trace::flash::Visibility;
    let slow = lead_run(10.0, 60.0, 0.0);
    let fast = lead_run(10.0, 144.0, 0.0);
    assert!(slow.flash && fast.flash);
    assert_eq!(slow.flash_events[0].lead_ms, fast.flash_events[0].lead_ms);
    assert_eq!(slow.flash_events[0].visibility, Some(Visibility::Possible));
    assert!((slow.flash_events[0].visible_probability - 0.6).abs() < 1e-6);
    assert_eq!(fast.flash_events[0].visibility, Some(Visibility::Certain));
    assert_eq!(fast.flash_events[0].visible_probability, 1.0);
    assert!((fast.frame_ms.unwrap() - 1000.0 / 144.0).abs() < 1e-9);
}

/// Two numeric events in one run (paints 400x300 -> 500x400 at +5 ms and -> 800x600 at +10 ms
/// after the reveal, 60 Hz): the run is seen if either is, 1 - (1 - 0.3)(1 - 0.6) = 0.72.
#[test]
fn test_run_probability_combines_events() {
    use spawn_at_verify::trace::flash::FlashParams;
    let ev = x11_lying_trace();
    let c = parse_client_events(
        &[
            client_line(10.0, "FIRST_PAINT", "400x300"),
            client_line(75.0, "FIRST_PAINT", "500x400"),
            client_line(80.0, "FIRST_PAINT", "800x600"),
        ]
        .join("\n"),
    );
    let a = anchor(1.0);
    let info = detect_flash(&FlashInputs {
        trace_events: &ev,
        client_events: &c,
        anchor: Some(&a),
        params: FlashParams::default(),
    });
    assert_eq!(info.flash_events.len(), 2);
    assert_eq!((info.flash_certain, info.flash_possible), (0, 2));
    assert!(
        (info.visible_probability.unwrap() - 0.72).abs() < 1e-3,
        "{:?}",
        info.visible_probability
    );
}
