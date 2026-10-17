//! Handlers for the `focus` and `defocus` subcommands, and `apply_focus_policy`, which applies the
//! `--focus` / `--no-focus` / `--defocus` flags after another command has changed a window.
//!
//! Each handler resolves one window from the current window list, then calls the backend's
//! `focus_window` or `defocus_window`. It does not decide how a compositor moves focus.

use crate::cli::{DefocusArgs, FocusModifierArgs, WindowTargetArgs};
use crate::platform::CompositorBackend;
use crate::target::{self, WindowSelector};

/// Applies the focus flag in `modifiers` to the window `target_id`.
///
/// `--defocus` calls `defocus_window(target_id, "mru", "")`; `--no-focus` does nothing; with
/// `--focus` or neither flag it calls `focus_window`. Unless `no_wait`, it then waits up to
/// 2000 ms for the window to report the new focus state; a timeout there is ignored, so the result
/// does not say whether focus really changed. Errors are those of the backend call.
pub async fn apply_focus_policy(
    backend: &dyn CompositorBackend,
    target_id: &str,
    modifiers: &FocusModifierArgs,
    no_wait: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    if modifiers.defocus {
        // "mru" is the mode `defocus` also uses when neither --to-desktop nor --to-window is
        // given; what a backend does with it is not verified here.
        backend.defocus_window(target_id, "mru", "").await?;
        if !no_wait {
            let _ = crate::commands::wait_for_state_change(
                backend,
                target_id,
                crate::commands::ExpectedState::Focused(false),
                std::time::Duration::from_millis(2000),
            )
            .await;
        }
    } else if modifiers.no_focus {
        // --no-focus: no backend call and no wait.
    } else {
        // --focus, or no flag at all (the default for the commands that call this).
        backend.focus_window(target_id).await?;
        if !no_wait {
            let _ = crate::commands::wait_for_state_change(
                backend,
                target_id,
                crate::commands::ExpectedState::Focused(true),
                std::time::Duration::from_millis(2000),
            )
            .await;
        }
    }
    Ok(())
}

/// Runs `spawn-at focus`: resolves the window from `args` and calls `focus_window` on it.
///
/// Unless `no_wait`, waits up to 2000 ms for the window to report `focused`; a timeout is
/// ignored and the command still returns `Ok`. Errors: invalid `args`, no matching window, or a
/// failing backend call.
pub async fn run_focus(
    backend: &dyn CompositorBackend,
    args: WindowTargetArgs,
    no_wait: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    args.validate()?;

    let windows = backend.get_windows().await?;
    let selector = WindowSelector::from(&args);
    let target_win = target::resolve_target(&windows, &selector)?;
    let target_id = target::resolve_target_id(target_win);

    backend.focus_window(&target_id).await?;

    if !no_wait {
        let _ = crate::commands::wait_for_state_change(
            backend,
            &target_id,
            crate::commands::ExpectedState::Focused(true),
            std::time::Duration::from_millis(2000),
        )
        .await;
    }

    Ok(())
}

/// Runs `spawn-at defocus`: resolves the window from `args` (the focused window when no selector
/// is given) and calls `defocus_window` with the mode from `args.mode_and_destination()`.
///
/// If the window is already not focused it prints a line to stdout and returns `Ok` without
/// calling the backend. Otherwise, unless `no_wait`, it waits up to 2000 ms for `focused` to
/// become false (a timeout is ignored) and prints "Defocused window ..." whether or not that
/// happened. Errors: invalid `args`, no matching window, or a failing backend call.
pub async fn run_defocus(
    backend: &dyn CompositorBackend,
    args: DefocusArgs,
    no_wait: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    args.validate()?;

    let windows = backend.get_windows().await?;
    let selector = args.get_selector();
    let target_win = target::resolve_target(&windows, &selector)?;
    let target_id = target::resolve_target_id(target_win);

    if !target_win.focused {
        println!(
            "Window '{}' is already not focused; nothing to defocus.",
            target_win.title
        );
        return Ok(());
    }

    let (mode, destination) = args.mode_and_destination();
    backend
        .defocus_window(&target_id, mode, destination)
        .await?;

    if !no_wait {
        let _ = crate::commands::wait_for_state_change(
            backend,
            &target_id,
            crate::commands::ExpectedState::Focused(false),
            std::time::Duration::from_millis(2000),
        )
        .await;
    }

    let yield_target = match mode {
        "desktop" => "desktop".to_string(),
        "window" => format!("window '{}'", destination),
        _ => "previous window/desktop".to_string(),
    };
    println!(
        "Defocused window '{}' (focus yielded to {}).",
        target_win.title, yield_target
    );

    Ok(())
}
