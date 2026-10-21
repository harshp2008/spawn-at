//! Everything Linux-specific: the three drivers (`gnome`, `sway`, `x11`), the backend choice
//! (`registry`), the Linux [`host::NativeHost`] and config store, and [`LinuxBackend`], the one
//! value the commands hold on this OS.
//!
//! [`LinuxBackend::bootstrap`] picks and builds the driver; `LinuxBackend` then forwards every
//! [`CompositorBackend`] call to it. `LinuxBackend` must not add behavior of its own on top of
//! the driver's: it only chooses and forwards (with one gap, see its docs).

pub mod config_store;
pub mod gnome;
pub mod host;
pub mod process_tree;
pub mod registry;
pub mod sway;
pub mod x11;
pub mod xdg;

// The process-ancestry check is shared by the neutral commands and the X11 driver.
pub use process_tree::is_process_descendant;

/// The Linux session actions: the GNOME ones (log out with `gnome-session-quit`, reload the X11
/// shell with keystrokes), whatever the backend in use. Returns a static value; nothing runs.
pub fn session_actions() -> &'static dyn crate::platform::session_actions::SessionActions {
    &gnome::GnomeSessionActions
}

/// The Linux update hooks: the GNOME ones, whatever the backend in use (they check
/// `XDG_CURRENT_DESKTOP` themselves). Returns a static value; nothing runs.
pub fn update_hooks() -> &'static dyn crate::platform::update_hooks::UpdateHooks {
    &gnome::GnomeUpdateHooks
}

// Clap derive: the `///` on this struct and on `BackendUninstallArgs` is part of the `--help`
// contract and must stay as it is; notes go in plain `//`. Only the GNOME flags exist at this
// commit (Sway and X11 add none).
/// The flags of the install command that belong to the Linux backends: the GNOME ones.
#[derive(clap::Args, Debug, Clone, Default)]
pub struct BackendInstallArgs {
    #[command(flatten)]
    pub gnome: gnome::GnomeInstallArgs,
}

/// The flags of the uninstall command that belong to the Linux backends: the GNOME ones.
#[derive(clap::Args, Debug, Clone, Default)]
pub struct BackendUninstallArgs {
    #[command(flatten)]
    pub gnome: gnome::GnomeUninstallArgs,
}

use crate::platform::{
    Armed, Batch, CompositorBackend, Driver, DriverError, InstallArgs, MonitorLayout,
    PlacementParams, Rect, UninstallArgs, WindowMetadata, WindowState,
};
use x11rb::connection::Connection as _;

/// Monitor rectangles from a fresh X11 connection (`DISPLAY`): the RANDR monitors if the server
/// reports any, else one rectangle of the whole screen at (0, 0). Empty when the connection
/// fails; there is no error value. Opens and drops a connection on every call.
// Caller: the GNOME driver (src/platform/linux/gnome/driver.rs), which uses it
// next to its own layout data; the X11 driver has its own queries.
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

/// The pointer position (root x, y) from a fresh X11 connection, or `None` when the connection
/// or the query fails. Opens and drops a connection on every call.
// Caller: the GNOME driver, which falls back to (0, 0) on `None`.
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

/// The Linux backend: holds one driver and forwards every call to it.
///
/// It forwards each [`CompositorBackend`] method, [`Driver::arm`] and [`Driver::disarm_entry`] to
/// the driver unchanged. [`Driver::disarm`] is not forwarded: `LinuxBackend` uses the trait default, which does nothing
/// and returns `Ok(false)`, whatever driver is inside.
// Surprising, left as is (comment-only change): the spawn command calls `disarm` on the value
// from `init_backend`, which is a `LinuxBackend` on Linux, so the GNOME driver's `disarm`
// (the extension's DisarmSpawn call) and the Sway driver's are not reached from there; read
// from src/commands/spawn.rs and the impls, not run. The driver inside is
// `GnomeWaylandDriver`, `SwayDriver` or `X11Driver`.
pub struct LinuxBackend {
    driver: Box<dyn CompositorBackend>,
}

impl LinuxBackend {
    #[cfg(test)]
    pub fn new(driver: Box<dyn CompositorBackend>) -> Self {
        Self { driver }
    }

    /// Builds the backend for this process. Reads `SPAWN_AT_BACKEND`, `XDG_CURRENT_DESKTOP` and
    /// `XDG_SESSION_TYPE`, then follows [`registry::candidates`]: Sway if forced, else the GNOME
    /// Wayland driver when the desktop is GNOME and it connects, else X11.
    ///
    /// Errors: [`DriverError::Execution`] with the [`registry::unsupported`] message on an
    /// unsupported desktop; the error of `SwayDriver::from_env` when Sway is forced and
    /// unreachable, with no fallback to X11. A
    /// GNOME connection failure is not an error: it falls through to X11.
    /// Side effects: opens the session D-Bus connection (GNOME) or the Sway socket.
    pub async fn bootstrap() -> Result<Box<dyn CompositorBackend>, DriverError> {
        // The order of the choice (forced X11, then GNOME Wayland, else X11) is in `registry`.
        let env = registry::Environment::from_process_env();
        if let Some(message) = registry::unsupported(&env) {
            return Err(DriverError::Execution(message.into()));
        }
        for id in registry::candidates(&env) {
            // Sway was asked for by name: if it cannot be reached this is an error that names
            // $SWAYSOCK, never a quiet switch to X11 (which would place XWayland windows only).
            if id == registry::BackendId::Sway {
                let driver = sway::SwayDriver::from_env()?;
                return Ok(Box::new(LinuxBackend {
                    driver: Box::new(driver),
                }));
            }
            if id == registry::BackendId::GnomeWayland {
                if let Ok(driver) = gnome::GnomeWaylandDriver::new().await {
                    return Ok(Box::new(LinuxBackend {
                        driver: Box::new(driver),
                    }));
                }
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

    async fn disarm_entry(&self, token: &str, entry_key: &str) -> Result<bool, DriverError> {
        self.driver.disarm_entry(token, entry_key).await
    }
}

#[async_trait::async_trait]
impl CompositorBackend for LinuxBackend {
    fn name(&self) -> &'static str {
        self.driver.name()
    }

    fn install_notice(&self) -> Option<String> {
        self.driver.install_notice()
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

    fn provisioner(&self) -> Option<&dyn crate::platform::provisioner::Provisioner> {
        self.driver.provisioner()
    }

    fn capabilities(&self) -> crate::platform::capability::Capabilities {
        self.driver.capabilities()
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

        async fn disarm(&self, token: &str) -> Result<bool, DriverError> {
            self.tracker.record(&format!("disarm:{token}"));
            Ok(true)
        }

        async fn disarm_entry(&self, token: &str, entry_key: &str) -> Result<bool, DriverError> {
            self.tracker
                .record(&format!("disarm_entry:{token}:{entry_key}"));
            Ok(true)
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
                rel: None,
                area: None,
                focused: true,
                maximized: false,
                minimized: false,
                monitor: Some(0),
                state: Some("normal".into()),
                workspace: Some(1),
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
            Some(5)
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

    /// `disarm_entry` reaches the driver. `disarm` does not: `LinuxBackend` keeps the trait
    /// default (`Ok(false)`, no call), which this test pins so that forwarding it by accident,
    /// and with it the whole-queue `DisarmSpawn`, fails here.
    #[tokio::test]
    async fn test_linux_backend_forwards_disarm_entry_but_not_disarm() {
        let tracker = Arc::new(CallTracker::default());
        let mock = TrackingMockDriver {
            tracker: tracker.clone(),
        };
        let backend = LinuxBackend::new(Box::new(mock));

        assert!(backend.disarm_entry("tok", "key-1").await.unwrap());
        assert!(tracker.has_called("disarm_entry:tok:key-1"));

        assert!(!backend.disarm("tok").await.unwrap());
        assert!(!tracker.has_called("disarm:tok"));
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
        assert_eq!(windows[0].monitor, Some(0));
        assert_eq!(windows[0].state.as_deref(), Some("normal"));
        assert_eq!(windows[0].workspace, Some(1));
        assert!(tracker.has_called("get_windows"));

        // 14a. get_layout
        let layout = backend.get_layout().await.unwrap();
        assert_eq!(layout.schema_version, 1);
        assert_eq!(layout.monitors.len(), 1);
        assert!(tracker.has_called("get_layout"));

        // 14b. protocol_version
        assert_eq!(backend.protocol_version().await, Some(5));
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
