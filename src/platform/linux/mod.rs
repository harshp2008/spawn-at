pub mod gnome;
pub mod x11;
pub mod xdg;

use crate::core::geometry::{Rect, TargetGeometry};
use crate::platform::{CompositorBackend, DriverError, InstallArgs, UninstallArgs, WindowState};
use crate::target::WindowMetadata;
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
                                width: w as u32,
                                height: h as u32,
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

/// Unified Linux compositor backend adapter that delegates to either GNOME or X11 driver.
pub struct LinuxBackend {
    driver: Box<dyn CompositorBackend>,
}

impl LinuxBackend {
    /// Inspects the runtime session environment and instantiates the appropriate compositor backend.
    pub async fn bootstrap() -> Result<Box<dyn CompositorBackend>, DriverError> {
        let session_type = std::env::var("XDG_SESSION_TYPE").unwrap_or_default().to_lowercase();
        let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default().to_uppercase();

        if session_type == "wayland" && desktop.contains("GNOME") {
            let driver = gnome::GnomeWaylandDriver::new().await?;
            Ok(Box::new(LinuxBackend {
                driver: Box::new(driver),
            }))
        } else {
            Ok(Box::new(LinuxBackend {
                driver: Box::new(x11::X11Driver),
            }))
        }
    }
}

#[async_trait::async_trait]
impl CompositorBackend for LinuxBackend {
    fn name(&self) -> &'static str {
        self.driver.name()
    }

    fn install(&self, args: &InstallArgs) -> Result<(), DriverError> {
        self.driver.install(args)
    }

    fn uninstall(&self, args: &UninstallArgs) -> Result<(), DriverError> {
        self.driver.uninstall(args)
    }

    fn supports_runtime_transform(&self) -> bool {
        self.driver.supports_runtime_transform()
    }

    fn resolve_id(&self, command: &[String], explicit_class: Option<&str>) -> String {
        self.driver.resolve_id(command, explicit_class)
    }

    async fn get_cursor_position(&self) -> Result<(i32, i32), DriverError> {
        self.driver.get_cursor_position().await
    }

    async fn get_monitors(&self) -> Result<Vec<Rect>, DriverError> {
        self.driver.get_monitors().await
    }

    async fn get_workareas(&self) -> Result<Vec<Rect>, DriverError> {
        self.driver.get_workareas().await
    }

    async fn get_windows(&self) -> Result<Vec<WindowMetadata>, DriverError> {
        self.driver.get_windows().await
    }

    async fn spawn_at(
        &self,
        app_id: &str,
        command: &[String],
        geom: &TargetGeometry,
    ) -> Result<(), DriverError> {
        self.driver.spawn_at(app_id, command, geom).await
    }

    async fn move_window(&self, target_id: &str, x: i32, y: i32) -> Result<(), DriverError> {
        self.driver.move_window(target_id, x, y).await
    }

    async fn move_resize_window(
        &self,
        target_id: &str,
        x: i32,
        y: i32,
        w: u32,
        h: u32,
    ) -> Result<(), DriverError> {
        self.driver.move_resize_window(target_id, x, y, w, h).await
    }

    async fn set_window_state(
        &self,
        target_id: &str,
        state: WindowState,
    ) -> Result<(), DriverError> {
        self.driver.set_window_state(target_id, state).await
    }

    async fn focus_window(&self, target_id: &str) -> Result<(), DriverError> {
        self.driver.focus_window(target_id).await
    }

    async fn defocus_window(&self, target_id: &str, to_target: &str) -> Result<(), DriverError> {
        self.driver.defocus_window(target_id, to_target).await
    }

    async fn run_daemon(&self) -> Result<(), DriverError> {
        self.driver.run_daemon().await
    }
}
