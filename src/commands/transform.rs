//! # Window Transformation Command Handler
//!
//! Provides the mathematical resolution and dispatch logic for moving, resizing,
//! anchoring, pivoting, and clamping active mapped windows.

use crate::cli::TransformArgs;
use crate::commands::focus::apply_focus_policy;
use crate::platform::capability::HelperNeed;
use crate::platform::{CompositorBackend, DriverError, PlacementParams};
use crate::target;
use spawn_at_core::geometry;

/// Executes the `transform` subcommand: resolves window coordinates and transforms target geometry.
pub async fn run_transform(
    backend: &dyn CompositorBackend,
    args: TransformArgs,
    no_wait: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    if !backend.capabilities().runtime_transform {
        return Err(DriverError::UnsupportedCapability(
            "Runtime window transformation is not supported by the active compositor driver",
        )
        .into());
    }

    args.validate()?;

    // 1. Query active windows via backend and resolve target window using TargetResolver
    let windows = backend.get_windows().await?;

    let selector = target::WindowSelector {
        id: args.id,
        class: args.class,
        title: args.title,
        pid: args.pid,
        focused: args.focused,
    };
    let target_win = target::resolve_target(&windows, &selector)?;

    // 2. Protocol version check and boundary resolution
    if args.geometry.area == geometry::Area::Screen {
        let need = HelperNeed::ScreenArea;
        if let Some(message) = backend.helper_status(need).await.message(need) {
            return Err(message.into());
        }
    }

    // 3. Resolve target boundary from --monitor (or window's current monitor if omitted)
    let is_cursor_anchor = args.geometry.anchor == Some(geometry::Anchor::Cursor);
    let is_cursor_monitor = args
        .geometry
        .monitor
        .as_deref()
        .is_some_and(|m| m.eq_ignore_ascii_case("cursor"));
    let needs_cursor = is_cursor_anchor || is_cursor_monitor;
    let (cursor_x, cursor_y) = if needs_cursor {
        backend.get_cursor_position().await?
    } else {
        (0, 0)
    };

    let is_unbounded = args.geometry.area == geometry::Area::Unbounded;

    let target_boundary = if is_unbounded {
        geometry::Rect::default()
    } else if let Some(ref mon_sel) = args.geometry.monitor {
        let boundary_rects = if args.geometry.area == geometry::Area::Screen {
            backend.get_monitors().await?
        } else {
            backend.get_workareas().await?
        };
        geometry::resolve_workarea(&boundary_rects, (cursor_x, cursor_y), mon_sel)?
    } else {
        let layout_res = backend.get_layout().await;
        if let Ok(layout) = layout_res {
            let win_rect = geometry::Rect {
                x: target_win.x,
                y: target_win.y,
                width: target_win.w as u32,
                height: target_win.h as u32,
            };
            let chosen = geometry::find_monitor_with_largest_overlap(&layout, win_rect)
                .or_else(|| {
                    let idx = target_win.monitor.unwrap_or(0).max(0) as usize;
                    layout.monitors.iter().find(|m| m.index == idx)
                })
                .or_else(|| layout.monitors.first());

            if let Some(m) = chosen {
                if args.geometry.area == geometry::Area::Screen {
                    m.screen.map(geometry::Rect::from).unwrap_or_default()
                } else {
                    m.workarea.map(geometry::Rect::from).unwrap_or_default()
                }
            } else {
                geometry::Rect::default()
            }
        } else {
            let boundary_rects = if args.geometry.area == geometry::Area::Screen {
                backend.get_monitors().await?
            } else {
                backend.get_workareas().await?
            };
            let mon_idx = target_win.monitor.unwrap_or(0).max(0) as usize;
            if mon_idx < boundary_rects.len() {
                boundary_rects[mon_idx]
            } else {
                boundary_rects.first().copied().unwrap_or_default()
            }
        }
    };

    // 4. Calculate target position and size via geometry solver
    let target_w = args
        .geometry
        .size
        .as_ref()
        .and_then(|s| s.first().and_then(|v| v.parse::<u32>().ok()))
        .unwrap_or(target_win.w as u32);
    let target_h = args
        .geometry
        .size
        .as_ref()
        .and_then(|s| s.get(1).and_then(|v| v.parse::<u32>().ok()))
        .unwrap_or(target_win.h as u32);

    let pos_or_offset = args.geometry.pos_or_offset();
    let has_anchor = args.geometry.anchor.is_some();
    let has_pos_or_offset = pos_or_offset.is_some();

    let mt = if is_unbounded {
        0
    } else {
        args.geometry.margin_top.unwrap_or(args.geometry.margin())
    };
    let mb = if is_unbounded {
        0
    } else {
        args.geometry
            .margin_bottom
            .unwrap_or(args.geometry.margin())
    };
    let ml = if is_unbounded {
        0
    } else {
        args.geometry.margin_left.unwrap_or(args.geometry.margin())
    };
    let mr = if is_unbounded {
        0
    } else {
        args.geometry.margin_right.unwrap_or(args.geometry.margin())
    };

    let (pos, offset, anchor, pivot) = if has_anchor {
        (
            None,
            pos_or_offset,
            args.geometry.anchor,
            args.geometry.pivot,
        )
    } else if has_pos_or_offset {
        (
            pos_or_offset,
            None,
            None,
            args.geometry.pivot.or(Some(geometry::Pivot::TopLeft)),
        )
    } else if let Some(p) = args.geometry.pivot {
        let (px, py) = match p {
            geometry::Pivot::TopLeft => (target_win.x, target_win.y),
            geometry::Pivot::TopRight => (target_win.x + target_win.w, target_win.y),
            geometry::Pivot::BottomLeft => (target_win.x, target_win.y + target_win.h),
            geometry::Pivot::BottomRight => {
                (target_win.x + target_win.w, target_win.y + target_win.h)
            }
            geometry::Pivot::Center => (
                target_win.x + target_win.w / 2,
                target_win.y + target_win.h / 2,
            ),
        };
        let (unclamped_x, unclamped_y) = geometry::apply_pivot(px, py, target_w, target_h, p);
        let (final_x, final_y) = if !is_unbounded && args.geometry.clamp() {
            let raw_rect = geometry::Rect {
                x: unclamped_x,
                y: unclamped_y,
                width: target_w,
                height: target_h,
            };
            let clamped = geometry::clamp_to_bounds(raw_rect, target_boundary, mt, mb, ml, mr, 0);
            (clamped.x, clamped.y)
        } else {
            (unclamped_x, unclamped_y)
        };
        if final_x != target_win.x || final_y != target_win.y {
            let ref_x = if is_unbounded { 0 } else { target_boundary.x };
            let ref_y = if is_unbounded { 0 } else { target_boundary.y };
            (
                Some((final_x - ref_x, final_y - ref_y)),
                None,
                None,
                Some(geometry::Pivot::TopLeft),
            )
        } else {
            (None, None, None, None)
        }
    } else {
        if args.geometry.size.is_some() && !is_unbounded && args.geometry.clamp() {
            let raw_rect = geometry::Rect {
                x: target_win.x,
                y: target_win.y,
                width: target_w,
                height: target_h,
            };
            let clamped = geometry::clamp_to_bounds(raw_rect, target_boundary, mt, mb, ml, mr, 0);
            if clamped.x != target_win.x || clamped.y != target_win.y {
                let ref_x = target_boundary.x;
                let ref_y = target_boundary.y;
                (
                    Some((clamped.x - ref_x, clamped.y - ref_y)),
                    None,
                    None,
                    Some(geometry::Pivot::TopLeft),
                )
            } else {
                (None, None, None, None)
            }
        } else {
            (None, None, None, None)
        }
    };

    let cursor_pos = if needs_cursor {
        Some((cursor_x, cursor_y))
    } else {
        None
    };

    let params = PlacementParams {
        pos,
        offset,
        anchor,
        pivot,
        size: if args.geometry.size.is_some() {
            Some((target_w, target_h))
        } else {
            None
        },
        margin: if is_unbounded {
            0
        } else {
            args.geometry.margin()
        },
        margin_top: if is_unbounded {
            None
        } else {
            args.geometry.margin_top
        },
        margin_bottom: if is_unbounded {
            None
        } else {
            args.geometry.margin_bottom
        },
        margin_left: if is_unbounded {
            None
        } else {
            args.geometry.margin_left
        },
        margin_right: if is_unbounded {
            None
        } else {
            args.geometry.margin_right
        },
        area: Some(args.geometry.area),
        cursor_pos,
        workarea: target_boundary,
        clamp: if is_unbounded {
            false
        } else {
            args.geometry.clamp()
        },
    };

    // 5. Execute transform via backend
    let target_id = target::resolve_target_id(target_win);

    backend
        .transform_window(
            &target_id,
            params.clone(),
            target_win.w as u32,
            target_win.h as u32,
        )
        .await?;

    apply_focus_policy(backend, &target_id, &args.focus_modifiers, no_wait).await?;

    if !no_wait {
        let expected = if args.focus_modifiers.focus {
            crate::commands::ExpectedState::Focused(true)
        } else if args.focus_modifiers.defocus {
            crate::commands::ExpectedState::Focused(false)
        } else {
            let (target_x, target_y) = if params.pos.is_some() || params.anchor.is_some() {
                let exp_rect = spawn_at_core::geometry::calculate_rect_from_placement(
                    &params, target_w, target_h,
                );
                (Some(exp_rect.x), Some(exp_rect.y))
            } else {
                (None, None)
            };
            crate::commands::ExpectedState::Geometry {
                x: target_x,
                y: target_y,
                w: if args.geometry.size.is_some() {
                    Some(target_w)
                } else {
                    None
                },
                h: if args.geometry.size.is_some() {
                    Some(target_h)
                } else {
                    None
                },
            }
        };

        let _ = crate::commands::wait_for_state_change(
            backend,
            &target_id,
            expected,
            std::time::Duration::from_millis(2000),
        )
        .await;

        let actual_rect_opt = crate::commands::settle_window_geometry(
            backend,
            target_win.id,
            &target_id,
            &params,
            std::time::Duration::from_millis(300),
        )
        .await;

        if let Some(actual_rect) = actual_rect_opt {
            let (_raised_w, _raised_h, size_raised) = if let Some(req) = params.size {
                let rw = actual_rect.width > req.0;
                let rh = actual_rect.height > req.1;
                (rw, rh, rw || rh)
            } else {
                (false, false, false)
            };
            crate::diagnostics::verify_and_report_transform(
                &params,
                actual_rect,
                None,
                size_raised,
            );
            if is_unbounded {
                let monitors = backend.get_monitors().await.unwrap_or_default();
                if geometry::window_straddles_monitors(actual_rect, &monitors) {
                    crate::diagnostics::render_info("Window straddles monitors.");
                }
            }
        }
    }

    Ok(())
}
