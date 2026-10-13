//! Wire paint times in the flash check: the client's commit read from the fixture's protocol dump
//! replaces the fixture's `FIRST_PAINT` stamp in the verdict, and both times are reported.

use super::flash::{evaluate, FlashCapture, EVENT_DIR_ENV, TRACE_DIR_ENV};
use crate::ext_logging::FakeExtension;
use crate::trace::flash::{FlashBasis, FlashKind, MonoBracket, PaintSource};
use std::process::Command;

/// Where the extension's clock starts on the monotonic clock in these slices: 1000 s.
const T0_NS: u64 = 1_000_000_000_000;

/// A journal slice for window 77 with two `SetLogging` echoes (at +5 ms and +9 ms) and a reveal
/// at +70 ms (same shape as the one in `flash_tests`).
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

/// The fixture's event text: 400x300 stamped at +8 ms and 800x600 stamped at `stamp_ms`.
fn client_text(stamp_ms: u64) -> String {
    format!(
        "{} FIXTURE FIRST_PAINT 400x300\n{} FIXTURE FIRST_PAINT 800x600\n",
        T0_NS + 8_000_000,
        T0_NS + stamp_ms * 1_000_000
    )
}

/// A protocol-dump text whose `TRACE_START` reading is monotonic `T0_NS` against realtime
/// 1_000_000_000 us (`[1000000.000]`), with a 400x300 buffer committed at +8 ms and an 800x600
/// one at `commit_ms` (both on the extension's clock, where `T0_NS` is +0).
fn wire_text(commit_ms: u64) -> String {
    let stamp = |ms: u64| {
        let us = 1_000_000_000 + ms * 1000;
        format!("[{}.{:03}]", us / 1000, us % 1000)
    };
    format!(
        "{T0_NS} FIXTURE TRACE_START realtime_ns=1000000000000\n\
         {} -> wl_shm_pool@10.create_buffer(new id wl_buffer@11, 0, 400, 300, 1600, 1)\n\
         {} -> wl_shm_pool@10.create_buffer(new id wl_buffer@12, 0, 800, 600, 3200, 1)\n\
         {} -> wl_surface@5.attach(wl_buffer@11, 0, 0)\n\
         {} -> wl_surface@5.commit()\n\
         {} -> wl_surface@5.attach(wl_buffer@12, 0, 0)\n\
         {} -> wl_surface@5.commit()\n",
        stamp(1),
        stamp(2),
        stamp(7),
        stamp(8),
        stamp(commit_ms - 1),
        stamp(commit_ms),
    )
}

/// The case TEST_PLAN 6.20 found under load: the 800x600 buffer was committed at +60 ms, ten
/// milliseconds before the +70 ms reveal, and the fixture stamped it at +95 ms. On the stamp the
/// run is a 25 ms flash; on the commit it is not. The verdict follows the commit, and both times
/// and the verdict the stamp alone would have given are reported.
#[test]
fn test_a_late_stamp_after_an_early_commit_is_not_a_flash_on_the_wire() {
    let info = evaluate(
        Some(&slice()),
        Some(77),
        &client_text(95),
        Some(&wire_text(60)),
        &brackets(),
        Default::default(),
    );
    assert!(!info.flash, "{:?}", info.flash_events);
    assert_eq!(info.flash_basis, Some(FlashBasis::Client));
    assert_eq!(info.paint_source, Some(PaintSource::Wire));
    assert_eq!(info.paint_size, Some((800, 600)));
    assert!(
        (info.paint_wire_ms.unwrap() + 10.0).abs() < 0.1,
        "{:?}",
        info.paint_wire_ms
    );
    assert!(
        (info.paint_stamp_ms.unwrap() - 25.0).abs() < 0.1,
        "{:?}",
        info.paint_stamp_ms
    );
    assert!((info.paint_stamp_lag_ms.unwrap() - 35.0).abs() < 0.1);
    assert_eq!(info.flash_by_stamp, Some(true));
}

/// A commit that really came after the reveal is a flash on the wire as well, and the event text
/// names the source. The stamp, written 3 ms after the commit, agrees.
#[test]
fn test_a_commit_after_the_reveal_is_a_flash_on_the_wire() {
    let info = evaluate(
        Some(&slice()),
        Some(77),
        &client_text(98),
        Some(&wire_text(95)),
        &brackets(),
        Default::default(),
    );
    assert!(info.flash, "{:?}", info.flash_events);
    assert_eq!(info.paint_source, Some(PaintSource::Wire));
    let e = &info.flash_events[0];
    assert_eq!(e.kind, FlashKind::SizeChangeAfterReveal);
    assert!((e.lead_ms - 25.0).abs() < 0.1, "{}", e.lead_ms);
    assert!(e.detail.contains("(wire commit)"), "{}", e.detail);
    assert!((info.paint_stamp_lag_ms.unwrap() - 3.0).abs() < 0.1);
    assert_eq!(info.flash_by_stamp, Some(true));
}

/// A stamp that is early but a commit that is late (the stamp can never be earlier than the
/// commit in the real fixture, but a different fixture could): the verdict is the wire's.
#[test]
fn test_the_wire_decides_even_when_the_stamp_disagrees() {
    let info = evaluate(
        Some(&slice()),
        Some(77),
        &client_text(40),
        Some(&wire_text(95)),
        &brackets(),
        Default::default(),
    );
    assert!(info.flash);
    assert_eq!(info.flash_by_stamp, Some(false));
}

/// Without a dump the fixture's stamp is the paint time and says so; nothing about the wire is
/// reported, and no `flash_by_stamp` is written because there is nothing to compare.
#[test]
fn test_without_a_dump_the_stamp_is_the_paint_time() {
    let info = evaluate(
        Some(&slice()),
        Some(77),
        &client_text(95),
        None,
        &brackets(),
        Default::default(),
    );
    assert!(info.flash);
    assert_eq!(info.paint_source, Some(PaintSource::Stamp));
    assert_eq!(info.paint_size, Some((800, 600)));
    assert!((info.paint_stamp_ms.unwrap() - 25.0).abs() < 0.1);
    assert_eq!(info.paint_wire_ms, None);
    assert_eq!(info.flash_by_stamp, None);
    assert!(info.flash_notes.is_empty(), "{:?}", info.flash_notes);
}

/// A dump was asked for but has no commit (the fixture crashed before drawing, or the file is
/// empty): the verdict falls back to the stamps and a note says so.
#[test]
fn test_an_empty_dump_falls_back_to_the_stamp_with_a_note() {
    let info = evaluate(
        Some(&slice()),
        Some(77),
        &client_text(95),
        Some(""),
        &brackets(),
        Default::default(),
    );
    assert!(info.flash);
    assert_eq!(info.paint_source, Some(PaintSource::Stamp));
    assert!(
        info.flash_notes
            .iter()
            .any(|n| n.contains("no wire paint times")),
        "{:?}",
        info.flash_notes
    );
}

/// The observe-only `X11_OBS_*` lines are in the slice and change no verdict: the flash result
/// with them in the slice equals the one without (same events, same flags, same notes).
#[test]
fn test_x11_observation_lines_in_the_slice_change_no_verdict() {
    let win = "win=[id=77 pid=4242 class=\"verify.x\" app=\"\" type=NORMAL proto=X11]";
    let extra = [
        format!("[spawn-at-time] +31.00ms | X11_OBS_START {win} kind=X11 wayland=false client_type=1 id=77 mono_ms=1031.000 damaged=available first_frame=available"),
        format!("[spawn-at-time] +45.00ms | X11_OBS_SIGNAL {win} signal=damaged id=77 mono_ms=1045.000 seq=1 texture=(800x600) actor=(800x600) frame=(0,0,800x600) buffer=(0,0,800x600)"),
        format!("[spawn-at-time] +70.50ms | X11_OBS_MARK {win} what=reveal id=77 mono_ms=1070.500"),
        format!("[spawn-at-time] +570.50ms | X11_OBS_END {win} id=77 reason=reveal+500ms mono_ms=1570.500 damaged=1 first_frame=0"),
    ]
    .join("\n");
    let plain = slice();
    let with_obs = format!("{plain}\n{extra}");
    for (stamp_ms, wire_ms) in [(95_u64, 60_u64), (40, 40)] {
        let a = evaluate(
            Some(&plain),
            Some(77),
            &client_text(stamp_ms),
            Some(&wire_text(wire_ms)),
            &brackets(),
            Default::default(),
        );
        let b = evaluate(
            Some(&with_obs),
            Some(77),
            &client_text(stamp_ms),
            Some(&wire_text(wire_ms)),
            &brackets(),
            Default::default(),
        );
        assert_eq!(a, b, "stamp {stamp_ms} wire {wire_ms}");
    }
    assert!(
        with_obs.contains("X11_OBS_SIGNAL"),
        "the raw lines stay in the slice"
    );
}

/// `--wire-paint` also hands the fixture the trace directory (the same private directory as the
/// events) and the dump is read by pid; without it the trace variable is absent and nothing is
/// read, even when a stray `.log` is in the directory.
#[test]
fn test_wire_capture_sets_the_trace_directory_and_reads_the_log_by_pid() {
    let ext = FakeExtension::new(true, &[]);
    ext.set_brackets(brackets());
    let env_of = |cap: &FlashCapture, key: &str| {
        let mut cmd = Command::new("/bin/true");
        cap.apply(&mut cmd);
        cmd.get_envs()
            .find(|(k, _)| *k == key)
            .and_then(|(_, v)| v.map(std::path::PathBuf::from))
    };

    let plain = FlashCapture::start(&ext, false).unwrap();
    assert!(env_of(&plain, EVENT_DIR_ENV).is_some());
    assert_eq!(env_of(&plain, TRACE_DIR_ENV), None);

    let wire = FlashCapture::start(&ext, true).unwrap();
    let dir = env_of(&wire, TRACE_DIR_ENV).expect("trace directory is passed");
    assert_eq!(Some(dir.clone()), env_of(&wire, EVENT_DIR_ENV));
    assert_eq!(wire.read_wire(Some(4242)), "", "no file yet");
    std::fs::write(dir.join("fixture-4242.log"), "dump\n").unwrap();
    assert_eq!(wire.read_wire(Some(4242)), "dump\n");
    assert_eq!(
        wire.read_wire(Some(1)),
        "",
        "another pid's file is not guessed"
    );
    assert_eq!(wire.read_wire(None), "");

    std::fs::write(
        env_of(&plain, EVENT_DIR_ENV)
            .unwrap()
            .join("fixture-4242.log"),
        "dump\n",
    )
    .unwrap();
    assert_eq!(plain.read_wire(Some(4242)), "", "not asked for: not read");
}
