//! # Window Lifecycle State Command Handlers
//!
//! Provides handlers for window state transformations (maximize, minimize, unminimize, restore).

use crate::cli::{MaximizeArgs, MinimizeArgs, RestoreArgs};
use crate::commands::focus::apply_focus_policy;
use crate::platform::{CompositorBackend, WindowState};
use crate::target::{self, WindowSelector};

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
