//! Standalone GUI test fixture window application (`winit` + `softbuffer`).

#[path = "fixture/mod.rs"]
mod fixture;

use clap::{Parser, ValueEnum};
use fixture::window::run_window;

/// Window backend protocol selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Protocol {
    /// Automatically detect based on WAYLAND_DISPLAY and DISPLAY environment variables.
    Auto,
    /// Force Wayland protocol.
    Wayland,
    /// Force X11 protocol.
    X11,
}

/// Window server-side decorations mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum DecorationMode {
    /// Enable window decorations.
    On,
    /// Disable window decorations for 1:1 frame-to-content placement parity.
    Off,
}

/// Command-line arguments for the verification fixture application.
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

    /// Hostile app: sleep this many ms before painting at a new size, so a resize is answered late
    #[arg(long, default_value_t = 0)]
    pub resize_delay_ms: u64,

    /// Hostile app: minimum window size WxH sent to the compositor (sets both minimums)
    #[arg(long, value_name = "WxH", value_parser = fixture::behaviour::parse_size)]
    pub min_size: Option<(u32, u32)>,

    /// Hostile app: maximum window size WxH sent to the compositor
    #[arg(long, value_name = "WxH", value_parser = fixture::behaviour::parse_size)]
    pub max_size: Option<(u32, u32)>,

    /// Hostile app: paint buffers rounded down to a multiple of WxH (a terminal's cell grid)
    #[arg(long, value_name = "WxH", value_parser = fixture::behaviour::parse_size)]
    pub resize_increment: Option<(u32, u32)>,

    /// Hostile app: not resizable (minimum = maximum = the initial size)
    #[arg(long, default_value_t = false)]
    pub fixed_size: bool,

    /// Hostile app: block the event loop this many ms right after the first paint (under 4000)
    #[arg(long, value_name = "MS", value_parser = fixture::behaviour::parse_stall_ms)]
    pub stall_after_map_ms: Option<u64>,

    /// Hostile app: after the first resize, alternate between two buffer sizes every 20 ms for this many ms
    #[arg(long, value_name = "MS")]
    pub flap_ms: Option<u64>,

    /// Test-only: paint every buffer as one solid loud colour that names its state (magenta at the
    /// initial size, green at any other size) so a pixel probe has ground truth. Off by default.
    #[arg(long, default_value_t = false)]
    pub state_colors: bool,
}

impl FixtureArgs {
    /// The hostile behaviours these arguments switch on.
    pub fn hostility(&self) -> fixture::behaviour::Hostility {
        fixture::behaviour::Hostility {
            resize_delay_ms: self.resize_delay_ms,
            resize_increment: self.resize_increment,
            stall_after_map_ms: self.stall_after_map_ms,
            flap_ms: self.flap_ms,
        }
    }

    /// The effective minimum window size: `--min-size` wins over the single-axis flags, and a
    /// missing axis is 0.
    pub fn effective_min(&self) -> Option<(u32, u32)> {
        match (self.min_size, self.min_width, self.min_height) {
            (Some(s), _, _) => Some(s),
            (None, None, None) => None,
            (None, w, h) => Some((w.unwrap_or(0), h.unwrap_or(0))),
        }
    }
}

/// Resolves the effective windowing protocol string based on requested mode and environment.
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

/// Entry point for the verification fixture binary.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = FixtureArgs::parse();
    fixture::trace::init();
    if args.state_colors {
        fixture::renderer::enable_state_colors((args.width, args.height));
    }

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

    /// Tests that auto protocol resolution prefers Wayland when WAYLAND_DISPLAY is available.
    #[test]
    fn test_resolve_backend_auto_prefers_wayland() {
        assert_eq!(
            resolve_backend(Protocol::Auto, Some("wayland-0"), Some(":0")).unwrap(),
            "wayland"
        );
    }

    /// Tests that auto protocol resolution falls back to X11 when only DISPLAY is available.
    #[test]
    fn test_resolve_backend_auto_falls_back_to_x11() {
        assert_eq!(
            resolve_backend(Protocol::Auto, None, Some(":0")).unwrap(),
            "x11"
        );
    }

    /// Tests that auto protocol resolution fails with an error when neither display variable is set.
    #[test]
    fn test_resolve_backend_auto_fails_without_display() {
        assert!(resolve_backend(Protocol::Auto, None, None).is_err());
    }

    /// Tests explicit Wayland protocol resolution requiring WAYLAND_DISPLAY.
    #[test]
    fn test_resolve_backend_explicit_wayland() {
        assert_eq!(
            resolve_backend(Protocol::Wayland, Some("wayland-0"), None).unwrap(),
            "wayland"
        );
        assert!(resolve_backend(Protocol::Wayland, None, Some(":0")).is_err());
    }

    /// Tests explicit X11 protocol resolution requiring DISPLAY.
    #[test]
    fn test_resolve_backend_explicit_x11() {
        assert_eq!(
            resolve_backend(Protocol::X11, None, Some(":0")).unwrap(),
            "x11"
        );
        assert!(resolve_backend(Protocol::X11, Some("wayland-0"), None).is_err());
    }

    /// Tests CLI argument parsing default values.
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

    /// Tests CLI argument parsing with custom flags.
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

#[cfg(test)]
mod hostile_flag_tests {
    use super::*;
    use clap::Parser;

    /// Without the hostile flags nothing is switched on and the old minimums read as before.
    #[test]
    fn test_hostile_flags_default_to_off() {
        let a = FixtureArgs::try_parse_from(["fixture"]).unwrap();
        assert!(a.hostility().is_default());
        assert_eq!(
            (a.min_size, a.max_size, a.resize_increment),
            (None, None, None)
        );
        assert!(!a.fixed_size);
        assert_eq!(a.effective_min(), None);
        let a = FixtureArgs::try_parse_from(["fixture", "--min-width", "300"]).unwrap();
        assert_eq!(a.effective_min(), Some((300, 0)));
    }

    /// Every hostile flag is read into the plan or the window hints.
    #[test]
    fn test_hostile_flags_are_parsed() {
        let a = FixtureArgs::try_parse_from([
            "fixture",
            "--resize-delay-ms",
            "400",
            "--min-size",
            "400x300",
            "--max-size",
            "600x450",
            "--resize-increment",
            "37x41",
            "--fixed-size",
            "--stall-after-map-ms",
            "2500",
            "--flap-ms",
            "2000",
        ])
        .unwrap();
        let h = a.hostility();
        assert_eq!(h.resize_delay_ms, 400);
        assert_eq!(h.resize_increment, Some((37, 41)));
        assert_eq!(h.stall_after_map_ms, Some(2500));
        assert_eq!(h.flap_ms, Some(2000));
        assert_eq!(a.effective_min(), Some((400, 300)));
        assert_eq!(a.max_size, Some((600, 450)));
        assert!(a.fixed_size);
    }

    /// `--state-colors` is off by default and a plain switch.
    #[test]
    fn test_state_colors_flag() {
        let off = FixtureArgs::try_parse_from(["fixture"]).unwrap();
        assert!(!off.state_colors);
        let on = FixtureArgs::try_parse_from(["fixture", "--state-colors"]).unwrap();
        assert!(on.state_colors);
    }

    /// `--min-size` beats the single-axis flags; a malformed size or a stall of 4000 ms or more is refused.
    #[test]
    fn test_hostile_flags_validate() {
        let a =
            FixtureArgs::try_parse_from(["fixture", "--min-width", "10", "--min-size", "300x200"])
                .unwrap();
        assert_eq!(a.effective_min(), Some((300, 200)));
        for bad in [
            vec!["--min-size", "300"],
            vec!["--max-size", "0x100"],
            vec!["--resize-increment", "axb"],
            vec!["--stall-after-map-ms", "4000"],
        ] {
            let mut argv = vec!["fixture"];
            argv.extend(bad.iter());
            assert!(FixtureArgs::try_parse_from(argv).is_err(), "{bad:?}");
        }
    }
}
