//! # Window Transformation Command Handler
//!
//! Provides the mathematical resolution and dispatch logic for moving, resizing,
//! anchoring, pivoting, and clamping active mapped windows.

use crate::cli::TransformArgs;
use crate::commands::focus::apply_focus_policy;
use crate::core::geometry::{self, PlacementParams};
use crate::core::types::Instruction;
use crate::platform::{CompositorBackend, DriverError};
use crate::target;

/// Executes the `transform` subcommand: resolves window coordinates and transforms target geometry.
pub async fn run_transform(
    backend: &dyn CompositorBackend,
    args: TransformArgs,
) -> Result<(), Box<dyn std::error::Error>> {
    if !backend.supports_runtime_transform() {
        return Err(DriverError::UnsupportedCapability(
            "Runtime window transformation is not supported by the active compositor driver",
        )
        .into());
    }

    args.validate()?;

    // 1. Query active windows via backend and resolve target window using TargetResolver
    let windows = backend.get_windows().await?;

    let selector = target::WindowSelector {
        class: args.class,
        title: args.title,
        pid: args.pid,
        focused: args.focused,
    };
    let target_win = target::resolve_target(&windows, &selector)?;

    // 2. Query active workareas and cursor coordinates
    let workareas = backend.get_workareas().await?;
    let (cursor_x, cursor_y) = backend.get_cursor_position().await?;

    // 3. Resolve target workarea from --monitor (or cursor location if --anchor cursor / --monitor cursor)
    let is_cursor_anchor = args.geometry.anchor == Some(geometry::Anchor::Cursor);
    let target_workarea = if is_cursor_anchor || args.geometry.monitor.eq_ignore_ascii_case("cursor") {
        geometry::resolve_workarea(&workareas, (cursor_x, cursor_y), "cursor")?
    } else {
        geometry::resolve_workarea(&workareas, (cursor_x, cursor_y), &args.geometry.monitor)?
    };

    // 4. Calculate target position and size via geometry solver
    let target_w = args
        .geometry
        .size
        .as_ref()
        .and_then(|s| s.get(0).and_then(|v| v.parse::<u32>().ok()))
        .unwrap_or(target_win.w as u32);
    let target_h = args
        .geometry
        .size
        .as_ref()
        .and_then(|s| s.get(1).and_then(|v| v.parse::<u32>().ok()))
        .unwrap_or(target_win.h as u32);

    let pos = if let Some(ref p) = args.geometry.pos {
        Some((p[0], p[1]))
    } else {
        None
    };
    
    let params = PlacementParams {
        pos,
        offset: None,
        anchor: args.geometry.anchor,
        pivot: args.geometry.pivot,
        size: Some((target_w, target_h)),
        margin: args.geometry.margin,
        margin_top: args.geometry.margin_top,
        margin_bottom: args.geometry.margin_bottom,
        margin_left: args.geometry.margin_left,
        margin_right: args.geometry.margin_right,
        area: Some(args.geometry.area),
        cursor_pos: Some((cursor_x, cursor_y)),
        workarea: target_workarea,
    };

    let payload = geometry::calculate_placement(params, target_win.w as u32, target_win.h as u32);

    // 5. Generate execution plan
    let instructions = vec![
        Instruction::Snapshot,
        Instruction::Cloak,
        Instruction::SetSize { w: payload.intended_w, h: payload.intended_h },
        Instruction::WaitForCommit { timeout_ms: 500 },
        Instruction::SetPositionAnchored(payload),
        Instruction::Uncloak,
        Instruction::DestroySnapshot,
    ];

    // 6. Execute batch via backend
    let target_id = target::resolve_target_id(target_win);

    backend
        .execute_batch(&target_id, &instructions)
        .await?;

    apply_focus_policy(backend, &target_id, &args.focus_modifiers).await?;

    Ok(())
}
