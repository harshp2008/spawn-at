//! Display refresh rate, read-only: `xrandr --query` on X11, Mutter's `GetCurrentState` over
//! `gdbus` on Wayland, 60 Hz (said so) when neither gives one. It sets the frame period the flash
//! visibility label is computed for; it changes nothing on the system.

use regex::Regex;
use std::process::Command;
use std::sync::OnceLock;

/// A refresh rate and where it came from.
#[derive(Debug, Clone, PartialEq)]
pub struct Refresh {
    /// Refresh rate in Hz.
    pub hz: f64,
    /// Where the rate was read (`xrandr`, `mutter-displayconfig`) or why 60 Hz was assumed.
    pub source: String,
    /// True when `hz` is the 60 Hz fallback, not a reading.
    pub assumed: bool,
}

/// The current mode's rate of the primary output in `xrandr --query` output, else of the first
/// output that has a current mode. A mode line marks its current rate with `*`
/// (`1920x1080     60.00*+  60.00 +  40.00`).
pub fn parse_xrandr(text: &str) -> Option<f64> {
    let mut primary: Option<f64> = None;
    let mut first: Option<f64> = None;
    let mut in_primary = false;
    let mut seen_for_output = false;
    for line in text.lines() {
        if !line.starts_with(' ') && line.contains(" connected") {
            in_primary = line.contains(" connected primary");
            seen_for_output = false;
            continue;
        }
        if seen_for_output || !line.starts_with(' ') {
            continue;
        }
        let rate = line
            .split_whitespace()
            .skip(1)
            .find(|t| t.contains('*'))
            .and_then(|t| t.trim_end_matches(['*', '+']).parse::<f64>().ok());
        if let Some(hz) = rate {
            seen_for_output = true;
            first.get_or_insert(hz);
            if in_primary {
                primary.get_or_insert(hz);
            }
        }
    }
    primary.or(first)
}

/// The current mode's rate in the first monitor of `GetCurrentState` output: the mode tuple
/// `('1920x1080@60.000', 1920, 1080, 60.0, 1.0, [1.0, 2.0], {'is-current': <true>, ...})`.
pub fn parse_display_config(text: &str) -> Option<f64> {
    let re = Regex::new(
        r"\('[^']*', \d+, \d+, ([0-9.]+), [0-9.]+, \[[^\]]*\], \{[^}]*'is-current': <true>",
    )
    .ok()?;
    re.captures(text)?.get(1)?.as_str().parse().ok()
}

/// Reads the refresh rate for `session_type` ("x11", "wayland", anything else falls back).
pub fn read_refresh(session_type: &str) -> Refresh {
    let attempt = |what: &str, cmd: &mut Command, parse: fn(&str) -> Option<f64>| match cmd.output()
    {
        Ok(o) if o.status.success() => parse(&String::from_utf8_lossy(&o.stdout))
            .ok_or_else(|| format!("{} output has no current mode", what)),
        Ok(o) => Err(format!("{} exited with {:?}", what, o.status.code())),
        Err(e) => Err(format!("{} could not be run: {}", what, e)),
    };
    let (what, result) = match session_type {
        "x11" => (
            "xrandr",
            attempt(
                "xrandr",
                Command::new("xrandr").arg("--query"),
                parse_xrandr,
            ),
        ),
        "wayland" => (
            "mutter-displayconfig",
            attempt(
                "gdbus GetCurrentState",
                Command::new("gdbus").args([
                    "call",
                    "--session",
                    "--dest",
                    "org.gnome.Mutter.DisplayConfig",
                    "--object-path",
                    "/org/gnome/Mutter/DisplayConfig",
                    "--method",
                    "org.gnome.Mutter.DisplayConfig.GetCurrentState",
                ]),
                parse_display_config,
            ),
        ),
        other => (
            "none",
            Err(format!("session type '{}' has no reader", other)),
        ),
    };
    match result {
        Ok(hz) if hz > 1.0 => Refresh {
            hz,
            source: what.to_string(),
            assumed: false,
        },
        Ok(hz) => Refresh {
            hz: 60.0,
            source: format!("{} gave an implausible {} Hz; 60 Hz assumed", what, hz),
            assumed: true,
        },
        Err(why) => Refresh {
            hz: 60.0,
            source: format!("{}; 60 Hz assumed", why),
            assumed: true,
        },
    }
}

/// The refresh rate of this process's session, read once from `XDG_SESSION_TYPE`.
pub fn refresh() -> &'static Refresh {
    static R: OnceLock<Refresh> = OnceLock::new();
    R.get_or_init(|| read_refresh(&std::env::var("XDG_SESSION_TYPE").unwrap_or_default()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `xrandr --query` of this laptop (trimmed): the `*` is on the 60.00 rate of eDP-1.
    #[test]
    fn test_parse_xrandr_reads_the_starred_rate() {
        let text = [
            "Screen 0: minimum 320 x 200, current 1920 x 1080, maximum 16384 x 16384",
            "eDP-1 connected primary 1920x1080+0+0 (normal left inverted right x axis y axis) 344mm x 194mm",
            "   1920x1080     60.00*+  60.00 +  60.00    40.00  ",
            "   1680x1050     60.00  ",
        ]
        .join("\n");
        assert_eq!(parse_xrandr(&text), Some(60.0));
    }

    /// With two outputs the primary one decides, whichever is listed first; a mode line of an
    /// output that is not connected (no `*`) is ignored.
    #[test]
    fn test_parse_xrandr_prefers_the_primary_output() {
        let text = [
            "HDMI-1 connected 1920x1080+1920+0 (normal) 600mm x 340mm",
            "   1920x1080     60.00*+",
            "DP-1 connected primary 2560x1440+0+0 (normal) 600mm x 340mm",
            "   2560x1440    143.91*+ 120.00",
            "VGA-1 disconnected (normal)",
        ]
        .join("\n");
        assert_eq!(parse_xrandr(&text), Some(143.91));
        assert_eq!(parse_xrandr("Screen 0: minimum\n"), None);
    }

    /// Real `GetCurrentState` output of this machine (trimmed to two modes): the mode flagged
    /// `is-current` gives the rate, not the first one listed.
    #[test]
    fn test_parse_display_config_reads_the_current_mode() {
        let text = "(uint32 2, [(('eDP-1', 'AUO', '0x6db2', '0x00000000'), \
[('1920x1080@40.000', 1920, 1080, 40.0, 1.0, [1.0, 2.0], {}), \
('1920x1080@144.000', 1920, 1080, 143.998, 1.0, [1.0, 2.0], {'is-current': <true>, 'is-preferred': <true>})], {'is-builtin': <true>})], [])";
        assert_eq!(parse_display_config(text), Some(143.998));
        assert_eq!(parse_display_config("(uint32 2, [], [])"), None);
    }

    /// An unknown session type has no reader: 60 Hz, flagged as assumed, with the reason.
    #[test]
    fn test_read_refresh_falls_back_to_60_and_says_so() {
        let r = read_refresh("tty");
        assert_eq!(r.hz, 60.0);
        assert!(r.assumed);
        assert!(r.source.contains("60 Hz assumed"), "{}", r.source);
    }
}
