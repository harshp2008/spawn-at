pub mod gnome;
pub mod x11;
pub mod xdg;

use crate::platform::{
    Armed, Batch, CompositorBackend, Driver, DriverError, InstallArgs, MonitorLayout,
    PlacementParams, Rect, UninstallArgs, WindowMetadata, WindowState,
};
use x11rb::connection::Connection as _;

/// Queries available display monitor bounding boxes via native X11 RANDR or screen fallback.
pub fn query_x11_monitors() -> Vec<Rect> {
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
pub fn query_x11_cursor() -> Option<(i32, i32)> {
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
    pub fn new(driver: Box<dyn CompositorBackend>) -> Self {
        Self { driver }
    }

    pub async fn bootstrap() -> Result<Box<dyn CompositorBackend>, DriverError> {
        let forced = std::env::var("SPAWN_AT_BACKEND")
            .unwrap_or_default()
            .to_lowercase();
        if forced == "x11" {
            return Ok(Box::new(LinuxBackend {
                driver: Box::new(x11::X11Driver),
            }));
        }

        let desktop = std::env::var("XDG_CURRENT_DESKTOP")
            .unwrap_or_default()
            .to_uppercase();

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

    fn supports_claim_wait(&self) -> bool {
        self.driver.supports_claim_wait()
    }

    fn supports_close(&self) -> bool {
        self.driver.supports_close()
    }

    async fn prepare_claim_wait(
        &self,
        target_id: &str,
    ) -> Result<Option<Box<dyn crate::platform::ClaimSubscription>>, DriverError> {
        self.driver.prepare_claim_wait(target_id).await
    }

    async fn get_window_rect(&self, id: u64) -> Result<Rect, DriverError> {
        self.driver.get_window_rect(id).await
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

    async fn get_layout(&self) -> Result<MonitorLayout, DriverError> {
        self.driver.get_layout().await
    }

    async fn protocol_version(&self) -> Option<u32> {
        self.driver.protocol_version().await
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
        self.driver
            .defocus_window(target_id, mode, destination)
            .await
    }

    async fn close_window(&self, target_id: &str) -> Result<(), DriverError> {
        self.driver.close_window(target_id).await
    }

    async fn post_spawn(&self, child_pid: u32, batch: &Batch) -> Result<(), DriverError> {
        self.driver.post_spawn(child_pid, batch).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::{ClaimSubscription, LayoutInsets, LayoutRect, MonitorInfo};
    use std::sync::{Arc, Mutex};

    #[derive(Default)]
    struct CallTracker {
        calls: Mutex<Vec<String>>,
    }

    impl CallTracker {
        fn record(&self, method: &str) {
            self.calls.lock().unwrap().push(method.to_string());
        }

        fn has_called(&self, method: &str) -> bool {
            self.calls.lock().unwrap().iter().any(|m| m == method)
        }
    }

    struct TrackingMockDriver {
        tracker: Arc<CallTracker>,
    }

    fn dummy_batch() -> Batch {
        Batch {
            id: 1,
            entries: Vec::new(),
            reveal: crate::platform::Reveal::Independent,
            focus: crate::platform::FocusIntent::Leave,
            urgency: crate::platform::Urgency::Normal,
            deadline: std::time::Duration::from_secs(2),
        }
    }

    #[async_trait::async_trait]
    impl Driver for TrackingMockDriver {
        async fn arm(&self, _batch: Batch) -> Result<Armed, DriverError> {
            self.tracker.record("arm");
            Ok(Armed {
                token: Some("tracked-arm-token".into()),
                launch_env: vec![("TRACKED".into(), "1".into())],
            })
        }
    }

    #[async_trait::async_trait]
    impl CompositorBackend for TrackingMockDriver {
        fn name(&self) -> &'static str {
            self.tracker.record("name");
            "tracking-driver"
        }

        fn install(&self, _args: &InstallArgs) -> Result<(), DriverError> {
            self.tracker.record("install");
            Ok(())
        }

        fn uninstall(&self, _args: &UninstallArgs) -> Result<(), DriverError> {
            self.tracker.record("uninstall");
            Ok(())
        }

        fn supports_runtime_transform(&self) -> bool {
            self.tracker.record("supports_runtime_transform");
            true
        }

        fn supports_claim_wait(&self) -> bool {
            self.tracker.record("supports_claim_wait");
            true
        }

        fn supports_close(&self) -> bool {
            self.tracker.record("supports_close");
            true
        }

        async fn prepare_claim_wait(
            &self,
            target_id: &str,
        ) -> Result<Option<Box<dyn ClaimSubscription>>, DriverError> {
            self.tracker
                .record(&format!("prepare_claim_wait:{}", target_id));
            Ok(None)
        }

        async fn get_window_rect(&self, id: u64) -> Result<Rect, DriverError> {
            self.tracker.record(&format!("get_window_rect:{}", id));
            Ok(Rect {
                x: 777,
                y: 888,
                width: 999,
                height: 111,
            })
        }

        fn resolve_id(&self, command: &[String], explicit_class: Option<&str>) -> String {
            self.tracker.record("resolve_id");
            format!(
                "{}:{:?}",
                command.first().unwrap_or(&"".to_string()),
                explicit_class
            )
        }

        async fn get_cursor_position(&self) -> Result<(i32, i32), DriverError> {
            self.tracker.record("get_cursor_position");
            Ok((123, 456))
        }

        async fn get_monitors(&self) -> Result<Vec<Rect>, DriverError> {
            self.tracker.record("get_monitors");
            Ok(vec![Rect {
                x: 1,
                y: 2,
                width: 3,
                height: 4,
            }])
        }

        async fn get_workareas(&self) -> Result<Vec<Rect>, DriverError> {
            self.tracker.record("get_workareas");
            Ok(vec![Rect {
                x: 5,
                y: 6,
                width: 7,
                height: 8,
            }])
        }

        async fn get_windows(&self) -> Result<Vec<WindowMetadata>, DriverError> {
            self.tracker.record("get_windows");
            Ok(vec![WindowMetadata {
                id: Some(999),
                pid: Some(1234),
                title: "Tracked".into(),
                class: "tracked".into(),
                app_id: None,
                x: 10,
                y: 20,
                w: 30,
                h: 40,
                focused: true,
                maximized: false,
                minimized: false,
            }])
        }

        async fn get_layout(&self) -> Result<MonitorLayout, DriverError> {
            self.tracker.record("get_layout");
            Ok(MonitorLayout {
                schema_version: 1,
                monitors: vec![MonitorInfo {
                    index: 0,
                    name: Some("mock".to_string()),
                    primary: true,
                    scale: Some(1.0),
                    screen: Some(LayoutRect {
                        x: 0,
                        y: 0,
                        w: 1920,
                        h: 1080,
                    }),
                    workarea: Some(LayoutRect {
                        x: 0,
                        y: 40,
                        w: 1920,
                        h: 1040,
                    }),
                    insets: Some(LayoutInsets {
                        top: 40,
                        right: 0,
                        bottom: 0,
                        left: 0,
                    }),
                }],
                note: None,
            })
        }

        async fn protocol_version(&self) -> Option<u32> {
            self.tracker.record("protocol_version");
            Some(4)
        }

        async fn transform_window(
            &self,
            target_id: &str,
            _params: PlacementParams,
            _current_w: u32,
            _current_h: u32,
        ) -> Result<(), DriverError> {
            self.tracker
                .record(&format!("transform_window:{}", target_id));
            Ok(())
        }

        async fn move_window(&self, target_id: &str, x: i32, y: i32) -> Result<(), DriverError> {
            self.tracker
                .record(&format!("move_window:{}:{},{}", target_id, x, y));
            Ok(())
        }

        async fn move_resize_window(
            &self,
            target_id: &str,
            x: i32,
            y: i32,
            w: u32,
            h: u32,
        ) -> Result<(), DriverError> {
            self.tracker.record(&format!(
                "move_resize_window:{}:{},{},{},{}",
                target_id, x, y, w, h
            ));
            Ok(())
        }

        async fn set_window_state(
            &self,
            target_id: &str,
            state: WindowState,
        ) -> Result<(), DriverError> {
            self.tracker
                .record(&format!("set_window_state:{}:{:?}", target_id, state));
            Ok(())
        }

        async fn focus_window(&self, target_id: &str) -> Result<(), DriverError> {
            self.tracker.record(&format!("focus_window:{}", target_id));
            Ok(())
        }

        async fn defocus_window(
            &self,
            target_id: &str,
            mode: &str,
            destination: &str,
        ) -> Result<(), DriverError> {
            self.tracker.record(&format!(
                "defocus_window:{}:{}:{}",
                target_id, mode, destination
            ));
            Ok(())
        }

        async fn close_window(&self, target_id: &str) -> Result<(), DriverError> {
            self.tracker.record(&format!("close_window:{}", target_id));
            Ok(())
        }

        async fn post_spawn(&self, child_pid: u32, _batch: &Batch) -> Result<(), DriverError> {
            self.tracker.record(&format!("post_spawn:{}", child_pid));
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_linux_backend_close_window_delegation() {
        let tracker = Arc::new(CallTracker::default());
        let mock = TrackingMockDriver {
            tracker: Arc::clone(&tracker),
        };
        let backend = LinuxBackend::new(Box::new(mock));

        assert!(backend.supports_close());
        assert!(backend.close_window("win-close-123").await.is_ok());
        assert!(tracker.has_called("close_window:win-close-123"));
    }

    #[tokio::test]
    async fn test_linux_backend_forwards_all_compositor_methods() {
        let tracker = Arc::new(CallTracker::default());
        let mock = TrackingMockDriver {
            tracker: Arc::clone(&tracker),
        };
        let backend = LinuxBackend::new(Box::new(mock));

        // 1. name
        assert_eq!(backend.name(), "tracking-driver");
        assert!(tracker.has_called("name"));

        // 2. arm
        let arm_res = backend.arm(dummy_batch()).await.unwrap();
        assert_eq!(arm_res.token.as_deref(), Some("tracked-arm-token"));
        assert!(tracker.has_called("arm"));

        // 3. install
        let install_args = InstallArgs {
            headless: true,
            ..Default::default()
        };
        assert!(backend.install(&install_args).is_ok());
        assert!(tracker.has_called("install"));

        // 4. uninstall
        let uninstall_args = UninstallArgs {
            headless: true,
            ..Default::default()
        };
        assert!(backend.uninstall(&uninstall_args).is_ok());
        assert!(tracker.has_called("uninstall"));

        // 5. supports_runtime_transform
        assert!(backend.supports_runtime_transform());
        assert!(tracker.has_called("supports_runtime_transform"));

        // 6. supports_claim_wait
        assert!(backend.supports_claim_wait());
        assert!(tracker.has_called("supports_claim_wait"));

        // 7. supports_close
        assert!(backend.supports_close());
        assert!(tracker.has_called("supports_close"));

        // 8. prepare_claim_wait
        let _ = backend.prepare_claim_wait("app-1").await;
        assert!(tracker.has_called("prepare_claim_wait:app-1"));

        // 9. get_window_rect
        let rect = backend.get_window_rect(42).await.unwrap();
        assert_eq!(rect.x, 777);
        assert!(tracker.has_called("get_window_rect:42"));

        // 10. resolve_id
        let resolved = backend.resolve_id(&["my-app".into()], Some("MyClass"));
        assert_eq!(resolved, "my-app:Some(\"MyClass\")");
        assert!(tracker.has_called("resolve_id"));

        // 11. get_cursor_position
        let cursor = backend.get_cursor_position().await.unwrap();
        assert_eq!(cursor, (123, 456));
        assert!(tracker.has_called("get_cursor_position"));

        // 12. get_monitors
        let monitors = backend.get_monitors().await.unwrap();
        assert_eq!(monitors.len(), 1);
        assert_eq!(monitors[0].x, 1);
        assert!(tracker.has_called("get_monitors"));

        // 13. get_workareas
        let workareas = backend.get_workareas().await.unwrap();
        assert_eq!(workareas.len(), 1);
        assert_eq!(workareas[0].x, 5);
        assert!(tracker.has_called("get_workareas"));

        // 14. get_windows
        let windows = backend.get_windows().await.unwrap();
        assert_eq!(windows.len(), 1);
        assert_eq!(windows[0].id, Some(999));
        assert!(tracker.has_called("get_windows"));

        // 14a. get_layout
        let layout = backend.get_layout().await.unwrap();
        assert_eq!(layout.schema_version, 1);
        assert_eq!(layout.monitors.len(), 1);
        assert!(tracker.has_called("get_layout"));

        // 14b. protocol_version
        assert_eq!(backend.protocol_version().await, Some(4));
        assert!(tracker.has_called("protocol_version"));

        // 15. transform_window
        let dummy_params = PlacementParams::default();
        assert!(backend
            .transform_window("win-1", dummy_params, 800, 600)
            .await
            .is_ok());
        assert!(tracker.has_called("transform_window:win-1"));

        // 16. move_window
        assert!(backend.move_window("win-2", 100, 200).await.is_ok());
        assert!(tracker.has_called("move_window:win-2:100,200"));

        // 17. move_resize_window
        assert!(backend
            .move_resize_window("win-3", 10, 20, 300, 400)
            .await
            .is_ok());
        assert!(tracker.has_called("move_resize_window:win-3:10,20,300,400"));

        // 18. set_window_state
        assert!(backend
            .set_window_state("win-4", WindowState::Maximize)
            .await
            .is_ok());
        assert!(tracker.has_called("set_window_state:win-4:Maximize"));

        // 19. focus_window
        assert!(backend.focus_window("win-5").await.is_ok());
        assert!(tracker.has_called("focus_window:win-5"));

        // 20. defocus_window
        assert!(backend.defocus_window("win-6", "desktop", "").await.is_ok());
        assert!(tracker.has_called("defocus_window:win-6:desktop:"));

        // 21. close_window
        assert!(backend.close_window("win-7").await.is_ok());
        assert!(tracker.has_called("close_window:win-7"));

        // 22. post_spawn
        let dummy_b = dummy_batch();
        assert!(backend.post_spawn(1234, &dummy_b).await.is_ok());
        assert!(tracker.has_called("post_spawn:1234"));
    }
}
