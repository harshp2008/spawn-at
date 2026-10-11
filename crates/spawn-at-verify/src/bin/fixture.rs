//! Standalone GUI test fixture window application (`winit` + `softbuffer`).

#[path = "fixture/mod.rs"]
mod fixture;

use clap::{Parser, ValueEnum};
use fixture::window::run_window;

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

    run_window(args, backend_str)
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
