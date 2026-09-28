use crate::geometry::{clamp_to_monitor, Rect, WindowInfo};
use std::error::Error;
use std::time::Duration;

pub struct X11Driver;

impl X11Driver {
    pub fn new() -> Self {
        Self
    }

    pub async fn get_cursor(&self) -> Result<(i32, i32), Box<dyn Error>> {
        let output = tokio::process::Command::new("xdotool")
            .args(["getmouselocation", "--shell"])
            .output()
            .await?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut x = 0;
        let mut y = 0;
        for line in stdout.lines() {
            if let Some(val) = line.strip_prefix("X=") {
                x = val.parse().unwrap_or(0);
            }
            if let Some(val) = line.strip_prefix("Y=") {
                y = val.parse().unwrap_or(0);
            }
        }
        Ok((x, y))
    }

    pub async fn get_monitors(&self) -> Result<Vec<Rect>, Box<dyn Error>> {
        let output = tokio::process::Command::new("xrandr").output().await?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut monitors = Vec::new();

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

        if monitors.is_empty() {
            monitors.push(Rect {
                x: 0,
                y: 0,
                width: 9999,
                height: 9999,
            });
        }
        Ok(monitors)
    }

    pub async fn get_windows(&self) -> Result<Vec<WindowInfo>, Box<dyn Error>> {
        let output = tokio::process::Command::new("xdotool")
            .args(["search", "--all", "--onlyvisible", "."])
            .output()
            .await?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let windows = stdout
            .lines()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .map(|id| WindowInfo {
                id,
                wm_class: String::new(),
                title: String::new(),
            })
            .collect();
        Ok(windows)
    }

    pub async fn move_window(
        &self,
        win_id: &str,
        target_x: i32,
        target_y: i32,
        w: Option<i32>,
        h: Option<i32>,
        monitor: &Rect,
        bound_top: i32,
        bound_bottom: i32,
        bound_left: i32,
        bound_right: i32,
        global_margin: i32,
    ) -> Result<(), Box<dyn Error>> {
        // 1. Immediate blind-fire placement
        let (init_x, init_y) = clamp_to_monitor(
            target_x,
            target_y,
            w.unwrap_or(0),
            h.unwrap_or(0),
            monitor,
            bound_top,
            bound_bottom,
            bound_left,
            bound_right,
            global_margin,
        );

        if let (Some(width), Some(height)) = (w, h) {
            let mut cmd2 = tokio::process::Command::new("xdotool");
            cmd2.arg("windowsize")
                .arg(win_id)
                .arg(width.to_string())
                .arg(height.to_string());
            let _ = cmd2.output().await;
        }

        let mut cmd = tokio::process::Command::new("xdotool");
        cmd.arg("windowmove")
            .arg(win_id)
            .arg(init_x.to_string())
            .arg(init_y.to_string());
        let _ = cmd.output().await;

        // 2. Stabilization Loop (30 iterations)
        for _ in 0..30 {
            tokio::time::sleep(Duration::from_millis(40)).await;

            let rect = self.get_frame_rect(win_id).await.ok();
            let curr_w = rect.as_ref().map(|r| r.width).unwrap_or(0);
            let curr_h = rect.as_ref().map(|r| r.height).unwrap_or(0);

            let (use_w, use_h) = if curr_w > 0 && curr_h > 0 {
                (curr_w, curr_h)
            } else {
                (w.unwrap_or(0), h.unwrap_or(0))
            };

            let (dyn_x, dyn_y) = clamp_to_monitor(
                target_x,
                target_y,
                use_w,
                use_h,
                monitor,
                bound_top,
                bound_bottom,
                bound_left,
                bound_right,
                global_margin,
            );

            if let (Some(width), Some(height)) = (w, h) {
                let mut cmd2 = tokio::process::Command::new("xdotool");
                cmd2.arg("windowsize")
                    .arg(win_id)
                    .arg(width.to_string())
                    .arg(height.to_string());
                let _ = cmd2.output().await;
            }

            let mut cmd = tokio::process::Command::new("xdotool");
            cmd.arg("windowmove")
                .arg(win_id)
                .arg(dyn_x.to_string())
                .arg(dyn_y.to_string());
            let _ = cmd.output().await;

            if let Some(r) = rect {
                let pos_matched = (r.x - dyn_x).abs() <= 50 && (r.y - dyn_y).abs() <= 50;
                let size_matched = match (w, h) {
                    (Some(width), Some(height)) => {
                        (r.width - width).abs() <= 50 && (r.height - height).abs() <= 50
                    }
                    _ => true,
                };

                if pos_matched && size_matched {
                    break;
                }
            }
        }

        let mut cmd3 = tokio::process::Command::new("xdotool");
        cmd3.arg("windowactivate").arg("--sync").arg(win_id);
        let _ = cmd3.output().await;

        Ok(())
    }

    async fn get_frame_rect(&self, win_id: &str) -> Result<Rect, Box<dyn Error>> {
        let output = tokio::process::Command::new("xdotool")
            .args(["getwindowgeometry", "--shell", win_id])
            .output()
            .await?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut x = 0;
        let mut y = 0;
        let mut width = 0;
        let mut height = 0;
        for line in stdout.lines() {
            if let Some(val) = line.strip_prefix("X=") {
                x = val.parse().unwrap_or(0);
            } else if let Some(val) = line.strip_prefix("Y=") {
                y = val.parse().unwrap_or(0);
            } else if let Some(val) = line.strip_prefix("WIDTH=") {
                width = val.parse().unwrap_or(0);
            } else if let Some(val) = line.strip_prefix("HEIGHT=") {
                height = val.parse().unwrap_or(0);
            }
        }
        Ok(Rect {
            x,
            y,
            width,
            height,
        })
    }
}
