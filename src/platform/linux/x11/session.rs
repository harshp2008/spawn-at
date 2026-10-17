//! # X11 Display Session and Window Operations
//!
//! [`X11Session`] is one blocking x11rb connection plus the interned atoms, with the queries
//! (monitors, work areas, windows) and the EWMH actions (move/resize, activate) the X11 driver
//! needs. `X11Driver` (`mod.rs`) opens a fresh session per call inside `spawn_blocking`, and
//! `interception.rs` adds the spawn loop on top. Nothing here is async and nothing keeps a
//! session between calls.
//!
//! Under XWayland the same code sees only XWayland windows. Most actions are sent as EWMH client
//! messages and their result is not read back: a window manager that ignores them gives `Ok`.
//! Flash behaviour of X11 and XWayland windows is recorded in `docs/TEST_LOG.md` (sections 6.15
//! and 6.17), not in this file.

use super::atoms::{x11_debug, Atoms};
use crate::platform::linux::process_tree::is_process_descendant;
use crate::platform::{DriverError, LayoutRect, MonitorInfo, MonitorLayout, Rect, WindowMetadata};
use crate::target::{self, WindowSelector};
use x11rb::connection::Connection;
use x11rb::protocol::randr::ConnectionExt as _;
use x11rb::protocol::xproto::{
    AtomEnum, ClientMessageEvent, ConfigureWindowAux, ConnectionExt as _, EventMask, InputFocus,
    Window,
};
use x11rb::rust_connection::RustConnection;

/// One connection to the X11 display (`$DISPLAY`), its default screen's root window and the
/// interned atoms. The fields are crate-visible for `X11Driver`, which sends a few client
/// messages itself.
pub struct X11Session {
    pub(crate) conn: RustConnection,
    pub(crate) screen_num: usize,
    pub(crate) root: Window,
    pub(crate) atoms: Atoms,
}

impl X11Session {
    // ------------------------------------------------------------------------
    // Block 1: Connection & Initialization
    // ------------------------------------------------------------------------

    /// Connects with `RustConnection::connect(None)` (so `$DISPLAY` decides) and interns the atoms
    /// in `atoms.rs`. Errors: `DriverError::Execution` if the connection or the interning fails.
    pub fn connect() -> Result<Self, DriverError> {
        let (conn, screen_num) = RustConnection::connect(None).map_err(|e| {
            DriverError::Execution(format!("Cannot connect to X11 display: {}", e).into())
        })?;
        let root = conn.setup().roots[screen_num].root;
        let atoms = Atoms::new(&conn)
            .map_err(|e| DriverError::Execution(format!("Failed to intern atoms: {}", e).into()))?
            .reply()
            .map_err(|e| {
                DriverError::Execution(format!("Failed to get atom replies: {}", e).into())
            })?;
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

    /// Monitor rectangles in pixels. Tries RANDR `GetMonitors`, then the RANDR CRTCs that have a
    /// nonzero size, then the root screen as one rectangle at (0, 0). A failing RANDR request is
    /// not reported; it falls to the next source, so this never returns `Err`.
    pub fn fetch_monitors(&self) -> Result<Vec<Rect>, DriverError> {
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

    /// Work areas from the root window's `_NET_WORKAREA` (up to 1024 32-bit values, entries with a
    /// zero width or height dropped); when that is missing or empty, the monitors from
    /// `fetch_monitors`. `_NET_WORKAREA` is one rectangle per virtual desktop in the EWMH
    /// convention, so the entries are not necessarily one per monitor (not verified per window
    /// manager).
    pub fn fetch_workareas(&self) -> Result<Vec<Rect>, DriverError> {
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

    /// The monitors of `fetch_monitors` with a work area each. The work area is the intersection of
    /// the screen rectangle with the FIRST rectangle of `_NET_WORKAREA`; where they do not overlap
    /// (or the property is missing) the work area is the whole screen. `primary` is set for index
    /// 0, and `name` and `scale` are `None`. The returned `note` says the work area is an
    /// approximation. Never returns `Err` in practice, since `fetch_monitors` does not.
    pub fn fetch_layout(&self) -> Result<MonitorLayout, DriverError> {
        let screens = self.fetch_monitors()?;
        let global_wa = if let Ok(cookie) = self.conn.get_property(
            false,
            self.root,
            self.atoms._NET_WORKAREA,
            AtomEnum::CARDINAL,
            0,
            4,
        ) {
            cookie.reply().ok().and_then(|reply| {
                let mut iter = reply.value32()?;
                let (x, y, w, h) = (iter.next()?, iter.next()?, iter.next()?, iter.next()?);
                Some(Rect {
                    x: x as i32,
                    y: y as i32,
                    width: w,
                    height: h,
                })
            })
        } else {
            None
        };

        let monitors = screens
            .into_iter()
            .enumerate()
            .map(|(i, screen_rect)| {
                let wa = if let Some(g) = global_wa {
                    let rx = screen_rect.x.max(g.x);
                    let ry = screen_rect.y.max(g.y);
                    let r_right =
                        (screen_rect.x + screen_rect.width as i32).min(g.x + g.width as i32);
                    let r_bottom =
                        (screen_rect.y + screen_rect.height as i32).min(g.y + g.height as i32);
                    let rw = if r_right > rx {
                        (r_right - rx) as u32
                    } else {
                        screen_rect.width
                    };
                    let rh = if r_bottom > ry {
                        (r_bottom - ry) as u32
                    } else {
                        screen_rect.height
                    };
                    LayoutRect {
                        x: rx,
                        y: ry,
                        w: rw,
                        h: rh,
                    }
                } else {
                    LayoutRect::from(screen_rect)
                };

                let screen_layout = LayoutRect::from(screen_rect);
                let insets = MonitorInfo::compute_insets(Some(screen_layout), Some(wa));
                MonitorInfo {
                    index: i,
                    name: None,
                    primary: i == 0,
                    scale: None,
                    screen: Some(screen_layout),
                    workarea: Some(wa),
                    insets,
                }
            })
            .collect();

        Ok(MonitorLayout {
            schema_version: 1,
            monitors,
            note: Some(
                "Per-monitor work area is an approximation derived from _NET_WORKAREA.".to_string(),
            ),
        })
    }

    /// The windows in the root's `_NET_CLIENT_LIST` (the first 4096 entries), each described by
    /// `get_window_metadata`. A window whose metadata cannot be read (for example it closed since)
    /// is left out without a message. Errors: `Execution` if the list itself cannot be read.
    pub fn fetch_windows(&self) -> Result<Vec<WindowMetadata>, DriverError> {
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

    /// One window as `WindowMetadata`. `pid` is `_NET_WM_PID`; the title is `_NET_WM_NAME`, else
    /// `WM_NAME`; `class` is the SECOND string of `WM_CLASS` (the class, not the instance), or
    /// the first if there is only one. `x`/`y` are the client area's position in root
    /// coordinates (`translate_coordinates`, else the plain geometry) and `w`/`h` the client
    /// size, without any frame. `focused` compares with `active_win`. `maximized` needs both
    /// maximize atoms; `minimized` is the `_NET_WM_STATE_HIDDEN` atom. `app_id`, `monitor` and
    /// `workspace` are always `None`. Errors: `Execution` if the geometry request fails; the
    /// other properties fail soft to empty or `None`.
    pub fn get_window_metadata(
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
        if let Ok(c) = self.conn.get_property(
            false,
            win,
            self.atoms._NET_WM_NAME,
            self.atoms.UTF8_STRING,
            0,
            1024,
        ) {
            if let Ok(r) = c.reply() {
                if !r.value.is_empty() {
                    title = String::from_utf8_lossy(&r.value).to_string();
                }
            }
        }
        if title.is_empty() {
            if let Ok(c) =
                self.conn
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
        if let Ok(c) =
            self.conn
                .get_property(false, win, AtomEnum::WM_CLASS, AtomEnum::STRING, 0, 1024)
        {
            if let Ok(r) = c.reply() {
                if !r.value.is_empty() {
                    let parts: Vec<&[u8]> = r
                        .value
                        .split(|&b| b == 0)
                        .filter(|s| !s.is_empty())
                        .collect();
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
        if let Ok(c) =
            self.conn
                .get_property(false, win, self.atoms._NET_WM_STATE, AtomEnum::ATOM, 0, 64)
        {
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

        let state = if minimized {
            Some("minimized".to_string())
        } else if maximized {
            Some("maximized".to_string())
        } else {
            Some("normal".to_string())
        };

        Ok(WindowMetadata {
            id: Some(win as u64),
            pid,
            title,
            class,
            app_id: None,
            x,
            y,
            w: geom.width as i32,
            h: geom.height as i32,
            rel: None,
            area: None,
            focused,
            maximized,
            minimized,
            monitor: None,
            state,
            workspace: None,
        })
    }

    // ------------------------------------------------------------------------
    // Block 3: Window Matching & Selection
    // ------------------------------------------------------------------------

    /// Turns a target string into a window id. A `0x` hex number or a decimal number is tried as
    /// a window id first (it counts if the server answers `GetGeometry` for it); otherwise a
    /// decimal number is a pid and anything else a class, matched by `target::resolve_target`
    /// over `fetch_windows`. A pid that happens to equal an existing window id is therefore
    /// taken as that window. Errors: `TargetNotFound`.
    pub fn resolve_target_window(&self, target_id: &str) -> Result<Window, DriverError> {
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
            id: None,
            class: if pid_opt.is_none() {
                Some(target_id.to_string())
            } else {
                None
            },
            title: None,
            pid: pid_opt,
            focused: false,
        };

        let matched =
            target::resolve_target(&windows, &selector).map_err(DriverError::TargetNotFound)?;
        let win_num = matched
            .id
            .ok_or_else(|| DriverError::TargetNotFound("Window missing ID".into()))?;
        Ok(win_num as Window)
    }

    /// True when `win` belongs to the launched program. Checks, in order: (1) `_NET_WM_PID` equals
    /// `child_pid` or descends from it (this check also applies to `is_preexisting` windows);
    /// (2) `_NET_STARTUP_ID` contains `startup_id` as a substring (when `startup_id` is not
    /// empty); (3) only when `!is_preexisting` and `app_hint` is neither empty nor `*`: the raw
    /// `WM_CLASS` text, lower-cased, contains the lower-cased hint as a substring. Rule 3 is a
    /// plain substring test, looser than `commands::match_window_app_hint`.
    pub fn check_single_window_match(
        &self,
        win: Window,
        child_pid: u32,
        app_hint: &str,
        startup_id: &str,
        is_preexisting: bool,
    ) -> bool {
        // Query _NET_WM_PID
        let mut win_pid: Option<u32> = None;
        if let Ok(pid_prop) =
            self.conn
                .get_property(false, win, self.atoms._NET_WM_PID, AtomEnum::CARDINAL, 0, 1)
        {
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
            win,
            win_pid,
            win_startup_id
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
            if let Ok(class_prop) =
                self.conn
                    .get_property(false, win, AtomEnum::WM_CLASS, AtomEnum::ANY, 0, 512)
            {
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

    /// `win` if it matches (see `check_single_window_match`), else the first direct child of
    /// `win` that matches (one level only: the client inside a window-manager frame), else `None`.
    pub fn find_matching_window(
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
                    if self.check_single_window_match(
                        child,
                        child_pid,
                        app_hint,
                        startup_id,
                        is_preexisting,
                    ) {
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

    /// Moves and/or resizes `win` (pixels; `pos` is the top-left corner). It sends BOTH an EWMH
    /// `_NET_MOVERESIZE_WINDOW` client message (gravity NorthWest) and a core `configure_window`
    /// with the same values, one after the other; the second is not a fallback for a failure of
    /// the first. The results of both requests are ignored and only the final flush can fail
    /// (`Execution`), so a window manager that refuses is not reported.
    pub fn move_resize(
        &self,
        win: Window,
        pos: Option<(i32, i32)>,
        size: Option<(u32, u32)>,
    ) -> Result<(), DriverError> {
        x11_debug!(
            "[spawn-at-x11] Sending _NET_MOVERESIZE_WINDOW for win={:?}, pos={:?}, size={:?}",
            win,
            pos,
            size
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
        self.conn
            .flush()
            .map_err(|e| DriverError::Execution(e.into()))?;
        Ok(())
    }

    /// Asks for `win` to be activated: an EWMH `_NET_ACTIVE_WINDOW` client message with source
    /// indication 2, then also `SetInputFocus` on the window. Both results are ignored; only the
    /// flush can fail (`Execution`).
    pub fn activate_window(&self, win: Window) -> Result<(), DriverError> {
        x11_debug!(
            "[spawn-at-x11] Sending _NET_ACTIVE_WINDOW ClientMessage for win={:?}",
            win
        );

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
        let _ = self
            .conn
            .set_input_focus(InputFocus::POINTER_ROOT, win, x11rb::CURRENT_TIME);
        self.conn
            .flush()
            .map_err(|e| DriverError::Execution(e.into()))?;
        Ok(())
    }
}
