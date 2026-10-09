//! # Native X11 Compositor Driver
//!
//! ## Architectural Notes on Native X11 Window Placement
//!
//! Under traditional X11:
//! - Window hierarchies and global coordinates are exposed across the entire X display.
//! - Window positioning and state transformations operate via the Extended Window Manager
//!   Hints (EWMH) specification (`_NET_MOVERESIZE_WINDOW`, `_NET_ACTIVE_WINDOW`, `_NET_WM_STATE`)
//!   and standard Inter-Client Communication Conventions Manual (ICCCM) conventions
//!   (`WM_NORMAL_HINTS`, `WM_CHANGE_STATE`).
//! - Unlike Wayland, window geometry and management are directly accessible via the X11 protocol.
//!
//! ### Zero-Flicker & Safe Size Spawning Pipeline:
//! 1. Calculate target anchor coordinates using `spawn-at-core::geometry`.
//! 2. Child process is launched with startup notification tokens (`DESKTOP_STARTUP_ID`).
//! 3. Driver establishes native X11 connection using `x11rb`.
//! 4. Pre-scans root window children to record pre-existing windows.
//! 5. Listens for root window `SubstructureNotify` (`CreateNotify`, `MapNotify`) and monitors
//!    candidate window `PropertyNotify` events with kernel sleeps to prevent busy-polling.
//! 6. Matches target window via PID (including `/proc` descendant tree), `_NET_STARTUP_ID`,
//!    or `WM_CLASS`.
//! 7. **Crucial:** Sets `WM_NORMAL_HINTS` (`XSizeHints`) with `USPosition | PPosition` and
//!    `_NET_WM_USER_TIME = 0` *before* the window maps to prevent the default top-left flash.
//! 8. Lets the window map naturally (ensuring toolkit minimum dimension negotiation).
//! 9. Queries the actual mapped width and height from X11 geometry.
//! 10. Recalculates the exact anchor coordinates using the actual dimensions.
//! 11. If geometry requires adjustment, updates coordinates via `_NET_MOVERESIZE_WINDOW`
//!     and flushes the connection immediately.

use crate::platform::{
    Armed, Batch, CompositorBackend, Driver, DriverError, PlacementParams, Rect,
    WindowMetadata, WindowState,
};
use crate::target::{self, WindowSelector};
use spawn_at_core::driver::FocusIntent;
use std::collections::HashSet;
use std::time::{Duration, Instant};
use x11rb::connection::Connection;
use x11rb::protocol::randr::ConnectionExt as _;
use x11rb::protocol::xproto::{
    AtomEnum, ClientMessageEvent, ConfigureWindowAux, ConnectionExt as _, EventMask,
    InputFocus, MapState, PropMode, Window,
};
use x11rb::rust_connection::RustConnection;
use x11rb::wrapper::ConnectionExt as _;

macro_rules! x11_debug {
    ($($arg:tt)*) => {
        if std::env::var("SPAWN_AT_DEBUG").map(|v| v == "1" || v == "true").unwrap_or(false) {
            eprintln!($($arg)*);
        }
    };
}

// ============================================================================
// 1. Atom Definitions & Manager
// ============================================================================

x11rb::atom_manager! {
    /// EWMH and ICCCM atoms used for window management, state control, and placement hints.
    pub Atoms: AtomsCookie {
        _NET_CLIENT_LIST,
        _NET_ACTIVE_WINDOW,
        _NET_WORKAREA,
        _NET_CURRENT_DESKTOP,
        _NET_WM_PID,
        _NET_WM_NAME,
        _NET_MOVERESIZE_WINDOW,
        _NET_WM_STATE,
        _NET_WM_STATE_MAXIMIZED_VERT,
        _NET_WM_STATE_MAXIMIZED_HORZ,
        _NET_WM_STATE_HIDDEN,
        _NET_WM_USER_TIME,
        _NET_STARTUP_ID,
        UTF8_STRING,
        WM_NORMAL_HINTS,
        WM_SIZE_HINTS,
        WM_CHANGE_STATE,
        WM_STATE,
    }
}

// ============================================================================
// 2. Geometry Calculation Helpers
// ============================================================================

/// Resolves a full-screen bounding rectangle from placement parameters and actual/target dimensions.
///
/// This calculates the target (x, y) coordinates on the target workarea according to:
/// - Explicit anchor positions (Center, TopLeft, BottomRight, etc.)
/// - Explicit offsets and margins
/// - Geometric pivot alignments
/// - Screen boundary clamping
pub fn calculate_rect_from_placement(params: &PlacementParams, win_w: u32, win_h: u32) -> Rect {
    let w = params.size.map(|s| s.0).unwrap_or(win_w);
    let h = params.size.map(|s| s.1).unwrap_or(win_h);

    let wa = params.workarea;
    let margin_top = params.margin_top.unwrap_or(params.margin);
    let margin_bottom = params.margin_bottom.unwrap_or(params.margin);
    let margin_left = params.margin_left.unwrap_or(params.margin);
    let margin_right = params.margin_right.unwrap_or(params.margin);

    let (anchor_x, anchor_y) = if let Some(anchor) = params.anchor {
        match anchor {
            spawn_at_core::geometry::Anchor::Center => (
                wa.x + (wa.width as i32) / 2,
                wa.y + (wa.height as i32) / 2,
            ),
            spawn_at_core::geometry::Anchor::TopLeft => (
                wa.x + margin_left,
                wa.y + margin_top,
            ),
            spawn_at_core::geometry::Anchor::TopRight => (
                wa.x + (wa.width as i32) - margin_right,
                wa.y + margin_top,
            ),
            spawn_at_core::geometry::Anchor::BottomLeft => (
                wa.x + margin_left,
                wa.y + (wa.height as i32) - margin_bottom,
            ),
            spawn_at_core::geometry::Anchor::BottomRight => (
                wa.x + (wa.width as i32) - margin_right,
                wa.y + (wa.height as i32) - margin_bottom,
            ),
            spawn_at_core::geometry::Anchor::Top => (
                wa.x + (wa.width as i32) / 2,
                wa.y + margin_top,
            ),
            spawn_at_core::geometry::Anchor::Bottom => (
                wa.x + (wa.width as i32) / 2,
                wa.y + (wa.height as i32) - margin_bottom,
            ),
            spawn_at_core::geometry::Anchor::Left => (
                wa.x + margin_left,
                wa.y + (wa.height as i32) / 2,
            ),
            spawn_at_core::geometry::Anchor::Right => (
                wa.x + (wa.width as i32) - margin_right,
                wa.y + (wa.height as i32) / 2,
            ),
            spawn_at_core::geometry::Anchor::Cursor => {
                if let Some(cursor) = params.cursor_pos {
                    cursor
                } else {
                    (wa.x + margin_left, wa.y + margin_top)
                }
            }
        }
    } else if let Some(pos) = params.pos {
        pos
    } else if let Some(cursor) = params.cursor_pos {
        cursor
    } else {
        (wa.x + margin_left, wa.y + margin_top)
    };

    let (ox, oy) = params.offset.unwrap_or((0, 0));
    let target_x = anchor_x + ox;
    let target_y = anchor_y + oy;

    let (x, y) = if let Some(pivot) = params.pivot {
        spawn_at_core::geometry::apply_pivot(target_x, target_y, w, h, pivot)
    } else if let Some(anchor) = params.anchor {
        match anchor {
            spawn_at_core::geometry::Anchor::Top => (target_x - (w as i32) / 2, target_y),
            spawn_at_core::geometry::Anchor::Bottom => (target_x - (w as i32) / 2, target_y - (h as i32)),
            spawn_at_core::geometry::Anchor::Left => (target_x, target_y - (h as i32) / 2),
            spawn_at_core::geometry::Anchor::Right => (target_x - (w as i32), target_y - (h as i32) / 2),
            spawn_at_core::geometry::Anchor::Center => (target_x - (w as i32) / 2, target_y - (h as i32) / 2),
            spawn_at_core::geometry::Anchor::TopLeft => (target_x, target_y),
            spawn_at_core::geometry::Anchor::TopRight => (target_x - (w as i32), target_y),
            spawn_at_core::geometry::Anchor::BottomLeft => (target_x, target_y - (h as i32)),
            spawn_at_core::geometry::Anchor::BottomRight => (target_x - (w as i32), target_y - (h as i32)),
            spawn_at_core::geometry::Anchor::Cursor => (target_x, target_y),
        }
    } else {
        (target_x, target_y)
    };

    let raw_rect = Rect { x, y, width: w, height: h };
    if params.clamp {
        spawn_at_core::geometry::clamp_to_bounds(
            raw_rect,
            wa,
            margin_top,
            margin_bottom,
            margin_left,
            margin_right,
            0,
        )
    } else {
        raw_rect
    }
}

// ============================================================================
// 3. Process Tree Introspection Helpers
// ============================================================================

/// Reads `/proc/{pid}/stat` on Linux to extract the Parent Process ID (PPID).
fn get_parent_pid(pid: u32) -> Option<u32> {
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
fn is_process_descendant(mut pid: u32, target_parent: u32) -> bool {
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

// ============================================================================
// 4. X11 Session Struct & Logical Implementations
// ============================================================================

/// Encapsulates an active, dedicated connection to the X11 display server.
struct X11Session {
    conn: RustConnection,
    screen_num: usize,
    root: Window,
    atoms: Atoms,
}

impl X11Session {
    // ------------------------------------------------------------------------
    // Block 1: Connection & Initialization
    // ------------------------------------------------------------------------

    /// Establishes a connection to the X11 display and interns all required EWMH/ICCCM atoms.
    fn connect() -> Result<Self, DriverError> {
        let (conn, screen_num) = RustConnection::connect(None).map_err(|e| {
            DriverError::Execution(format!("Cannot connect to X11 display: {}", e).into())
        })?;
        let root = conn.setup().roots[screen_num].root;
        let atoms = Atoms::new(&conn)
            .map_err(|e| DriverError::Execution(format!("Failed to intern atoms: {}", e).into()))?
            .reply()
            .map_err(|e| DriverError::Execution(format!("Failed to get atom replies: {}", e).into()))?;
        Ok(Self {
            conn,
            screen_num,
            root,
            atoms,
        })
    }

    // ------------------------------------------------------------------------
    // Block 2: Query Methods
    // ------------------------------------------------------------------------

    /// Queries connected monitor rectangles using RANDR extension protocol, falling back to screen size.
    fn fetch_monitors(&self) -> Result<Vec<Rect>, DriverError> {
        x11_debug!("[spawn-at-x11] Querying monitors via RANDR protocol...");

        // Try modern RANDR get_monitors
        if let Ok(reply) = self.conn.randr_get_monitors(self.root, true) {
            if let Ok(reply) = reply.reply() {
                if !reply.monitors.is_empty() {
                    let mut monitors = Vec::new();
                    for m in reply.monitors {
                        monitors.push(Rect {
                            x: m.x as i32,
                            y: m.y as i32,
                            width: m.width as u32,
                            height: m.height as u32,
                        });
                    }
                    return Ok(monitors);
                }
            }
        }

        // Fallback: RANDR CRTC resources
        if let Ok(res) = self.conn.randr_get_screen_resources_current(self.root) {
            if let Ok(res) = res.reply() {
                let mut monitors = Vec::new();
                for crtc in res.crtcs {
                    if let Ok(info) = self.conn.randr_get_crtc_info(crtc, res.config_timestamp) {
                        if let Ok(info) = info.reply() {
                            if info.width > 0 && info.height > 0 {
                                monitors.push(Rect {
                                    x: info.x as i32,
                                    y: info.y as i32,
                                    width: info.width as u32,
                                    height: info.height as u32,
                                });
                            }
                        }
                    }
                }
                if !monitors.is_empty() {
                    return Ok(monitors);
                }
            }
        }

        // Final fallback: Core X11 root screen geometry
        let screen = &self.conn.setup().roots[self.screen_num];
        Ok(vec![Rect {
            x: 0,
            y: 0,
            width: screen.width_in_pixels as u32,
            height: screen.height_in_pixels as u32,
        }])
    }

    /// Queries desktop workareas using `_NET_WORKAREA` property, falling back to physical monitors.
    fn fetch_workareas(&self) -> Result<Vec<Rect>, DriverError> {
        x11_debug!("[spawn-at-x11] Querying workareas via _NET_WORKAREA property...");

        if let Ok(cookie) = self.conn.get_property(
            false,
            self.root,
            self.atoms._NET_WORKAREA,
            AtomEnum::CARDINAL,
            0,
            1024,
        ) {
            if let Ok(reply) = cookie.reply() {
                if let Some(mut iter) = reply.value32() {
                    let mut workareas = Vec::new();
                    while let (Some(x), Some(y), Some(w), Some(h)) =
                        (iter.next(), iter.next(), iter.next(), iter.next())
                    {
                        if w > 0 && h > 0 {
                            workareas.push(Rect {
                                x: x as i32,
                                y: y as i32,
                                width: w,
                                height: h,
                            });
                        }
                    }
                    if !workareas.is_empty() {
                        return Ok(workareas);
                    }
                }
            }
        }
        self.fetch_monitors()
    }

    /// Queries all open windows on the X11 server via `_NET_CLIENT_LIST`.
    fn fetch_windows(&self) -> Result<Vec<WindowMetadata>, DriverError> {
        x11_debug!("[spawn-at-x11] Querying windows via _NET_CLIENT_LIST...");

        let client_list = self
            .conn
            .get_property(
                false,
                self.root,
                self.atoms._NET_CLIENT_LIST,
                AtomEnum::WINDOW,
                0,
                4096,
            )
            .map_err(|e| DriverError::Execution(e.into()))?
            .reply()
            .map_err(|e| DriverError::Execution(e.into()))?;

        let active_win = self
            .conn
            .get_property(
                false,
                self.root,
                self.atoms._NET_ACTIVE_WINDOW,
                AtomEnum::WINDOW,
                0,
                1,
            )
            .ok()
            .and_then(|c| c.reply().ok())
            .and_then(|r| r.value32().and_then(|mut it| it.next()));

        let mut windows = Vec::new();
        if let Some(win_ids) = client_list.value32() {
            for win in win_ids {
                if let Ok(meta) = self.get_window_metadata(win, active_win) {
                    windows.push(meta);
                }
            }
        }
        Ok(windows)
    }

    /// Introspects an individual window's properties: PID, titles, class names, geometry, and focus.
    fn get_window_metadata(
        &self,
        win: Window,
        active_win: Option<Window>,
    ) -> Result<WindowMetadata, DriverError> {
        let pid = self
            .conn
            .get_property(false, win, self.atoms._NET_WM_PID, AtomEnum::CARDINAL, 0, 1)
            .ok()
            .and_then(|c| c.reply().ok())
            .and_then(|r| r.value32().and_then(|mut it| it.next()));

        let mut title = String::new();
        if let Ok(c) = self
            .conn
            .get_property(false, win, self.atoms._NET_WM_NAME, self.atoms.UTF8_STRING, 0, 1024)
        {
            if let Ok(r) = c.reply() {
                if !r.value.is_empty() {
                    title = String::from_utf8_lossy(&r.value).to_string();
                }
            }
        }
        if title.is_empty() {
            if let Ok(c) = self
                .conn
                .get_property(false, win, AtomEnum::WM_NAME, AtomEnum::STRING, 0, 1024)
            {
                if let Ok(r) = c.reply() {
                    if !r.value.is_empty() {
                        title = String::from_utf8_lossy(&r.value).to_string();
                    }
                }
            }
        }

        let mut class = String::new();
        if let Ok(c) = self
            .conn
            .get_property(false, win, AtomEnum::WM_CLASS, AtomEnum::STRING, 0, 1024)
        {
            if let Ok(r) = c.reply() {
                if !r.value.is_empty() {
                    let parts: Vec<&[u8]> = r.value.split(|&b| b == 0).filter(|s| !s.is_empty()).collect();
                    if parts.len() >= 2 {
                        class = String::from_utf8_lossy(parts[1]).to_string();
                    } else if let Some(first) = parts.first() {
                        class = String::from_utf8_lossy(first).to_string();
                    }
                }
            }
        }

        let geom = self
            .conn
            .get_geometry(win)
            .map_err(|e| DriverError::Execution(e.into()))?
            .reply()
            .map_err(|e| DriverError::Execution(e.into()))?;

        let (x, y) = match self.conn.translate_coordinates(win, self.root, 0, 0) {
            Ok(c) => match c.reply() {
                Ok(r) => (r.dst_x as i32, r.dst_y as i32),
                Err(_) => (geom.x as i32, geom.y as i32),
            },
            Err(_) => (geom.x as i32, geom.y as i32),
        };

        let focused = active_win == Some(win);

        let mut maximized = false;
        let mut minimized = false;
        if let Ok(c) = self.conn.get_property(
            false,
            win,
            self.atoms._NET_WM_STATE,
            AtomEnum::ATOM,
            0,
            64,
        ) {
            if let Ok(r) = c.reply() {
                if let Some(atoms) = r.value32() {
                    let mut max_h = false;
                    let mut max_v = false;
                    for atom in atoms {
                        if atom == self.atoms._NET_WM_STATE_MAXIMIZED_HORZ {
                            max_h = true;
                        } else if atom == self.atoms._NET_WM_STATE_MAXIMIZED_VERT {
                            max_v = true;
                        } else if atom == self.atoms._NET_WM_STATE_HIDDEN {
                            minimized = true;
                        }
                    }
                    maximized = max_h && max_v;
                }
            }
        }

        Ok(WindowMetadata {
            id: Some(win as u64),
            pid,
            title,
            class,
            x,
            y,
            w: geom.width as i32,
            h: geom.height as i32,
            focused,
            maximized,
            minimized,
        })
    }

    // ------------------------------------------------------------------------
    // Block 3: Window Matching & Selection
    // ------------------------------------------------------------------------

    /// Resolves a user-provided target identifier (numeric hex/decimal ID, PID, or app class name)
    /// to a concrete X11 Window handle.
    fn resolve_target_window(&self, target_id: &str) -> Result<Window, DriverError> {
        let parsed_id = if let Some(hex) = target_id.strip_prefix("0x") {
            u32::from_str_radix(hex, 16).ok()
        } else {
            target_id.parse::<u32>().ok()
        };

        if let Some(win_id) = parsed_id {
            if let Ok(c) = self.conn.get_geometry(win_id) {
                if c.reply().is_ok() {
                    return Ok(win_id);
                }
            }
        }

        let windows = self.fetch_windows()?;
        let pid_opt = target_id.parse::<u32>().ok();
        let selector = WindowSelector {
            class: if pid_opt.is_none() {
                Some(target_id.to_string())
            } else {
                None
            },
            title: None,
            pid: pid_opt,
            focused: false,
        };

        let matched = target::resolve_target(&windows, &selector)
            .map_err(DriverError::TargetNotFound)?;
        let win_num = matched
            .id
            .ok_or_else(|| DriverError::TargetNotFound("Window missing ID".into()))?;
        Ok(win_num as Window)
    }

    /// Checks whether an individual window matches the target PID, Startup ID, or Class hint.
    fn check_single_window_match(
        &self,
        win: Window,
        child_pid: u32,
        app_hint: &str,
        startup_id: &str,
        is_preexisting: bool,
    ) -> bool {
        // Query _NET_WM_PID
        let mut win_pid: Option<u32> = None;
        if let Ok(pid_prop) = self.conn.get_property(
            false,
            win,
            self.atoms._NET_WM_PID,
            AtomEnum::CARDINAL,
            0,
            1,
        ) {
            if let Ok(reply) = pid_prop.reply() {
                if let Some(mut it) = reply.value32() {
                    win_pid = it.next();
                }
            }
        }

        // Query _NET_STARTUP_ID
        let mut win_startup_id: Option<String> = None;
        if let Ok(startup_prop) = self.conn.get_property(
            false,
            win,
            self.atoms._NET_STARTUP_ID,
            AtomEnum::ANY,
            0,
            512,
        ) {
            if let Ok(reply) = startup_prop.reply() {
                if !reply.value.is_empty() {
                    win_startup_id = Some(String::from_utf8_lossy(&reply.value).to_string());
                }
            }
        }

        // Debug logging for every match attempt
        x11_debug!(
            "[spawn-at-x11] Checking match for window {:?} (PID: {:?}, StartupID: {:?})",
            win, win_pid, win_startup_id
        );

        // 1. Direct PID or process-tree descendant match
        if let Some(pid) = win_pid {
            if pid == child_pid || is_process_descendant(pid, child_pid) {
                return true;
            }
        }

        // 2. Startup notification ID match
        if let Some(ref s_id) = win_startup_id {
            if !startup_id.is_empty() && s_id.contains(startup_id) {
                return true;
            }
        }

        // 3. Application class match (only for newly created windows, not pre-existing)
        if !is_preexisting && !app_hint.is_empty() && app_hint != "*" {
            if let Ok(class_prop) = self.conn.get_property(
                false,
                win,
                AtomEnum::WM_CLASS,
                AtomEnum::ANY,
                0,
                512,
            ) {
                if let Ok(reply) = class_prop.reply() {
                    let class_str = String::from_utf8_lossy(&reply.value).to_lowercase();
                    if class_str.contains(&app_hint.to_lowercase()) {
                        return true;
                    }
                }
            }
        }

        false
    }

    /// Resolves a matching client window from `win`, checking children in case `win` is a window manager frame.
    fn find_matching_window(
        &self,
        win: Window,
        child_pid: u32,
        app_hint: &str,
        startup_id: &str,
        is_preexisting: bool,
    ) -> Option<Window> {
        if self.check_single_window_match(win, child_pid, app_hint, startup_id, is_preexisting) {
            return Some(win);
        }

        // Check children if window manager reparented the client window into a frame
        if let Ok(tree) = self.conn.query_tree(win) {
            if let Ok(reply) = tree.reply() {
                for &child in &reply.children {
                    if self.check_single_window_match(child, child_pid, app_hint, startup_id, is_preexisting) {
                        return Some(child);
                    }
                }
            }
        }

        None
    }

    // ------------------------------------------------------------------------
    // Block 4: Window Actions & Management
    // ------------------------------------------------------------------------

    /// Moves and/or resizes a target window using EWMH `_NET_MOVERESIZE_WINDOW` ClientMessage,
    /// falling back to core X11 `configure_window`.
    fn move_resize(
        &self,
        win: Window,
        pos: Option<(i32, i32)>,
        size: Option<(u32, u32)>,
    ) -> Result<(), DriverError> {
        x11_debug!(
            "[spawn-at-x11] Sending _NET_MOVERESIZE_WINDOW for win={:?}, pos={:?}, size={:?}",
            win, pos, size
        );

        // Bit flags according to EWMH specification:
        // Bits 0-7: gravity (1 = NorthWest)
        // Bit 8: x present (0x0100)
        // Bit 9: y present (0x0200)
        // Bit 10: width present (0x0400)
        // Bit 11: height present (0x0800)
        let mut flags: u32 = 0x0001;
        let (x, y) = pos.unwrap_or((0, 0));
        if pos.is_some() {
            flags |= 0x0100 | 0x0200;
        }
        let (w, h) = size.unwrap_or((0, 0));
        if size.is_some() {
            flags |= 0x0400 | 0x0800;
        }

        let event = ClientMessageEvent::new(
            32,
            win,
            self.atoms._NET_MOVERESIZE_WINDOW,
            [flags, x as u32, y as u32, w, h],
        );
        let _ = self.conn.send_event(
            false,
            self.root,
            EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY,
            event,
        );

        // Core X11 configure_window fallback
        let mut aux = ConfigureWindowAux::new();
        if pos.is_some() {
            aux = aux.x(x).y(y);
        }
        if size.is_some() {
            aux = aux.width(w).height(h);
        }
        let _ = self.conn.configure_window(win, &aux);
        self.conn.flush().map_err(|e| DriverError::Execution(e.into()))?;
        Ok(())
    }

    /// Activates and raises a window using EWMH `_NET_ACTIVE_WINDOW` ClientMessage.
    fn activate_window(&self, win: Window) -> Result<(), DriverError> {
        x11_debug!("[spawn-at-x11] Sending _NET_ACTIVE_WINDOW ClientMessage for win={:?}", win);

        let event = ClientMessageEvent::new(
            32,
            win,
            self.atoms._NET_ACTIVE_WINDOW,
            [2, x11rb::CURRENT_TIME, 0, 0, 0], // Source indication = 2 (pager / taskbar / tool)
        );
        let _ = self.conn.send_event(
            false,
            self.root,
            EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY,
            event,
        );
        let _ = self.conn.set_input_focus(InputFocus::POINTER_ROOT, win, x11rb::CURRENT_TIME);
        self.conn.flush().map_err(|e| DriverError::Execution(e.into()))?;
        Ok(())
    }

    // ------------------------------------------------------------------------
    // Block 5: Spawn Interception Pipeline
    // ------------------------------------------------------------------------

    /// Intercepts a freshly spawned process's window natively on X11:
    /// 1. Subscribes root window to `EventMask::SUBSTRUCTURE_NOTIFY`.
    /// 2. Records pre-existing windows to avoid false positives.
    /// 3. Intercepts incoming events (`CreateNotify`, `MapNotify`, `PropertyNotify`) with non-busy polling.
    /// 4. Injects `WM_NORMAL_HINTS` with `USPosition | PPosition` and `_NET_WM_USER_TIME = 0` before map.
    /// 5. Awaits natural toolkit dimension negotiation.
    /// 6. Reads final geometry and applies precise anchor coordinate positioning.
    fn handle_spawn_interception(
        &self,
        child_pid: u32,
        batch: &Batch,
    ) -> Result<(), DriverError> {
        let entry = match batch.entries.first() {
            Some(e) => e,
            None => return Ok(()),
        };

        x11_debug!(
            "[spawn-at-x11] Starting interception for PID: {} | App: {} | Startup ID: {}",
            child_pid, entry.app_hint, entry.key
        );

        // Step 1: Calculate provisional target coordinates using spawn-at-core geometry math
        let (prov_w, prov_h) = entry.placement.size.unwrap_or((800, 600));
        let prov_rect = calculate_rect_from_placement(&entry.placement, prov_w, prov_h);

        // Step 2: Register SubstructureNotify on the root window to capture window lifecycle events
        use x11rb::protocol::xproto::ChangeWindowAttributesAux;
        let aux = ChangeWindowAttributesAux::new().event_mask(EventMask::SUBSTRUCTURE_NOTIFY);
        let _ = self.conn.change_window_attributes(self.root, &aux);
        let _ = self.conn.flush();

        x11_debug!("[spawn-at-x11] Listening for X11 CreateNotify/MapRequest events on root window...");

        // Record pre-existing windows on the display
        let mut pre_existing: HashSet<Window> = HashSet::new();
        if let Ok(tree) = self.conn.query_tree(self.root) {
            if let Ok(tree_reply) = tree.reply() {
                for &win in &tree_reply.children {
                    pre_existing.insert(win);
                }
            }
        }

        let mut target_win: Option<Window> = None;
        let start_time = Instant::now();
        let timeout = Duration::from_millis(5000);
        let mut candidate_windows: Vec<Window> = Vec::new();

        // Check if the window already appeared right before event listener registration
        for &win in &pre_existing {
            if let Some(matched) = self.find_matching_window(win, child_pid, &entry.app_hint, &entry.key, true) {
                x11_debug!("[spawn-at-x11] MATCH FOUND! Window ID: {}", matched);
                target_win = Some(matched);
                break;
            }
        }

        // Step 3: Event-driven interception loop using non-busy polling
        while target_win.is_none() && start_time.elapsed() < timeout {
            while let Ok(Some(event)) = self.conn.poll_for_event() {
                let win_id = match &event {
                    x11rb::protocol::Event::CreateNotify(e) => Some(e.window),
                    x11rb::protocol::Event::MapRequest(e) => Some(e.window),
                    x11rb::protocol::Event::MapNotify(e) => Some(e.window),
                    x11rb::protocol::Event::ConfigureNotify(e) => Some(e.window),
                    x11rb::protocol::Event::PropertyNotify(e) => Some(e.window),
                    _ => None,
                };

                x11_debug!("[spawn-at-x11] Event received: {:?} for window ID: {:?}", event, win_id);

                if let Some(w) = win_id {
                    if !pre_existing.contains(&w) && !candidate_windows.contains(&w) {
                        candidate_windows.push(w);
                        // Subscribe to PropertyNotify on newly created candidate windows
                        let w_aux = ChangeWindowAttributesAux::new()
                            .event_mask(EventMask::PROPERTY_CHANGE | EventMask::STRUCTURE_NOTIFY);
                        let _ = self.conn.change_window_attributes(w, &w_aux);
                        let _ = self.conn.flush();
                    }

                    if let Some(matched) = self.find_matching_window(
                        w,
                        child_pid,
                        &entry.app_hint,
                        &entry.key,
                        pre_existing.contains(&w),
                    ) {
                        x11_debug!("[spawn-at-x11] MATCH FOUND! Window ID: {}", matched);
                        target_win = Some(matched);
                        break;
                    }
                }
            }

            if target_win.is_some() {
                break;
            }

            // Periodically re-evaluate candidate windows for asynchronously updated properties
            for &cand in &candidate_windows {
                if let Some(matched) = self.find_matching_window(cand, child_pid, &entry.app_hint, &entry.key, false) {
                    x11_debug!("[spawn-at-x11] MATCH FOUND! Window ID: {}", matched);
                    target_win = Some(matched);
                    break;
                }
            }

            if target_win.is_some() {
                break;
            }

            // Kernel sleep to avoid CPU busy-polling (0% CPU impact)
            std::thread::sleep(Duration::from_millis(15));
        }

        let win = match target_win {
            Some(w) => w,
            None => {
                x11_debug!(
                    "[spawn-at-x11] ERROR: Timeout reached. No window matched PID {} or Startup ID {}.",
                    child_pid, entry.key
                );
                return Ok(());
            }
        };

        // Step 4: Inject WM_NORMAL_HINTS (ICCCM XSizeHints) with USPosition | PPosition flags
        // before the window maps to eliminate the default top-left flash.
        x11_debug!(
            "[spawn-at-x11] Setting WM_NORMAL_HINTS to x={}, y={}, w={}, h={}",
            prov_rect.x, prov_rect.y, prov_w, prov_h
        );

        let mut hints = [0u32; 18];
        let mut flags = (1 << 0) | (1 << 2); // USPosition | PPosition
        if let Some((w, h)) = entry.placement.size {
            flags |= (1 << 1) | (1 << 3); // USSize | PSize
            hints[3] = w;
            hints[4] = h;
        }
        hints[0] = flags;
        hints[1] = prov_rect.x as u32;
        hints[2] = prov_rect.y as u32;

        let _ = self.conn.change_property32(
            PropMode::REPLACE,
            win,
            AtomEnum::WM_NORMAL_HINTS,
            self.atoms.WM_SIZE_HINTS,
            &hints,
        );

        // Suppress focus stealing races by setting _NET_WM_USER_TIME = 0
        let _ = self.conn.change_property32(
            PropMode::REPLACE,
            win,
            self.atoms._NET_WM_USER_TIME,
            AtomEnum::CARDINAL,
            &[0],
        );

        // Provisional configure placement
        let aux = ConfigureWindowAux::new().x(prov_rect.x).y(prov_rect.y);
        let _ = self.conn.configure_window(win, &aux);
        let _ = self.conn.flush();

        // Step 5: Await natural window map and geometry settlement to allow toolkit layout negotiation
        let map_timeout = Duration::from_millis(3000);
        let map_start = Instant::now();
        let mut is_viewable = false;

        while map_start.elapsed() < map_timeout {
            if let Ok(attrs) = self.conn.get_window_attributes(win) {
                if let Ok(attrs_reply) = attrs.reply() {
                    if attrs_reply.map_state == MapState::VIEWABLE {
                        is_viewable = true;
                    }
                }
            }

            while let Ok(Some(event)) = self.conn.poll_for_event() {
                if let x11rb::protocol::Event::MapNotify(e) = event {
                    if e.window == win {
                        is_viewable = true;
                    }
                }
            }

            if is_viewable {
                // If user specified an explicit size, we don't need to wait for natural size negotiation
                if entry.placement.size.is_some() {
                    break;
                }

                // Check if geometry has expanded beyond placeholder dummy size (e.g. 10x10)
                if let Ok(c) = self.conn.get_geometry(win) {
                    if let Ok(g) = c.reply() {
                        if g.width > 50 && g.height > 50 {
                            // Give toolkit a brief grace period to finish any immediate layout settle
                            std::thread::sleep(Duration::from_millis(40));
                            break;
                        }
                    }
                }
            }

            std::thread::sleep(Duration::from_millis(15));
        }

        // Step 6: Query actual settled mapped width and height
        let actual_geom = match self.conn.get_geometry(win) {
            Ok(c) => match c.reply() {
                Ok(g) => g,
                Err(_) => return Ok(()),
            },
            Err(_) => return Ok(()),
        };
        let mut actual_w = actual_geom.width as u32;
        let mut actual_h = actual_geom.height as u32;

        // Fallback if window is still dummy placeholder size (< 50px)
        if actual_w <= 50 || actual_h <= 50 {
            actual_w = prov_w;
            actual_h = prov_h;
        }

        x11_debug!("[spawn-at-x11] Actual mapped geometry: w={}, h={}", actual_w, actual_h);

        // Step 7: Recalculate anchor coordinates with negotiated dimensions
        let final_rect = calculate_rect_from_placement(&entry.placement, actual_w, actual_h);
        x11_debug!(
            "[spawn-at-x11] Final calculated position: x={}, y={}",
            final_rect.x, final_rect.y
        );

        // Step 8: Apply final placement adjustment via _NET_MOVERESIZE_WINDOW
        x11_debug!(
            "[spawn-at-x11] Applying final move_resize to x={}, y={}",
            final_rect.x, final_rect.y
        );
        let _ = self.move_resize(win, Some((final_rect.x, final_rect.y)), entry.placement.size);

        // Apply batch focus policy
        if batch.focus == FocusIntent::Exclusive {
            let _ = self.activate_window(win);
        }

        let _ = self.conn.flush();
        Ok(())
    }
}

// ============================================================================
// 5. Driver Trait & CompositorBackend Implementations
// ============================================================================

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
                .map_err(|e| DriverError::Execution(format!("Failed to query pointer: {}", e).into()))?
                .reply()
                .map_err(|e| DriverError::Execution(format!("Failed to read pointer reply: {}", e).into()))?;
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
            session.move_resize(
                win,
                Some((rect.x, rect.y)),
                Some((rect.width, rect.height)),
            )
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
    async fn set_window_state(&self, target_id: &str, state: WindowState) -> Result<(), DriverError> {
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
            session.conn.flush().map_err(|e| DriverError::Execution(e.into()))?;
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
            session.conn.flush().map_err(|e| DriverError::Execution(e.into()))?;
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

// ============================================================================
// 6. Unit Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_x11_driver_capabilities() {
        let driver = X11Driver;
        assert_eq!(driver.name(), "X11");
        assert!(driver.supports_runtime_transform());
    }

    #[tokio::test]
    async fn test_x11_arm_generates_tokens() {
        use spawn_at_core::driver::{Batch, Entry, Reveal, Urgency};

        let driver = X11Driver;
        let batch = Batch {
            id: 1,
            entries: vec![Entry {
                key: "test-token-123".into(),
                app_hint: "test-app".into(),
                placement: PlacementParams::default(),
            }],
            reveal: Reveal::Together,
            focus: FocusIntent::Exclusive,
            urgency: Urgency::Normal,
            deadline: Duration::from_millis(5000),
        };

        let armed = driver.arm(batch).await.unwrap();
        assert!(armed
            .launch_env
            .iter()
            .any(|(k, v)| k == "DESKTOP_STARTUP_ID" && v == "test-token-123"));
        assert!(armed
            .launch_env
            .iter()
            .any(|(k, v)| k == "XDG_ACTIVATION_TOKEN" && v == "test-token-123"));
    }

    #[test]
    fn test_calculate_rect_center_anchor() {
        use spawn_at_core::geometry::Anchor;

        let params = PlacementParams {
            anchor: Some(Anchor::Center),
            workarea: Rect {
                x: 0,
                y: 0,
                width: 1920,
                height: 1080,
            },
            ..Default::default()
        };

        let rect = calculate_rect_from_placement(&params, 800, 600);
        assert_eq!(rect.width, 800);
        assert_eq!(rect.height, 600);
        assert_eq!(rect.x, (1920 - 800) / 2); // 560
        assert_eq!(rect.y, (1080 - 600) / 2); // 240
    }

    #[test]
    fn test_calculate_rect_top_left_with_margins() {
        use spawn_at_core::geometry::Anchor;

        let params = PlacementParams {
            anchor: Some(Anchor::TopLeft),
            margin_left: Some(50),
            margin_top: Some(30),
            workarea: Rect {
                x: 100,
                y: 50,
                width: 1920,
                height: 1080,
            },
            ..Default::default()
        };

        let rect = calculate_rect_from_placement(&params, 400, 300);
        assert_eq!(rect.x, 100 + 50); // 150
        assert_eq!(rect.y, 50 + 30);  // 80
        assert_eq!(rect.width, 400);
        assert_eq!(rect.height, 300);
    }

    #[test]
    fn test_calculate_rect_explicit_pos_and_pivot() {
        use spawn_at_core::geometry::Pivot;

        let params = PlacementParams {
            pos: Some((500, 400)),
            pivot: Some(Pivot::Center),
            workarea: Rect {
                x: 0,
                y: 0,
                width: 1920,
                height: 1080,
            },
            ..Default::default()
        };

        let rect = calculate_rect_from_placement(&params, 200, 100);
        assert_eq!(rect.x, 500 - 100); // 400
        assert_eq!(rect.y, 400 - 50);  // 350
        assert_eq!(rect.width, 200);
        assert_eq!(rect.height, 100);
    }

    #[test]
    fn test_compare_x11_and_mechanics_anchor_outputs() {
        use spawn_at_core::geometry::{Anchor, Pivot};
        use crate::platform::linux::gnome::mechanics::calculate_placement;

        let wa = Rect {
            x: 100,
            y: 50,
            width: 1920,
            height: 1080,
        };

        let anchors = [
            Anchor::Center,
            Anchor::TopLeft,
            Anchor::TopRight,
            Anchor::BottomLeft,
            Anchor::BottomRight,
            Anchor::Top,
            Anchor::Bottom,
            Anchor::Left,
            Anchor::Right,
        ];

        let pivots = [
            Pivot::TopLeft,
            Pivot::TopRight,
            Pivot::BottomLeft,
            Pivot::BottomRight,
            Pivot::Center,
        ];

        for anchor in &anchors {
            for pivot in &pivots {
                let params = PlacementParams {
                    anchor: Some(*anchor),
                    pivot: Some(*pivot),
                    margin: 16,
                    margin_top: Some(20),
                    margin_bottom: Some(25),
                    margin_left: Some(30),
                    margin_right: Some(35),
                    offset: Some((10, -15)),
                    workarea: wa,
                    ..Default::default()
                };

                let win_w = 600;
                let win_h = 400;

                // 1. Output from native X11 duplicate
                let x11_rect = calculate_rect_from_placement(&params, win_w, win_h);

                // 2. Output from mechanics.rs
                let payload = calculate_placement(&params, win_w, win_h);

                // In GNOME Shell extension, the raw coordinate formula is:
                let ml = params.margin_left.unwrap();
                let mr = params.margin_right.unwrap();
                let mt = params.margin_top.unwrap();
                let mb = params.margin_bottom.unwrap();

                let raw_x = payload.screen_anchor_x
                    - (payload.pivot_u * (win_w as f64)).round() as i32
                    + payload.offset_x;
                let raw_y = payload.screen_anchor_y
                    - (payload.pivot_v * (win_h as f64)).round() as i32
                    + payload.offset_y;

                let min_x = wa.x + ml;
                let min_y = wa.y + mt;
                let max_x = (wa.x + (wa.width as i32) - mr - (win_w as i32)).max(min_x);
                let max_y = (wa.y + (wa.height as i32) - mb - (win_h as i32)).max(min_y);

                let extension_x = raw_x.clamp(min_x, max_x);
                let extension_y = raw_y.clamp(min_y, max_y);

                // Discrepancy Note (F25 / Plan Change 3):
                // x11.rs:181 passes `params.margin` as `global_margin` into `clamp_to_bounds`,
                // which computes `bound_left + global_margin`. Because `bound_left` already
                // contains `params.margin_left.unwrap_or(params.margin)`, X11 double-adds
                // `params.margin` when clamping!
                // Mechanics / GNOME Extension computes `min_x = wa.x + ml` (single margin).
                //
                // With the double-margin bug fixed, X11 output exactly matches GNOME Extension / core:
                assert_eq!(
                    x11_rect.x, extension_x,
                    "X11 calculation mismatch with extension for anchor {:?}, pivot {:?}",
                    anchor, pivot
                );
                assert_eq!(
                    x11_rect.y, extension_y,
                    "X11 calculation mismatch with extension for anchor {:?}, pivot {:?}",
                    anchor, pivot
                );
            }
        }
    }

    #[test]
    fn test_x11_margin_24_top_right_leaves_24px_not_48px() {
        use spawn_at_core::geometry::Anchor;

        let wa = Rect {
            x: 0,
            y: 0,
            width: 1920,
            height: 1080,
        };
        let params = PlacementParams {
            anchor: Some(Anchor::TopRight),
            margin: 24,
            workarea: wa,
            ..Default::default()
        };

        let win_w = 600;
        let win_h = 400;
        let rect = calculate_rect_from_placement(&params, win_w, win_h);

        // Right edge distance: 1920 - (rect.x + win_w)
        let right_margin = 1920 - (rect.x + (win_w as i32));
        // Top edge distance: rect.y
        let top_margin = rect.y;

        assert_eq!(right_margin, 24, "Right margin must be 24px, not 48px");
        assert_eq!(top_margin, 24, "Top margin must be 24px, not 48px");
    }

    #[test]
    fn test_x11_clamp_false_bypasses_bounds() {
        let params = PlacementParams {
            pos: Some((2500, -500)),
            size: Some((400, 300)),
            workarea: Rect { x: 0, y: 0, width: 1920, height: 1080 },
            clamp: false,
            ..Default::default()
        };
        let rect = calculate_rect_from_placement(&params, 400, 300);
        assert_eq!(rect.x, 2500);
        assert_eq!(rect.y, -500);

        let clamped_params = PlacementParams {
            pos: Some((2500, -500)),
            size: Some((400, 300)),
            workarea: Rect { x: 0, y: 0, width: 1920, height: 1080 },
            clamp: true,
            ..Default::default()
        };
        let clamped_rect = calculate_rect_from_placement(&clamped_params, 400, 300);
        assert_ne!(clamped_rect.x, 2500);
        assert_ne!(clamped_rect.y, -500);
    }
}
