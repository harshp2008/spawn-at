//! # Spawn Command Handler
//!
//! Orchestrates the multi-stage window placement and application launch pipeline:
//! 1. Resolves geometry and workareas from compositor backend.
//! 2. Arms the backend driver (fail-open on driver error).
//! 3. Launches the child process with armed environment variables.
//! 4. Executes post-spawn interception hooks and waits for window mapping.

use crate::cli::args::SpawnArgs;
use crate::commands::wait_for_spawn;
use crate::platform::{CompositorBackend, DriverError, PlacementParams};
use spawn_at_core::driver::{Batch, Entry, FocusIntent, Reveal, Urgency};
use spawn_at_core::geometry::{
    calculate_rect_from_placement, check_geometry_diagnostics, resolve_workarea, Anchor,
    GeometryDiagnostic,
};
use std::time::Duration;

/// Executes the spawn subcommand pipeline.
pub async fn run_spawn(
    driver: &(dyn CompositorBackend + Sync),
    spawn_args: SpawnArgs,
    no_wait: bool,
) -> Result<(), DriverError> {
    if spawn_args.command.is_empty() {
        return Err(DriverError::Execution("No command specified to spawn.".into()));
    }

    if let Err(e) = spawn_args.validate() {
        return Err(DriverError::Execution(e.into()));
    }

    // 1. Calculate target geometry parameters
    let is_cursor_anchor = spawn_args.geometry.anchor == Some(Anchor::Cursor);
    let (cursor_x, cursor_y) = driver.get_cursor_position().await.unwrap_or((0, 0));
    let workareas = driver.get_workareas().await.unwrap_or_default();

    let target_workarea = if is_cursor_anchor || spawn_args.geometry.monitor.eq_ignore_ascii_case("cursor") {
        resolve_workarea(&workareas, (cursor_x, cursor_y), "cursor").unwrap_or_default()
    } else {
        resolve_workarea(&workareas, (cursor_x, cursor_y), &spawn_args.geometry.monitor).unwrap_or_default()
    };

    let pos = spawn_args.geometry.pos.as_ref().map(|p| (p[0], p[1]));
    let size = spawn_args.geometry.size.as_ref().and_then(|s| {
        if s.len() == 2 {
            let w = s[0].parse::<u32>().ok()?;
            let h = s[1].parse::<u32>().ok()?;
            Some((w, h))
        } else {
            None
        }
    });

    let params = PlacementParams {
        pos,
        offset: None,
        size,
        anchor: spawn_args.geometry.anchor,
        pivot: spawn_args.geometry.pivot,
        margin: spawn_args.geometry.margin,
        margin_top: spawn_args.geometry.margin_top,
        margin_bottom: spawn_args.geometry.margin_bottom,
        margin_left: spawn_args.geometry.margin_left,
        margin_right: spawn_args.geometry.margin_right,
        area: Some(spawn_args.geometry.area),
        cursor_pos: Some((cursor_x, cursor_y)),
        workarea: target_workarea,
        clamp: spawn_args.geometry.clamp,
    };

    for diag in check_geometry_diagnostics(&params) {
        match diag {
            GeometryDiagnostic::Oversized { w, h } => {
                eprintln!(
                    "\n\x1b[1;33m[spawn-at] WARN:\x1b[0m Window is oversized ({w}x{h}); bottom and right margins ignored."
                );
            }
            GeometryDiagnostic::SubMinimumSize { w, h } => {
                eprintln!(
                    "\n\x1b[1;36m[spawn-at] INFO:\x1b[0m Requested size ({w}x{h}) is below toolkit minimums; window will expand."
                );
            }
            _ => {}
        }
    }

    // 2. Generate unique activation token / entry key conforming to freedesktop startup notification spec
    let ts_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let entry_key = format!(
        "spawn-at-{}-{}_TIME{}",
        std::process::id(),
        ts_ms,
        ts_ms
    );

    let app_hint = spawn_args.class.clone().unwrap_or_else(|| {
        driver.resolve_id(&spawn_args.command, None)
    });

    let batch = Batch {
        id: 1,
        entries: vec![Entry {
            key: entry_key.clone(),
            app_hint: app_hint.clone(),
            placement: params.clone(),
        }],
        reveal: Reveal::Together,
        focus: FocusIntent::Exclusive,
        urgency: Urgency::Normal,
        deadline: Duration::from_millis(15000),
    };

    // Pre-record existing window IDs so wait_for_spawn captures the newly spawned window accurately
    let pre_existing_ids: std::collections::HashSet<u64> = driver
        .get_windows()
        .await
        .unwrap_or_default()
        .into_iter()
        .filter_map(|w| w.id)
        .collect();

    // 3. Arm the driver with the declarative batch intent (fail-open on arm failure)
    let armed_opt = match driver.arm(batch.clone()).await {
        Ok(a) => Some(a),
        Err(e) => {
            eprintln!(
                "\x1b[1;33m[spawn-at] Warning:\x1b[0m Placement arming failed [driver: {}]: {}. Launching application without placement.",
                driver.name(),
                e
            );
            None
        }
    };

    // 4. Launch child process with armed environment variables
    let mut cmd = std::process::Command::new(&spawn_args.command[0]);
    cmd.args(&spawn_args.command[1..]);
    if let Some(ref armed) = armed_opt {
        for (k, v) in &armed.launch_env {
            cmd.env(k, v);
        }
    }

    let child = match cmd.spawn() {
        Ok(child) => child,
        Err(e) => {
            if let Some(ref armed) = armed_opt {
                if let Some(ref token) = armed.token {
                    let _ = driver.disarm(token).await;
                }
            }
            return Err(DriverError::Execution(format!(
                "Failed to spawn command '{}': {}",
                spawn_args.command[0], e
            ).into()));
        }
    };

    if let Some(ref _armed) = armed_opt {
        // Polymorphic post-spawn interception hook
        if let Err(e) = driver.post_spawn(child.id(), &batch).await {
            eprintln!("\x1b[1;33m[spawn-at] Warning:\x1b[0m Window post-spawn hook failed: {}", e);
        }

        if !no_wait {
            match wait_for_spawn(
                driver,
                child.id(),
                &app_hint,
                &entry_key,
                &pre_existing_ids,
                Duration::from_millis(2000),
            )
            .await
            {
                Ok(win) => {
                    let final_win = if let Some(id) = win.id {
                        driver
                            .get_windows()
                            .await
                            .ok()
                            .and_then(|wins| wins.into_iter().find(|w| w.id == Some(id)))
                            .unwrap_or(win)
                    } else {
                        win
                    };

                    let expected = calculate_rect_from_placement(
                        &params,
                        final_win.w as u32,
                        final_win.h as u32,
                    );

                    let x_diff = (final_win.x - expected.x).abs();
                    let y_diff = (final_win.y - expected.y).abs();

                    if x_diff <= 1 && y_diff <= 1 {
                        eprintln!(
                            "[spawn-at] INFO: Window mapped at final size ({}x{}) and positioned at ({}, {}).",
                            final_win.w, final_win.h, final_win.x, final_win.y
                        );
                    } else {
                        eprintln!(
                            "\x1b[1;33m[spawn-at] Warning:\x1b[0m Window mapped at ({}, {}) [{}x{}] but expected position was ({}, {}). Window was not positioned at target.",
                            final_win.x, final_win.y, final_win.w, final_win.h, expected.x, expected.y
                        );
                    }
                }
                Err(e) => {
                    eprintln!(
                        "\x1b[1;33m[spawn-at] Warning:\x1b[0m Timed out waiting for window to map: {}",
                        e
                    );
                }
            }
        }
    }

    Ok(())
}
