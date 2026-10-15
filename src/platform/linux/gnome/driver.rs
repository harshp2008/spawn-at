//! The GNOME Wayland driver: D-Bus communication with the extension, arming, claims, and the
//! window queries and actions of the compositor backend.

use super::detect::{detect_gnome_shell_major_version, is_wayland_session};
use super::{dbus, mechanics};
use crate::platform::{
    Armed, Batch, CompositorBackend, Driver, DriverError, InstallArgs, MonitorInfo, MonitorLayout,
    PlacementParams, Rect, UninstallArgs, WindowMetadata, WindowState,
};

// ============================================================================
// 4. Driver Construction & Internal State
// ============================================================================

/// Compositor backend driver for GNOME sessions, communicating via the SpawnAt D-Bus extension.
pub struct GnomeWaylandDriver {
    proxy: dbus::SpawnAtProxy<'static>,
}

impl GnomeWaylandDriver {
    /// Constructs a new `GnomeWaylandDriver` by establishing a session bus D-Bus connection.
    pub async fn new() -> Result<Self, DriverError> {
        let connection = zbus::Connection::session().await.map_err(|e| {
            DriverError::IpcError(format!("Failed to connect to D-Bus session bus: {}", e))
        })?;
        let proxy = dbus::SpawnAtProxy::new(&connection).await.map_err(|e| {
            DriverError::IpcError(format!("Failed to initialize SpawnAt D-Bus proxy: {}", e))
        })?;

        Ok(Self { proxy })
    }

    /// Constructs a `GnomeWaylandDriver` with a custom proxy (useful for testing against stubs).
    #[cfg(test)]
    pub fn from_proxy(proxy: dbus::SpawnAtProxy<'static>) -> Self {
        Self { proxy }
    }
}

// ============================================================================
// 5. Pre-Map Arming Driver Trait
// ============================================================================

pub const EXPECTED_PROTOCOL_VERSION: u32 = 3;

/// The first extension protocol that sends the `ResizeOutcome` signal.
pub const RESIZE_OUTCOME_PROTOCOL: u32 = 6;

use crate::platform::{ClaimResult, ClaimSubscription};
use futures_util::{FutureExt, StreamExt};
use spawn_at_core::resize_report::ResizeReport;
use std::time::Duration;

pub struct GnomeClaimSubscription {
    stream: dbus::SpawnClaimedStream<'static>,
    /// Subscribed only when the extension speaks protocol 6 or newer.
    resize_stream: Option<dbus::ResizeOutcomeStream<'static>>,
    target_id: String,
}

impl GnomeClaimSubscription {
    /// The newest `ResizeOutcome` already received for `claimed_target`, without waiting.
    ///
    /// The extension emits `ResizeOutcome` before `SpawnClaimed` and both arrive on one bus
    /// connection in that order, so when the claim has been read the outcome is already queued.
    /// Not waiting matters: windows the extension waits for with the legacy rule send no outcome,
    /// and a wait here would delay every such spawn.
    fn take_resize(&mut self, claimed_target: &str) -> Option<ResizeReport> {
        let stream = self.resize_stream.as_mut()?;
        let mut found = None;
        while let Some(Some(sig)) = stream.next().now_or_never() {
            if let Ok(args) = sig.args() {
                if args.target_id == claimed_target {
                    found = Some(ResizeReport::from_wire(
                        args.outcome,
                        args.final_w,
                        args.final_h,
                        args.waited_ms,
                        args.reason,
                    ));
                }
            }
        }
        found
    }
}

#[async_trait::async_trait]
impl ClaimSubscription for GnomeClaimSubscription {
    async fn wait_claim(&mut self, timeout: Duration) -> Result<ClaimResult, DriverError> {
        let deadline = tokio::time::Instant::now() + timeout;
        while tokio::time::Instant::now() < deadline {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            let next_signal = tokio::time::timeout(remaining, self.stream.next()).await;
            match next_signal {
                Ok(Some(sig)) => {
                    if let Ok(args) = sig.args() {
                        let sig_target = args.target_id;
                        if self.target_id == "*"
                            || sig_target == self.target_id
                            || sig_target == "*"
                        {
                            if !args.success {
                                return Err(DriverError::Execution(
                                    format!("Compositor placement failed: {}", args.error).into(),
                                ));
                            }
                            let claimed = sig_target.to_string();
                            let result = ClaimResult {
                                target_id: claimed.clone(),
                                success: args.success,
                                window_id: args.window_id,
                                x: args.x,
                                y: args.y,
                                w: args.w,
                                h: args.h,
                                size_raised: args.size_raised,
                                error: args.error.to_string(),
                                resize: None,
                            };
                            return Ok(ClaimResult {
                                resize: self.take_resize(&claimed),
                                ..result
                            });
                        }
                    }
                }
                Ok(None) => break,
                Err(_) => break,
            }
        }
        Err(DriverError::TargetNotFound(format!(
            "Timed out waiting for claim signal for target '{}'",
            self.target_id
        )))
    }
}

#[async_trait::async_trait]
impl Driver for GnomeWaylandDriver {
    /// Arms the GNOME Shell extension with declarative batch entries translated to low-level instructions.
    async fn arm(&self, batch: Batch) -> Result<Armed, DriverError> {
        // Check extension protocol version; warn on mismatch without hard-failing
        let ext_version = self.proxy.protocol_version().await.unwrap_or(0);
        if ext_version < EXPECTED_PROTOCOL_VERSION {
            crate::diagnostics::render_warning(
                "Extension protocol mismatch (detected older extension). \
                 Please log out and back in to load the updated extension.",
            );
        }

        let mut launch_env = Vec::new();
        let mut armed_token = None;
        let is_solitary = batch.entries.len() == 1;

        for entry in &batch.entries {
            let target_id = if is_solitary {
                if !entry.app_hint.is_empty() {
                    entry.app_hint.as_str()
                } else {
                    "*"
                }
            } else {
                entry.key.as_str()
            };

            let instructions = mechanics::build_instructions_for_entry(entry);
            let instructions_json = serde_json::to_string(&instructions).map_err(|e| {
                DriverError::Execution(format!("Failed to serialize instructions: {}", e).into())
            })?;

            self.proxy
                .arm_spawn(target_id, &instructions_json)
                .await
                .map_err(|e| {
                    DriverError::IpcError(format!(
                        "Failed to communicate with SpawnAt GNOME extension via D-Bus: {}\n\
                         Reason: The extension does not appear to be running on the session bus.\n\
                         Fix: Run 'spawn-at install' to install and activate the extension.",
                        e
                    ))
                })?;

            launch_env.push(("XDG_ACTIVATION_TOKEN".to_string(), entry.key.clone()));
            launch_env.push(("DESKTOP_STARTUP_ID".to_string(), entry.key.clone()));
            armed_token = Some(target_id.to_string());
        }

        Ok(Armed {
            launch_env,
            token: armed_token,
        })
    }

    /// Disarms a pending target in the GNOME Shell extension (best-effort, idempotent).
    async fn disarm(&self, token: &str) -> Result<bool, DriverError> {
        self.proxy
            .disarm_spawn(token)
            .await
            .map_err(|e| DriverError::IpcError(format!("Failed to disarm spawn on D-Bus: {}", e)))
    }
}

// ============================================================================
// 6. Runtime Compositor Backend Trait
// ============================================================================

#[async_trait::async_trait]
impl CompositorBackend for GnomeWaylandDriver {
    fn name(&self) -> &'static str {
        "GNOME Wayland"
    }

    fn supports_runtime_transform(&self) -> bool {
        true
    }

    fn supports_claim_wait(&self) -> bool {
        true
    }

    fn supports_close(&self) -> bool {
        true
    }

    fn capabilities(&self) -> crate::platform::capability::Capabilities {
        crate::platform::capability::Capabilities {
            runtime_transform: true,
            claim_wait: true,
            close: true,
            reveal_control: true,
            needs_helper: true,
        }
    }

    async fn prepare_claim_wait(
        &self,
        target_id: &str,
    ) -> Result<Option<Box<dyn ClaimSubscription>>, DriverError> {
        let ext_version = self.proxy.protocol_version().await.unwrap_or(0);
        if ext_version < 2 {
            crate::diagnostics::render_info(
                "GNOME Shell extension does not support SpawnClaimed signal (protocol version < 2); falling back to polling.",
            );
            return Ok(None);
        }

        let stream = self.proxy.receive_spawn_claimed().await.map_err(|e| {
            DriverError::IpcError(format!("Failed to subscribe to SpawnClaimed: {}", e))
        })?;

        // An older extension has no ResizeOutcome signal; the CLI then simply has no outcome.
        let resize_stream = if ext_version >= RESIZE_OUTCOME_PROTOCOL {
            Some(self.proxy.receive_resize_outcome().await.map_err(|e| {
                DriverError::IpcError(format!("Failed to subscribe to ResizeOutcome: {}", e))
            })?)
        } else {
            None
        };

        Ok(Some(Box::new(GnomeClaimSubscription {
            stream,
            resize_stream,
            target_id: target_id.to_string(),
        })))
    }

    async fn get_window_rect(&self, id: u64) -> Result<Rect, DriverError> {
        let windows = self.get_windows().await?;
        windows
            .into_iter()
            .find(|w| w.id == Some(id))
            .map(|w| Rect {
                x: w.x,
                y: w.y,
                width: w.w as u32,
                height: w.h as u32,
            })
            .ok_or_else(|| DriverError::TargetNotFound(format!("Window ID {} not found", id)))
    }

    fn resolve_id(&self, command: &[String], explicit_class: Option<&str>) -> String {
        let bin = command.first().map(|s| s.as_str()).unwrap_or("");
        crate::platform::linux::xdg::resolve_linux_app_id(bin, explicit_class)
    }

    /// The notice for the detected GNOME Shell and session: none for GNOME 46 on Wayland, the only
    /// configuration spawn-at has been run on.
    fn install_notice(&self) -> Option<String> {
        crate::platform::install_notice::gnome_notice(
            detect_gnome_shell_major_version(),
            is_wayland_session(),
        )
    }

    /// Installs the embedded GNOME Shell extension (see `deploy.rs`).
    fn install(&self, args: &InstallArgs) -> Result<(), DriverError> {
        self.install_impl(args)
    }

    /// Disables and removes the GNOME Shell extension (see `deploy.rs`).
    fn uninstall(&self, args: &UninstallArgs) -> Result<(), DriverError> {
        self.uninstall_impl(args)
    }

    async fn transform_window(
        &self,
        target_id: &str,
        params: PlacementParams,
        current_w: u32,
        current_h: u32,
    ) -> Result<(), DriverError> {
        let has_move = params.anchor.is_some() || params.pos.is_some();
        let mut instructions = vec![
            mechanics::Instruction::Snapshot,
            mechanics::Instruction::Cloak,
        ];
        if let Some(size) = params.size {
            instructions.push(mechanics::Instruction::SetSize {
                w: size.0,
                h: size.1,
            });
            instructions.push(mechanics::Instruction::wait_for_commit(
                500,
                &spawn_at_core::wait_override::WaitOverride::default(),
            ));
        }
        if has_move {
            let payload = mechanics::calculate_placement(&params, current_w, current_h);
            instructions.push(mechanics::Instruction::SetPositionAnchored(payload));
        }
        instructions.push(mechanics::Instruction::Uncloak);
        instructions.push(mechanics::Instruction::DestroySnapshot);
        let instructions_json = serde_json::to_string(&instructions)
            .map_err(|e| DriverError::Execution(format!("Serialization error: {}", e).into()))?;

        self.proxy
            .execute_batch(target_id, &instructions_json)
            .await
            .map_err(|e| DriverError::IpcError(e.to_string()))?;

        Ok(())
    }

    /// Queries pointer position from the GNOME extension over D-Bus, with fallback to xdotool.
    async fn get_cursor_position(&self) -> Result<(i32, i32), DriverError> {
        if let Ok(coords) = self.proxy.get_cursor().await {
            return Ok(coords);
        }
        if let Ok(coords) = self.proxy.get_pointer().await {
            return Ok(coords);
        }
        Ok(crate::platform::linux::query_x11_cursor().unwrap_or((0, 0)))
    }

    async fn get_monitors(&self) -> Result<Vec<Rect>, DriverError> {
        let layout = self.get_layout().await?;
        let screens: Vec<Rect> = layout
            .monitors
            .into_iter()
            .filter_map(|m| m.screen.map(Rect::from))
            .collect();
        if !screens.is_empty() {
            return Ok(screens);
        }
        let x11_screens = crate::platform::linux::query_x11_monitors();
        if !x11_screens.is_empty() {
            return Ok(x11_screens);
        }
        self.get_workareas().await
    }

    async fn get_layout(&self) -> Result<MonitorLayout, DriverError> {
        let version = self.proxy.protocol_version().await.unwrap_or(0);
        if version >= 4 {
            if let Ok(json) = self.proxy.get_layout().await {
                if let Ok(mut layout) = serde_json::from_str::<MonitorLayout>(&json) {
                    for m in &mut layout.monitors {
                        if m.insets.is_none() {
                            m.insets = MonitorInfo::compute_insets(m.screen, m.workarea);
                        }
                    }
                    return Ok(layout);
                }
            }
        }

        // Stale extension or protocol < 4 degradation:
        // Degrades to the work area only, with screen set to null and a note. No guessed values.
        let workareas = self.get_workareas().await.unwrap_or_default();
        let monitors = workareas
            .into_iter()
            .enumerate()
            .map(|(i, wa)| MonitorInfo {
                index: i,
                name: None,
                primary: i == 0,
                scale: None,
                screen: None,
                workarea: Some(wa.into()),
                insets: None,
            })
            .collect();
        let note = Some(format!(
            "extension is older than the CLI expects (protocol {}, need 4): run `spawn-at install` and log out and back in",
            version
        ));
        Ok(MonitorLayout {
            schema_version: 1,
            monitors,
            note,
        })
    }

    async fn protocol_version(&self) -> Option<u32> {
        Some(self.proxy.protocol_version().await.unwrap_or(0))
    }

    async fn get_workareas(&self) -> Result<Vec<Rect>, DriverError> {
        let json = self
            .proxy
            .get_workareas()
            .await
            .map_err(|e| DriverError::IpcError(e.to_string()))?;
        serde_json::from_str(&json)
            .map_err(|e| DriverError::IpcError(format!("Failed to parse workareas: {}", e)))
    }

    async fn get_windows(&self) -> Result<Vec<WindowMetadata>, DriverError> {
        let json = self
            .proxy
            .get_windows()
            .await
            .map_err(|e| DriverError::IpcError(e.to_string()))?;
        serde_json::from_str(&json)
            .map_err(|e| DriverError::IpcError(format!("Failed to parse windows: {}", e)))
    }

    async fn move_window(&self, target_id: &str, x: i32, y: i32) -> Result<(), DriverError> {
        self.proxy
            .move_window(target_id, x, y)
            .await
            .map_err(|e| DriverError::IpcError(e.to_string()))
    }

    async fn set_window_state(
        &self,
        target_id: &str,
        state: WindowState,
    ) -> Result<(), DriverError> {
        let state_str = match state {
            WindowState::Maximize => "maximize",
            WindowState::Minimize => "minimize",
            WindowState::Unminimize => "unminimize",
            WindowState::Restore => "restore",
        };
        let success = self
            .proxy
            .set_window_state(target_id, state_str)
            .await
            .map_err(|e| DriverError::IpcError(e.to_string()))?;

        if !success {
            return Err(DriverError::TargetNotFound(format!(
                "Compositor failed to set state '{:?}' on target '{}'",
                state, target_id
            )));
        }
        Ok(())
    }

    async fn focus_window(&self, target_id: &str) -> Result<(), DriverError> {
        let success = self
            .proxy
            .focus_window(target_id)
            .await
            .map_err(|e| DriverError::IpcError(e.to_string()))?;

        if !success {
            return Err(DriverError::TargetNotFound(format!(
                "Compositor failed to focus window target '{}'",
                target_id
            )));
        }
        Ok(())
    }

    async fn defocus_window(
        &self,
        target_id: &str,
        mode: &str,
        destination: &str,
    ) -> Result<(), DriverError> {
        let success = self
            .proxy
            .defocus_window(target_id, mode, destination)
            .await
            .map_err(|e| DriverError::IpcError(e.to_string()))?;

        if !success {
            return Err(DriverError::TargetNotFound(format!(
                "Compositor failed to defocus window target '{}'",
                target_id
            )));
        }
        Ok(())
    }

    async fn close_window(&self, target_id: &str) -> Result<(), DriverError> {
        let version = self.proxy.protocol_version().await.unwrap_or(0);
        if version < 3 {
            return Err(DriverError::Execution(
                format!(
                    "extension is older than the CLI expects (protocol {}, need 3): run `spawn-at install` and log out and back in",
                    version
                )
                .into(),
            ));
        }

        let success = match self.proxy.close_window(target_id).await {
            Ok(s) => s,
            Err(e) => {
                let err_str = e.to_string();
                if err_str.contains("UnknownMethod") || err_str.contains("MethodNotFound") {
                    return Err(DriverError::Execution(
                        format!(
                            "extension is older than the CLI expects (protocol {}, need 3): run `spawn-at install` and log out and back in",
                            version
                        )
                        .into(),
                    ));
                }
                return Err(DriverError::IpcError(err_str));
            }
        };

        if !success {
            return Err(DriverError::TargetNotFound(format!(
                "Compositor failed to close window target '{}' (window not found)",
                target_id
            )));
        }
        Ok(())
    }
}
