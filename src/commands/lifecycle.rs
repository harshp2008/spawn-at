//! Handlers for the `close`, `maximize`, `minimize` and `restore` subcommands (and the unused
//! `run_unminimize`).
//!
//! Each one resolves a single window from `get_windows()`, sends the request through the backend
//! (`close_window` or `set_window_state`) and, unless `no_wait`, waits for the window list to show
//! the result. It does not decide how a compositor maximizes or minimizes a window.
//!
//! The waits of `maximize`, `minimize` and `restore` ignore their result (`let _ =`), so a window
//! that never reaches the state still gives exit code 0. Only `close` turns a failed wait into an
//! error.

use crate::cli::{CloseArgs, MaximizeArgs, MinimizeArgs, RestoreArgs};
use crate::commands::focus::apply_focus_policy;
use crate::platform::{CompositorBackend, WindowState};
use crate::target::{self, WindowSelector};

/// Runs `spawn-at close` with a 1500 ms wait for the window to disappear; see
/// [`run_close_with_timeout`].
pub async fn run_close(
    backend: &dyn CompositorBackend,
    args: CloseArgs,
    no_wait: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    run_close_with_timeout(
        backend,
        args,
        no_wait,
        std::time::Duration::from_millis(1500),
    )
    .await
}

/// Asks the backend to close the matching window, then (unless `no_wait`) polls `get_windows()`
/// every 50 ms for up to `timeout` until the window is gone.
///
/// Errors: invalid `args`; `capabilities().close` is false ("this backend cannot close windows");
/// no matching window; a failing `close_window`; or the window still listed after `timeout`
/// ("did not close within ..."), which happens for example when the application keeps the window
/// open to ask the user something. A failing `get_windows()` during the wait counts as an empty
/// list, i.e. as closed. The window is looked up by id when it has one, else by
/// `matches_target` on the resolved target id.
pub async fn run_close_with_timeout(
    backend: &dyn CompositorBackend,
    args: CloseArgs,
    no_wait: bool,
    timeout: std::time::Duration,
) -> Result<(), Box<dyn std::error::Error>> {
    args.validate()?;

    if !backend.capabilities().close {
        return Err("this backend cannot close windows".into());
    }

    let windows = backend.get_windows().await?;
    let selector = WindowSelector::from(&args.target);
    let target_win = target::resolve_target(&windows, &selector)?;
    let target_id = target::resolve_target_id(target_win);
    let win_id = target_win.id;
    let win_title = target_win.title.clone();

    backend.close_window(&target_id).await?;

    if !no_wait {
        let start = std::time::Instant::now();
        let poll_interval = std::time::Duration::from_millis(50);

        let mut still_open = true;
        while start.elapsed() < timeout {
            tokio::time::sleep(poll_interval).await;
            let current_windows = backend.get_windows().await.unwrap_or_default();
            let found = current_windows.iter().any(|w| {
                if let Some(id) = win_id {
                    w.id == Some(id)
                } else {
                    crate::commands::matches_target(w, &target_id)
                }
            });
            if !found {
                still_open = false;
                break;
            }
        }

        if still_open {
            let id_str = match win_id {
                Some(id) => format!(" (id: {})", id),
                None => String::new(),
            };
            return Err(format!(
                "Window '{}'{} did not close within {:?}; window remains present.",
                win_title, id_str, timeout
            )
            .into());
        }
    }

    Ok(())
}

/// Runs `spawn-at maximize`: sends `WindowState::Maximize`, applies the focus flag from
/// `args.focus_modifiers` (see `apply_focus_policy`), then unless `no_wait` waits up to 2000 ms for
/// `maximized`; a timeout is ignored. Errors: invalid `args`, no matching window, or a failing
/// backend call.
pub async fn run_maximize(
    backend: &dyn CompositorBackend,
    args: MaximizeArgs,
    no_wait: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    args.validate()?;

    let windows = backend.get_windows().await?;
    let selector = WindowSelector::from(&args.target);
    let target_win = target::resolve_target(&windows, &selector)?;
    let target_id = target::resolve_target_id(target_win);

    backend
        .set_window_state(&target_id, WindowState::Maximize)
        .await?;
    apply_focus_policy(backend, &target_id, &args.focus_modifiers, no_wait).await?;

    if !no_wait {
        let _ = crate::commands::wait_for_state_change(
            backend,
            &target_id,
            crate::commands::ExpectedState::Maximized(true),
            std::time::Duration::from_millis(2000),
        )
        .await;
    }

    Ok(())
}

/// Runs `spawn-at minimize`: sends `WindowState::Minimize`, then unless `no_wait` waits up to
/// 2000 ms for `minimized` (a window that drops out of the list also counts); a timeout is
/// ignored. `MinimizeArgs` has no focus flags, so no focus call is made. Errors: invalid `args`,
/// no matching window, or a failing backend call.
pub async fn run_minimize(
    backend: &dyn CompositorBackend,
    args: MinimizeArgs,
    no_wait: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    args.validate()?;

    let windows = backend.get_windows().await?;
    let selector = WindowSelector::from(&args.target);
    let target_win = target::resolve_target(&windows, &selector)?;
    let target_id = target::resolve_target_id(target_win);

    backend
        .set_window_state(&target_id, WindowState::Minimize)
        .await?;

    if !no_wait {
        let _ = crate::commands::wait_for_state_change(
            backend,
            &target_id,
            crate::commands::ExpectedState::Minimized(true),
            std::time::Duration::from_millis(2000),
        )
        .await;
    }

    Ok(())
}

/// Unminimizes a minimized window (sends `WindowState::Unminimize`). There is no `unminimize`
/// subcommand: see the note below.
// No call site: the CLI aliases `unminimize`/`unminimise` to the `restore` subcommand (cli/mod.rs),
// which runs `run_restore`. Kept because it is the only sender of `WindowState::Unminimize`, i.e.
// the `unminimize` case of the extension's SetWindowState method (protocol surface, node-tested).
#[allow(dead_code)]
pub async fn run_unminimize(
    backend: &dyn CompositorBackend,
    args: MinimizeArgs,
    no_wait: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    args.validate()?;

    let windows = backend.get_windows().await?;
    let selector = WindowSelector::from(&args.target);
    let target_win = target::resolve_target(&windows, &selector)?;
    let target_id = target::resolve_target_id(target_win);

    backend
        .set_window_state(&target_id, WindowState::Unminimize)
        .await?;

    if !no_wait {
        let _ = crate::commands::wait_for_state_change(
            backend,
            &target_id,
            crate::commands::ExpectedState::Minimized(false),
            std::time::Duration::from_millis(2000),
        )
        .await;
    }

    Ok(())
}

/// Runs `spawn-at restore` (also reached through the `unmaximize` and `unminimize` aliases):
/// sends `WindowState::Restore`, applies the focus flag from `args.focus_modifiers`, then unless
/// `no_wait` waits up to 2000 ms for the window to be neither maximized nor minimized; a timeout
/// is ignored. Errors: invalid `args`, no matching window, or a failing backend call.
// The GNOME extension's `restore` case (modern/ and legacy/ extension.js) unminimizes a minimized
// window and unmaximizes a maximized one; other backends are not verified.
pub async fn run_restore(
    backend: &dyn CompositorBackend,
    args: RestoreArgs,
    no_wait: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    args.validate()?;

    let windows = backend.get_windows().await?;
    let selector = WindowSelector::from(&args.target);
    let target_win = target::resolve_target(&windows, &selector)?;
    let target_id = target::resolve_target_id(target_win);

    backend
        .set_window_state(&target_id, WindowState::Restore)
        .await?;
    apply_focus_policy(backend, &target_id, &args.focus_modifiers, no_wait).await?;

    if !no_wait {
        let _ = crate::commands::wait_for_state_change(
            backend,
            &target_id,
            crate::commands::ExpectedState::Restored,
            std::time::Duration::from_millis(2000),
        )
        .await;
    }

    Ok(())
}
