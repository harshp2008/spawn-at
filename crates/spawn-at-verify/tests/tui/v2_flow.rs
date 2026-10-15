//! Version 2 specs in the TUI (`docs/DESIGN_RUNNER_SETUP.md` sections 3.4 and 6), headless: the
//! drawer shows the setup, the extra actions and the checks; an Auto test closes its window at
//! once; a Both test keeps its fixture window until the verdict, and Ctrl-C closes it too.
//!
//! `spawn-at` and the fixture are shell scripts in a temporary directory, so no window, display
//! or compositor is involved.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use spawn_at_verify::preflight::{PreflightReport, PreflightVerdict};
use spawn_at_verify::probe::EnvironmentInfo;
use spawn_at_verify::runner::{AutoStatus, FinalStatus, HumanVerdictKind, RunOptions};
use spawn_at_verify::tui::app::TuiApp;
use spawn_at_verify::tui::views::build_drawer_content;
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
exec sleep 60
"#;

const SPAWN_AT: &str = r#"#!/bin/sh
D="$(dirname "$0")"
case "$1" in
  --version) printf 'spawn-at test double\nPrevious Commit ID: abc1234\n' ;;
  query)
    case "$2" in
      windows)
        [ -f "$D/state" ] || { echo '[]'; exit 0; }
        MX=false; [ -f "$D/maxed" ] && MX=true
        MN=false; [ -f "$D/mined" ] && MN=true
        awk -v mx=$MX -v mn=$MN 'BEGIN{printf "["} {sep=(NR>1)?",":""; printf "%s{\"id\":%d,\"pid\":%d,\"title\":\"%s\",\"x\":0,\"y\":0,\"w\":400,\"h\":300,\"focused\":false,\"maximized\":%s,\"minimized\":%s}", sep, 999+NR, $1, $2, mx, mn} END{print "]"}' "$D/state" ;;
      layout) echo '{"schema_version":1,"monitors":[{"index":0,"primary":true,"workarea":{"x":0,"y":0,"w":1920,"h":1080}}]}' ;;
    esac ;;
  close) echo "close $3" >> "$D/events" ;;
  focus) echo "focus" >> "$D/events" ;;
  restore) echo "restore" >> "$D/events"; rm -f "$D/maxed" "$D/mined" ;;
  maximize) echo "maximize" >> "$D/events"; : > "$D/maxed" ;;
  minimize) echo "minimize" >> "$D/events"; : > "$D/mined" ;;
esac
"#;

fn spec(id: &str, kind: &str) -> String {
    format!(
        r#"
schema_version = 2
id = "{id}"
category = "transforms"
title = "{id}"
description = "focus a fixture window, then restore it"
kind = "{kind}"
tags = ["moves-focus"]

[[setup]]
name = "w"
args = ["--width", "400"]

[action]
command = "focus"
args = ["--id", "${{w}}"]

[[then]]
command = "restore"
args = ["--id", "${{w}}", "--no-focus"]

[[check]]
after = 0
window = "w"
state = "present"

[human]
watch = "Did the window come to the front and go back without a jump?"
"#
    )
}

struct Rig {
    root: PathBuf,
}

impl Rig {
    fn new(tag: &str) -> Rig {
        let root = std::env::temp_dir().join(format!(
            "spawn-at-verify-tuiv2-{}-{}",
            std::process::id(),
            tag
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("bin")).unwrap();
        std::fs::create_dir_all(root.join("verify/transforms")).unwrap();
        write_exec(&root.join("bin/spawn-at"), SPAWN_AT);
        write_exec(&root.join("bin/fixture"), FIXTURE);
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
        self.app_for(self.root.join("verify"))
    }

    /// The TUI over the repository's own catalog, with the stand-in binary instead of `spawn-at`.
    fn catalog_app(&self) -> TuiApp {
        self.app_for(spawn_at_verify::tui::find_verify_dir().expect("verify/ directory"))
    }

    fn app_for(&self, verify: PathBuf) -> TuiApp {
        let options = RunOptions {
            bin_path: self.root.join("bin/spawn-at"),
            ..Default::default()
        };
        let mut app = TuiApp::new(verify, options).expect("TuiApp::new");
        // The stand-in binary is not a build of this repository: judge the run as if preflight passed.
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

    fn lines(&self, file: &str) -> Vec<String> {
        std::fs::read_to_string(self.root.join("bin").join(file))
            .unwrap_or_default()
            .lines()
            .map(str::to_string)
            .collect()
    }

    fn fixture_pid(&self) -> i32 {
        self.lines("state")[0]
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
        .unwrap_or_else(|| panic!("{id} is not in the list"));
}

#[test]
fn test_drawer_shows_setup_then_and_checks() {
    let rig = Rig::new("drawer");
    let app = rig.app();
    let test = app.registry.get_test("transforms/both").unwrap().clone();
    let (_, plain) = build_drawer_content(&app, &test, 100, false);
    assert!(plain.contains("Setup w"), "{plain}");
    assert!(plain.contains("fixture window --width 400"), "{plain}");
    assert!(plain.contains("Then 1"), "{plain}");
    assert!(plain.contains("restore --id ${w} --no-focus"), "{plain}");
    assert!(plain.contains("Check w"), "{plain}");
    assert!(plain.contains("present (after action 0)"), "{plain}");
}

#[test]
fn test_an_auto_test_closes_its_window_at_once() {
    let rig = Rig::new("auto");
    let mut app = rig.app();
    select(&mut app, "transforms/auto");
    app.run_selected();
    let r = &app.results["transforms/auto"];
    assert_eq!(r.auto_status, AutoStatus::AutoPass, "{:?}", r.message);
    assert_eq!(rig.lines("events"), vec!["focus", "restore", "close 1000"]);
    assert!(!alive(rig.fixture_pid()));
    assert!(app.tracker.owned.is_empty());
}

#[test]
fn test_a_both_test_keeps_its_window_until_the_verdict() {
    let rig = Rig::new("both");
    let mut app = rig.app();
    select(&mut app, "transforms/both");
    app.run_selected();
    assert_eq!(app.waiting_for_human.as_deref(), Some("transforms/both"));
    assert_eq!(
        app.results["transforms/both"].auto_status,
        AutoStatus::AutoPass
    );
    assert_eq!(
        rig.lines("events"),
        vec!["focus", "restore"],
        "no close yet"
    );
    assert_eq!(app.tracker.owned.len(), 1);
    assert!(alive(rig.fixture_pid()), "the reviewer needs the window");

    app.record_verdict(HumanVerdictKind::Pass);
    assert_eq!(rig.lines("events"), vec!["focus", "restore", "close 1000"]);
    assert!(!alive(rig.fixture_pid()));
    assert!(app.tracker.owned.is_empty());
    assert_eq!(
        app.results["transforms/both"].final_status,
        FinalStatus::Pass
    );
}

#[test]
fn test_ctrl_c_while_a_both_test_waits_closes_its_window() {
    let rig = Rig::new("ctrlc");
    let mut app = rig.app();
    select(&mut app, "transforms/both");
    app.run_selected();
    assert!(alive(rig.fixture_pid()));

    app.handle_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
    assert!(app.should_quit);
    assert_eq!(
        rig.lines("events").last().map(String::as_str),
        Some("close 1000")
    );
    assert!(!alive(rig.fixture_pid()));
    assert!(app.tracker.owned.is_empty());
}

// ---- the version 2 catalog specs in the TUI (docs/DESIGN_RUNNER_SETUP.md section 7) ----

/// The ids the catalog adds for existing windows, as the TUI lists them.
const CATALOG_V2_IDS: &[&str] = &[
    "focus/activate-fixture",
    "focus/defocus-active-window",
    "focus/switch-between-windows",
    "queries/query-layout-json",
    "queries/query-pointer",
    "queries/query-windows-lists-fixture",
    "robustness/close-by-id",
    "robustness/nonexistent-window-id",
    "robustness/sway-defocus-unsupported",
    "robustness/sway-maximize-unsupported",
    "robustness/sway-query-pointer-unsupported",
    "transforms/maximize-and-restore",
    "transforms/minimize-and-restore",
    "transforms/move-flash",
    "transforms/reposition-by-id",
    "transforms/resize-by-id",
    "transforms/resize-flash",
];

#[test]
fn test_catalog_specs_are_listed_grouped_and_marked_by_kind() {
    let rig = Rig::new("catalog-list");
    let app = rig.catalog_app();
    let listed: Vec<String> = app.filtered_tests().iter().map(|t| t.id.clone()).collect();
    for id in CATALOG_V2_IDS {
        assert!(listed.contains(&id.to_string()), "{id} is not listed");
    }
    assert_eq!(
        listed
            .iter()
            .filter(|i| i.starts_with("transforms/anchor-corners/"))
            .count(),
        5,
        "the anchor matrix lists five rows"
    );
    // Groups: the TUI's category names for the four new directories.
    use spawn_at_verify::tui::views::theme::get_test_category;
    for (id, group) in [
        ("transforms/resize-by-id", "Transforms"),
        ("focus/activate-fixture", "Focus"),
        ("queries/query-layout-json", "Queries"),
        ("robustness/close-by-id", "Robustness"),
    ] {
        // The list passes the spec's category, not its id.
        let category = app.registry.get_test(id).unwrap().category().to_string();
        assert_eq!(get_test_category(&category), group, "{id}");
    }
    // Auto vs Both, and the tag that keeps focus-moving specs out of quick.
    let kind = |id: &str| app.registry.get_test(id).unwrap().kind;
    use spawn_at_verify::schema::TestKind;
    for id in [
        "transforms/resize-flash",
        "transforms/move-flash",
        "transforms/maximize-and-restore",
        "transforms/minimize-and-restore",
    ] {
        assert_eq!(kind(id), TestKind::Both, "{id}");
    }
    assert_eq!(kind("transforms/resize-by-id"), TestKind::Auto);
    for id in CATALOG_V2_IDS {
        let t = app.registry.get_test(id).unwrap();
        assert!(!t.required, "{id} must not be required");
        if !t.v2.setup.is_empty() {
            assert!(t.tags.iter().any(|x| x == "moves-focus"), "{id}");
        }
    }
}

#[test]
fn test_a_catalog_spec_shows_setup_action_and_check_lines() {
    let rig = Rig::new("catalog-drawer");
    let app = rig.catalog_app();
    let test = app
        .registry
        .get_test("transforms/maximize-and-restore")
        .unwrap()
        .clone();
    let (_, plain) = build_drawer_content(&app, &test, 110, false);
    for want in [
        "Setup w",
        "fixture window --width 400 --height 300",
        "maximize",
        "Then 1",
        "restore --id ${w} --no-focus",
        "Check w",
        "maximized (after action 0)",
        "normal (after action 1)",
        "same rectangle as before (after action 1)",
        "Did the window maximize",
    ] {
        assert!(plain.contains(want), "drawer lacks '{want}':\n{plain}");
    }
}

#[test]
fn test_a_catalog_both_spec_waits_for_the_verdict_with_its_window_open() {
    let rig = Rig::new("catalog-both");
    let mut app = rig.catalog_app();
    select(&mut app, "transforms/maximize-and-restore");
    app.run_selected();
    let id = "transforms/maximize-and-restore";
    assert_eq!(app.waiting_for_human.as_deref(), Some(id));
    let r = &app.results[id];
    assert_eq!(r.auto_status, AutoStatus::AutoPass, "{:?}", r.message);
    assert_eq!(r.final_status, FinalStatus::Pending);
    assert_eq!(
        rig.lines("events"),
        vec!["maximize", "restore"],
        "no close yet"
    );
    assert!(alive(rig.fixture_pid()), "the reviewer needs the window");

    app.record_verdict(HumanVerdictKind::Pass);
    assert_eq!(
        rig.lines("events").last().map(String::as_str),
        Some("close 1000")
    );
    assert!(!alive(rig.fixture_pid()));
    assert_eq!(app.results[id].final_status, FinalStatus::Pass);
    assert!(app.waiting_for_human.is_none());
}

#[test]
fn test_a_spec_for_another_backend_is_shown_as_skipped_and_starts_nothing() {
    let rig = Rig::new("catalog-skip");
    let mut app = rig.catalog_app();
    // The TUI environment here is GNOME; this spec is for Sway only.
    let id = "robustness/sway-defocus-unsupported";
    select(&mut app, id);
    app.run_selected();
    let r = &app.results[id];
    assert_eq!(r.auto_status, AutoStatus::Skipped);
    assert_eq!(r.final_status, FinalStatus::Skipped);
    assert_eq!(
        r.message.as_deref(),
        Some("unsupported on this backend (gnome): spec requires sway")
    );
    assert!(rig.lines("state").is_empty(), "no fixture may be started");
    assert!(rig.lines("events").is_empty(), "no action may be run");
    assert!(app.waiting_for_human.is_none());

    // How it looks: the list symbol and the result box say skipped.
    use ratatui::{backend::TestBackend, Terminal};
    use spawn_at_verify::tui::views::draw_ui;
    let mut terminal = Terminal::new(TestBackend::new(140, 50)).unwrap();
    terminal.draw(|f| draw_ui(f, &app)).unwrap();
    let screen = super::dump_buffer_to_string(&terminal, 140, 50);
    assert!(screen.contains("⊘"), "the list marks it skipped:\n{screen}");
    assert!(
        screen.contains("SKIPPED"),
        "the result box says SKIPPED:\n{screen}"
    );
    assert!(
        screen.contains("unsupported on this backend"),
        "the reason is shown:\n{screen}"
    );
}
