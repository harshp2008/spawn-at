//! # Window Focus Command Handlers
//!
//! Provides handlers for focusing, defocusing, and enforcing focus modifier policies
//! across window management operations.

use crate::cli::{DefocusArgs, FocusModifierArgs, WindowTargetArgs};
use crate::platform::CompositorBackend;
use crate::target::{self, WindowSelector};

/// Evaluates and applies the requested `FocusModifierArgs` policy to the target window.
pub async fn apply_focus_policy(
    backend: &dyn CompositorBackend,
    target_id: &str,
    modifiers: &FocusModifierArgs,
    no_wait: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    if modifiers.defocus {
        // Active eviction: strip focus and yield to previous window
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
        // Passive: leave compositor focus state untouched
    } else {
        // Default (or explicit --focus): ensure target window gets focus
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

/// Executes the `focus` subcommand: identifies target window and transfers input focus to it.
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

/// Executes the `defocus` subcommand: evicts input focus from the target window to desktop or previous window.
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
