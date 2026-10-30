//! `xflash observe`: a read-only observer on the Xwayland display.
//!
//! It selects structure, substructure and property events on the root window and on every window
//! it finds below it, creates one XDamage object per window (RawRectangles), and writes one JSON
//! line per event with the `CLOCK_MONOTONIC` nanosecond stamp taken when the event was read. It
//! never configures, maps, unmaps, reparents or draws on a window. The only requests it sends
//! that touch the server are `ChangeWindowAttributes` (its own event mask), `DamageCreate` and
//! `DamageSubtract` (its own damage objects), and reads (`GetProperty`, `GetGeometry`,
//! `GetWindowAttributes`, `QueryTree`, `InternAtom`).
//!
//! Limits, by construction: a stamp is the time this process read the event, not the time the
//! server queued it, so under CPU load it is an upper bound; a window is tracked from its
//! `CreateNotify`, so events between its creation and the first reply are only covered by the
//! `snap` record written right after tracking starts.

use crate::util::{js, mono_ns};
use std::collections::HashMap;
use std::fs::File;
use std::io::{LineWriter, Write};
use x11rb::connection::Connection;
use x11rb::protocol::damage::{ConnectionExt as _, ReportLevel};
use x11rb::protocol::sync;
use x11rb::protocol::xproto::{
    Atom, AtomEnum, ChangeWindowAttributesAux, ConnectionExt as _, EventMask, MapState, Window,
};
use x11rb::protocol::Event;
use x11rb::rust_connection::RustConnection;

struct Atoms {
    net_wm_pid: Atom,
    net_wm_state: Atom,
    net_wm_name: Atom,
    gtk_frame_extents: Atom,
    net_frame_extents: Atom,
    utf8: Atom,
    /// `_NET_WM_SYNC_REQUEST_COUNTER` (CARDINAL): the XSync counter the WM updates on a
    /// programmatic resize and the client advances when it has drawn the result.
    sync_counter: Atom,
    wm_protocols: Atom,
}

struct Observer {
    conn: RustConnection,
    root: Window,
    atoms: Atoms,
    out: LineWriter<File>,
    tracked: HashMap<Window, u8>,
    names: HashMap<Atom, String>,
    /// Caps that keep a busy client from flooding the file or the process (see `Caps`).
    caps: Caps,
    /// `--sync`: track `_NET_WM_SYNC_REQUEST_COUNTER` values per window (one XSync query per
    /// tracked counter after each event; changes are written as `sync` lines).
    sync_on: bool,
    sync_counters: HashMap<Window, u32>,
    sync_last: HashMap<u32, (i32, u32)>,
    bytes: u64,
    sec_start: u64,
    dmg_in_sec: u32,
    dmg_dropped: u64,
    capped: bool,
}

/// Limits on what the observer writes: at most `max_damage_per_s` DamageNotify lines per second
/// (the rest are counted in a `dropped` line, and are still subtracted so the server keeps
/// sending), and at most `max_bytes` in the file; past that only structural events (create, map,
/// unmap, destroy, reparent, configure) are written.
struct Caps {
    max_damage_per_s: u32,
    max_bytes: u64,
}

pub fn run(args: &[String]) -> Result<(), String> {
    let mut out_path = None;
    let mut level = ReportLevel::RAW_RECTANGLES;
    let mut max_damage_per_s: u32 = 400;
    let mut max_bytes: u64 = 40_000_000;
    let mut sync_on = false;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--out" => {
                out_path = args.get(i + 1).cloned();
                i += 1;
            }
            "--sync" => {
                sync_on = true;
            }
            "--damage" => {
                level = match args.get(i + 1).map(|s| s.as_str()) {
                    Some("nonempty") => ReportLevel::NON_EMPTY,
                    Some("raw") => ReportLevel::RAW_RECTANGLES,
                    other => return Err(format!("--damage raw|nonempty, got {:?}", other)),
                };
                i += 1;
            }
            "--max-damage-per-s" => {
                max_damage_per_s = args.get(i + 1).and_then(|s| s.parse().ok()).ok_or("--max-damage-per-s N")?;
                i += 1;
            }
            "--max-mb" => {
                let mb: u64 = args.get(i + 1).and_then(|s| s.parse().ok()).ok_or("--max-mb N")?;
                max_bytes = mb * 1_000_000;
                i += 1;
            }
            other => return Err(format!("observe: unknown argument {other}")),
        }
        i += 1;
    }
    let out_path = out_path.ok_or("observe: --out FILE is required")?;
    let file = File::create(&out_path).map_err(|e| format!("{out_path}: {e}"))?;

    let (conn, screen_num) = x11rb::connect(None).map_err(|e| format!("connect: {e}"))?;
    let root = conn.setup().roots[screen_num].root;
    let dv = conn
        .damage_query_version(1, 1)
        .map_err(|e| e.to_string())?
        .reply()
        .map_err(|e| format!("XDamage unavailable: {e}"))?;
    // XSync is needed only to read `_NET_WM_SYNC_REQUEST_COUNTER` values. Querying a client's
    // counter does not change it and sends the client no event.
    if sync_on {
        let _ = sync::initialize(&conn, 3, 1).map(|c| c.reply());
    }

    let intern = |name: &str| -> Result<Atom, String> {
        conn.intern_atom(false, name.as_bytes())
            .map_err(|e| e.to_string())?
            .reply()
            .map(|r| r.atom)
            .map_err(|e| e.to_string())
    };
    let atoms = Atoms {
        net_wm_pid: intern("_NET_WM_PID")?,
        net_wm_state: intern("_NET_WM_STATE")?,
        net_wm_name: intern("_NET_WM_NAME")?,
        gtk_frame_extents: intern("_GTK_FRAME_EXTENTS")?,
        net_frame_extents: intern("_NET_FRAME_EXTENTS")?,
        utf8: intern("UTF8_STRING")?,
        sync_counter: intern("_NET_WM_SYNC_REQUEST_COUNTER")?,
        wm_protocols: intern("WM_PROTOCOLS")?,
    };

    let mut ob = Observer {
        conn,
        root,
        atoms,
        out: LineWriter::new(file),
        tracked: HashMap::new(),
        names: HashMap::new(),
        caps: Caps { max_damage_per_s, max_bytes },
        sync_on,
        sync_counters: HashMap::new(),
        sync_last: HashMap::new(),
        bytes: 0,
        sec_start: 0,
        dmg_in_sec: 0,
        dmg_dropped: 0,
        capped: false,
    };
    let display = std::env::var("DISPLAY").unwrap_or_default();
    ob.emit(
        mono_ns(),
        "hello",
        &format!(
            "\"display\":{},\"damage\":\"{}.{}\",\"level\":{},\"tool\":\"xflash 0.1\",\"pid\":{}",
            js(&display),
            dv.major_version,
            dv.minor_version,
            if level == ReportLevel::NON_EMPTY {
                "\"nonempty\""
            } else {
                "\"raw\""
            },
            std::process::id()
        ),
    );

    // Root: substructure notifications only (no redirect, so no MapRequest and nothing is held).
    let mask = ChangeWindowAttributesAux::new().event_mask(EventMask::SUBSTRUCTURE_NOTIFY);
    ob.conn
        .change_window_attributes(root, &mask)
        .map_err(|e| e.to_string())?;
    ob.tracked.insert(root, 0);
    ob.conn.flush().map_err(|e| e.to_string())?;

    // Windows that already exist (frames and clients, two levels).
    let mut existing = Vec::new();
    if let Ok(tree) = ob.conn.query_tree(root).map_err(|e| e.to_string())?.reply() {
        for c in tree.children {
            existing.push(c);
        }
    }
    for w in existing {
        ob.track(w, 1, level);
    }

    loop {
        let ev = ob.conn.wait_for_event().map_err(|e| format!("event: {e}"))?;
        let t = mono_ns();
        ob.handle(t, ev, level);
        ob.poll_sync(mono_ns());
    }
}

impl Observer {
    fn emit(&mut self, t: u64, ev: &str, fields: &str) {
        let line = if fields.is_empty() {
            format!("{{\"t\":{},\"ev\":\"{}\"}}\n", t, ev)
        } else {
            format!("{{\"t\":{},\"ev\":\"{}\",{}}}\n", t, ev, fields)
        };
        self.bytes += line.len() as u64;
        let _ = self.out.write_all(line.as_bytes());
    }

    /// Writes a high-rate event unless a cap says no. Returns whether it was written.
    fn emit_bulk(&mut self, t: u64, ev: &str, fields: &str) -> bool {
        if self.bytes >= self.caps.max_bytes {
            if !self.capped {
                self.capped = true;
                self.emit(t, "cap", &format!("\"max_bytes\":{}", self.caps.max_bytes));
            }
            return false;
        }
        if t / 1_000_000_000 != self.sec_start {
            if self.dmg_dropped > 0 {
                let d = self.dmg_dropped;
                self.emit(t, "dropped", &format!("\"damage\":{},\"sec\":{}", d, self.sec_start));
                self.dmg_dropped = 0;
            }
            self.sec_start = t / 1_000_000_000;
            self.dmg_in_sec = 0;
        }
        if self.dmg_in_sec >= self.caps.max_damage_per_s {
            self.dmg_dropped += 1;
            return false;
        }
        self.dmg_in_sec += 1;
        self.emit(t, ev, fields);
        true
    }

    fn atom_name(&mut self, a: Atom) -> String {
        if let Some(n) = self.names.get(&a) {
            return n.clone();
        }
        let n = self
            .conn
            .get_atom_name(a)
            .ok()
            .and_then(|c| c.reply().ok())
            .map(|r| String::from_utf8_lossy(&r.name).into_owned())
            .unwrap_or_else(|| format!("atom{a}"));
        self.names.insert(a, n.clone());
        n
    }

    /// Starts watching `w`: its own events, its children's, and a damage object. Then writes a
    /// `snap` record with what the server says about it right now.
    fn track(&mut self, w: Window, depth: u8, level: ReportLevel) {
        if self.tracked.contains_key(&w) {
            return;
        }
        self.tracked.insert(w, depth);
        let mask = ChangeWindowAttributesAux::new().event_mask(
            EventMask::STRUCTURE_NOTIFY | EventMask::PROPERTY_CHANGE | EventMask::SUBSTRUCTURE_NOTIFY,
        );
        let _ = self.conn.change_window_attributes(w, &mask);
        if let Ok(id) = self.conn.generate_id() {
            let _ = self.conn.damage_create(id, w, level);

        }
        let _ = self.conn.flush();
        let t = mono_ns();
        let attrs = self.conn.get_window_attributes(w).ok().and_then(|c| c.reply().ok());
        let geom = self.conn.get_geometry(w).ok().and_then(|c| c.reply().ok());
        let (state, ovr) = match &attrs {
            Some(a) => (
                match a.map_state {
                    MapState::UNMAPPED => "unmapped",
                    MapState::UNVIEWABLE => "unviewable",
                    _ => "viewable",
                },
                a.override_redirect,
            ),
            None => ("gone", false),
        };
        let g = match &geom {
            Some(g) => format!(
                "\"x\":{},\"y\":{},\"w\":{},\"h\":{}",
                g.x, g.y, g.width, g.height
            ),
            None => "\"x\":null,\"y\":null,\"w\":null,\"h\":null".into(),
        };
        let parent = self
            .conn
            .query_tree(w)
            .ok()
            .and_then(|c| c.reply().ok())
            .map(|r| (r.parent, r.children));
        let (pw, kids) = parent.unwrap_or((0, Vec::new()));
        self.emit(
            t,
            "snap",
            &format!(
                "\"win\":{},\"parent\":{},\"map\":\"{}\",\"override\":{},{}",
                w, pw, state, ovr, g
            ),
        );
        self.props(t, w);
        if depth > 0 {
            for k in kids {
                self.track(k, depth - 1, level);
            }
        }
    }



    fn read_prop(&self, w: Window, atom: Atom, ty: impl Into<Atom>, len: u32) -> Option<(Vec<u8>, u8)> {
        let r = self
            .conn
            .get_property(false, w, atom, ty, 0, len)
            .ok()?
            .reply()
            .ok()?;
        if r.value.is_empty() {
            return None;
        }
        Some((r.value, r.format))
    }

    fn cardinals(bytes: &[u8]) -> Vec<u32> {
        bytes
            .chunks_exact(4)
            .map(|c| u32::from_ne_bytes([c[0], c[1], c[2], c[3]]))
            .collect()
    }

    /// A full `props` record: class, pid, title, size hints, EWMH state and frame extents.
    fn props(&mut self, t: u64, w: Window) {
        self.note_sync_counter(t, w);
        let mut f = vec![format!("\"win\":{}", w)];
        if let Some((v, _)) = self.read_prop(w, AtomEnum::WM_CLASS.into(), AtomEnum::STRING, 256) {
            let parts: Vec<String> = v
                .split(|b| *b == 0)
                .filter(|p| !p.is_empty())
                .map(|p| String::from_utf8_lossy(p).into_owned())
                .collect();
            f.push(format!(
                "\"instance\":{},\"class\":{}",
                js(parts.first().map(|s| s.as_str()).unwrap_or("")),
                js(parts.get(1).map(|s| s.as_str()).unwrap_or(""))
            ));
        }
        if let Some((v, _)) = self.read_prop(w, self.atoms.net_wm_pid, AtomEnum::CARDINAL, 1) {
            if let Some(p) = Self::cardinals(&v).first() {
                f.push(format!("\"pid\":{}", p));
            }
        }
        let title = self
            .read_prop(w, self.atoms.net_wm_name, self.atoms.utf8, 256)
            .or_else(|| self.read_prop(w, AtomEnum::WM_NAME.into(), AtomEnum::STRING, 256));
        if let Some((v, _)) = title {
            f.push(format!("\"title\":{}", js(&String::from_utf8_lossy(&v))));
        }
        if let Some(h) = self.hints_json(w) {
            f.push(format!("\"hints\":{}", h));
        }
        if let Some(s) = self.state_json(w) {
            f.push(format!("\"net_wm_state\":{}", s));
        }
        if let Some((v, _)) = self.read_prop(w, self.atoms.wm_protocols, AtomEnum::ATOM, 32) {
            let names: Vec<String> = Self::cardinals(&v)
                .into_iter()
                .map(|a| js(&self.atom_name(a)))
                .collect();
            f.push(format!("\"wm_protocols\":[{}]", names.join(",")));
        }
        for (atom, key) in [
            (self.atoms.gtk_frame_extents, "gtk_frame_extents"),
            (self.atoms.net_frame_extents, "net_frame_extents"),
        ] {
            if let Some((v, _)) = self.read_prop(w, atom, AtomEnum::CARDINAL, 4) {
                f.push(format!("\"{}\":{:?}", key, Self::cardinals(&v)));
            }
        }
        if f.len() == 1 {
            return; // nothing readable: a bare window with no properties yet
        }
        let fields = f.join(",");
        self.emit(t, "props", &fields);
    }

    fn hints_json(&self, w: Window) -> Option<String> {
        let (v, _) = self.read_prop(w, AtomEnum::WM_NORMAL_HINTS.into(), AtomEnum::WM_SIZE_HINTS, 18)?;
        let c = Self::cardinals(&v);
        if c.len() < 18 {
            return None;
        }
        let flags = c[0];
        Some(format!(
            "{{\"flags\":{},\"us_pos\":{},\"us_size\":{},\"p_pos\":{},\"p_size\":{},\"min\":[{},{}],\"max\":[{},{}],\"inc\":[{},{}],\"base\":[{},{}],\"gravity\":{}}}",
            flags,
            flags & 1 != 0,
            flags & 2 != 0,
            flags & 4 != 0,
            flags & 8 != 0,
            if flags & 16 != 0 { c[5] as i64 } else { -1 },
            if flags & 16 != 0 { c[6] as i64 } else { -1 },
            if flags & 32 != 0 { c[7] as i64 } else { -1 },
            if flags & 32 != 0 { c[8] as i64 } else { -1 },
            if flags & 64 != 0 { c[9] as i64 } else { -1 },
            if flags & 64 != 0 { c[10] as i64 } else { -1 },
            if flags & 256 != 0 { c[15] as i64 } else { -1 },
            if flags & 256 != 0 { c[16] as i64 } else { -1 },
            if flags & 512 != 0 { c[17] as i64 } else { -1 },
        ))
    }

    fn state_json(&mut self, w: Window) -> Option<String> {
        let (v, _) = self.read_prop(w, self.atoms.net_wm_state, AtomEnum::ATOM, 64)?;
        let names: Vec<String> = Self::cardinals(&v)
            .into_iter()
            .map(|a| js(&self.atom_name(a)))
            .collect();
        Some(format!("[{}]", names.join(",")))
    }

    /// Records `_NET_WM_SYNC_REQUEST_COUNTER` when a window gains one and queries its value now.
    /// Called at tracking time, at MapNotify and on a PropertyNotify for the counter.
    fn note_sync_counter(&mut self, t: u64, w: Window) {
        if !self.sync_on {
            return;
        }
        let Some((v, _)) = self.read_prop(w, self.atoms.sync_counter, AtomEnum::CARDINAL, 1) else {
            return;
        };
        let Some(c) = Self::cardinals(&v).first().copied() else { return };
        if c == 0 || self.sync_counters.get(&w) == Some(&c) {
            return;
        }
        self.sync_counters.insert(w, c);
        self.emit(t, "sync", &format!("\"win\":{},\"counter\":{},\"event\":\"attach\"", w, c));
        self.poll_sync(t);
    }

    /// One XSyncQueryCounter per tracked counter; writes a `sync` line when the 64-bit value
    /// changes. A counter that errors (the client or window is gone) is dropped from tracking.
    fn poll_sync(&mut self, t: u64) {
        if !self.sync_on {
            return;
        }
        let pairs: Vec<(Window, u32)> = self.sync_counters.iter().map(|(w, c)| (*w, *c)).collect();
        for (w, c) in pairs {
            match sync::query_counter(&self.conn, c).map(|k| k.reply()) {
                Ok(Ok(r)) => {
                    let v = (r.counter_value.hi, r.counter_value.lo);
                    if self.sync_last.insert(c, v) != Some(v) {
                        self.emit(
                            t,
                            "sync",
                            &format!(
                                "\"win\":{},\"counter\":{},\"event\":\"value\",\"hi\":{},\"lo\":{}",
                                w, c, v.0, v.1
                            ),
                        );
                    }
                }
                _ => {
                    self.sync_counters.remove(&w);
                    self.sync_last.remove(&c);
                }
            }
        }
    }

    fn handle(&mut self, t: u64, ev: Event, level: ReportLevel) {
        match ev {
            Event::CreateNotify(e) => {
                self.emit(
                    t,
                    "CreateNotify",
                    &format!(
                        "\"win\":{},\"parent\":{},\"x\":{},\"y\":{},\"w\":{},\"h\":{},\"override\":{}",
                        e.window, e.parent, e.x, e.y, e.width, e.height, e.override_redirect
                    ),
                );
                self.track(e.window, 1, level);
            }
            Event::DestroyNotify(e) => {
                if e.event == e.window || !self.tracked.contains_key(&e.window) {
                    self.emit(t, "DestroyNotify", &format!("\"win\":{}", e.window));
                }
                self.tracked.remove(&e.window);
            }
            Event::MapNotify(e) => {
                if e.event == e.window || !self.tracked.contains_key(&e.window) {
                    self.emit(
                        t,
                        "MapNotify",
                        &format!("\"win\":{},\"override\":{}", e.window, e.override_redirect),
                    );
                    self.props(mono_ns(), e.window);
                }
            }
            Event::UnmapNotify(e) => {
                if e.event == e.window || !self.tracked.contains_key(&e.window) {
                    self.emit(t, "UnmapNotify", &format!("\"win\":{}", e.window));
                }
            }
            Event::ReparentNotify(e) => {
                if e.event == e.window || !self.tracked.contains_key(&e.window) {
                    self.emit(
                        t,
                        "ReparentNotify",
                        &format!("\"win\":{},\"parent\":{},\"x\":{},\"y\":{}", e.window, e.parent, e.x, e.y),
                    );
                }
                self.track(e.parent, 1, level);
                self.track(e.window, 1, level);
            }
            Event::ConfigureNotify(e) => {
                if e.event == e.window || !self.tracked.contains_key(&e.window) {
                    let synthetic = e.response_type & 0x80 != 0;
                    self.emit(
                        t,
                        "ConfigureNotify",
                        &format!(
                            "\"win\":{},\"x\":{},\"y\":{},\"w\":{},\"h\":{},\"border\":{},\"synthetic\":{},\"override\":{}",
                            e.window, e.x, e.y, e.width, e.height, e.border_width, synthetic, e.override_redirect
                        ),
                    );
                }
            }
            Event::PropertyNotify(e) => {
                let name = self.atom_name(e.atom);
                if e.atom == self.atoms.sync_counter {
                    self.note_sync_counter(t, e.window);
                }
                let watched = matches!(
                    name.as_str(),
                    "WM_NORMAL_HINTS"
                        | "_NET_WM_STATE"
                        | "_GTK_FRAME_EXTENTS"
                        | "_NET_FRAME_EXTENTS"
                        | "WM_CLASS"
                        | "_NET_WM_PID"
                        | "_NET_WM_NAME"
                        | "WM_NAME"
                        | "WM_STATE"
                );
                if watched {
                    self.emit(
                        t,
                        "PropertyNotify",
                        &format!("\"win\":{},\"prop\":{},\"deleted\":{}", e.window, js(&name), e.state == x11rb::protocol::xproto::Property::DELETE),
                    );
                    // One read of the changed property only (the full set is read at tracking time
                    // and at MapNotify): a burst of property events costs one round trip each.
                    let value = match name.as_str() {
                        "WM_NORMAL_HINTS" => self.hints_json(e.window),
                        "_NET_WM_STATE" => self.state_json(e.window),
                        "_GTK_FRAME_EXTENTS" | "_NET_FRAME_EXTENTS" => self
                            .read_prop(e.window, e.atom, AtomEnum::CARDINAL, 4)
                            .map(|(v, _)| format!("{:?}", Self::cardinals(&v))),
                        _ => None,
                    };
                    if let Some(v) = value {
                        self.emit(mono_ns(), "propval", &format!("\"win\":{},\"prop\":{},\"value\":{}", e.window, js(&name), v));
                    }
                }
            }
            Event::DamageNotify(e) => {
                self.emit_bulk(
                    t,
                    "DamageNotify",
                    &format!(
                        "\"win\":{},\"ax\":{},\"ay\":{},\"aw\":{},\"ah\":{},\"gw\":{},\"gh\":{}",
                        e.drawable, e.area.x, e.area.y, e.area.width, e.area.height, e.geometry.width, e.geometry.height
                    ),
                );
                let _ = self.conn.damage_subtract(e.damage, x11rb::NONE, x11rb::NONE);
                let _ = self.conn.flush();
            }
            _ => {}
        }
    }
}
