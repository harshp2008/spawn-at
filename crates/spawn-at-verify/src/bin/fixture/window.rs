//! Window creation, winit event loop, and signal handling.

use super::renderer::draw_frame;
use crate::{DecorationMode, FixtureArgs};
use std::process;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Creates the window, sets up event loops and signal handlers, and executes the fixture runtime.
pub(crate) fn run_window(
    args: FixtureArgs,
    backend_str: &'static str,
) -> Result<(), Box<dyn std::error::Error>> {
    // Register signal handlers for clean SIGTERM and SIGINT termination
    let term_flag = Arc::new(AtomicBool::new(false));
    signal_hook::flag::register(signal_hook::consts::SIGTERM, Arc::clone(&term_flag))?;
    signal_hook::flag::register(signal_hook::consts::SIGINT, Arc::clone(&term_flag))?;

    // Construct event loop with explicit forced backend on Linux
    #[cfg(target_os = "linux")]
    let event_loop = {
        use winit::platform::wayland::EventLoopBuilderExtWayland;
        use winit::platform::x11::EventLoopBuilderExtX11;

        let mut builder = winit::event_loop::EventLoopBuilder::new();
        match backend_str {
            "wayland" => {
                builder.with_wayland();
            }
            "x11" => {
                builder.with_x11();
            }
            _ => (),
        }
        builder.build()?
    };

    #[cfg(not(target_os = "linux"))]
    let event_loop = winit::event_loop::EventLoopBuilder::new().build()?;

    let mut window_builder = winit::window::WindowBuilder::new()
        .with_title(&args.title)
        .with_decorations(args.decorations == DecorationMode::On);

    // Size hints for the compositor: the minimum (`--min-size`, or the single-axis flags with a
    // missing axis as 0), the maximum, and `--fixed-size` (not resizable).
    if backend_str == "x11" {
        window_builder =
            window_builder.with_inner_size(winit::dpi::PhysicalSize::new(args.width, args.height));
        if let Some((mw, mh)) = args.effective_min() {
            window_builder =
                window_builder.with_min_inner_size(winit::dpi::PhysicalSize::new(mw, mh));
        }
        if let Some((xw, xh)) = args.max_size {
            window_builder =
                window_builder.with_max_inner_size(winit::dpi::PhysicalSize::new(xw, xh));
        }
    } else {
        window_builder =
            window_builder.with_inner_size(winit::dpi::LogicalSize::new(args.width, args.height));
        if let Some((mw, mh)) = args.effective_min() {
            window_builder =
                window_builder.with_min_inner_size(winit::dpi::LogicalSize::new(mw, mh));
        }
        if let Some((xw, xh)) = args.max_size {
            window_builder =
                window_builder.with_max_inner_size(winit::dpi::LogicalSize::new(xw, xh));
        }
    }
    if args.fixed_size {
        window_builder = window_builder.with_resizable(false);
    }

    // Set Wayland app_id and X11 WM_CLASS
    #[cfg(target_os = "linux")]
    {
        use winit::platform::wayland::WindowBuilderExtWayland;
        use winit::platform::x11::WindowBuilderExtX11;

        if backend_str == "wayland" {
            window_builder = WindowBuilderExtWayland::with_name(window_builder, &args.app_id, "");
        } else {
            window_builder =
                WindowBuilderExtX11::with_name(window_builder, &args.app_id, &args.app_id);
        }
    }

    let window = std::rc::Rc::new(window_builder.build(&event_loop)?);
    let scale_factor = window.scale_factor();
    let context = softbuffer::Context::new(window.clone())?;
    let mut surface = softbuffer::Surface::new(&context, window.clone())?;

    let pid = process::id();
    let start_time = Instant::now();
    let paint_deadline = if args.paint_delay_ms > 0 {
        Some(start_time + Duration::from_millis(args.paint_delay_ms))
    } else {
        None
    };

    let exit_deadline = args
        .exit_after_ms
        .map(|ms| start_time + Duration::from_millis(ms));

    let mut remaximize_deadline = args
        .remaximize_after_ms
        .map(|ms| start_time + Duration::from_millis(ms));

    // Hostile-app behaviours (harness only); all off unless a flag asks (behaviour.rs).
    let hostile = args.hostility();
    let initial_size = (args.width, args.height);
    let mut stalled = false;
    // The flap: the size the compositor configured when it began, the tick it is at, the next
    // tick's time, and whether it already ran (it runs once).
    let mut flap_base: Option<(u32, u32)> = None;
    let mut flap_tick: u64 = 0;
    let mut flap_next = Instant::now();
    let mut flap_done = false;

    let mut painted = false;
    let mut ready_printed = false;
    // Size of the last buffer painted, so `FIRST_PAINT` is written once per new size.
    let mut last_painted_size: Option<(u32, u32)> = None;

    // If no paint delay, paint immediately
    if paint_deadline.is_none() {
        let size = window.inner_size();
        if let Err(e) = draw_frame(
            &mut surface,
            size.width,
            size.height,
            &args.title,
            args.paint_delay_ms,
            &args.expect,
        ) {
            eprintln!("[fixture] Frame render failed: {}", e);
        } else {
            super::trace::note_paint(&mut last_painted_size, size.width, size.height);
            painted = true;
            if args.print_ready && !ready_printed {
                println!(
                    "[fixture] READY protocol={} pid={} title=\"{}\" size={}x{} scale_factor={:.2}",
                    backend_str, pid, args.title, size.width, size.height, scale_factor
                );
                ready_printed = true;
            }
        }
    }

    super::trace::log("EVENT_LOOP_ENTER", "");
    event_loop.run(move |event, window_target| {
        if let winit::event::Event::NewEvents(cause) = &event {
            super::trace::log("NEW_EVENTS", &format!("{:?}", cause));
        }
        // Check signal termination
        if term_flag.load(Ordering::Relaxed) {
            window_target.exit();
            return;
        }

        // Check auto-exit timeout
        if let Some(deadline) = exit_deadline {
            if Instant::now() >= deadline {
                window_target.exit();
                return;
            }
        }

        // Check remaximize timeout
        if let Some(deadline) = remaximize_deadline {
            if Instant::now() >= deadline {
                window.set_maximized(true);
                remaximize_deadline = None;
            }
        }

        // Hung app: once, right after the first paint, the event loop is blocked, so every
        // configure the compositor sends meanwhile goes unanswered.
        if let Some(ms) = hostile.stall_after_map_ms {
            if painted && !stalled {
                stalled = true;
                super::trace::event("STALL_BEGIN", &ms.to_string());
                std::thread::sleep(Duration::from_millis(ms));
                super::trace::event("STALL_END", &ms.to_string());
            }
        }

        // Restless app: every FLAP_TICK_MS commit the next of two alternating buffer sizes, then
        // settle at the size the compositor configured when the flap began.
        if let Some(base) = flap_base {
            if Instant::now() >= flap_next {
                flap_tick += 1;
                let total = hostile.flap_ticks().unwrap_or(0);
                let size = if flap_tick > total {
                    flap_base = None;
                    flap_done = true;
                    base
                } else {
                    super::behaviour::Hostility::flap_size(base, flap_tick)
                };
                // winit pins the xdg window geometry to the configured size, so a different
                // buffer alone would not change the frame: ask for the window size too.
                let _ = window.request_inner_size(winit::dpi::PhysicalSize::new(size.0, size.1));
                if draw_frame(
                    &mut surface,
                    size.0,
                    size.1,
                    &args.title,
                    args.paint_delay_ms,
                    &args.expect,
                )
                .is_ok()
                {
                    super::trace::note_paint(&mut last_painted_size, size.0, size.1);
                }
                super::trace::event("FLAP_TICK", &format!("{} {}x{}", flap_tick, size.0, size.1));
                flap_next += Duration::from_millis(super::behaviour::FLAP_TICK_MS);
            }
        }

        // Check delayed paint
        if !painted {
            if let Some(deadline) = paint_deadline {
                if Instant::now() >= deadline {
                    let size = window.inner_size();
                    if let Err(e) = draw_frame(
                        &mut surface,
                        size.width,
                        size.height,
                        &args.title,
                        args.paint_delay_ms,
                        &args.expect,
                    ) {
                        eprintln!("[fixture] Delayed frame render failed: {}", e);
                    } else {
                        super::trace::note_paint(
                            &mut last_painted_size,
                            size.width,
                            size.height,
                        );
                        painted = true;
                        if args.print_ready && !ready_printed {
                            println!(
                                "[fixture] READY protocol={} pid={} title=\"{}\" size={}x{} scale_factor={:.2}",
                                backend_str, pid, args.title, size.width, size.height, scale_factor
                            );
                            ready_printed = true;
                        }
                    }
                }
            }
        }

        match event {
            winit::event::Event::WindowEvent {
                event: winit::event::WindowEvent::CloseRequested,
                ..
            } => {
                window_target.exit();
            }
            winit::event::Event::WindowEvent {
                event: winit::event::WindowEvent::Resized(physical_size),
                ..
            } => {
                super::trace::event(
                    "RESIZED",
                    &format!("{}x{}", physical_size.width, physical_size.height),
                );
                let configured = (physical_size.width, physical_size.height);
                // The first resize away from the initial size starts the flap, if asked for.
                if hostile.flap_ms.is_some()
                    && !flap_done
                    && flap_base.is_none()
                    && painted
                    && configured != initial_size
                {
                    flap_base = Some(configured);
                    flap_tick = 0;
                    flap_next = Instant::now();
                    super::trace::event(
                        "FLAP_BEGIN",
                        &format!("{}x{} {}ms", configured.0, configured.1, hostile.flap_ms.unwrap_or(0)),
                    );
                } else if painted && flap_base.is_none() {
                    let plan = hostile.plan_resize(configured, last_painted_size);
                    for (tag, detail) in &plan.events {
                        super::trace::event(tag, detail);
                    }
                    if plan.delay_ms > 0 {
                        std::thread::sleep(Duration::from_millis(plan.delay_ms));
                    }
                    if plan.draw != configured {
                        // A grid-snapping app also sets its own window size (see the flap).
                        let _ = window.request_inner_size(winit::dpi::PhysicalSize::new(
                            plan.draw.0,
                            plan.draw.1,
                        ));
                    }
                    let drawn = draw_frame(
                        &mut surface,
                        plan.draw.0,
                        plan.draw.1,
                        &args.title,
                        args.paint_delay_ms,
                        &args.expect,
                    );
                    if drawn.is_ok() {
                        super::trace::note_paint(&mut last_painted_size, plan.draw.0, plan.draw.1);
                    }
                    super::trace::log(
                        "COMMIT_DONE",
                        &format!("{}x{}", plan.draw.0, plan.draw.1),
                    );
                }
            }
            winit::event::Event::WindowEvent {
                event: winit::event::WindowEvent::RedrawRequested,
                ..
            } => {
                let size = window.inner_size();
                super::trace::log("REDRAW_REQUESTED", &format!("{}x{}", size.width, size.height));
                if painted && flap_base.is_none() {
                    // A redraw is never delayed: only a resize to a new size is slow.
                    let draw = super::behaviour::snap(
                        (size.width, size.height),
                        hostile.resize_increment,
                    );
                    if draw != (size.width, size.height) {
                        let _ = window
                            .request_inner_size(winit::dpi::PhysicalSize::new(draw.0, draw.1));
                    }
                    let drawn = draw_frame(
                        &mut surface,
                        draw.0,
                        draw.1,
                        &args.title,
                        args.paint_delay_ms,
                        &args.expect,
                    );
                    if drawn.is_ok() {
                        super::trace::note_paint(&mut last_painted_size, draw.0, draw.1);
                    }
                    super::trace::log("COMMIT_DONE", &format!("{}x{}", draw.0, draw.1));
                }
            }
            winit::event::Event::WindowEvent {
                event: winit::event::WindowEvent::Moved(position),
                ..
            } => {
                super::trace::event("MOVED", &format!("{},{}", position.x, position.y));
            }
            winit::event::Event::AboutToWait => {
                let mut next_wake = Instant::now() + Duration::from_millis(16);
                if !painted {
                    if let Some(p) = paint_deadline {
                        if p < next_wake {
                            next_wake = p;
                        }
                    }
                }
                if let Some(e) = exit_deadline {
                    if e < next_wake {
                        next_wake = e;
                    }
                }
                if flap_base.is_some() && flap_next < next_wake {
                    next_wake = flap_next;
                }
                if let Some(r) = remaximize_deadline {
                    if r < next_wake {
                        next_wake = r;
                    }
                }
                window_target
                    .set_control_flow(winit::event_loop::ControlFlow::WaitUntil(next_wake));
            }
            _ => (),
        }
    })?;

    Ok(())
}
