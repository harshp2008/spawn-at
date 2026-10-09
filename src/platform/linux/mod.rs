pub mod gnome;
pub mod x11;
pub mod xdg;

use crate::platform::{
    Armed, Batch, CompositorBackend, Driver, DriverError, InstallArgs, PlacementParams, Rect,
    UninstallArgs, WindowMetadata, WindowState,
};
use x11rb::connection::Connection as _;

/// Queries available display monitor bounding boxes via native X11 RANDR or screen fallback.
pub fn query_xrandr_monitors() -> Vec<Rect> {
    if let Ok((conn, screen_num)) = x11rb::rust_connection::RustConnection::connect(None) {
        let root = conn.setup().roots[screen_num].root;
        use x11rb::protocol::randr::ConnectionExt as _;
        if let Ok(reply) = conn.randr_get_monitors(root, true) {
            if let Ok(reply) = reply.reply() {
                if !reply.monitors.is_empty() {
                    return reply
                        .monitors
                        .into_iter()
                        .map(|m| Rect {
                            x: m.x as i32,
                            y: m.y as i32,
                            width: m.width as u32,
                            height: m.height as u32,
                        })
                        .collect();
                }
            }
        }
        let screen = &conn.setup().roots[screen_num];
        return vec![Rect {
            x: 0,
            y: 0,
            width: screen.width_in_pixels as u32,
            height: screen.height_in_pixels as u32,
        }];
    }
    Vec::new()
}

/// Queries current pointer coordinates using native X11 protocol.
pub fn query_xdotool_cursor() -> Option<(i32, i32)> {
    if let Ok((conn, screen_num)) = x11rb::rust_connection::RustConnection::connect(None) {
        let root = conn.setup().roots[screen_num].root;
        use x11rb::protocol::xproto::ConnectionExt as _;
        if let Ok(reply) = conn.query_pointer(root) {
            if let Ok(reply) = reply.reply() {
                return Some((reply.root_x as i32, reply.root_y as i32));
            }
        }
    }
    None
}

/// Reads `/proc/{pid}/stat` on Linux to extract the Parent Process ID (PPID).
pub fn get_parent_pid(pid: u32) -> Option<u32> {
    let stat = std::fs::read_to_string(format!("/proc/{}/stat", pid)).ok()?;
    let ppid_str = stat.split(')').nth(1)?;
    let parts: Vec<&str> = ppid_str.split_whitespace().collect();
    if parts.len() >= 2 {
        parts[1].parse::<u32>().ok()
    } else {
        None
    }
}

/// Checks whether `pid` is a descendant of `target_parent` by walking the process tree.
pub fn is_process_descendant(mut pid: u32, target_parent: u32) -> bool {
    for _ in 0..10 {
        if pid == target_parent {
            return true;
        }
        if pid <= 1 {
            break;
        }
        match get_parent_pid(pid) {
            Some(ppid) if ppid != pid => pid = ppid,
            _ => break,
        }
    }
    false
}

/// Unified Linux compositor backend adapter that delegates to either GNOME or X11 driver.
pub struct LinuxBackend {
    driver: Box<dyn CompositorBackend>,
}

impl LinuxBackend {
    pub async fn bootstrap() -> Result<Box<dyn CompositorBackend>, DriverError> {
        let forced = std::env::var("SPAWN_AT_BACKEND").unwrap_or_default().to_lowercase();
        if forced == "x11" {
            return Ok(Box::new(LinuxBackend {
                driver: Box::new(x11::X11Driver),
            }));
        }

        let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default().to_uppercase();

        if desktop.contains("GNOME") {
            if let Ok(driver) = gnome::GnomeWaylandDriver::new().await {
                return Ok(Box::new(LinuxBackend {
                    driver: Box::new(driver),
                }));
            }
        }

        Ok(Box::new(LinuxBackend {
            driver: Box::new(x11::X11Driver),
        }))
    }
}

#[async_trait::async_trait]
impl Driver for LinuxBackend {
    async fn arm(&self, batch: Batch) -> Result<Armed, DriverError> {
        self.driver.arm(batch).await
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

    async fn transform_window(
        &self,
        target_id: &str,
        params: PlacementParams,
        current_w: u32,
        current_h: u32,
    ) -> Result<(), DriverError> {
        self.driver
            .transform_window(target_id, params, current_w, current_h)
            .await
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

    async fn defocus_window(
        &self,
        target_id: &str,
        mode: &str,
        destination: &str,
    ) -> Result<(), DriverError> {
        self.driver.defocus_window(target_id, mode, destination).await
    }

    async fn post_spawn(&self, child_pid: u32, batch: &Batch) -> Result<(), DriverError> {
        self.driver.post_spawn(child_pid, batch).await
    }
}
