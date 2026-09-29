pub mod xdg;

use crate::geometry::Rect;
use std::process::Command;

/// Queries available display monitor bounding boxes via xrandr.
pub fn query_xrandr_monitors() -> Vec<Rect> {
    let mut monitors = Vec::new();
    if let Ok(output) = Command::new("xrandr").output() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            if line.contains(" connected ") {
                if let Some(geom) = line
                    .split_whitespace()
                    .find(|s| s.contains('x') && s.contains('+'))
                {
                    let parts: Vec<&str> = geom.split(&['x', '+'][..]).collect();
                    if parts.len() >= 4 {
                        if let (Ok(w), Ok(h), Ok(x), Ok(y)) = (
                            parts[0].parse::<i32>(),
                            parts[1].parse::<i32>(),
                            parts[2].parse::<i32>(),
                            parts[3].parse::<i32>(),
                        ) {
                            monitors.push(Rect {
                                x,
                                y,
                                width: w,
                                height: h,
                            });
                        }
                    }
                }
            }
        }
    }
    monitors
}

/// Queries current pointer coordinates using xdotool (fallback for X11 / XWayland).
pub fn query_xdotool_cursor() -> Option<(i32, i32)> {
    if let Ok(output) = Command::new("xdotool")
        .args(["getmouselocation", "--shell"])
        .output()
    {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let mut x = None;
            let mut y = None;
            for line in stdout.lines() {
                if let Some(val) = line.strip_prefix("X=") {
                    x = val.parse::<i32>().ok();
                }
                if let Some(val) = line.strip_prefix("Y=") {
                    y = val.parse::<i32>().ok();
                }
            }
            if let (Some(x), Some(y)) = (x, y) {
                return Some((x, y));
            }
        }
    }
    None
}
