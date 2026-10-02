//! # Window Transformation Command Handler
//!
//! Provides the mathematical resolution and dispatch logic for moving, resizing,
//! anchoring, pivoting, and clamping active mapped windows.

use crate::cli::TransformArgs;
use crate::commands::focus::apply_focus_policy;
use crate::core::geometry::{self, Rect};
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

    // 3. Resolve target workarea from --monitor (or cursor location if --cursor / --monitor cursor)
    let target_workarea = if args.cursor || args.monitor.eq_ignore_ascii_case("cursor") {
        geometry::resolve_workarea(&workareas, (cursor_x, cursor_y), "cursor")?
    } else {
        geometry::resolve_workarea(&workareas, (cursor_x, cursor_y), &args.monitor)?
    };

    // 4. Determine target size
    let (target_w, target_h) = if let Some(ref s) = args.size {
        if s.len() != 2 {
            return Err("Size argument must contain exactly width and height: --size W H".into());
        }
        (s[0], s[1])
    } else {
        (target_win.w, target_win.h)
    };

    // 5. Determine target position
    let (target_x, target_y) = if let Some(anchor) = args.anchor {
        geometry::apply_anchor(
            target_workarea,
            target_w as u32,
            target_h as u32,
            anchor,
            args.margin,
        )
    } else if args.cursor {
        geometry::apply_pivot(
            cursor_x,
            cursor_y,
            target_w as u32,
            target_h as u32,
            args.pivot,
        )
    } else if let Some(ref pos) = args.pos {
        if pos.len() != 2 {
            return Err(
                "Position argument must contain exactly X and Y coordinates: --pos X Y".into(),
            );
        }
        geometry::apply_pivot(
            pos[0],
            pos[1],
            target_w as u32,
            target_h as u32,
            args.pivot,
        )
    } else {
        (target_win.x, target_win.y)
    };

    // 6. If clamp is true, run clamp_rect against target workarea and margin
    let (final_x, final_y, final_w, final_h) = if args.clamp {
        let clamped = geometry::clamp_to_bounds(
            Rect {
                x: target_x,
                y: target_y,
                width: target_w as u32,
                height: target_h as u32,
            },
            target_workarea,
            0,
            0,
            0,
            0,
            args.margin,
        );
        (clamped.x, clamped.y, clamped.width, clamped.height)
    } else {
        (target_x, target_y, target_w as u32, target_h as u32)
    };

    // 7. Move/resize window via backend
    let target_id = target::resolve_target_id(target_win);

    backend
        .move_resize_window(&target_id, final_x, final_y, final_w, final_h)
        .await?;

    apply_focus_policy(backend, &target_id, &args.focus_modifiers).await?;

    Ok(())
}
