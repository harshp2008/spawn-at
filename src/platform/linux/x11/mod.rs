//! # Native X11 Compositor Driver
//!
//! Submodules:
//! - `atoms`: X11 atom manager and diagnostic logging macros.
//! - `process`: Process tree introspection via `/proc/{pid}/stat`.
//! - `session`: Dedicated X11 connection, window queries, and basic EWMH operations.
//! - `interception`: Spawn interception event loop and geometry negotiation.

pub mod atoms;
pub mod interception;
pub mod process;
pub mod session;

#[cfg(test)]
mod tests;

pub use atoms::Atoms;
pub use session::X11Session;

use crate::platform::{
    Armed, Batch, CompositorBackend, Driver, DriverError, MonitorLayout, PlacementParams, Rect,
    WindowMetadata, WindowState,
};
use x11rb::connection::Connection as _;
use x11rb::protocol::xproto::{
    ClientMessageEvent, ConnectionExt as _, EventMask, InputFocus, Window,
};

/// Resolves a full-screen bounding rectangle from placement parameters and actual/target dimensions.
pub fn calculate_rect_from_placement(params: &PlacementParams, win_w: u32, win_h: u32) -> Rect {
    spawn_at_core::geometry::calculate_rect_from_placement(params, win_w, win_h)
}

/// Native X11 Compositor Driver implementing `Driver` and `CompositorBackend`.
pub struct X11Driver;

#[async_trait::async_trait]
impl Driver for X11Driver {
    /// Arms launch environment variables (`DESKTOP_STARTUP_ID`, `XDG_ACTIVATION_TOKEN`).
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

    /// Disarm is a no-op on X11 as startup IDs are handled statelessly.
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

    fn resolve_id(&self, command: &[String], explicit_class: Option<&str>) -> String {
        let bin = command.first().map(|s| s.as_str()).unwrap_or("");
        crate::platform::linux::xdg::resolve_linux_app_id(bin, explicit_class)
    }

    /// Queries pointer position natively from the X11 root window.
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

    /// Queries monitor rectangles using RANDR extension protocol.
    async fn get_monitors(&self) -> Result<Vec<Rect>, DriverError> {
        tokio::task::spawn_blocking(|| {
            let session = X11Session::connect()?;
            session.fetch_monitors()
        })
        .await
        .map_err(|e| DriverError::Execution(e.into()))?
    }

    /// Queries usable desktop workareas using `_NET_WORKAREA`.
    async fn get_workareas(&self) -> Result<Vec<Rect>, DriverError> {
        tokio::task::spawn_blocking(|| {
            let session = X11Session::connect()?;
            session.fetch_workareas()
        })
        .await
        .map_err(|e| DriverError::Execution(e.into()))?
    }

    /// Queries active managed windows on the X11 server.
    async fn get_windows(&self) -> Result<Vec<WindowMetadata>, DriverError> {
        tokio::task::spawn_blocking(|| {
            let session = X11Session::connect()?;
            session.fetch_windows()
        })
        .await
        .map_err(|e| DriverError::Execution(e.into()))?
    }

    /// Queries monitor layout: screen rectangles from RANDR and approximate per-monitor work areas from _NET_WORKAREA.
    async fn get_layout(&self) -> Result<MonitorLayout, DriverError> {
        tokio::task::spawn_blocking(|| {
            let session = X11Session::connect()?;
            session.fetch_layout()
        })
        .await
        .map_err(|e| DriverError::Execution(e.into()))?
    }

    /// Repositions and resizes a target window according to placement parameters.
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

    /// Modifies window state via EWMH `_NET_WM_STATE` and ICCCM `WM_CHANGE_STATE`.
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

    /// Gives input focus to the specified target window via `_NET_ACTIVE_WINDOW`.
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

    /// Evicts focus from the target window to desktop, previous window, or destination.
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

    /// Requests graceful window closure via `_NET_CLOSE_WINDOW`.
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

    /// Post-spawn interception hook implementing zero-flicker X11 placement.
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
