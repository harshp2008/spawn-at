//! Verdict feedback and focus return, headless: a verdict key shows in the next frame while the
//! window is still being closed, teardown trouble is a warning and never a verdict, Esc records
//! nothing, `r` starts over, the selected row's marker stays visible in every state, and the
//! terminal gets focus back after the automatic part of a Both test.
//!
//! `spawn-at` and the fixture are shell scripts in a temporary directory. The stand-in lists a
//! "terminal" window (id 7) that starts focused; launching a fixture moves focus to it, `focus
//! --id N` moves it to N (unless a `nofocus` file exists), and every command is appended to
//! `cmds` so a test can see exactly what was sent to which window.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{backend::TestBackend, style::Color, Terminal};
use spawn_at_verify::preflight::{PreflightReport, PreflightVerdict};
use spawn_at_verify::probe::EnvironmentInfo;
use spawn_at_verify::runner::owned::{guard_user_window, FOCUS_NOT_RETURNED};
use spawn_at_verify::runner::{
    AutoStatus, FinalStatus, HumanVerdict, HumanVerdictKind, RunOptions, TestResult,
};
use spawn_at_verify::tui::app::{Pending, TuiApp};
use spawn_at_verify::tui::views::panels::result_box::get_result_content;
use spawn_at_verify::tui::views::panels::tests_list::{marker_color, SELECTED_ROW_BG};
use spawn_at_verify::tui::views::theme::status_symbol_and_color;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

const FIXTURE: &str = r#"#!/bin/sh
D="$(dirname "$0")"
title=""
while [ $# -gt 0 ]; do
  case "$1" in --title) title="$2"; shift ;; esac
  shift
done
printf '%s %s\n' "$$" "$title" >> "$D/state"
n=$(wc -l < "$D/state")
echo $((999+n)) > "$D/focus_id"
exec sleep 60
"#;

const SPAWN_AT: &str = r#"#!/bin/sh
D="$(dirname "$0")"
echo "$*" >> "$D/cmds"
case "$1" in
  --version) printf 'spawn-at test double\nPrevious Commit ID: abc1234\n' ;;
  query)
    case "$2" in
      windows)
        F=$(cat "$D/focus_id")
        { echo "7 term"; [ -f "$D/state" ] && cat "$D/state"; } | awk -v f="$F" 'BEGIN{printf "["} {sep=(NR>1)?",":""; id=(NR==1)?7:999+NR-1; printf "%s{\"id\":%d,\"pid\":%d,\"title\":\"%s\",\"x\":0,\"y\":0,\"w\":400,\"h\":300,\"focused\":%s,\"maximized\":false,\"minimized\":false}", sep, id, $1, $2, (id==f)?"true":"false"} END{print "]"}' ;;
      layout) echo '{"schema_version":1,"monitors":[{"index":0,"primary":true,"workarea":{"x":0,"y":0,"w":1920,"h":1080}}]}' ;;
    esac ;;
  focus) [ -f "$D/nofocus" ] || echo "$3" > "$D/focus_id" ;;
  close) [ -f "$D/noclose" ] || true ;;
esac
"#;

fn spec(id: &str, kind: &str) -> String {
    format!(
        r#"
schema_version = 2
id = "{id}"
category = "transforms"
title = "{id}"
description = "focus a fixture window"
kind = "{kind}"
tags = ["moves-focus"]

[[setup]]
name = "w"
args = ["--width", "400"]

[action]
command = "focus"
args = ["--id", "${{w}}"]

[[check]]
after = 0
window = "w"
state = "present"

[human]
watch = "Did the window come to the front?"
"#
    )
}

struct Rig {
    root: PathBuf,
}

impl Rig {
    fn new(tag: &str) -> Rig {
        let root = std::env::temp_dir().join(format!(
            "spawn-at-verify-verdict-{}-{}",
            std::process::id(),
            tag
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("bin")).unwrap();
        std::fs::create_dir_all(root.join("verify/transforms")).unwrap();
        write_exec(&root.join("bin/spawn-at"), SPAWN_AT);
        write_exec(&root.join("bin/fixture"), FIXTURE);
        std::fs::write(root.join("bin/focus_id"), "7\n").unwrap();
        std::fs::write(
            root.join("verify/transforms/auto.toml"),
            spec("transforms/auto", "auto"),
        )
        .unwrap();
        std::fs::write(
            root.join("verify/transforms/both.toml"),
            spec("transforms/both", "both"),
        )
        .unwrap();
        Rig { root }
    }

    fn app(&self) -> TuiApp {
        let options = RunOptions {
            bin_path: self.root.join("bin/spawn-at"),
            ..Default::default()
        };
        let mut app = TuiApp::new(self.root.join("verify"), options).expect("TuiApp::new");
        app.preflight = Some(PreflightReport {
            detected_commit: None,
            binary_version_output: String::new(),
            repo_head: None,
            repo_dirty: false,
            is_dirty_build: false,
            verdict: PreflightVerdict::Pass,
        });
        app.env_info = Some(EnvironmentInfo {
            session_type: "wayland".into(),
            desktop: "gnome".into(),
            gnome_version: None,
            monitors_count: 1,
            primary_workarea: None,
            independent_workarea: None,
        });
        app
    }

    fn cmds(&self) -> Vec<String> {
        std::fs::read_to_string(self.root.join("bin/cmds"))
            .unwrap_or_default()
            .lines()
            .map(str::to_string)
            .collect()
    }

    fn focused(&self) -> String {
        std::fs::read_to_string(self.root.join("bin/focus_id"))
            .unwrap()
            .trim()
            .to_string()
    }

    fn fixture_pid(&self) -> i32 {
        std::fs::read_to_string(self.root.join("bin/state"))
            .unwrap()
            .lines()
            .next()
            .unwrap()
            .split(' ')
            .next()
            .unwrap()
            .parse()
            .unwrap()
    }
}

impl Drop for Rig {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn write_exec(path: &Path, text: &str) {
    std::fs::write(path, text).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

fn alive(pid: i32) -> bool {
    // SAFETY: signal 0 only checks that the pid exists.
    unsafe { libc::kill(pid, 0) == 0 }
}

fn select(app: &mut TuiApp, id: &str) {
    app.selected_index = app
        .filtered_tests()
        .iter()
        .position(|t| t.id == id)
        .expect("test in list");
}

fn key(app: &mut TuiApp, c: char) {
    app.handle_key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE));
}

/// Draws the whole UI, as the event loop does before running a pending step.
fn frame(app: &TuiApp) -> Terminal<TestBackend> {
    let mut t = Terminal::new(TestBackend::new(120, 40)).unwrap();
    t.draw(|f| spawn_at_verify::tui::views::draw_ui(f, app))
        .unwrap();
    t
}

fn screen(t: &Terminal<TestBackend>) -> String {
    let buf = t.backend().buffer();
    let mut out = String::new();
    for y in 0..40 {
        for x in 0..120 {
            out.push_str(buf[(x, y)].symbol());
        }
        out.push('\n');
    }
    out
}

/// A Both test run to the point where a person is asked for the verdict.
fn waiting(rig: &Rig) -> TuiApp {
    let mut app = rig.app();
    select(&mut app, "transforms/both");
    key(&mut app, 'r');
    app.run_all_pending();
    assert_eq!(app.waiting_for_human.as_deref(), Some("transforms/both"));
    assert!(alive(rig.fixture_pid()));
    app
}

#[test]
fn test_verdict_key_shows_marker_and_result_in_the_next_frame_before_the_window_closes() {
    let rig = Rig::new("instant");
    let mut app = waiting(&rig);
    let pid = rig.fixture_pid();

    key(&mut app, 'p');

    // The teardown has not run: it is pending, the window is still there ...
    assert!(app.has_pending(), "teardown must be pending, not done");
    assert!(alive(pid), "the window is still open in this frame");
    // ... and the very next frame already shows the verdict and "closing window...".
    let res = &app.results["transforms/both"];
    assert_eq!(res.human_verdict.verdict, HumanVerdictKind::Pass);
    assert_eq!(res.final_status, FinalStatus::Pass);
    let (sym, color) = status_symbol_and_color(app.selected_test().unwrap(), Some(res), false);
    assert_eq!((sym, color), ("✔", Color::Green));
    let (plain, _, _) = get_result_content(&app);
    assert!(plain.contains("PASS"), "{plain}");
    assert!(plain.contains("closing window..."), "{plain}");
    let shown = screen(&frame(&app));
    assert!(shown.contains("closing window..."), "{shown}");

    // The teardown then finishes without touching the verdict.
    app.run_all_pending();
    assert!(!alive(pid));
    assert!(app.tracker.owned.is_empty());
    assert_eq!(
        app.results["transforms/both"].final_status,
        FinalStatus::Pass
    );
    let (plain, _, _) = get_result_content(&app);
    assert!(!plain.contains("closing window..."), "{plain}");
}

#[test]
fn test_each_verdict_key_updates_the_row_at_once() {
    for (c, want) in [
        ('p', HumanVerdictKind::Pass),
        ('x', HumanVerdictKind::Fail),
        ('s', HumanVerdictKind::Skip),
        ('k', HumanVerdictKind::Flaky),
    ] {
        let rig = Rig::new(&format!("keys-{c}"));
        let mut app = waiting(&rig);
        key(&mut app, c);
        assert_eq!(app.results["transforms/both"].human_verdict.verdict, want);
        assert!(app.waiting_for_human.is_none(), "key {c}");
        app.run_all_pending();
    }
}

#[test]
fn test_a_teardown_problem_is_a_warning_and_the_verdict_stays() {
    let rig = Rig::new("leftover");
    std::fs::write(rig.root.join("bin/noclose"), "").unwrap();
    let mut app = waiting(&rig);

    key(&mut app, 'p');
    app.run_all_pending();

    // The stand-in's windows never go away from its list, so the close cannot be confirmed.
    let notes = app.tracker.notes.get("transforms/both");
    assert!(
        !notes.is_empty(),
        "a window that stays listed after close must be reported"
    );
    let res = &app.results["transforms/both"];
    assert_eq!(res.human_verdict.verdict, HumanVerdictKind::Pass);
    assert_eq!(res.final_status, FinalStatus::Pass);
    let (plain, _, _) = get_result_content(&app);
    assert!(plain.starts_with("✔ PASS"), "verdict first: {plain}");
    assert!(plain.contains("⚠"), "{plain}");
}

#[test]
fn test_esc_records_no_verdict_and_closes_the_window_after_the_redraw() {
    let rig = Rig::new("esc");
    let mut app = waiting(&rig);
    let pid = rig.fixture_pid();

    app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert!(app.waiting_for_human.is_none());
    assert_eq!(
        app.results["transforms/both"].human_verdict.verdict,
        HumanVerdictKind::Unset,
        "Esc must not record a verdict"
    );
    assert!(alive(pid), "closing is deferred until after the redraw");
    app.run_all_pending();
    assert!(!alive(pid));
    assert_eq!(
        app.results["transforms/both"].human_verdict.verdict,
        HumanVerdictKind::Unset
    );
}

#[test]
fn test_r_resets_the_marker_at_once_and_runs_the_test_again() {
    let rig = Rig::new("rerun");
    let mut app = waiting(&rig);
    let first_pid = rig.fixture_pid();

    key(&mut app, 'r');
    // Next frame: running, no old verdict, old window not yet closed.
    let res = &app.results["transforms/both"];
    assert_eq!(res.auto_status, AutoStatus::Running);
    assert_eq!(res.human_verdict.verdict, HumanVerdictKind::Unset);
    let (sym, _) = status_symbol_and_color(app.selected_test().unwrap(), Some(res), false);
    assert_eq!(sym, "◐");
    assert!(alive(first_pid));

    app.run_all_pending();
    assert!(!alive(first_pid), "the first window is closed");
    assert_eq!(app.waiting_for_human.as_deref(), Some("transforms/both"));
    assert_eq!(
        app.results["transforms/both"].auto_status,
        AutoStatus::AutoPass
    );
}

#[test]
fn test_one_final_verdict_per_run() {
    let rig = Rig::new("once");
    let mut app = waiting(&rig);
    key(&mut app, 'p');
    // A second key before the teardown must not record again: nothing is waiting any more.
    key(&mut app, 'x');
    app.run_all_pending();
    // 'x' with nothing waiting targets the selected test; the first verdict was Pass and the
    // list holds a single result for the test.
    assert_eq!(
        app.results
            .keys()
            .filter(|k| k.as_str() == "transforms/both")
            .count(),
        1
    );
}

#[test]
fn test_a_run_shows_running_in_the_next_frame_and_runs_after_the_redraw() {
    let rig = Rig::new("running");
    let mut app = rig.app();
    select(&mut app, "transforms/auto");

    key(&mut app, 'r');

    assert!(app.has_pending());
    assert!(!rig.root.join("bin/state").exists(), "nothing ran yet");
    let res = &app.results["transforms/auto"];
    assert_eq!(res.auto_status, AutoStatus::Running);
    let (plain, _, _) = get_result_content(&app);
    assert!(plain.contains("Running"), "{plain}");
    assert!(screen(&frame(&app)).contains("Running"));

    app.run_all_pending();
    assert_eq!(
        app.results["transforms/auto"].auto_status,
        AutoStatus::AutoPass
    );
    assert!(app.pending.is_empty());
}

#[test]
fn test_an_idle_app_has_nothing_pending() {
    let rig = Rig::new("idle");
    let app = rig.app();
    assert!(!app.has_pending(), "an idle loop must not spin");
    assert!(app.pending.is_empty());
    let _ = Pending::RunQueueStep;
}

#[test]
fn test_focus_goes_back_to_the_terminal_before_the_verdict_prompt() {
    let rig = Rig::new("refocus");
    let app = waiting(&rig);
    assert_eq!(rig.focused(), "7", "the terminal has focus again");
    assert!(alive(rig.fixture_pid()), "the fixture window stays open");
    assert!(
        app.tracker.notes.get("transforms/both").is_empty(),
        "no warning when focus came back"
    );
    // Only `focus` was ever sent for the terminal's id.
    for c in rig.cmds() {
        if c.contains("--id 7") || c.ends_with(" 7") || c.contains(" 7 ") {
            assert!(
                c.starts_with("focus") || c.starts_with("query"),
                "non-focus command sent to the user's window: {c}"
            );
        }
    }
    assert!(rig
        .cmds()
        .iter()
        .any(|c| c.starts_with("focus") && c.contains('7')));
}

#[test]
fn test_a_failed_refocus_is_a_warning_not_a_verdict() {
    let rig = Rig::new("nofocus");
    std::fs::write(rig.root.join("bin/nofocus"), "").unwrap();
    let mut app = rig.app();
    select(&mut app, "transforms/both");
    key(&mut app, 'r');
    app.run_all_pending();

    assert_eq!(
        app.tracker.notes.get("transforms/both"),
        vec![FOCUS_NOT_RETURNED.to_string()]
    );
    let (plain, _, _) = get_result_content(&app);
    assert!(
        plain.contains("could not return focus to your terminal"),
        "{plain}"
    );
    // The automatic result is untouched by the warning.
    assert_eq!(
        app.results["transforms/both"].auto_status,
        AutoStatus::AutoPass
    );
    key(&mut app, 'x');
    app.run_all_pending();
}

#[test]
fn test_no_focus_is_sent_when_no_window_was_focused_at_the_start() {
    let rig = Rig::new("unknown");
    // Nobody is focused at the start.
    std::fs::write(rig.root.join("bin/focus_id"), "0\n").unwrap();
    let mut app = rig.app();
    select(&mut app, "transforms/both");
    key(&mut app, 'r');
    app.run_all_pending();
    let focus_cmds: Vec<String> = rig
        .cmds()
        .into_iter()
        .filter(|c| c.starts_with("focus"))
        .collect();
    // Only the spec's own `focus --id ${w}` (the fixture window), never one for window 7.
    assert!(
        focus_cmds.iter().all(|c| !c.contains(" 7")),
        "{focus_cmds:?}"
    );
    assert!(app.tracker.notes.get("transforms/both").is_empty());
}

#[test]
fn test_auto_tests_are_unchanged_by_the_focus_return() {
    let rig = Rig::new("auto");
    let mut app = rig.app();
    select(&mut app, "transforms/auto");
    key(&mut app, 'r');
    app.run_all_pending();
    assert!(app.waiting_for_human.is_none());
    assert!(app.tracker.owned.is_empty());
    assert_eq!(
        app.results["transforms/auto"].auto_status,
        AutoStatus::AutoPass
    );
}

#[test]
fn test_guard_rejects_any_command_but_focus_on_the_users_window() {
    let a = |s: &[&str]| s.iter().map(|x| x.to_string()).collect::<Vec<_>>();
    for cmd in [
        "close",
        "move",
        "resize",
        "minimize",
        "maximize",
        "restore",
        "reposition",
    ] {
        let err = guard_user_window(Some(7), &a(&[cmd, "--id", "7"]));
        assert!(err.is_err(), "{cmd} on the user's window must be refused");
        let err = guard_user_window(Some(7), &a(&[cmd, "--id=7"]));
        assert!(err.is_err(), "{cmd} --id=7 must be refused");
    }
    assert!(guard_user_window(Some(7), &a(&["focus", "--id", "7"])).is_ok());
    assert!(guard_user_window(Some(7), &a(&["close", "--id", "1000"])).is_ok());
    assert!(guard_user_window(None, &a(&["close", "--id", "7"])).is_ok());
}

#[test]
fn test_marker_stays_visible_on_the_selected_row_in_every_state() {
    let rig = Rig::new("marker");
    let mut app = rig.app();
    select(&mut app, "transforms/both");
    let test = app.selected_test().unwrap().clone();
    let base: TestResult = {
        key(&mut app, 'r');
        app.results["transforms/both"].clone() // the "running" placeholder
    };
    app.pending.clear();
    app.queue.clear();

    let with = |f: &dyn Fn(&mut TestResult)| {
        let mut r = base.clone();
        f(&mut r);
        r
    };
    let states: Vec<(&str, Option<TestResult>, bool)> = vec![
        ("not run", None, false),
        ("running", Some(base.clone()), false),
        ("waiting", Some(base.clone()), true),
        (
            "pass",
            Some(with(&|r| {
                r.auto_status = AutoStatus::AutoPass;
                r.final_status = FinalStatus::Pass;
                r.human_verdict = HumanVerdict {
                    verdict: HumanVerdictKind::Pass,
                    note: None,
                };
            })),
            false,
        ),
        (
            "fail",
            Some(with(&|r| {
                r.final_status = FinalStatus::Fail;
                r.human_verdict.verdict = HumanVerdictKind::Fail;
            })),
            false,
        ),
        (
            "skipped",
            Some(with(&|r| {
                r.auto_status = AutoStatus::Skipped;
                r.final_status = FinalStatus::Skipped;
            })),
            false,
        ),
        (
            "flaky",
            Some(with(&|r| {
                r.auto_status = AutoStatus::AutoPass;
                r.final_status = FinalStatus::Flaky;
                r.human_verdict.verdict = HumanVerdictKind::Flaky;
            })),
            false,
        ),
        (
            "blocked",
            Some(with(&|r| {
                r.auto_status = AutoStatus::Blocked;
            })),
            false,
        ),
        (
            "error",
            Some(with(&|r| {
                r.auto_status = AutoStatus::Error;
                r.final_status = FinalStatus::Error;
            })),
            false,
        ),
    ];

    for (name, res, is_waiting) in states {
        let (sym, color) = status_symbol_and_color(&test, res.as_ref(), is_waiting);
        // The colour the selected row really draws the marker in.
        let drawn = marker_color(color, true);
        assert_ne!(
            drawn, SELECTED_ROW_BG,
            "{name}: marker invisible on the selected row"
        );

        // And in a rendered frame: find the marker cell on the selected row ("> <marker>").
        match &res {
            Some(r) => {
                app.results.insert(test.id.clone(), r.clone());
            }
            None => {
                app.results.remove(&test.id);
            }
        }
        app.waiting_for_human = if is_waiting {
            Some(test.id.clone())
        } else {
            None
        };
        let t = frame(&app);
        let buf = t.backend().buffer();
        let mut found = false;
        for y in 0..40u16 {
            for x in 0..118u16 {
                if buf[(x, y)].symbol() == ">" && buf[(x + 1, y)].symbol() == " " {
                    let cell = &buf[(x + 2, y)];
                    if cell.symbol() == sym {
                        found = true;
                        assert_eq!(cell.bg, SELECTED_ROW_BG, "{name}: selected row background");
                        assert_ne!(cell.fg, cell.bg, "{name}: marker fg equals row bg");
                        assert_eq!(cell.fg, drawn, "{name}");
                    }
                }
            }
        }
        assert!(
            found,
            "{name}: marker {sym:?} not found on the selected row"
        );
    }
    // The old behaviour, for the record: the dot was DarkGray on DarkGray.
    assert_eq!(marker_color(Color::DarkGray, false), Color::DarkGray);
    assert_eq!(marker_color(Color::DarkGray, true), Color::White);
}

/// The focus return is recorded and shown whatever happens: after `r` the Result panel names the
/// terminal that was recorded at start, says `focus` was sent to it once (exit 0) and which
/// window had focus afterwards, and the verdict teardown keeps a report that still matches.
#[test]
fn test_result_panel_shows_the_focus_return_chain() {
    let rig = Rig::new("chain");
    let mut app = rig.app();
    assert_eq!(app.terminal_window_id, Some(7), "recorded at start");
    select(&mut app, "transforms/both");
    key(&mut app, 'r');
    app.run_all_pending();
    let r = &app.focus_reports["transforms/both"];
    assert!(r.matches, "{:?}", r.lines());
    let (plain, _, _) = get_result_content(&app);
    assert!(
        plain.contains("focus terminal: id 7 class '' title 'term'"),
        "{plain}"
    );
    assert!(
        plain.contains("focus focus command: sent 1x: #1 exit 0"),
        "{plain}"
    );
    assert!(plain.contains("matches terminal: yes"), "{plain}");
    key(&mut app, 'p');
    app.run_all_pending();
    assert!(app.focus_reports["transforms/both"].matches);
    assert_eq!(rig.focused(), "7");
}

/// A focus that never lands: the report says three sends, what had focus instead, and NO.
#[test]
fn test_result_panel_shows_a_failed_focus_return_exactly() {
    let rig = Rig::new("chain-fail");
    std::fs::write(rig.root.join("bin/nofocus"), "").unwrap();
    let mut app = rig.app();
    app.focus_timings = spawn_at_verify::runner::focus_return::FocusTimings {
        settle: std::time::Duration::from_millis(5),
        attempts: 3,
        wait: std::time::Duration::from_millis(40),
        poll: std::time::Duration::from_millis(10),
    };
    select(&mut app, "transforms/both");
    key(&mut app, 'r');
    app.run_all_pending();
    let lines = app.focus_reports["transforms/both"].lines();
    assert!(
        lines[1].starts_with("focus command: sent 3x: #1 exit 0"),
        "{lines:?}"
    );
    assert!(lines[2].contains("matches terminal: NO"), "{lines:?}");
    assert!(
        lines[2].contains("id 1000"),
        "the fixture still had focus: {lines:?}"
    );
    let (plain, _, _) = get_result_content(&app);
    assert!(plain.contains("matches terminal: NO"), "{plain}");
}

/// No window focused at start: the panel says none was found and why, and nothing is sent.
#[test]
fn test_result_panel_says_when_no_terminal_was_found() {
    let rig = Rig::new("chain-none");
    std::fs::write(rig.root.join("bin/focus_id"), "0\n").unwrap();
    let mut app = rig.app();
    assert_eq!(app.terminal_window_id, None);
    select(&mut app, "transforms/both");
    key(&mut app, 'r');
    app.run_all_pending();
    let (plain, _, _) = get_result_content(&app);
    assert!(
        plain.contains("terminal: none found (no window reported focused (1 listed))"),
        "{plain}"
    );
    assert!(
        plain.contains("not sent (no terminal window recorded)"),
        "{plain}"
    );
}
