//! Tests for `run_focus`, `run_defocus`, `run_maximize`, `run_minimize`, `run_restore` and
//! `apply_focus_policy` (audit proposal P1, gaps 1, 2 and 6 in `docs/TEST_COVERAGE_AUDIT.md`).
//!
//! Two stand-in backends:
//! - `Recording` overrides the control methods, logs every call as a string (`focus 42`,
//!   `defocus 42 mru `, `state 42 Maximize`) and, when `apply` is true, changes its window list the
//!   way a compositor would so the 2 s state wait ends at once.
//! - `Bare` implements only `get_windows`. Its control methods are the trait defaults, which is
//!   exactly what Sway gets for defocus/maximize/minimize/restore (`src/platform/mod.rs:327-343`).
//!
//! Tests marked "CHARACTERIZATION" pin what the code does today, not what it should do.

use super::{run_defocus, run_focus, run_maximize, run_minimize, run_restore};
use crate::cli::{
    DefocusArgs, FocusModifierArgs, MaximizeArgs, MinimizeArgs, RestoreArgs, WindowTargetArgs,
};
use crate::commands::focus::apply_focus_policy;
use crate::platform::{CompositorBackend, DriverError, WindowMetadata, WindowState};
use std::sync::Mutex;
use std::time::Instant;

fn window(id: u64, focused: bool) -> WindowMetadata {
    WindowMetadata {
        id: Some(id),
        pid: Some(4000 + id as u32),
        title: format!("Win {id}"),
        class: "fixture".into(),
        focused,
        ..Default::default()
    }
}

struct Recording {
    windows: Mutex<Vec<WindowMetadata>>,
    calls: Mutex<Vec<String>>,
    /// Mutate the window list on each control call so the state wait is satisfied.
    apply: bool,
    /// Make every control call fail with this driver error text.
    fail_with: Option<&'static str>,
}

impl Recording {
    fn new(windows: Vec<WindowMetadata>, apply: bool) -> Self {
        Self {
            windows: Mutex::new(windows),
            calls: Mutex::new(Vec::new()),
            apply,
            fail_with: None,
        }
    }
    fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }
    fn check_fail(&self) -> Result<(), DriverError> {
        match self.fail_with {
            Some(msg) => Err(DriverError::Execution(msg.into())),
            None => Ok(()),
        }
    }
}

#[async_trait::async_trait]
impl crate::platform::Driver for Recording {
    async fn arm(
        &self,
        _batch: crate::platform::Batch,
    ) -> Result<crate::platform::Armed, DriverError> {
        Ok(crate::platform::Armed::default())
    }
}

#[async_trait::async_trait]
impl CompositorBackend for Recording {
    fn name(&self) -> &'static str {
        "Recording"
    }
    fn resolve_id(&self, _c: &[String], _k: Option<&str>) -> String {
        "rec".into()
    }
    async fn get_windows(&self) -> Result<Vec<WindowMetadata>, DriverError> {
        Ok(self.windows.lock().unwrap().clone())
    }
    async fn focus_window(&self, target_id: &str) -> Result<(), DriverError> {
        self.calls
            .lock()
            .unwrap()
            .push(format!("focus {target_id}"));
        self.check_fail()?;
        if self.apply {
            for w in self.windows.lock().unwrap().iter_mut() {
                w.focused = w.id.map(|i| i.to_string()) == Some(target_id.to_string());
            }
        }
        Ok(())
    }
    async fn defocus_window(
        &self,
        target_id: &str,
        mode: &str,
        destination: &str,
    ) -> Result<(), DriverError> {
        self.calls
            .lock()
            .unwrap()
            .push(format!("defocus {target_id} {mode} {destination}"));
        self.check_fail()?;
        if self.apply {
            for w in self.windows.lock().unwrap().iter_mut() {
                w.focused = false;
            }
        }
        Ok(())
    }
    async fn set_window_state(
        &self,
        target_id: &str,
        state: WindowState,
    ) -> Result<(), DriverError> {
        self.calls
            .lock()
            .unwrap()
            .push(format!("state {target_id} {state:?}"));
        self.check_fail()?;
        if self.apply {
            for w in self.windows.lock().unwrap().iter_mut() {
                match state {
                    WindowState::Maximize => w.maximized = true,
                    WindowState::Minimize => w.minimized = true,
                    WindowState::Unminimize => w.minimized = false,
                    WindowState::Restore => {
                        w.maximized = false;
                        w.minimized = false;
                    }
                }
            }
        }
        Ok(())
    }
}

/// Only `get_windows`; every control method is the trait default (the Sway situation).
struct Bare(Vec<WindowMetadata>);

#[async_trait::async_trait]
impl crate::platform::Driver for Bare {
    async fn arm(
        &self,
        _batch: crate::platform::Batch,
    ) -> Result<crate::platform::Armed, DriverError> {
        Ok(crate::platform::Armed::default())
    }
}

#[async_trait::async_trait]
impl CompositorBackend for Bare {
    fn name(&self) -> &'static str {
        "Bare"
    }
    fn resolve_id(&self, _c: &[String], _k: Option<&str>) -> String {
        "bare".into()
    }
    async fn get_windows(&self) -> Result<Vec<WindowMetadata>, DriverError> {
        Ok(self.0.clone())
    }
}

fn by_id(id: u64) -> WindowTargetArgs {
    WindowTargetArgs {
        id: Some(id),
        ..Default::default()
    }
}

fn defocus_args(id: Option<u64>) -> DefocusArgs {
    DefocusArgs {
        target: WindowTargetArgs {
            id,
            ..Default::default()
        },
        ..Default::default()
    }
}

const UNSUPPORTED: &str = "The active compositor does not support this feature: ";

// ---- run_focus ----

#[tokio::test]
async fn test_run_focus_calls_the_driver_with_the_resolved_id_and_waits_until_focused() {
    let b = Recording::new(vec![window(41, true), window(42, false)], true);
    run_focus(&b, by_id(42), false).await.unwrap();
    assert_eq!(b.calls(), vec!["focus 42"]);
    assert!(b.windows.lock().unwrap()[1].focused);
}

#[tokio::test]
async fn test_run_focus_unknown_id_reports_not_found_and_sends_nothing() {
    let b = Recording::new(vec![window(41, true)], true);
    let err = run_focus(&b, by_id(99), true).await.unwrap_err();
    assert_eq!(err.to_string(), "No window with id 99");
    assert!(b.calls().is_empty());
}

#[tokio::test]
async fn test_run_focus_without_selector_is_rejected_before_any_query() {
    let b = Recording::new(vec![window(41, true)], true);
    let err = run_focus(&b, WindowTargetArgs::default(), true)
        .await
        .unwrap_err();
    assert!(err.to_string().starts_with("At least one window selector"));
    assert!(b.calls().is_empty());
}

#[tokio::test]
async fn test_run_focus_unsupported_backend_error_names_focus_window() {
    let b = Bare(vec![window(42, false)]);
    let err = run_focus(&b, by_id(42), true).await.unwrap_err();
    assert_eq!(err.to_string(), format!("{UNSUPPORTED}focus_window"));
}

#[tokio::test]
async fn test_run_focus_driver_error_reaches_the_caller() {
    let mut b = Recording::new(vec![window(42, false)], true);
    b.fail_with = Some("Compositor failed to focus window");
    let err = run_focus(&b, by_id(42), true).await.unwrap_err();
    assert_eq!(err.to_string(), "Compositor failed to focus window");
}

/// CHARACTERIZATION (audit gap 6): the window never becomes focused, the 2000 ms wait result is
/// dropped with `let _ =`, and the command still returns Ok (exit 0 in `main.rs`). It documents
/// current behavior; reporting the timeout would change the exit code and may be wanted later.
#[tokio::test]
async fn test_run_focus_state_never_changes_still_succeeds_after_the_2s_wait() {
    let b = Recording::new(vec![window(42, false)], false);
    let start = Instant::now();
    assert!(run_focus(&b, by_id(42), false).await.is_ok());
    assert!(start.elapsed() >= std::time::Duration::from_millis(1900));
    assert!(!b.windows.lock().unwrap()[0].focused);
}

#[tokio::test]
async fn test_run_focus_no_wait_returns_without_polling() {
    let b = Recording::new(vec![window(42, false)], false);
    let start = Instant::now();
    run_focus(&b, by_id(42), true).await.unwrap();
    assert!(start.elapsed() < std::time::Duration::from_millis(500));
}

// ---- run_defocus ----

#[tokio::test]
async fn test_run_defocus_focused_window_defaults_to_mru_mode() {
    let b = Recording::new(vec![window(42, true)], true);
    run_defocus(&b, defocus_args(Some(42)), false)
        .await
        .unwrap();
    assert_eq!(b.calls(), vec!["defocus 42 mru "]);
}

#[tokio::test]
async fn test_run_defocus_without_selector_targets_the_focused_window() {
    let b = Recording::new(vec![window(41, false), window(42, true)], true);
    run_defocus(&b, defocus_args(None), true).await.unwrap();
    assert_eq!(b.calls(), vec!["defocus 42 mru "]);
}

#[tokio::test]
async fn test_run_defocus_to_desktop_and_to_window_pass_mode_and_destination() {
    let b = Recording::new(vec![window(42, true)], true);
    let mut args = defocus_args(Some(42));
    args.to_desktop = true;
    run_defocus(&b, args, true).await.unwrap();

    let b2 = Recording::new(vec![window(42, true)], true);
    let mut args = defocus_args(Some(42));
    args.to_window = Some("Terminal".into());
    run_defocus(&b2, args, true).await.unwrap();

    assert_eq!(b.calls(), vec!["defocus 42 desktop "]);
    assert_eq!(b2.calls(), vec!["defocus 42 window Terminal"]);
}

/// A window that is not focused is a no-op: Ok, nothing sent to the driver.
#[tokio::test]
async fn test_run_defocus_already_unfocused_window_is_a_noop() {
    let b = Recording::new(vec![window(42, false)], true);
    run_defocus(&b, defocus_args(Some(42)), false)
        .await
        .unwrap();
    assert!(b.calls().is_empty());
}

#[tokio::test]
async fn test_run_defocus_no_focused_window_and_no_selector_is_not_found() {
    let b = Recording::new(vec![window(42, false)], true);
    let err = run_defocus(&b, defocus_args(None), true).await.unwrap_err();
    assert!(err
        .to_string()
        .contains("no active window currently has focus"));
    assert!(b.calls().is_empty());
}

#[tokio::test]
async fn test_run_defocus_unknown_id_reports_not_found() {
    let b = Recording::new(vec![window(42, true)], true);
    let err = run_defocus(&b, defocus_args(Some(7)), true)
        .await
        .unwrap_err();
    assert_eq!(err.to_string(), "No window with id 7");
}

#[tokio::test]
async fn test_run_defocus_unsupported_backend_error_names_defocus_window() {
    let b = Bare(vec![window(42, true)]);
    let err = run_defocus(&b, defocus_args(Some(42)), true)
        .await
        .unwrap_err();
    assert_eq!(err.to_string(), format!("{UNSUPPORTED}defocus_window"));
}

/// CHARACTERIZATION (gap 6): focus never leaves the window; Ok after the 2 s wait anyway.
#[tokio::test]
async fn test_run_defocus_state_never_changes_still_succeeds_after_the_2s_wait() {
    let b = Recording::new(vec![window(42, true)], false);
    let start = Instant::now();
    assert!(run_defocus(&b, defocus_args(Some(42)), false).await.is_ok());
    assert!(start.elapsed() >= std::time::Duration::from_millis(1900));
    assert!(b.windows.lock().unwrap()[0].focused);
}

// ---- run_maximize / run_minimize / run_restore ----

#[tokio::test]
async fn test_run_maximize_sends_maximize_then_focus_by_default() {
    let b = Recording::new(vec![window(42, false)], true);
    let args = MaximizeArgs {
        target: by_id(42),
        ..Default::default()
    };
    run_maximize(&b, args, false).await.unwrap();
    assert_eq!(b.calls(), vec!["state 42 Maximize", "focus 42"]);
    assert!(b.windows.lock().unwrap()[0].maximized);
}

#[tokio::test]
async fn test_run_maximize_no_focus_sends_only_the_state_change() {
    let b = Recording::new(vec![window(42, false)], true);
    let args = MaximizeArgs {
        target: by_id(42),
        focus_modifiers: FocusModifierArgs {
            no_focus: true,
            ..Default::default()
        },
    };
    run_maximize(&b, args, true).await.unwrap();
    assert_eq!(b.calls(), vec!["state 42 Maximize"]);
}

#[tokio::test]
async fn test_run_maximize_defocus_sends_state_then_mru_defocus() {
    let b = Recording::new(vec![window(42, true)], true);
    let args = MaximizeArgs {
        target: by_id(42),
        focus_modifiers: FocusModifierArgs {
            defocus: true,
            ..Default::default()
        },
    };
    run_maximize(&b, args, true).await.unwrap();
    assert_eq!(b.calls(), vec!["state 42 Maximize", "defocus 42 mru "]);
}

#[tokio::test]
async fn test_run_minimize_sends_minimize_and_never_touches_focus() {
    let b = Recording::new(vec![window(42, true)], true);
    let args = MinimizeArgs { target: by_id(42) };
    run_minimize(&b, args, false).await.unwrap();
    assert_eq!(b.calls(), vec!["state 42 Minimize"]);
}

#[tokio::test]
async fn test_run_restore_sends_restore_then_focus_by_default() {
    let mut w = window(42, false);
    w.maximized = true;
    let b = Recording::new(vec![w], true);
    let args = RestoreArgs {
        target: by_id(42),
        ..Default::default()
    };
    run_restore(&b, args, false).await.unwrap();
    assert_eq!(b.calls(), vec!["state 42 Restore", "focus 42"]);
    assert!(!b.windows.lock().unwrap()[0].maximized);
}

#[tokio::test]
async fn test_state_commands_unknown_id_report_not_found_and_send_nothing() {
    let b = Recording::new(vec![window(42, true)], true);
    let e1 = run_maximize(
        &b,
        MaximizeArgs {
            target: by_id(9),
            ..Default::default()
        },
        true,
    )
    .await
    .unwrap_err();
    let e2 = run_minimize(&b, MinimizeArgs { target: by_id(9) }, true)
        .await
        .unwrap_err();
    let e3 = run_restore(
        &b,
        RestoreArgs {
            target: by_id(9),
            ..Default::default()
        },
        true,
    )
    .await
    .unwrap_err();
    for e in [e1, e2, e3] {
        assert_eq!(e.to_string(), "No window with id 9");
    }
    assert!(b.calls().is_empty());
}

#[tokio::test]
async fn test_state_commands_without_selector_are_rejected() {
    let b = Recording::new(vec![window(42, true)], true);
    let e = run_minimize(
        &b,
        MinimizeArgs {
            target: WindowTargetArgs::default(),
        },
        true,
    )
    .await
    .unwrap_err();
    assert!(e.to_string().starts_with("At least one window selector"));
    assert!(b.calls().is_empty());
}

/// Sway declares maximize, minimize and restore unsupported: all three surface the default
/// `set_window_state` error, and restore/maximize stop before any focus call.
#[tokio::test]
async fn test_state_commands_unsupported_backend_error_names_set_window_state() {
    let b = Bare(vec![window(42, true)]);
    let want = format!("{UNSUPPORTED}set_window_state");
    let e1 = run_maximize(
        &b,
        MaximizeArgs {
            target: by_id(42),
            ..Default::default()
        },
        true,
    )
    .await
    .unwrap_err();
    let e2 = run_minimize(&b, MinimizeArgs { target: by_id(42) }, true)
        .await
        .unwrap_err();
    let e3 = run_restore(
        &b,
        RestoreArgs {
            target: by_id(42),
            ..Default::default()
        },
        true,
    )
    .await
    .unwrap_err();
    for e in [e1, e2, e3] {
        assert_eq!(e.to_string(), want);
    }
}

#[tokio::test]
async fn test_state_command_driver_error_reaches_the_caller() {
    let mut b = Recording::new(vec![window(42, true)], true);
    b.fail_with = Some("Compositor failed to maximize window");
    let err = run_maximize(
        &b,
        MaximizeArgs {
            target: by_id(42),
            ..Default::default()
        },
        true,
    )
    .await
    .unwrap_err();
    assert_eq!(err.to_string(), "Compositor failed to maximize window");
}

/// CHARACTERIZATION (gap 6): maximize/minimize/restore return Ok even though the window never
/// reaches the requested state. Each waits the full 2000 ms for its own state (and maximize and
/// restore also for the focus change), then ignores the result. Pinned as current behavior.
#[tokio::test]
async fn test_state_commands_state_never_changes_still_succeed_after_the_2s_wait() {
    let mut w = window(42, true);
    w.maximized = true;
    let b = Recording::new(vec![w], false);
    let start = Instant::now();
    assert!(run_minimize(&b, MinimizeArgs { target: by_id(42) }, false)
        .await
        .is_ok());
    let mut args = MaximizeArgs {
        target: by_id(42),
        ..Default::default()
    };
    args.focus_modifiers.no_focus = true;
    // The fixture window starts maximized, so this maximize wait ends at once; minimize (never
    // minimized) and restore (still maximized) each wait the full 2000 ms: about 4 s in total.
    assert!(run_maximize(&b, args, false).await.is_ok());
    let mut args = RestoreArgs {
        target: by_id(42),
        ..Default::default()
    };
    args.focus_modifiers.no_focus = true;
    assert!(run_restore(&b, args, false).await.is_ok());
    assert!(start.elapsed() >= std::time::Duration::from_millis(3900));
    assert_eq!(
        b.calls(),
        vec!["state 42 Minimize", "state 42 Maximize", "state 42 Restore"]
    );
}

// ---- apply_focus_policy ----

#[tokio::test]
async fn test_apply_focus_policy_default_and_explicit_focus_call_focus_window() {
    for m in [
        FocusModifierArgs::default(),
        FocusModifierArgs {
            focus: true,
            ..Default::default()
        },
    ] {
        let b = Recording::new(vec![window(42, false)], true);
        apply_focus_policy(&b, "42", &m, false).await.unwrap();
        assert_eq!(b.calls(), vec!["focus 42"]);
    }
}

#[tokio::test]
async fn test_apply_focus_policy_no_focus_touches_nothing() {
    let b = Recording::new(vec![window(42, false)], true);
    let m = FocusModifierArgs {
        no_focus: true,
        ..Default::default()
    };
    apply_focus_policy(&b, "42", &m, false).await.unwrap();
    assert!(b.calls().is_empty());
}

#[tokio::test]
async fn test_apply_focus_policy_defocus_uses_mru_with_empty_destination() {
    let b = Recording::new(vec![window(42, true)], true);
    let m = FocusModifierArgs {
        defocus: true,
        ..Default::default()
    };
    apply_focus_policy(&b, "42", &m, false).await.unwrap();
    assert_eq!(b.calls(), vec!["defocus 42 mru "]);
}

/// `defocus` wins over the other flags in the code (it is tested first); clap makes the
/// combination unreachable from the CLI, so this only pins the order in the function.
#[tokio::test]
async fn test_apply_focus_policy_defocus_takes_precedence_when_flags_are_combined() {
    let b = Recording::new(vec![window(42, true)], true);
    let m = FocusModifierArgs {
        defocus: true,
        no_focus: true,
        focus: true,
    };
    apply_focus_policy(&b, "42", &m, true).await.unwrap();
    assert_eq!(b.calls(), vec!["defocus 42 mru "]);
}

#[tokio::test]
async fn test_apply_focus_policy_unsupported_backend_errors_except_for_no_focus() {
    let b = Bare(vec![window(42, true)]);
    let e = apply_focus_policy(&b, "42", &FocusModifierArgs::default(), true)
        .await
        .unwrap_err();
    assert_eq!(e.to_string(), format!("{UNSUPPORTED}focus_window"));
    let d = FocusModifierArgs {
        defocus: true,
        ..Default::default()
    };
    let e = apply_focus_policy(&b, "42", &d, true).await.unwrap_err();
    assert_eq!(e.to_string(), format!("{UNSUPPORTED}defocus_window"));
    let n = FocusModifierArgs {
        no_focus: true,
        ..Default::default()
    };
    assert!(apply_focus_policy(&b, "42", &n, true).await.is_ok());
}

/// CHARACTERIZATION (gap 6): focus is requested, the window never becomes focused, the helper
/// waits 2000 ms and returns Ok.
#[tokio::test]
async fn test_apply_focus_policy_ignores_a_timed_out_wait() {
    let b = Recording::new(vec![window(42, false)], false);
    let start = Instant::now();
    assert!(
        apply_focus_policy(&b, "42", &FocusModifierArgs::default(), false)
            .await
            .is_ok()
    );
    assert!(start.elapsed() >= std::time::Duration::from_millis(1900));
}
