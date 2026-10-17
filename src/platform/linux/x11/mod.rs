//! # Native X11 Compositor Driver
//!
//! Submodules:
//! - `atoms`: X11 atom manager and diagnostic logging macros.
//! - `session`: Dedicated X11 connection, window queries, and basic EWMH operations.
//! - `interception`: Spawn interception event loop and geometry negotiation.
//!
//! [`X11Driver`] is the backend for an X11 session and, through XWayland, for the X11 windows of
//! a Wayland session. It is chosen by `registry.rs` when GNOME is not available, and when
//! `SPAWN_AT_BACKEND=x11` forces it. Every method opens its own `X11Session` inside
//! `spawn_blocking` and keeps no state. Placement happens in `post_spawn`, after the program has
//! started; `arm` only prepares the launch environment. Under XWayland the driver sees only
//! XWayland windows. Flash measurements for X11 and XWayland windows are in `docs/TEST_LOG.md`
//! (sections 6.15 and 6.17).
//!
//! It must not name a Wayland compositor. Most actions are EWMH client messages whose result is
//! not read back, so `Ok` means the request was sent, not that the window manager obeyed.

pub mod atoms;
pub mod interception;
pub mod session;

#[cfg(test)]
mod tests;

pub use session::X11Session;

use crate::platform::{
    Armed, Batch, CompositorBackend, Driver, DriverError, MonitorLayout, PlacementParams, Rect,
    WindowMetadata, WindowState,
};
use x11rb::connection::Connection as _;
use x11rb::protocol::xproto::{
    ClientMessageEvent, ConnectionExt as _, EventMask, InputFocus, Window,
};

/// The rectangle `params` give for a window of `win_w` x `win_h` pixels; a call to
/// `spawn_at_core::geometry::calculate_rect_from_placement`, kept so the X11 files and their
/// tests have one local name for it. Pure.
pub fn calculate_rect_from_placement(params: &PlacementParams, win_w: u32, win_h: u32) -> Rect {
    spawn_at_core::geometry::calculate_rect_from_placement(params, win_w, win_h)
}

/// The X11 driver; a unit struct with no state.
pub struct X11Driver;

#[async_trait::async_trait]
impl Driver for X11Driver {
    /// Returns the launch environment only: `XDG_ACTIVATION_TOKEN` and `DESKTOP_STARTUP_ID`, both
    /// set to each entry's `key`, one pair per entry; `token` is the last entry's key. Nothing is
    /// sent to the X server and it never fails. The window is found later by `post_spawn`, which
    /// matches `_NET_STARTUP_ID` against the first entry's key among other rules.
    async fn arm(&self, batch: Batch) -> Result<Armed, DriverError> {
        let mut launch_env = Vec::new();
        let mut armed_token = None;
        for entry in &batch.entries {
            launch_env.push(("XDG_ACTIVATION_TOKEN".to_string(), entry.key.clone()));
            launch_env.push(("DESKTOP_STARTUP_ID".to_string(), entry.key.clone()));
            armed_token = Some(entry.key.clone());
        }
        Ok(Armed {
            launch_env,
            token: armed_token,
        })
    }

    /// Does nothing (there is nothing registered to remove) and returns `Ok(true)`, which here
    /// does not mean something was disarmed. `LinuxBackend` does not forward `disarm` anyway
    /// (grep -n "fn disarm" src/platform/linux/mod.rs).
    async fn disarm(&self, _token: &str) -> Result<bool, DriverError> {
        Ok(true)
    }
}

#[async_trait::async_trait]
impl CompositorBackend for X11Driver {
    fn name(&self) -> &'static str {
        "X11"
    }

    fn supports_runtime_transform(&self) -> bool {
        true
    }

    fn supports_close(&self) -> bool {
        true
    }

    fn capabilities(&self) -> crate::platform::capability::Capabilities {
        crate::platform::capability::Capabilities {
            runtime_transform: true,
            claim_wait: false,
            close: true,
            reveal_control: false,
            needs_helper: false,
        }
    }

    fn resolve_id(&self, command: &[String], explicit_class: Option<&str>) -> String {
        let bin = command.first().map(|s| s.as_str()).unwrap_or("");
        crate::platform::linux::xdg::resolve_linux_app_id(bin, explicit_class)
    }

    /// Pointer position in root coordinates (pixels) from `QueryPointer` on a new connection.
    /// Errors: `Execution` if the connection or the query fails.
    async fn get_cursor_position(&self) -> Result<(i32, i32), DriverError> {
        tokio::task::spawn_blocking(|| {
            let session = X11Session::connect()?;
            let reply = session
                .conn
                .query_pointer(session.root)
                .map_err(|e| {
                    DriverError::Execution(format!("Failed to query pointer: {}", e).into())
                })?
                .reply()
                .map_err(|e| {
                    DriverError::Execution(format!("Failed to read pointer reply: {}", e).into())
                })?;
            Ok((reply.root_x as i32, reply.root_y as i32))
        })
        .await
        .map_err(|e| DriverError::Execution(e.into()))?
    }

    /// Monitor rectangles (see `X11Session::fetch_monitors` for the fallbacks).
    async fn get_monitors(&self) -> Result<Vec<Rect>, DriverError> {
        tokio::task::spawn_blocking(|| {
            let session = X11Session::connect()?;
            session.fetch_monitors()
        })
        .await
        .map_err(|e| DriverError::Execution(e.into()))?
    }

    /// Work areas from `_NET_WORKAREA`, else the monitors (see `X11Session::fetch_workareas`).
    async fn get_workareas(&self) -> Result<Vec<Rect>, DriverError> {
        tokio::task::spawn_blocking(|| {
            let session = X11Session::connect()?;
            session.fetch_workareas()
        })
        .await
        .map_err(|e| DriverError::Execution(e.into()))?
    }

    /// The windows in `_NET_CLIENT_LIST` (see `X11Session::fetch_windows`).
    async fn get_windows(&self) -> Result<Vec<WindowMetadata>, DriverError> {
        tokio::task::spawn_blocking(|| {
            let session = X11Session::connect()?;
            session.fetch_windows()
        })
        .await
        .map_err(|e| DriverError::Execution(e.into()))?
    }

    /// Monitor layout: screen rectangles, and per-monitor work areas that are an approximation
    /// derived from `_NET_WORKAREA` (see `X11Session::fetch_layout`).
    async fn get_layout(&self) -> Result<MonitorLayout, DriverError> {
        tokio::task::spawn_blocking(|| {
            let session = X11Session::connect()?;
            session.fetch_layout()
        })
        .await
        .map_err(|e| DriverError::Execution(e.into()))?
    }

    /// Moves and resizes the target window to the rectangle `params` give for a window of
    /// `current_w` x `current_h` pixels. The size is always sent, also when `params` have none
    /// (it is then the current size). Errors: `TargetNotFound` for an unknown target; the move
    /// itself reports no refusal (see `X11Session::move_resize`).
    async fn transform_window(
        &self,
        target_id: &str,
        params: PlacementParams,
        current_w: u32,
        current_h: u32,
    ) -> Result<(), DriverError> {
        let target_id = target_id.to_string();
        tokio::task::spawn_blocking(move || {
            let session = X11Session::connect()?;
            let win = session.resolve_target_window(&target_id)?;
            let rect = calculate_rect_from_placement(&params, current_w, current_h);
            session.move_resize(win, Some((rect.x, rect.y)), Some((rect.width, rect.height)))
        })
        .await
        .map_err(|e| DriverError::Execution(e.into()))?
    }

    /// Moves a target window to coordinates (x, y).
    async fn move_window(&self, target_id: &str, x: i32, y: i32) -> Result<(), DriverError> {
        let target_id = target_id.to_string();
        tokio::task::spawn_blocking(move || {
            let session = X11Session::connect()?;
            let win = session.resolve_target_window(&target_id)?;
            session.move_resize(win, Some((x, y)), None)
        })
        .await
        .map_err(|e| DriverError::Execution(e.into()))?
    }

    /// Moves and resizes a target window to (x, y, w, h).
    async fn move_resize_window(
        &self,
        target_id: &str,
        x: i32,
        y: i32,
        w: u32,
        h: u32,
    ) -> Result<(), DriverError> {
        let target_id = target_id.to_string();
        tokio::task::spawn_blocking(move || {
            let session = X11Session::connect()?;
            let win = session.resolve_target_window(&target_id)?;
            session.move_resize(win, Some((x, y)), Some((w, h)))
        })
        .await
        .map_err(|e| DriverError::Execution(e.into()))?
    }

    /// Sends the state change as client messages to the root window: maximize and restore add
    /// or remove both `_NET_WM_STATE_MAXIMIZED_*` atoms (restore also maps the window),
    /// minimize sends `WM_CHANGE_STATE` (iconic) and a `_NET_WM_STATE` add of `_HIDDEN`, and
    /// unminimize maps and activates the window. The results of the sends are ignored; only
    /// the target lookup, the activation and the flush can fail. Whether a window manager
    /// honours the `_HIDDEN` add from a client is not verified.
    async fn set_window_state(
        &self,
        target_id: &str,
        state: WindowState,
    ) -> Result<(), DriverError> {
        let target_id = target_id.to_string();
        tokio::task::spawn_blocking(move || {
            let session = X11Session::connect()?;
            let win = session.resolve_target_window(&target_id)?;

            match state {
                WindowState::Maximize => {
                    let event = ClientMessageEvent::new(
                        32,
                        win,
                        session.atoms._NET_WM_STATE,
                        [
                            1, // _NET_WM_STATE_ADD
                            session.atoms._NET_WM_STATE_MAXIMIZED_HORZ,
                            session.atoms._NET_WM_STATE_MAXIMIZED_VERT,
                            1,
                            0,
                        ],
                    );
                    let _ = session.conn.send_event(
                        false,
                        session.root,
                        EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY,
                        event,
                    );
                }
                WindowState::Restore => {
                    let event = ClientMessageEvent::new(
                        32,
                        win,
                        session.atoms._NET_WM_STATE,
                        [
                            0, // _NET_WM_STATE_REMOVE
                            session.atoms._NET_WM_STATE_MAXIMIZED_HORZ,
                            session.atoms._NET_WM_STATE_MAXIMIZED_VERT,
                            1,
                            0,
                        ],
                    );
                    let _ = session.conn.send_event(
                        false,
                        session.root,
                        EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY,
                        event,
                    );
                    let _ = session.conn.map_window(win);
                }
                WindowState::Minimize => {
                    let event = ClientMessageEvent::new(
                        32,
                        win,
                        session.atoms.WM_CHANGE_STATE,
                        [3, 0, 0, 0, 0], // 3 = IconicState
                    );
                    let _ = session.conn.send_event(
                        false,
                        session.root,
                        EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY,
                        event,
                    );
                    let state_event = ClientMessageEvent::new(
                        32,
                        win,
                        session.atoms._NET_WM_STATE,
                        [1, session.atoms._NET_WM_STATE_HIDDEN, 0, 1, 0],
                    );
                    let _ = session.conn.send_event(
                        false,
                        session.root,
                        EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY,
                        state_event,
                    );
                }
                WindowState::Unminimize => {
                    let _ = session.conn.map_window(win);
                    session.activate_window(win)?;
                }
            }
            session
                .conn
                .flush()
                .map_err(|e| DriverError::Execution(e.into()))?;
            Ok(())
        })
        .await
        .map_err(|e| DriverError::Execution(e.into()))?
    }

    /// Activates the target window (`X11Session::activate_window`: `_NET_ACTIVE_WINDOW` plus
    /// `SetInputFocus`).
    async fn focus_window(&self, target_id: &str) -> Result<(), DriverError> {
        let target_id = target_id.to_string();
        tokio::task::spawn_blocking(move || {
            let session = X11Session::connect()?;
            let win = session.resolve_target_window(&target_id)?;
            session.activate_window(win)
        })
        .await
        .map_err(|e| DriverError::Execution(e.into()))?
    }

    /// Moves input focus away from the target. `mode` `"desktop"` sets the focus to the root
    /// window; otherwise a non-empty `destination` that resolves is activated (and the root gets
    /// the focus if it does not resolve); with no destination the first window of
    /// `_NET_CLIENT_LIST` other than the target is activated, and the root gets the focus if
    /// there is none. The target must resolve first (`TargetNotFound` otherwise).
    async fn defocus_window(
        &self,
        target_id: &str,
        mode: &str,
        destination: &str,
    ) -> Result<(), DriverError> {
        let target_id = target_id.to_string();
        let mode = mode.to_string();
        let destination = destination.to_string();
        tokio::task::spawn_blocking(move || {
            let session = X11Session::connect()?;
            let win = session.resolve_target_window(&target_id)?;

            if mode == "desktop" {
                let _ = session.conn.set_input_focus(
                    InputFocus::POINTER_ROOT,
                    session.root,
                    x11rb::CURRENT_TIME,
                );
            } else if !destination.is_empty() {
                if let Ok(dest_win) = session.resolve_target_window(&destination) {
                    session.activate_window(dest_win)?;
                } else {
                    let _ = session.conn.set_input_focus(
                        InputFocus::POINTER_ROOT,
                        session.root,
                        x11rb::CURRENT_TIME,
                    );
                }
            } else {
                let windows = session.fetch_windows().unwrap_or_default();
                if let Some(other) = windows.iter().find(|w| w.id != Some(win as u64)) {
                    if let Some(other_id) = other.id {
                        session.activate_window(other_id as Window)?;
                    } else {
                        let _ = session.conn.set_input_focus(
                            InputFocus::POINTER_ROOT,
                            session.root,
                            x11rb::CURRENT_TIME,
                        );
                    }
                } else {
                    let _ = session.conn.set_input_focus(
                        InputFocus::POINTER_ROOT,
                        session.root,
                        x11rb::CURRENT_TIME,
                    );
                }
            }
            session
                .conn
                .flush()
                .map_err(|e| DriverError::Execution(e.into()))?;
            Ok(())
        })
        .await
        .map_err(|e| DriverError::Execution(e.into()))?
    }

    /// Sends `_NET_CLOSE_WINDOW` (timestamp `CURRENT_TIME`, source indication 2) to the root
    /// window: a close request, not a kill. Errors: `TargetNotFound`, or `Execution` if the send
    /// or the flush fails; whether the window closed is not checked.
    async fn close_window(&self, target_id: &str) -> Result<(), DriverError> {
        let target_id = target_id.to_string();
        tokio::task::spawn_blocking(move || {
            let session = X11Session::connect()?;
            let win = session.resolve_target_window(&target_id)?;
            let event = ClientMessageEvent::new(
                32,
                win,
                session.atoms._NET_CLOSE_WINDOW,
                [x11rb::CURRENT_TIME, 2, 0, 0, 0],
            );
            session
                .conn
                .send_event(
                    false,
                    session.root,
                    EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY,
                    event,
                )
                .map_err(|e| DriverError::Execution(e.into()))?;
            session
                .conn
                .flush()
                .map_err(|e| DriverError::Execution(e.into()))?;
            Ok(())
        })
        .await
        .map_err(|e| DriverError::Execution(e.into()))?
    }

    /// Finds the new window and places it (`X11Session::handle_spawn_interception`): waits up to
    /// 5 s to find it and up to 3 s for it to map, so the call can block that long. Whether the
    /// result is flicker-free is not claimed here; see the module header.
    async fn post_spawn(&self, child_pid: u32, batch: &Batch) -> Result<(), DriverError> {
        let batch = batch.clone();
        tokio::task::spawn_blocking(move || {
            let session = X11Session::connect()?;
            session.handle_spawn_interception(child_pid, &batch)
        })
        .await
        .map_err(|e| DriverError::Execution(e.into()))?
    }
}
