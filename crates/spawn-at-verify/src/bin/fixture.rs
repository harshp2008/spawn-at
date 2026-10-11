//! spawn-at-verify GUI Fixture Application
//!
//! A dedicated, lightweight GUI window client designed specifically for test verification.
//! Supports Wayland and X11 protocol forcing, client-side decoration control, delayed buffer
//! presentation for cold-start cloak testing, minimum dimension bounds, and controlled lifecycle.

use clap::{Parser, ValueEnum};
use std::num::NonZeroU32;
use std::process;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Protocol {
    Auto,
    Wayland,
    X11,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum DecorationMode {
    On,
    Off,
}

#[derive(Parser, Debug, Clone)]
#[command(name = "fixture", about = "Window verification fixture application")]
pub struct FixtureArgs {
    /// Window backend protocol
    #[arg(long, value_enum, default_value_t = Protocol::Auto)]
    pub protocol: Protocol,

    /// Window title
    #[arg(long, default_value = "spawn-at-fixture")]
    pub title: String,

    /// Application ID (Wayland app_id and X11 WM_CLASS)
    #[arg(long, default_value = "spawn-at-fixture")]
    pub app_id: String,

    /// Initial client content width
    #[arg(long, default_value_t = 400)]
    pub width: u32,

    /// Initial client content height
    #[arg(long, default_value_t = 300)]
    pub height: u32,

    /// Minimum window width (for size_raised verification)
    #[arg(long)]
    pub min_width: Option<u32>,

    /// Minimum window height (for size_raised verification)
    #[arg(long)]
    pub min_height: Option<u32>,

    /// Window frame decorations mode (default off for 1:1 frame-to-content placement parity)
    #[arg(long, value_enum, default_value_t = DecorationMode::Off)]
    pub decorations: DecorationMode,

    /// Presentation delay in milliseconds before attaching/painting first buffer
    #[arg(long, default_value_t = 0)]
    pub paint_delay_ms: u64,

    /// Trigger re-maximization after specified milliseconds (for unmaximize + re-assert tests)
    #[arg(long)]
    pub remaximize_after_ms: Option<u64>,

    /// Automatically terminate after specified milliseconds
    #[arg(long)]
    pub exit_after_ms: Option<u64>,

    /// Print structured readiness line to stdout when window is mapped/presented
    #[arg(long, default_value_t = false)]
    pub print_ready: bool,

    /// Expected visual behavior description shown on the window canvas
    #[arg(long, default_value = "")]
    pub expect: String,
}

mod font;

struct Surface<'a> {
    buffer: &'a mut [u32],
    stride: usize,
    height: usize,
}

impl<'a> Surface<'a> {
    fn draw_char(&mut self, c: char, px: usize, py: usize, scale: usize, color: u32) {
        let ascii = c as u32;
        if !(32..=126).contains(&ascii) {
            return;
        }
        let glyph = font::FONT_DATA[(ascii - 32) as usize];
        for (row, &row_bits) in glyph.iter().enumerate().take(font::FONT_HEIGHT) {
            for col in 0..font::FONT_WIDTH {
                if (row_bits >> (9 - col)) & 1 == 1 {
                    for sy in 0..scale {
                        for sx in 0..scale {
                            let x = px + col * scale + sx;
                            let y = py + row * scale + sy;
                            if x < self.stride && y < self.height {
                                self.buffer[y * self.stride + x] = color;
                            }
                        }
                    }
                }
            }
        }
    }

    fn draw_text_line(&mut self, text: &str, px: usize, py: usize, scale: usize, color: u32) {
        let char_w = font::FONT_WIDTH * scale + (scale / 2).max(1);
        for (i, c) in text.chars().enumerate() {
            let x = px + i * char_w;
            if x + char_w > self.stride {
                break;
            }
            self.draw_char(c, x, py, scale, color);
        }
    }
}

/// Resolves actual protocol and checks environment variables.
pub fn resolve_backend(
    requested: Protocol,
    wayland_display: Option<&str>,
    display: Option<&str>,
) -> Result<&'static str, String> {
    match requested {
        Protocol::Auto => {
            if wayland_display.is_some() {
                Ok("wayland")
            } else if display.is_some() {
                Ok("x11")
            } else {
                Err("No display available: neither WAYLAND_DISPLAY nor DISPLAY is set".into())
            }
        }
        Protocol::Wayland => {
            if wayland_display.is_some() {
                Ok("wayland")
            } else {
                Err("Wayland backend requested, but WAYLAND_DISPLAY is not set".into())
            }
        }
        Protocol::X11 => {
            if display.is_some() {
                Ok("x11")
            } else {
                Err("X11 backend requested, but DISPLAY is not set".into())
            }
        }
    }
}

fn draw_frame(
    surface: &mut softbuffer::Surface<
        std::rc::Rc<winit::window::Window>,
        std::rc::Rc<winit::window::Window>,
    >,
    width: u32,
    height: u32,
    title: &str,
    paint_delay_ms: u64,
    expected_text: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    if let (Some(nz_w), Some(nz_h)) = (NonZeroU32::new(width), NonZeroU32::new(height)) {
        surface.resize(nz_w, nz_h)?;
        let mut buffer = surface.buffer_mut()?;

        // Background: #1e2238 (slate dark blue)
        let bg_color = 0xFF1E2238u32;
        // Border: #38bdf8 (cyan 400)
        let border_color = 0xFF38BDF8u32;
        let border_px = 3u32;

        let w_usize = width as usize;
        let h_usize = height as usize;

        for y in 0..height {
            for x in 0..width {
                let idx = (y * width + x) as usize;
                if x < border_px
                    || x >= width.saturating_sub(border_px)
                    || y < border_px
                    || y >= height.saturating_sub(border_px)
                {
                    buffer[idx] = border_color;
                } else {
                    buffer[idx] = bg_color;
                }
            }
        }

        let mut surf = Surface {
            buffer: &mut buffer,
            stride: w_usize,
            height: h_usize,
        };

        // Title line
        let title_line = format!("Title: {}", title);
        surf.draw_text_line(&title_line, 20, 24, 1, 0xFF38BDF8);

        // Paint delay line
        let delay_line = format!("Paint Delay: {} ms", paint_delay_ms);
        surf.draw_text_line(&delay_line, 20, 52, 1, 0xFFFBBF24);

        // Expected header
        surf.draw_text_line("Expected Behavior:", 20, 84, 1, 0xFF94A3B8);

        // Expected description (word wrapped at 32 characters)
        let desc = if expected_text.is_empty() {
            "Window positioned by spawn-at"
        } else {
            expected_text
        };

        let mut cur_y = 108;
        let mut line_buf = String::new();
        for word in desc.split_whitespace() {
            if line_buf.len() + word.len() + 1 > 32 && !line_buf.is_empty() {
                surf.draw_text_line(&line_buf, 20, cur_y, 1, 0xFF4ADE80);
                cur_y += 24;
                line_buf.clear();
            }
            if !line_buf.is_empty() {
                line_buf.push(' ');
            }
            line_buf.push_str(word);
        }
        if !line_buf.is_empty() {
            surf.draw_text_line(&line_buf, 20, cur_y, 1, 0xFF4ADE80);
        }

        buffer.present()?;
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = FixtureArgs::parse();

    let wayland_display = std::env::var("WAYLAND_DISPLAY").ok();
    let display = std::env::var("DISPLAY").ok();

    let backend_str = resolve_backend(
        args.protocol,
        wayland_display.as_deref(),
        display.as_deref(),
    )
    .map_err(|e| {
        eprintln!("[fixture] Error: {}", e);
        e
    })?;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_backend_auto_prefers_wayland() {
        assert_eq!(
            resolve_backend(Protocol::Auto, Some("wayland-0"), Some(":0")).unwrap(),
            "wayland"
        );
    }

    #[test]
    fn test_resolve_backend_auto_falls_back_to_x11() {
        assert_eq!(
            resolve_backend(Protocol::Auto, None, Some(":0")).unwrap(),
            "x11"
        );
    }

    #[test]
    fn test_resolve_backend_auto_fails_without_display() {
        assert!(resolve_backend(Protocol::Auto, None, None).is_err());
    }

    #[test]
    fn test_resolve_backend_explicit_wayland() {
        assert_eq!(
            resolve_backend(Protocol::Wayland, Some("wayland-0"), None).unwrap(),
            "wayland"
        );
        assert!(resolve_backend(Protocol::Wayland, None, Some(":0")).is_err());
    }

    #[test]
    fn test_resolve_backend_explicit_x11() {
        assert_eq!(
            resolve_backend(Protocol::X11, None, Some(":0")).unwrap(),
            "x11"
        );
        assert!(resolve_backend(Protocol::X11, Some("wayland-0"), None).is_err());
    }

    #[test]
    fn test_cli_argument_parsing_defaults() {
        use clap::Parser;
        let args = FixtureArgs::try_parse_from(["fixture"]).unwrap();
        assert_eq!(args.protocol, Protocol::Auto);
        assert_eq!(args.title, "spawn-at-fixture");
        assert_eq!(args.app_id, "spawn-at-fixture");
        assert_eq!(args.width, 400);
        assert_eq!(args.height, 300);
        assert_eq!(args.decorations, DecorationMode::Off);
        assert_eq!(args.paint_delay_ms, 0);
        assert_eq!(args.min_width, None);
        assert_eq!(args.min_height, None);
        assert_eq!(args.remaximize_after_ms, None);
        assert_eq!(args.exit_after_ms, None);
        assert!(!args.print_ready);
        assert_eq!(args.expect, "");
    }

    #[test]
    fn test_cli_argument_parsing_custom_flags() {
        use clap::Parser;
        let args = FixtureArgs::try_parse_from([
            "fixture",
            "--protocol",
            "wayland",
            "--title",
            "Custom Title",
            "--app-id",
            "custom.app.id",
            "--width",
            "640",
            "--height",
            "480",
            "--min-width",
            "300",
            "--min-height",
            "200",
            "--decorations",
            "on",
            "--paint-delay-ms",
            "150",
            "--remaximize-after-ms",
            "250",
            "--exit-after-ms",
            "1000",
            "--print-ready",
            "--expect",
            "Should appear centered",
        ])
        .unwrap();

        assert_eq!(args.protocol, Protocol::Wayland);
        assert_eq!(args.title, "Custom Title");
        assert_eq!(args.app_id, "custom.app.id");
        assert_eq!(args.width, 640);
        assert_eq!(args.height, 480);
        assert_eq!(args.min_width, Some(300));
        assert_eq!(args.min_height, Some(200));
        assert_eq!(args.decorations, DecorationMode::On);
        assert_eq!(args.paint_delay_ms, 150);
        assert_eq!(args.remaximize_after_ms, Some(250));
        assert_eq!(args.exit_after_ms, Some(1000));
        assert!(args.print_ready);
        assert_eq!(args.expect, "Should appear centered");
    }
}
