//! # Window Lifecycle State Command Handlers
//!
//! Provides handlers for window state transformations (maximize, minimize, unminimize, restore).

use crate::cli::{CloseArgs, MaximizeArgs, MinimizeArgs, RestoreArgs};
use crate::commands::focus::apply_focus_policy;
use crate::platform::{CompositorBackend, WindowState};
use crate::target::{self, WindowSelector};

/// Executes the `close` subcommand: requests graceful window closure and verifies termination.
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

/// Executes the `close` subcommand with a specified verification timeout.
pub async fn run_close_with_timeout(
    backend: &dyn CompositorBackend,
    args: CloseArgs,
    no_wait: bool,
    timeout: std::time::Duration,
) -> Result<(), Box<dyn std::error::Error>> {
    args.validate()?;

    if !backend.supports_close() {
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

/// Executes the `maximize` subcommand: sets the window state to maximized and applies focus policy.
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

/// Executes the `minimize` subcommand: minimizes the target window into the panel or dock.
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

/// Executes the `unminimize` subcommand: unminimizes and restores a minimized window to the screen.
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

/// Executes the `restore` subcommand: unmaximizes and restores the window to its floating geometry.
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
