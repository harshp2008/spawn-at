//! The GNOME Wayland driver: D-Bus communication with the extension, arming, claims, and the
//! window queries and actions of the compositor backend.
//!
//! [`GnomeWaylandDriver`] is a thin client of the extension's session-bus interface (`dbus.rs`).
//! It keeps no state besides the proxy, so every call is one D-Bus round trip and the extension
//! is the only place that knows about windows. `LinuxBackend` wraps it; the spawn command calls
//! it only through that wrapper.
//!
//! Version gating: each method reads the extension's `ProtocolVersion` itself and treats a failed
//! read as 0, so an unreachable extension looks like "older extension". The thresholds used here
//! are 2 (claim signal), 3 (close, and the version `arm` warns below), 4 (layout) and 6
//! (resize outcome). The extension reports 6 (modern) or 5 (legacy), read from
//! `assets/gnome/*/extension.js`.
//!
//! It must not touch files or the Shell process; installing the extension is the provisioner's
//! job (`deploy.rs`).

use super::{dbus, mechanics};
use crate::platform::{
    Armed, Batch, CompositorBackend, Driver, DriverError, MonitorInfo, MonitorLayout,
    PlacementParams, Rect, WindowMetadata, WindowState,
};

// ============================================================================
// 4. Driver Construction & Internal State
// ============================================================================

/// The driver for a GNOME Shell session that has the spawn-at extension: it arms placements,
/// waits for claims and forwards window queries and actions to the extension over D-Bus.
///
/// Holds one proxy on the session bus. It does not check the session type itself; the choice of
/// this driver is made in `registry.rs`.
pub struct GnomeWaylandDriver {
    proxy: dbus::SpawnAtProxy<'static>,
}

impl GnomeWaylandDriver {
    /// Connects to the session bus and builds the proxy for the extension's interface.
    ///
    /// Errors: [`DriverError::IpcError`] when the session bus connection or the proxy cannot be
    /// created. Whether this already fails when the extension is not loaded is not verified;
    /// `arm` reports that case when the first call is made.
    pub async fn new() -> Result<Self, DriverError> {
        let connection = zbus::Connection::session().await.map_err(|e| {
            DriverError::IpcError(format!("Failed to connect to D-Bus session bus: {}", e))
        })?;
        let proxy = dbus::SpawnAtProxy::new(&connection).await.map_err(|e| {
            DriverError::IpcError(format!("Failed to initialize SpawnAt D-Bus proxy: {}", e))
        })?;

        Ok(Self { proxy })
    }

    /// Wraps a proxy built by a test, for example one pointed at a stub service on a private bus.
    #[cfg(test)]
    pub fn from_proxy(proxy: dbus::SpawnAtProxy<'static>) -> Self {
        Self { proxy }
    }
}

// ============================================================================
// 5. Pre-Map Arming Driver Trait
// ============================================================================

/// The extension protocol below which `arm` prints a warning (it still arms). Nothing else in
/// this file reads it.
pub const EXPECTED_PROTOCOL_VERSION: u32 = 3;

/// The first extension protocol that sends the `ResizeOutcome` signal. `prepare_claim_wait`
/// subscribes to that signal only at this version or higher.
pub const RESIZE_OUTCOME_PROTOCOL: u32 = 6;

use crate::platform::{ClaimResult, ClaimSubscription};
use futures_util::{FutureExt, StreamExt};
use spawn_at_core::resize_report::ResizeReport;
use std::time::Duration;

/// Streams of the `SpawnClaimed` (and, on protocol 6, `ResizeOutcome`) signals, opened by
/// `prepare_claim_wait` before `arm` so no signal is lost between arming and waiting.
/// `target_id` is the id this spawn armed under; `"*"` accepts any claim.
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
    ///
    /// Drains every queued `ResizeOutcome`, including ones for other targets (they are
    /// discarded), and keeps the last one that matches. `None` without a resize stream (protocol
    /// below 6) or when nothing matched. A signal whose arguments fail to parse is skipped.
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
    /// Waits up to `timeout` for a `SpawnClaimed` signal for this spawn's target.
    ///
    /// A signal matches when this subscription's target is `"*"`, when the signal's target equals
    /// it, or when the signal's target is `"*"`. Other signals, and signals whose arguments
    /// fail to parse, are skipped without error and the wait continues until the deadline.
    ///
    /// Errors:
    /// - [`DriverError::Execution`] ("Compositor placement failed: ...") when the matching
    ///   signal has `success` false;
    /// - [`DriverError::TargetNotFound`] ("Timed out waiting for claim signal ...") when the
    ///   deadline passes. The same error is returned when the signal stream ends, so the message
    ///   says "timed out" in that case too.
    ///
    /// On success the result carries the signal's geometry, and the `ResizeOutcome` that came
    /// with it, if any (see `take_resize`).
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
    /// Registers each entry of `batch` with the extension (`ArmSpawn`), before the application
    /// is started.
    ///
    /// The id an entry is armed under: for a one-entry batch its `app_hint`, or `"*"` when the
    /// hint is empty; for a batch of several entries, each entry's `key`. In the extension a
    /// named registration expires after 15 s and the `"*"` wildcard after 1.2 s
    /// (`ARM_EXPIRY_MS`, `WILDCARD_EXPIRY_MS`).
    ///
    /// Returns the launch environment (`XDG_ACTIVATION_TOKEN` and `DESKTOP_STARTUP_ID`, both set
    /// to the entry's `key`, one pair per entry) and `token`, the id of the LAST entry armed
    /// (`None` for an empty batch). A caller that passes the token to `disarm` can therefore
    /// withdraw only that last registration.
    ///
    /// Errors: [`DriverError::Execution`] when the instructions cannot be serialised;
    /// [`DriverError::IpcError`] when an `ArmSpawn` call fails, whatever the cause (the message
    /// always says the extension does not appear to be running). Entries armed before the failing
    /// one stay registered: nothing here withdraws them and no token is returned for them.
    /// `Ok` does not prove the extension accepted the instructions: it logs invalid JSON and
    /// returns normally.
    ///
    /// A protocol below [`EXPECTED_PROTOCOL_VERSION`], or a failed version read, prints a warning
    /// and carries on.
    async fn arm(&self, batch: Batch) -> Result<Armed, DriverError> {
        // The version check only warns; it never stops the arm.
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

    /// Asks the extension to drop every registration under `token` (`DisarmSpawn`). `Ok(true)`
    /// means something was removed, `Ok(false)` that nothing was registered under that id (for
    /// example it had already expired or been claimed), so calling it twice is harmless.
    ///
    /// Errors: [`DriverError::IpcError`] when the D-Bus call fails. This function does not
    /// swallow errors; a caller that wants best-effort ignores the result.
    // `LinuxBackend` does not forward `disarm`, so the spawn command never reaches this
    // function (grep -n "fn disarm" src/platform/linux/mod.rs finds no override there).
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

    /// Opens the `SpawnClaimed` stream (and the `ResizeOutcome` stream on protocol 6 or newer)
    /// for `target_id`. `Ok(None)` with an info message when the protocol is below 2, which
    /// includes a failed version read, since that counts as 0.
    ///
    /// Errors: [`DriverError::IpcError`] when a subscription cannot be made. The spawn command
    /// turns that into "no claim wait" without a message (`.ok().flatten()` in
    /// `src/commands/spawn.rs`), so the error is not shown to the user.
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

    /// The frame of the window with Mutter id `id`, from `GetWindows` (active workspace only).
    /// Width and height are cast from `i32` to `u32` with `as`, so a negative value would wrap.
    ///
    /// Errors: whatever `get_windows` returns, or [`DriverError::TargetNotFound`].
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

    fn provisioner(&self) -> Option<&dyn crate::platform::provisioner::Provisioner> {
        Some(self)
    }

    /// Sends one `ExecuteBatch` to an existing window: snapshot, cloak, then, if a size is
    /// asked, set the size and wait for the commit (500 ms, default wait override), then, if an
    /// anchor or position is asked, the anchored move, then uncloak and destroy the snapshot.
    /// `current_w` and `current_h` (pixels) are used only to compute the move.
    ///
    /// Errors: [`DriverError::Execution`] on a serialisation failure, [`DriverError::IpcError`]
    /// when the call fails. `Ok(())` does not show that a window matched `target_id`: the
    /// extension returns normally and only logs when none matches or the JSON is invalid.
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

    /// Pointer position in pixels. Tries `GetCursor`, then `GetPointer`, then a fresh X11
    /// connection (`query_x11_cursor`, through x11rb, not xdotool), then `(0, 0)`. It never
    /// returns an error, so `(0, 0)` can mean "every source failed".
    async fn get_cursor_position(&self) -> Result<(i32, i32), DriverError> {
        if let Ok(coords) = self.proxy.get_cursor().await {
            return Ok(coords);
        }
        if let Ok(coords) = self.proxy.get_pointer().await {
            return Ok(coords);
        }
        Ok(crate::platform::linux::query_x11_cursor().unwrap_or((0, 0)))
    }

    /// Full-screen rectangles of the monitors: the `screen` fields of `get_layout`; else the
    /// X11 RandR monitors of a fresh connection (`query_x11_monitors`); else the work areas
    /// (which are smaller than the screens). Errors only from that last step.
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

    /// The monitor layout from `GetLayout` on protocol 4 or newer, with `insets` computed from
    /// the screen and work-area rectangles when the extension sent none.
    ///
    /// Otherwise (protocol below 4, a failed `GetLayout` call, or JSON that does not parse) it
    /// returns the work areas only, with `screen` and `scale` unset, and a `note` saying the
    /// extension is older than the CLI expects. That note names the version read, so after a
    /// failed call on protocol 4 or newer it still says "need 4". A failing `get_workareas` is
    /// swallowed there and gives an empty monitor list. Never returns `Err`.
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

        // Work areas only. `primary` is set to "index 0" below, which is an assumption: this
        // path has no primary-monitor information.
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

    /// Always `Some`: a failed read of the property is reported as `Some(0)`, so "no extension"
    /// and "extension with protocol 0" look the same to the caller.
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

    /// Moves the window matching `target_id` to `(x, y)` pixels. `Ok(())` also when no window
    /// matches: `MoveWindow` returns nothing and the extension does not report that case.
    async fn move_window(&self, target_id: &str, x: i32, y: i32) -> Result<(), DriverError> {
        self.proxy
            .move_window(target_id, x, y)
            .await
            .map_err(|e| DriverError::IpcError(e.to_string()))
    }

    /// Sends `maximize`, `minimize`, `unminimize` or `restore` to the window matching
    /// `target_id`. The extension's `unmaximize` string is never sent from here.
    ///
    /// Errors: [`DriverError::IpcError`] on a D-Bus failure; [`DriverError::TargetNotFound`]
    /// when the extension answers `false`, which for these four strings means no window matched.
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

    /// `DefocusWindow` with `mode` and `destination` passed through unchanged. The extension
    /// reads `"desktop"` and `"window"` (with a destination); any other mode, for example the
    /// `"mru"` the focus command sends, activates the next window of the most-recently-used list
    /// that is not minimised and not skip-taskbar. `TargetNotFound` when no window matched
    /// `target_id`.
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

    /// Asks the window to close (the extension sends a delete request, not a kill).
    ///
    /// Errors: [`DriverError::Execution`] when the protocol is below 3 (a failed version read
    /// counts as 0) or when the error text of the call contains "UnknownMethod" or
    /// "MethodNotFound" (a string match, so it depends on the wording of the bus error);
    /// [`DriverError::IpcError`] for other D-Bus failures; [`DriverError::TargetNotFound`] when
    /// the extension answers `false` (no window matched, or the window cannot be deleted).
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
