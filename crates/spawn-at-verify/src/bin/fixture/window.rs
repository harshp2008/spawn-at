//! Window creation, winit event loop, and signal handling.

use super::renderer::draw_frame;
use crate::{DecorationMode, FixtureArgs};
use std::process;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

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

    if backend_str == "x11" {
        window_builder =
            window_builder.with_inner_size(winit::dpi::PhysicalSize::new(args.width, args.height));
        if let (Some(mw), Some(mh)) = (args.min_width, args.min_height) {
            window_builder =
                window_builder.with_min_inner_size(winit::dpi::PhysicalSize::new(mw, mh));
        } else if let Some(mw) = args.min_width {
            window_builder =
                window_builder.with_min_inner_size(winit::dpi::PhysicalSize::new(mw, 0));
        } else if let Some(mh) = args.min_height {
            window_builder =
                window_builder.with_min_inner_size(winit::dpi::PhysicalSize::new(0, mh));
        }
    } else {
        window_builder =
            window_builder.with_inner_size(winit::dpi::LogicalSize::new(args.width, args.height));
        if let (Some(mw), Some(mh)) = (args.min_width, args.min_height) {
            window_builder =
                window_builder.with_min_inner_size(winit::dpi::LogicalSize::new(mw, mh));
        } else if let Some(mw) = args.min_width {
            window_builder =
                window_builder.with_min_inner_size(winit::dpi::LogicalSize::new(mw, 0));
        } else if let Some(mh) = args.min_height {
            window_builder =
                window_builder.with_min_inner_size(winit::dpi::LogicalSize::new(0, mh));
        }
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

    let mut painted = false;
    let mut ready_printed = false;

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

    event_loop.run(move |event, window_target| {
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
                if painted {
                    let _ = draw_frame(
                        &mut surface,
                        physical_size.width,
                        physical_size.height,
                        &args.title,
                        args.paint_delay_ms,
                        &args.expect,
                    );
                }
            }
            winit::event::Event::WindowEvent {
                event: winit::event::WindowEvent::RedrawRequested,
                ..
            } => {
                if painted {
                    let size = window.inner_size();
                    let _ = draw_frame(
                        &mut surface,
                        size.width,
                        size.height,
                        &args.title,
                        args.paint_delay_ms,
                        &args.expect,
                    );
                }
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
