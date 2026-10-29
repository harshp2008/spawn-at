//! `xflash cell`: one cell of the bisect (docs/evidence/x11-flash-02/bisect-2026-10-29.md), run only
//! inside the PRIVATE headless Shell (through `psh`). A cell launches one application a few times
//! in one configuration (protocol, launcher, observer, startup-id handling, close method), closes it
//! gracefully, and records after every run whether the Shell is still alive, whether it logged the
//! timestamp-0 warning, and whether the window had focus when it was closed.
//!
//! Exit codes: 0 all runs finished, 3 the private Shell died (the lines of its log since the cell
//! began are saved), 2 setup problem. It never signals a process it did not record.

use crate::run::{all_ids, apps, health, pid_alive, signal, window_entry, windows_json, SHELL_PID_FILE};
use crate::util::{js, mono_ns, num_field, raw_field, str_field};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{
    Atom, AtomEnum, ClientMessageEvent, ConnectionExt as _, EventMask, Window,
};

struct Cell {
    label: String,
    app: String,
    wayland: bool,
    plain: bool,
    observer: bool,
    unset_startup: bool,
    focus: bool,
    bg_window: bool,
    startup_id: Option<String>,
    close: String,
    runs: usize,
    settle_ms: u64,
    out: PathBuf,
    shell_log: PathBuf,
}

fn parse(args: &[String]) -> Result<Cell, String> {
    let mut c = Cell {
        label: String::new(),
        app: String::new(),
        wayland: false,
        plain: false,
        observer: false,
        unset_startup: false,
        focus: false,
        bg_window: false,
        startup_id: None,
        close: "spawn-at".into(),
        runs: 3,
        settle_ms: 2500,
        out: PathBuf::new(),
        shell_log: PathBuf::new(),
    };
    let mut pid_file = None;
    let mut i = 0;
    while i < args.len() {
        let f = args[i].as_str();
        match f {
            "--wayland" => c.wayland = true,
            "--plain" => c.plain = true,
            "--observer" => c.observer = true,
            "--unset-startup" => c.unset_startup = true,
            "--focus" => c.focus = true,
            "--bg-window" => c.bg_window = true,
            _ => {
                let v = args.get(i + 1).ok_or_else(|| format!("{f} needs a value"))?.clone();
                match f {
                    "--label" => c.label = v,
                    "--app" => c.app = v,
                    "--startup-id" => c.startup_id = Some(v),
                    "--close" => c.close = v,
                    "--runs" => c.runs = v.parse().map_err(|_| "--runs N")?,
                    "--settle-ms" => c.settle_ms = v.parse().map_err(|_| "--settle-ms N")?,
                    "--out" => c.out = PathBuf::from(v),
                    "--shell-log" => c.shell_log = PathBuf::from(v),
                    "--shell-pid-file" => pid_file = Some(PathBuf::from(v)),
                    other => return Err(format!("cell: unknown argument {other}")),
                }
                i += 1;
            }
        }
        i += 1;
    }
    let _ = SHELL_PID_FILE.set(pid_file);
    if c.label.is_empty() || c.app.is_empty() || c.out.as_os_str().is_empty() || c.shell_log.as_os_str().is_empty() {
        return Err("cell: --label, --app, --out and --shell-log are required".into());
    }
    Ok(c)
}

fn log_len(p: &PathBuf) -> u64 {
    fs::metadata(p).map(|m| m.len()).unwrap_or(0)
}

fn log_since(p: &PathBuf, off: u64) -> String {
    let all = fs::read(p).unwrap_or_default();
    String::from_utf8_lossy(&all[(off as usize).min(all.len())..]).into_owned()
}

fn warnings(text: &str) -> Vec<String> {
    text.lines()
        .filter(|l| !l.contains("[spawn-at-time]"))
        .filter(|l| l.contains("warning") || l.contains("assertion") || l.contains("Bail out") || l.contains("crashed") || l.contains("Buggy") || l.contains("ERROR") || l.contains("CRITICAL"))
        .map(|l| l.chars().take(200).collect::<String>())
        .collect()
}

fn atom(conn: &impl Connection, name: &str) -> Option<Atom> {
    conn.intern_atom(false, name.as_bytes()).ok()?.reply().ok().map(|r| r.atom)
}

/// Sends WM_DELETE_WINDOW (ICCCM, timestamp CurrentTime) to the top-level X window whose
/// _NET_WM_PID is `pid`. This is what a window manager's close button sends; it is the only message
/// this tool sends to a client, and only in the private session.
fn wm_delete(pid: u64) -> Result<bool, String> {
    let (conn, sn) = x11rb::connect(None).map_err(|e| e.to_string())?;
    let root = conn.setup().roots[sn].root;
    let (pidatom, protos, del) = (
        atom(&conn, "_NET_WM_PID").ok_or("atom")?,
        atom(&conn, "WM_PROTOCOLS").ok_or("atom")?,
        atom(&conn, "WM_DELETE_WINDOW").ok_or("atom")?,
    );
    let mut stack: Vec<(Window, u8)> = vec![(root, 0)];
    while let Some((w, d)) = stack.pop() {
        if let Some(r) = conn.query_tree(w).ok().and_then(|c| c.reply().ok()) {
            for c in r.children {
                let p = conn
                    .get_property(false, c, pidatom, AtomEnum::CARDINAL, 0, 1)
                    .ok()
                    .and_then(|x| x.reply().ok())
                    .and_then(|x| x.value32().and_then(|mut v| v.next()));
                if p == Some(pid as u32) {
                    let ok = conn
                        .get_property(false, c, protos, AtomEnum::ATOM, 0, 16)
                        .ok()
                        .and_then(|x| x.reply().ok())
                        .and_then(|x| x.value32().map(|v| v.collect::<Vec<_>>()))
                        .map(|v| v.contains(&del))
                        .unwrap_or(false);
                    if ok {
                        let ev = ClientMessageEvent::new(32, c, protos, [del, 0, 0, 0, 0]);
                        conn.send_event(false, c, EventMask::NO_EVENT, ev).map_err(|e| e.to_string())?;
                        conn.flush().map_err(|e| e.to_string())?;
                        return Ok(true);
                    }
                }
                if d < 1 {
                    stack.push((c, d + 1));
                }
            }
        }
    }
    Ok(false)
}

fn environ_of(pid: u64) -> String {
    let raw = fs::read(format!("/proc/{}/environ", pid)).unwrap_or_default();
    let get = |k: &str| -> String {
        raw.split(|b| *b == 0)
            .filter_map(|e| std::str::from_utf8(e).ok())
            .find_map(|e| e.strip_prefix(&format!("{}=", k)).map(|v| js(v)))
            .unwrap_or_else(|| "null".into())
    };
    format!("{{\"DESKTOP_STARTUP_ID\":{},\"XDG_ACTIVATION_TOKEN\":{},\"GDK_BACKEND\":{}}}", get("DESKTOP_STARTUP_ID"), get("XDG_ACTIVATION_TOKEN"), get("GDK_BACKEND"))
}

pub fn run(args: &[String]) -> Result<(), String> {
    let c = parse(args)?;
    fs::create_dir_all(&c.out).map_err(|e| e.to_string())?;
    let all = apps();
    let app = all.iter().find(|a| a.id == c.app).ok_or("unknown app")?;
    let h0 = health();
    if h0.shell_pid.is_empty() {
        return Err("the private Shell is not running".into());
    }
    let mut base: Vec<u64> = all_ids(&windows_json()).into_iter().map(|(i, _)| i).collect();
    if !base.is_empty() {
        return Err(format!("the private Shell already has windows: {:?}", base));
    }
    let mut res = OpenOptions::new().create(true).append(true).open(c.out.join("cells.jsonl")).map_err(|e| e.to_string())?;
    let cell_off = log_len(&c.shell_log);
    let mut obs_pid: Option<u64> = None;
    let mut obs_child = None;
    if c.observer {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let ch = Command::new(exe)
            .args(["observe", "--out"])
            .arg(c.out.join(format!("{}.xobs.jsonl", c.label)))
            .stdin(Stdio::null())
            .spawn()
            .map_err(|e| e.to_string())?;
        obs_pid = Some(ch.id() as u64);
        obs_child = Some(ch);
        std::thread::sleep(Duration::from_millis(1500));
    }
    let cfg = format!(
        "{{\"label\":{},\"app\":{},\"mode\":{},\"launcher\":{},\"observer\":{},\"unset_startup\":{},\"startup_id\":{},\"close\":{}}}",
        js(&c.label), js(&c.app), js(if c.wayland { "wayland" } else { "x11" }), js(if c.plain { "plain" } else { "spawn-at" }),
        c.observer, c.unset_startup, c.startup_id.as_deref().map(js).unwrap_or_else(|| "null".into()), js(&c.close)
    );
    let mut crashed = false;
    // A second, native Wayland window kept open for the whole cell (eog, Wayland), so that closing
    // the focused test window makes Mutter hand focus to another window, as in the live session
    // (where the Shell logged "Failed to focus window W..." for a Wayland window).
    let mut bg_id: Option<u64> = None;
    let mut bg_child = None;
    if c.bg_window {
        let eog = all.iter().find(|a| a.id == "eog").ok_or("eog")?;
        let mut cmd = Command::new("eog");
        cmd.arg("--new-instance");
        for (k, v) in eog.env_wl {
            cmd.env(k, v);
        }
        cmd.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
        bg_child = cmd.spawn().ok();
        let until = Instant::now() + Duration::from_secs(12);
        while Instant::now() < until && bg_id.is_none() {
            std::thread::sleep(Duration::from_millis(150));
            bg_id = all_ids(&windows_json()).into_iter().map(|(i, _)| i).find(|i| !base.contains(i));
        }
        if let Some(b) = bg_id {
            base.push(b);
            let _ = Command::new("spawn-at").args(["focus", "--id", &b.to_string()]).stdout(Stdio::null()).stderr(Stdio::null()).status();
            std::thread::sleep(Duration::from_millis(800));
        } else {
            return Err("the background Wayland window did not appear".into());
        }
    }
    for n in 0..c.runs {
        if health() != h0 {
            crashed = true;
            break;
        }
        let log_before = log_len(&c.shell_log);
        let size = app.small;
        let argv: Vec<&str> = app.argv.iter().map(|s| *s).collect();
        let envs = if c.wayland { app.env_wl } else { app.env_x11 };
        let class = if c.wayland { app.class_wl } else { app.class_x11 };
        let t0 = mono_ns();
        let mut plain_child = None;
        let mut spawn_json = String::from("null");
        let mut wid: Option<u64> = None;
        if c.plain {
            let mut cmd = Command::new(argv[0]);
            cmd.args(&argv[1..]);
            for (k, v) in envs {
                cmd.env(k, v);
            }
            cmd.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
            plain_child = cmd.spawn().ok();
            let until = Instant::now() + Duration::from_secs(12);
            while Instant::now() < until && wid.is_none() {
                std::thread::sleep(Duration::from_millis(100));
                wid = all_ids(&windows_json()).into_iter().map(|(i, _)| i).find(|i| !base.contains(i));
            }
        } else {
            let mut cmd = Command::new("spawn-at");
            cmd.arg("spawn");
            if !class.is_empty() {
                cmd.args(["-c", class]);
            }
            cmd.args(["--no-focus", "--anchor", "bottom-right", "--size", &size.0.to_string(), &size.1.to_string(), "-m", "24", "--json", "--"]);
            if c.unset_startup || c.startup_id.is_some() {
                cmd.arg("env");
                if c.unset_startup {
                    cmd.args(["-u", "DESKTOP_STARTUP_ID", "-u", "XDG_ACTIVATION_TOKEN"]);
                }
                if let Some(s) = &c.startup_id {
                    cmd.arg(format!("DESKTOP_STARTUP_ID={}", s));
                }
            }
            cmd.args(&argv);
            for (k, v) in envs {
                cmd.env(k, v);
            }
            let out_f = c.out.join("tmp_out.txt");
            cmd.stdin(Stdio::null()).stdout(File::create(&out_f).map_err(|e| e.to_string())?).stderr(Stdio::null());
            let mut ch = cmd.spawn().map_err(|e| e.to_string())?;
            let until = Instant::now() + Duration::from_secs(25);
            while Instant::now() < until {
                if let Ok(Some(_)) = ch.try_wait() {
                    break;
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            let _ = ch.try_wait();
            let txt = fs::read_to_string(&out_f).unwrap_or_default();
            wid = num_field(&txt, "window_id").map(|v| v as u64);
            if !txt.trim().is_empty() {
                spawn_json = txt.trim().to_string();
            }
        }
        std::thread::sleep(Duration::from_millis(c.settle_ms));
        // The private headless Shell has no user activity, so a new window is not given focus the
        // way it is in the live session. `--focus` asks the extension to focus it (private session
        // only), to reach the state both live crashes had: a focused window at close.
        if c.focus {
            if let Some(w) = wid {
                let _ = Command::new("spawn-at").args(["focus", "--id", &w.to_string()]).stdout(Stdio::null()).stderr(Stdio::null()).status();
                std::thread::sleep(Duration::from_millis(600));
            }
        }
        let post = windows_json();
        let entry = wid.and_then(|w| window_entry(&post, w));
        let wpid = entry.as_deref().and_then(|e| num_field(e, "pid")).map(|v| v as u64);
        let focused = entry.as_deref().map(|e| raw_field(e, "focused").unwrap_or("null").to_string()).unwrap_or_else(|| "null".into());
        let class_seen = entry.as_deref().and_then(|e| str_field(e, "class")).unwrap_or_default();
        let env_seen = wpid.map(environ_of).unwrap_or_else(|| "null".into());
        let state_before_close = health() == h0;

        // ---- close ----
        let mut close_note = String::new();
        if let Some(w) = wid {
            match c.close.as_str() {
                "wmdelete" => match wpid.map(wm_delete) {
                    Some(Ok(true)) => close_note.push_str("sent WM_DELETE_WINDOW"),
                    Some(Ok(false)) => close_note.push_str("no X window with that pid and WM_DELETE_WINDOW"),
                    Some(Err(e)) => close_note.push_str(&format!("x error {e}")),
                    None => close_note.push_str("no pid"),
                },
                "key" => {
                    let ok = Command::new("xdotool").args(["key", "ctrl+w"]).status().map(|s| s.success()).unwrap_or(false);
                    close_note.push_str(if ok { "xdotool ctrl+w sent" } else { "xdotool failed" });
                }
                _ => {
                    let _ = Command::new("spawn-at").args(["close", "--id", &w.to_string()]).stdout(Stdio::null()).stderr(Stdio::null()).status();
                    close_note.push_str("spawn-at close");
                }
            }
            let until = Instant::now() + Duration::from_secs(6);
            let mut gone = false;
            while Instant::now() < until {
                if health() != h0 {
                    crashed = true;
                    break;
                }
                if window_entry(&windows_json(), w).is_none() {
                    gone = true;
                    break;
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            if !gone && !crashed && c.close != "spawn-at" {
                // The chosen method did not close it: finish with the graceful request, noted.
                let _ = Command::new("spawn-at").args(["close", "--id", &w.to_string()]).stdout(Stdio::null()).stderr(Stdio::null()).status();
                close_note.push_str("; method did not close, fell back to spawn-at close");
                let until = Instant::now() + Duration::from_secs(5);
                while Instant::now() < until && !crashed {
                    if health() != h0 {
                        crashed = true;
                        break;
                    }
                    if window_entry(&windows_json(), w).is_none() {
                        gone = true;
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(100));
                }
            }
            if gone {
                close_note.push_str("; window gone");
            }
        }
        std::thread::sleep(Duration::from_millis(1500));
        if health() != h0 {
            crashed = true;
        }
        // Tidy: a plain or lingering process of ours, only after its window is gone and only by recorded pid.
        if !crashed {
            if let Some(p) = wpid {
                let until = Instant::now() + Duration::from_secs(4);
                while pid_alive(p) && Instant::now() < until {
                    std::thread::sleep(Duration::from_millis(100));
                }
                if pid_alive(p) {
                    signal(p, libc::SIGTERM);
                }
            }
        }
        if let Some(mut ch) = plain_child {
            if !crashed {
                let _ = ch.try_wait();
            }
        }
        let slice = log_since(&c.shell_log, log_before);
        let w = warnings(&slice);
        let rec = format!(
            "{{\"cell\":{},\"run\":{},\"t_start\":{},\"window_id\":{},\"pid\":{},\"class\":{},\"focused_at_close\":{},\"env\":{},\"shell_ok_before_close\":{},\"close\":{},\"crashed\":{},\"timestamp0_warning\":{},\"active_window_warning\":{},\"warnings\":{},\"spawn_json\":{}}}\n",
            cfg, n, t0,
            wid.map(|v| v.to_string()).unwrap_or_else(|| "null".into()),
            wpid.map(|v| v.to_string()).unwrap_or_else(|| "null".into()),
            js(&class_seen), focused, env_seen, state_before_close, js(&close_note), crashed,
            slice.contains("timestamp of 0"), slice.contains("_NET_ACTIVE_WINDOW"),
            js(&w.join(" | ")), spawn_json
        );
        let _ = res.write_all(rec.as_bytes());
        let _ = res.flush();
        eprintln!("[{}] run {} focused={} close='{}' crashed={} ts0={}", c.label, n + 1, focused, close_note, crashed, slice.contains("timestamp of 0"));
        if crashed {
            break;
        }
        std::thread::sleep(Duration::from_secs(3));
    }
    if let (Some(b), false) = (bg_id, crashed) {
        let _ = Command::new("spawn-at").args(["close", "--id", &b.to_string()]).stdout(Stdio::null()).stderr(Stdio::null()).status();
        std::thread::sleep(Duration::from_secs(2));
    }
    drop(bg_child);
    if let Some(p) = obs_pid {
        signal(p, libc::SIGTERM);
    }
    if let Some(mut ch) = obs_child {
        let _ = ch.wait();
    }
    let _ = fs::remove_file(c.out.join("tmp_out.txt"));
    if crashed {
        let text = log_since(&c.shell_log, cell_off);
        let tail: Vec<&str> = text.lines().filter(|l| !l.contains("[spawn-at-time]")).collect();
        let keep = tail.len().saturating_sub(60);
        let _ = fs::write(c.out.join(format!("{}.shell-crash.txt", c.label)), tail[keep..].join("\n"));
        eprintln!("[{}] PRIVATE SHELL DIED; its log tail is saved", c.label);
        return Err("private shell died".into());
    }
    Ok(())
}
