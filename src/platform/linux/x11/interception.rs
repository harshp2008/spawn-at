//! # X11 Spawn Interception Pipeline

use super::atoms::x11_debug;
use super::calculate_rect_from_placement;
use super::session::X11Session;
use crate::platform::{Batch, DriverError};
use spawn_at_core::driver::FocusIntent;
use std::collections::HashSet;
use std::time::{Duration, Instant};
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{
    AtomEnum, ChangeWindowAttributesAux, ConfigureWindowAux, ConnectionExt as _, EventMask,
    MapState, PropMode, Window,
};
use x11rb::wrapper::ConnectionExt as _;

impl X11Session {
    /// Intercepts a freshly spawned process's window natively on X11:
    /// 1. Subscribes root window to `EventMask::SUBSTRUCTURE_NOTIFY`.
    /// 2. Records pre-existing windows to avoid false positives.
    /// 3. Intercepts incoming events (`CreateNotify`, `MapNotify`, `PropertyNotify`) with non-busy polling.
    /// 4. Injects `WM_NORMAL_HINTS` with `USPosition | PPosition` and `_NET_WM_USER_TIME = 0` before map.
    /// 5. Awaits natural toolkit dimension negotiation.
    /// 6. Reads final geometry and applies precise anchor coordinate positioning.
    pub fn handle_spawn_interception(
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
            child_pid,
            entry.app_hint,
            entry.key
        );

        // Step 1: Calculate provisional target coordinates using spawn-at-core geometry math
        let (prov_w, prov_h) = entry.placement.size.unwrap_or((800, 600));
        let prov_rect = calculate_rect_from_placement(&entry.placement, prov_w, prov_h);

        // Step 2: Register SubstructureNotify on the root window to capture window lifecycle events
        let aux = ChangeWindowAttributesAux::new().event_mask(EventMask::SUBSTRUCTURE_NOTIFY);
        let _ = self.conn.change_window_attributes(self.root, &aux);
        let _ = self.conn.flush();

        x11_debug!(
            "[spawn-at-x11] Listening for X11 CreateNotify/MapRequest events on root window..."
        );

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
            if let Some(matched) =
                self.find_matching_window(win, child_pid, &entry.app_hint, &entry.key, true)
            {
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

                x11_debug!(
                    "[spawn-at-x11] Event received: {:?} for window ID: {:?}",
                    event,
                    win_id
                );

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
                if let Some(matched) =
                    self.find_matching_window(cand, child_pid, &entry.app_hint, &entry.key, false)
                {
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
            prov_rect.x,
            prov_rect.y,
            prov_w,
            prov_h
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

        x11_debug!(
            "[spawn-at-x11] Actual mapped geometry: w={}, h={}",
            actual_w,
            actual_h
        );

        // Step 7: Recalculate anchor coordinates with negotiated dimensions
        let final_rect = calculate_rect_from_placement(&entry.placement, actual_w, actual_h);
        x11_debug!(
            "[spawn-at-x11] Final calculated position: x={}, y={}",
            final_rect.x,
            final_rect.y
        );

        // Step 8: Apply final placement adjustment via _NET_MOVERESIZE_WINDOW
        x11_debug!(
            "[spawn-at-x11] Applying final move_resize to x={}, y={}",
            final_rect.x,
            final_rect.y
        );
        let _ = self.move_resize(
            win,
            Some((final_rect.x, final_rect.y)),
            entry.placement.size,
        );

        // Apply batch focus policy
        if batch.focus == FocusIntent::Exclusive {
            let _ = self.activate_window(win);
        }

        let _ = self.conn.flush();
        Ok(())
    }
}
