//! The whole focus-return chain against a real compositor: a private headless Sway.
//!
//! Ignored by default (it needs that Sway); run it with `scripts/focus-chain-sway.sh`, which starts
//! the private session through `scripts/sway-headless.sh`, sets `SPAWN_AT_BACKEND=sway` and
//! `SWAYSOCK`, and stops the session. The test refuses to run anywhere else.
//!
//! A stand-in "terminal" (a fixture window) is focused and the TUI app starts, so it records that
//! window as its terminal. A `Both` test is run with `r`: a fixture window opens and takes focus,
//! the automatic phase ends, focus goes back to the terminal; then the verdict key closes the
//! fixture and focus is still on the terminal. Real `spawn-at focus`, `close` and `query windows`
//! are used throughout; only the compositor is Sway rather than GNOME.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use spawn_at_verify::preflight::{PreflightReport, PreflightVerdict};
use spawn_at_verify::probe::EnvironmentInfo;
use spawn_at_verify::runner::owned::{CliControl, Control, WindowInfo};
use spawn_at_verify::runner::RunOptions;
use spawn_at_verify::tui::app::TuiApp;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const SPEC: &str = r#"
schema_version = 2
id = "focus/chain-sway"
category = "focus"
title = "focus chain"
description = "focus a fixture window"
kind = "both"
tags = ["moves-focus"]

[requires]
backend = ["sway"]

[[setup]]
name = "w"
args = ["--width", "400", "--height", "300"]

[action]
command = "focus"
args = ["--id", "${w}"]

[[check]]
after = 0
window = "w"
state = "present"

[human]
watch = "Did the window come to the front?"
"#;

fn target_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/debug")
}

/// Panics unless this process is aimed at the private headless Sway of `sway-headless.sh`.
fn require_private_sway() {
    assert_eq!(
        std::env::var("SPAWN_AT_BACKEND").as_deref(),
        Ok("sway"),
        "run this through scripts/focus-chain-sway.sh"
    );
    let sock = std::env::var("SWAYSOCK").expect("SWAYSOCK");
    let dir = std::path::Path::new(&sock).parent().expect("socket dir");
    assert!(
        dir.file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.starts_with("spawn-at-sway.")),
        "SWAYSOCK {sock} is not in a private spawn-at-sway.* directory"
    );
    assert!(
        dir.join(".spawn-at-sway-harness").exists(),
        "no harness marker next to {sock}"
    );
}

fn wait_for<T>(what: &str, mut f: impl FnMut() -> Option<T>) -> T {
    let end = Instant::now() + Duration::from_secs(8);
    loop {
        if let Some(v) = f() {
            return v;
        }
        assert!(Instant::now() < end, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn press(app: &mut TuiApp, c: char) {
    app.handle_key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE));
}

fn focused(control: &CliControl) -> Option<WindowInfo> {
    control.windows().ok()?.into_iter().find(|w| w.focused)
}

#[test]
#[ignore = "needs the private headless Sway: run scripts/focus-chain-sway.sh"]
fn test_focus_chain_on_private_sway() {
    require_private_sway();
    let dir = target_dir();
    let bin = dir.join("spawn-at");
    let control = CliControl { bin: bin.clone() };

    // The stand-in terminal: a window of our own child, focused before the app starts.
    let mut term = Command::new(dir.join("fixture"))
        .args([
            "--title",
            "chain-terminal",
            "--app-id",
            "chain.terminal",
            "--exit-after-ms",
            "120000",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("start the stand-in terminal");
    let result = std::panic::catch_unwind(|| {
        let term_id = wait_for("the terminal window", || {
            control
                .windows()
                .ok()?
                .into_iter()
                .find(|w| w.title == "chain-terminal")
                .map(|w| w.id)
        });
        control.focus(term_id);
        wait_for("the terminal to have focus", || {
            focused(&control).filter(|w| w.id == term_id)
        });

        let root =
            std::env::temp_dir().join(format!("spawn-at-focus-chain-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("verify/focus")).unwrap();
        std::fs::write(root.join("verify/focus/chain-sway.toml"), SPEC).unwrap();
        let options = RunOptions {
            bin_path: bin.clone(),
            ..Default::default()
        };
        let mut app = TuiApp::new(root.join("verify"), options).expect("TuiApp::new");
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
            desktop: "sway".into(),
            gnome_version: None,
            monitors_count: 1,
            primary_workarea: None,
            independent_workarea: None,
        });
        // 1. The app recorded the focused window as its terminal.
        assert_eq!(
            app.terminal_window_id,
            Some(term_id),
            "{}",
            app.terminal_note
        );

        // 2. `r`: a fixture opens and takes focus; the automatic part ends; focus comes back.
        app.selected_index = app
            .filtered_tests()
            .iter()
            .position(|t| t.id == "focus/chain-sway")
            .expect("spec in the list");
        press(&mut app, 'r');
        app.run_all_pending();
        let report = app.focus_reports["focus/chain-sway"].clone();
        println!("{}", report.lines().join("\n"));
        let fixture = control
            .windows()
            .unwrap()
            .into_iter()
            .find(|w| w.title.starts_with("verify-"))
            .expect("the fixture window is still open for the verdict");
        assert_eq!(
            focused(&control).map(|w| w.id),
            Some(term_id),
            "the terminal must have focus before the verdict prompt"
        );
        assert!(report.matches, "{:?}", report.lines());
        assert!(
            !report.sent.is_empty() || report.focused_after.is_some(),
            "{:?}",
            report.lines()
        );

        // 3. The verdict closes the fixture; focus is still on the terminal.
        press(&mut app, 'p');
        app.run_all_pending();
        wait_for("the fixture window to close", || {
            (!control.windows().ok()?.iter().any(|w| w.id == fixture.id)).then_some(())
        });
        assert_eq!(focused(&control).map(|w| w.id), Some(term_id));
        assert!(app.focus_reports["focus/chain-sway"].matches);
        assert!(
            control.windows().unwrap().iter().any(|w| w.id == term_id),
            "the terminal was never closed"
        );
        let _ = std::fs::remove_dir_all(&root);
    });
    // Our own child, recorded above: the only process signalled.
    let _ = term.kill();
    let _ = term.wait();
    if let Err(e) = result {
        std::panic::resume_unwind(e);
    }
}
