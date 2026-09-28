use crate::geometry::{clamp_to_monitor, Rect, WindowInfo};
use std::error::Error;
use std::time::Duration;
use zbus::Connection;

#[zbus::proxy(
    interface = "org.gnome.Shell.Extensions.Windows",
    default_service = "org.gnome.Shell",
    default_path = "/org/gnome/Shell/Extensions/Windows"
)]
pub trait GnomeWindows {
    fn get_coordinates(&self) -> zbus::Result<(i32, i32)>;
    fn list(&self) -> zbus::Result<String>;
    fn get_frame_rect(&self, winid: u32) -> zbus::Result<String>;
    fn move_(&self, winid: u32, x: i32, y: i32) -> zbus::Result<()>;
    fn move_resize(&self, winid: u32, x: i32, y: i32, width: u32, height: u32) -> zbus::Result<()>;
    fn unmaximize(&self, winid: u32) -> zbus::Result<()>;
}

pub struct GnomeDriver {
    proxy: GnomeWindowsProxy<'static>,
}

impl GnomeDriver {
    pub async fn new() -> Result<Self, Box<dyn Error>> {
        let conn = Connection::session().await?;
        let proxy = GnomeWindowsProxy::new(&conn).await?;
        Ok(Self { proxy })
    }

    pub async fn get_cursor(&self) -> Result<(i32, i32), Box<dyn Error>> {
        let (x, y) = self.proxy.get_coordinates().await?;
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

        for monitor in &mut monitors {
            if monitor.y == 0 {
                monitor.y = 36;
                monitor.height -= 36;
            }
        }

        if monitors.is_empty() {
            monitors.push(Rect {
                x: 0,
                y: 36,
                width: 9999,
                height: 9999,
            });
        }
        Ok(monitors)
    }

    pub async fn move_window(
        &self,
        win_id: &str,
        target_x: i32,
        target_y: i32,
        target_w: Option<i32>,
        target_h: Option<i32>,
        monitor: &Rect,
        bound_top: i32,
        bound_bottom: i32,
        bound_left: i32,
        bound_right: i32,
        global_margin: i32,
    ) -> Result<(), Box<dyn Error>> {
        let win_id_u32 = win_id.parse::<u32>().unwrap_or_default();
        if win_id_u32 == 0 {
            return Ok(());
        }

        // 1. Phase 1: Pre-Calculated Initial Strike
        self.proxy.unmaximize(win_id_u32).await.ok();

        let init_w = target_w.unwrap_or(0);
        let init_h = target_h.unwrap_or(0);
        let (clamp_x, clamp_y) = clamp_to_monitor(
            target_x,
            target_y,
            init_w,
            init_h,
            monitor,
            bound_top,
            bound_bottom,
            bound_left,
            bound_right,
            global_margin,
        );

        if let (Some(w), Some(h)) = (target_w, target_h) {
            if w > 0 && h > 0 {
                self.proxy
                    .move_resize(win_id_u32, clamp_x, clamp_y, w as u32, h as u32)
                    .await
                    .ok();
            } else {
                self.proxy.move_(win_id_u32, clamp_x, clamp_y).await.ok();
            }
        } else {
            self.proxy.move_(win_id_u32, clamp_x, clamp_y).await.ok();
        }

        // 2. Phase 2 & 3: Reality-Based Clamping and Ceasefire (30 iterations, 30ms interval)
        for _ in 0..30 {
            tokio::time::sleep(Duration::from_millis(30)).await;

            let rect = self.get_frame_rect(win_id_u32).await.ok();
            let curr_w = rect.as_ref().map(|r| r.width).unwrap_or(0);
            let curr_h = rect.as_ref().map(|r| r.height).unwrap_or(0);

            let use_w = if curr_w > 0 { curr_w } else { init_w };
            let use_h = if curr_h > 0 { curr_h } else { init_h };

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

            // Only move, do not resize again
            self.proxy.move_(win_id_u32, dyn_x, dyn_y).await.ok();

            if let Some(r) = rect {
                if (r.x - dyn_x).abs() <= 50 && (r.y - dyn_y).abs() <= 50 {
                    break;
                }
            }
        }

        Ok(())
    }

    async fn get_frame_rect(&self, win_id: u32) -> Result<Rect, Box<dyn Error>> {
        let body_str = self.proxy.get_frame_rect(win_id).await?;
        let parsed: serde_json::Value = serde_json::from_str(&body_str)?;
        if let Some(obj) = parsed.as_object() {
            let x = obj.get("x").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
            let y = obj.get("y").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
            let w = obj.get("width").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
            let h = obj.get("height").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
            return Ok(Rect {
                x,
                y,
                width: w,
                height: h,
            });
        }
        Err("Failed to parse get_frame_rect JSON".into())
    }

    pub async fn get_windows(&self) -> Result<Vec<WindowInfo>, Box<dyn Error>> {
        let json_str = self.proxy.list().await?;
        let json_val: serde_json::Value = serde_json::from_str(&json_str)?;
        let mut windows = Vec::new();
        if let Some(arr) = json_val.as_array() {
            for item in arr {
                let id = item
                    .get("id")
                    .and_then(|v| v.as_u64())
                    .map(|id| id.to_string())
                    .unwrap_or_default();
                let wm_class = item
                    .get("wm_class")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let title = item
                    .get("title")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                if !id.is_empty() {
                    windows.push(WindowInfo {
                        id,
                        wm_class,
                        title,
                    });
                }
            }
        }
        Ok(windows)
    }
}
