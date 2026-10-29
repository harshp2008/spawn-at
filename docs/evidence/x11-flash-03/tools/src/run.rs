//! `xflash run`: drives one batch of spawns of ONE real application and records, for each run, the
//! monotonic stamps and the answers of `spawn-at`. It starts the X-side observer (`xflash
//! observe`) as a child for the whole batch, turns the extension's timing log on, and at the end
//! copies the extension's lines out of the journal.
//!
//! Safety rules, all enforced here:
//! - Resumable: `progress.jsonl` gets one line per finished run; a restarted batch (same
//!   arguments, same seed, same `--out`) skips every key already there. A crash loses at most the
//!   run that was in flight.
//! - One window at a time: a run starts only when no window exists that was not there when the
//!   batch started, and 3 s after the previous window is gone.
//! - Graceful close only (`spawn-at close --id`, which asks the window to close). A process is
//!   signalled only if it is the pid the compositor reported for the window we spawned, the window
//!   is already gone, and it has not exited 5 s later: SIGTERM first, SIGKILL as a last resort.
//!   Never by name, never while the window still exists.
//! - No focus request of any kind: windows are spawned with `--no-focus` and this tool has no X11
//!   code and no `activate`/`focus` call.
//! - Shell watch: the pid and start time of gnome-shell and the boot id are recorded. If they
//!   differ from the last recorded values (in this batch or in the state file of the previous
//!   batch) the tool writes a `crash` event and stops; `--ack-restart` accepts one such change.

use crate::util::{js, mono_ns, num_field, raw_field, str_field, Rng};
use std::collections::HashSet;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// A real application the batch can spawn.
pub struct App {
    pub id: &'static str,
    pub toolkit: &'static str,
    /// Value of `spawn-at spawn --class` for an X11 window (WM_CLASS class) and for a Wayland one.
    pub class_x11: &'static str,
    pub class_wl: &'static str,
    /// argv; `{PROFILE}` becomes a fresh directory, `{OZONE}` x11 or wayland.
    pub argv: &'static [&'static str],
    pub env_x11: &'static [(&'static str, &'static str)],
    pub env_wl: &'static [(&'static str, &'static str)],
    /// Whether a native Wayland control run exists for it.
    pub wayland: bool,
    /// Needs a throwaway profile directory per run.
    pub profile: bool,
    /// The window's pid is a process of ours and may be signalled once its window is gone.
    pub kill_ok: bool,
    pub settle_ms: u64,
    /// The two requested sizes (smaller and larger than the app's own default).
    pub small: (u32, u32),
    pub large: (u32, u32),
}

pub fn apps() -> Vec<App> {
    vec![
        App {
            id: "xterm",
            toolkit: "Xt (X11 only)",
            class_x11: "XTerm",
            class_wl: "",
            argv: &["xterm"],
            env_x11: &[],
            env_wl: &[],
            wayland: false,
            profile: false,
            kill_ok: true,
            settle_ms: 2500,
            small: (400, 260),
            large: (900, 560),
        },
        App {
            id: "gnome-text-editor",
            toolkit: "GTK4",
            class_x11: "org.gnome.TextEditor",
            class_wl: "org.gnome.TextEditor",
            argv: &["gnome-text-editor", "--new-window"],
            env_x11: &[("GDK_BACKEND", "x11"), ("GSETTINGS_BACKEND", "memory")],
            env_wl: &[("GSETTINGS_BACKEND", "memory")],
            wayland: true,
            profile: false,
            kill_ok: true,
            settle_ms: 2500,
            small: (640, 420),
            large: (1100, 760),
        },
        App {
            // X11-FLASH-02i: the editor without `--new-window`, to separate "the app presents its
            // window on a launch request" from "the app only maps a window".
            id: "gnome-text-editor-nowindow",
            toolkit: "GTK4",
            class_x11: "org.gnome.TextEditor",
            class_wl: "org.gnome.TextEditor",
            argv: &["gnome-text-editor"],
            env_x11: &[("GDK_BACKEND", "x11"), ("GSETTINGS_BACKEND", "memory")],
            env_wl: &[("GSETTINGS_BACKEND", "memory")],
            wayland: true,
            profile: false,
            kill_ok: true,
            settle_ms: 2500,
            small: (640, 420),
            large: (1100, 760),
        },
        App {
            id: "eog",
            toolkit: "GTK3",
            class_x11: "Eog",
            class_wl: "org.gnome.eog",
            argv: &["eog", "--new-instance"],
            env_x11: &[("GDK_BACKEND", "x11"), ("GSETTINGS_BACKEND", "memory")],
            env_wl: &[("GSETTINGS_BACKEND", "memory")],
            wayland: true,
            profile: false,
            kill_ok: true,
            settle_ms: 2500,
            small: (600, 420),
            large: (1100, 760),
        },
        App {
            id: "google-chrome",
            toolkit: "Chromium",
            // `--class=xflash-chrome` sets the window's class on both protocols.
            class_x11: "xflash-chrome",
            class_wl: "xflash-chrome",
            argv: &[
                "google-chrome",
                "--user-data-dir={PROFILE}",
                "--no-first-run",
                "--no-default-browser-check",
                "--disable-sync",
                "--no-sandbox",
                "--ozone-platform={OZONE}",
                "--class=xflash-chrome",
                "about:blank",
            ],
            env_x11: &[],
            env_wl: &[],
            wayland: true,
            profile: true,
            kill_ok: true,
            settle_ms: 4000,
            small: (700, 480),
            large: (1200, 800),
        },
        App {
            id: "firefox",
            toolkit: "Gecko (snap)",
            class_x11: "firefox",
            class_wl: "firefox",
            argv: &[
                "firefox",
                "--no-remote",
                "--new-instance",
                "--profile",
                "{PROFILE}",
                "about:blank",
            ],
            env_x11: &[("MOZ_ENABLE_WAYLAND", "0"), ("GDK_BACKEND", "x11")],
            env_wl: &[("MOZ_ENABLE_WAYLAND", "1")],
            wayland: true,
            profile: true,
            kill_ok: true,
            settle_ms: 4500,
            small: (800, 560),
            large: (1300, 860),
        },
        // X11-FLASH-02i exposure map (with DING loaded): the rest of the installed set, in the same
        // shape as the apps above. Settles are shorter because the matrix only asks "crash or not".
        App {
            id: "gnome-calculator",
            toolkit: "GTK4",
            class_x11: "org.gnome.Calculator",
            class_wl: "org.gnome.Calculator",
            argv: &["gnome-calculator"],
            env_x11: &[("GDK_BACKEND", "x11"), ("GSETTINGS_BACKEND", "memory")],
            env_wl: &[("GSETTINGS_BACKEND", "memory")],
            wayland: true,
            profile: false,
            kill_ok: true,
            settle_ms: 2000,
            small: (500, 400),
            large: (900, 700),
        },
        App {
            id: "nautilus",
            toolkit: "GTK4",
            class_x11: "org.gnome.Nautilus",
            class_wl: "org.gnome.Nautilus",
            argv: &["nautilus", "--new-window"],
            env_x11: &[("GDK_BACKEND", "x11"), ("GSETTINGS_BACKEND", "memory")],
            env_wl: &[("GSETTINGS_BACKEND", "memory")],
            wayland: true,
            profile: false,
            kill_ok: true,
            settle_ms: 3000,
            small: (700, 480),
            large: (1100, 760),
        },
        App {
            id: "gnome-terminal",
            toolkit: "GTK3 (server)",
            class_x11: "Gnome-terminal",
            class_wl: "org.gnome.Terminal",
            argv: &["gnome-terminal"],
            env_x11: &[("GDK_BACKEND", "x11"), ("GSETTINGS_BACKEND", "memory")],
            env_wl: &[("GSETTINGS_BACKEND", "memory")],
            wayland: true,
            profile: false,
            kill_ok: false,
            settle_ms: 3000,
            small: (700, 460),
            large: (1100, 700),
        },
        App {
            id: "code",
            toolkit: "Electron",
            class_x11: "Code",
            class_wl: "code",
            argv: &[
                "code",
                "--user-data-dir={PROFILE}",
                "--extensions-dir={PROFILE}/ext",
                "--no-sandbox",
                "--ozone-platform={OZONE}",
                "--disable-gpu",
                "--new-window",
                "--skip-welcome",
            ],
            env_x11: &[],
            env_wl: &[],
            wayland: true,
            profile: true,
            kill_ok: true,
            settle_ms: 5000,
            small: (800, 560),
            large: (1200, 800),
        },
        App {
            // scratch/xflash02/swing/SwingApp.class, compiled from the source quoted in
            // ding-crash.md; not committed (committed tooling is Rust).
            id: "java-swing",
            toolkit: "Swing (AWT X11)",
            class_x11: "SwingApp",
            class_wl: "",
            argv: &["java", "-cp", "scratch/xflash02/swing", "SwingApp"],
            env_x11: &[],
            env_wl: &[],
            wayland: false,
            profile: false,
            kill_ok: true,
            settle_ms: 3000,
            small: (600, 400),
            large: (900, 650),
        },
        App {
            // Qt 5.15 Widgets: the qt5 tools are installed, there is no distro Qt app.
            id: "qt-assistant",
            toolkit: "Qt 5.15 Widgets",
            class_x11: "assistant",
            class_wl: "assistant",
            argv: &["/usr/lib/x86_64-linux-gnu/qt5/bin/assistant"],
            env_x11: &[("QT_QPA_PLATFORM", "xcb")],
            env_wl: &[("QT_QPA_PLATFORM", "wayland")],
            wayland: true,
            profile: false,
            kill_ok: true,
            settle_ms: 3000,
            small: (700, 500),
            large: (1000, 700),
        },
        // X11-FLASH-03: more toolkits for RUNBOOK-03b. Class names are the first guess and are
        // checked by the runbook's smoke step (`status=ok` and a window id for 1 run each).
        App {
            id: "mousepad",
            toolkit: "GTK3",
            class_x11: "Mousepad",
            class_wl: "Mousepad",
            argv: &["mousepad", "--disable-server"],
            env_x11: &[("GDK_BACKEND", "x11"), ("GSETTINGS_BACKEND", "memory")],
            env_wl: &[("GDK_BACKEND", "wayland"), ("GSETTINGS_BACKEND", "memory")],
            wayland: true,
            profile: false,
            kill_ok: true,
            settle_ms: 2500,
            small: (600, 420),
            large: (1000, 700),
        },
        App {
            id: "evince",
            toolkit: "GTK3",
            class_x11: "Evince",
            class_wl: "org.gnome.Evince",
            argv: &["evince"],
            env_x11: &[("GDK_BACKEND", "x11"), ("GSETTINGS_BACKEND", "memory")],
            env_wl: &[("GSETTINGS_BACKEND", "memory")],
            wayland: true,
            profile: false,
            kill_ok: true,
            settle_ms: 2500,
            small: (600, 420),
            large: (1000, 700),
        },
        App {
            id: "gnome-system-monitor",
            toolkit: "GTK4 (libadwaita)",
            class_x11: "gnome-system-monitor",
            class_wl: "gnome-system-monitor",
            argv: &["gnome-system-monitor"],
            env_x11: &[("GDK_BACKEND", "x11"), ("GSETTINGS_BACKEND", "memory")],
            env_wl: &[("GSETTINGS_BACKEND", "memory")],
            wayland: true,
            profile: false,
            kill_ok: true,
            settle_ms: 3000,
            small: (700, 500),
            large: (1000, 700),
        },
        App {
            id: "baobab",
            toolkit: "GTK4 (libadwaita)",
            class_x11: "baobab",
            class_wl: "org.gnome.baobab",
            argv: &["baobab"],
            env_x11: &[("GDK_BACKEND", "x11"), ("GSETTINGS_BACKEND", "memory")],
            env_wl: &[("GSETTINGS_BACKEND", "memory")],
            wayland: true,
            profile: false,
            kill_ok: true,
            settle_ms: 3000,
            small: (700, 500),
            large: (1000, 700),
        },
        App {
            // GPU terminal (OpenGL); kitty chooses its display server with an option, not an env var.
            id: "kitty",
            toolkit: "GLFW/OpenGL",
            class_x11: "xflash-kitty",
            class_wl: "xflash-kitty",
            argv: &["kitty", "--class", "xflash-kitty", "-o", "linux_display_server={OZONE}", "-o", "confirm_os_window_close=0"],
            env_x11: &[],
            env_wl: &[],
            wayland: true,
            profile: false,
            kill_ok: true,
            settle_ms: 3000,
            small: (700, 460),
            large: (1000, 700),
        },
        App {
            id: "xclock",
            toolkit: "Xt/Athena (X11 only)",
            class_x11: "XClock",
            class_wl: "",
            argv: &["xclock", "-digital"],
            env_x11: &[],
            env_wl: &[],
            wayland: false,
            profile: false,
            kill_ok: true,
            settle_ms: 2000,
            small: (300, 120),
            large: (600, 200),
        },
        App {
            id: "xcalc",
            toolkit: "Xt/Athena (X11 only)",
            class_x11: "XCalc",
            class_wl: "",
            argv: &["xcalc"],
            env_x11: &[],
            env_wl: &[],
            wayland: false,
            profile: false,
            kill_ok: true,
            settle_ms: 2000,
            small: (300, 400),
            large: (500, 600),
        },
    ]
}

struct Opts {
    out: PathBuf,
    state: PathBuf,
    app: String,
    cond: String,
    reps: usize,
    ctl_reps: usize,
    seed: u64,
    warmup_s: u64,
    burn: String,
    burn_threads: u32,
    settle_override: Option<u64>,
    probe: bool,
    sizes: Option<((u32, u32), (u32, u32))>,
    max_runs: Option<usize>,
    ack_restart: bool,
    shell_log: Option<PathBuf>,
    shell_pid_file: Option<PathBuf>,
}

fn parse(args: &[String]) -> Result<Opts, String> {
    let mut o = Opts {
        out: PathBuf::new(),
        state: PathBuf::from("scratch/xflash02/state.json"),
        app: String::new(),
        cond: "idle".into(),
        reps: 20,
        ctl_reps: 0,
        seed: 1,
        warmup_s: 30,
        burn: "target/debug/xtask".into(),
        burn_threads: 22,
        settle_override: None,
        probe: false,
        sizes: None,
        max_runs: None,
        ack_restart: false,
        shell_log: None,
        shell_pid_file: None,
    };
    let mut i = 0;
    while i < args.len() {
        let flag = args[i].as_str();
        if flag == "--probe" || flag == "--ack-restart" {
            if flag == "--probe" {
                o.probe = true;
            } else {
                o.ack_restart = true;
            }
            i += 1;
            continue;
        }
        let val = args.get(i + 1).ok_or_else(|| format!("{flag} needs a value"))?;
        match flag {
            "--out" => o.out = PathBuf::from(val),
            "--state" => o.state = PathBuf::from(val),
            "--shell-log" => o.shell_log = Some(PathBuf::from(val)),
            "--shell-pid-file" => o.shell_pid_file = Some(PathBuf::from(val)),
            "--app" => o.app = val.clone(),
            "--cond" => o.cond = val.clone(),
            "--reps" => o.reps = val.parse().map_err(|_| "--reps N")?,
            "--ctl-reps" => o.ctl_reps = val.parse().map_err(|_| "--ctl-reps N")?,
            "--seed" => o.seed = val.parse().map_err(|_| "--seed N")?,
            "--warmup-s" => o.warmup_s = val.parse().map_err(|_| "--warmup-s N")?,
            "--burn" => o.burn = val.clone(),
            "--burn-threads" => o.burn_threads = val.parse().map_err(|_| "--burn-threads N")?,
            "--settle-ms" => o.settle_override = Some(val.parse().map_err(|_| "--settle-ms N")?),
            "--max-runs" => o.max_runs = Some(val.parse().map_err(|_| "--max-runs N")?),
            "--sizes" => {
                // WxH,WxH : small, large
                let mut it = val.split(',').map(|s| {
                    let mut p = s.split('x');
                    (
                        p.next().and_then(|x| x.parse().ok()).unwrap_or(0u32),
                        p.next().and_then(|x| x.parse().ok()).unwrap_or(0u32),
                    )
                });
                o.sizes = Some((it.next().unwrap_or((0, 0)), it.next().unwrap_or((0, 0))));
            }
            other => return Err(format!("run: unknown argument {other}")),
        }
        i += 2;
    }
    if o.out.as_os_str().is_empty() {
        return Err("run: --out DIR is required".into());
    }
    if o.app.is_empty() {
        return Err("run: --app NAME is required (one application per batch)".into());
    }
    Ok(o)
}

#[derive(Clone)]
struct Planned {
    wayland: bool,
    size_case: &'static str,
}

pub fn sh_out(prog: &str, args: &[&str]) -> String {
    Command::new(prog)
        .args(args)
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default()
}

fn read_loadavg() -> String {
    fs::read_to_string("/proc/loadavg")
        .map(|s| s.split_whitespace().take(3).collect::<Vec<_>>().join(" "))
        .unwrap_or_default()
}

/// Identity of the running session: boot id, gnome-shell pid and its start time (field 22 of
/// /proc/PID/stat, in clock ticks since boot). A change in any of them is a Shell restart.
#[derive(Clone, PartialEq, Debug)]
pub struct Health {
    pub boot_id: String,
    pub shell_pid: String,
    pub shell_start: String,
}

/// Where the Shell under test is recorded when it is not the live one: `--shell-pid-file` (the
/// private headless Shell of psh-start.sh). Set once at the start of a batch.
pub static SHELL_PID_FILE: std::sync::OnceLock<Option<PathBuf>> = std::sync::OnceLock::new();

pub fn health() -> Health {
    let first = match SHELL_PID_FILE.get().and_then(|o| o.as_ref()) {
        // A private Shell: the pid in the file, and only while that pid really is a gnome-shell.
        Some(f) => {
            let p = fs::read_to_string(f).unwrap_or_default().trim().to_string();
            let comm = fs::read_to_string(format!("/proc/{}/comm", p)).unwrap_or_default();
            if comm.trim() == "gnome-shell" { p } else { String::new() }
        }
        None => sh_out("pgrep", &["-x", "gnome-shell"]).lines().next().unwrap_or("").to_string(),
    };
    let start = fs::read_to_string(format!("/proc/{}/stat", first))
        .ok()
        .and_then(|s| {
            // The command name is in parentheses and may contain spaces: split after the last ')'.
            let rest = s.rsplit_once(") ")?.1.to_string();
            rest.split_whitespace().nth(19).map(|v| v.to_string())
        })
        .unwrap_or_default();
    Health {
        boot_id: fs::read_to_string("/proc/sys/kernel/random/boot_id")
            .unwrap_or_default()
            .trim()
            .to_string(),
        shell_pid: first,
        shell_start: start,
    }
}

fn health_json(h: &Health) -> String {
    format!(
        "{{\"boot_id\":{},\"shell_pid\":{},\"shell_start\":{}}}",
        js(&h.boot_id),
        js(&h.shell_pid),
        js(&h.shell_start)
    )
}

fn parse_health(s: &str) -> Option<Health> {
    Some(Health {
        boot_id: str_field(s, "boot_id")?,
        shell_pid: str_field(s, "shell_pid")?,
        shell_start: str_field(s, "shell_start")?,
    })
}

pub fn set_logging(on: bool) -> bool {
    Command::new("gdbus")
        .args([
            "call",
            "--session",
            "--dest",
            "org.gnome.Shell",
            "--object-path",
            "/org/gnome/Shell/Extensions/SpawnAt",
            "--method",
            "org.gnome.Shell.Extensions.SpawnAt.SetLogging",
            if on { "true" } else { "false" },
        ])
        .stdout(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// `n` bracketed SetLogging(true) calls. Each leaves a `SET_LOGGING` echo in the extension log;
/// the analyser pairs the echoes with these brackets in order (the method of the verify tool's
/// ClockAnchor, crates/spawn-at-verify/src/trace/flash.rs). It is a cross-check: the
/// `mono_ms` fields of the extension's `X11_OBS_*` lines give the clock origin directly.
fn calibrate(log: &mut File, tag: &str, n: usize) {
    for k in 0..n {
        let before = mono_ns();
        let ok = set_logging(true);
        let after = mono_ns();
        let _ = writeln!(
            log,
            "{{\"ev\":\"calib\",\"phase\":{},\"k\":{},\"before\":{},\"after\":{},\"ok\":{}}}",
            js(tag),
            k,
            before,
            after,
            ok
        );
        std::thread::sleep(Duration::from_millis(40));
    }
}

pub fn windows_json() -> String {
    Command::new(std::env::var_os("SPAWN_AT").unwrap_or_else(|| "spawn-at".into()))
        .args(["query", "windows", "--json"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        .unwrap_or_default()
}

/// The object of `spawn-at query windows --json` whose id is `id`.
pub fn window_entry(all: &str, id: u64) -> Option<String> {
    let pat = format!("{{\"id\":{},", id);
    let start = all.find(&pat)?;
    let rest = &all[start..];
    let end = rest[1..].find("{\"id\":").map(|e| e + 1).unwrap_or(rest.len());
    Some(rest[..end].to_string())
}

pub fn all_ids(all: &str) -> Vec<(u64, u64)> {
    let mut v = Vec::new();
    let mut rest = all;
    while let Some(p) = rest.find("{\"id\":") {
        let seg = &rest[p..];
        let end = seg[1..].find("{\"id\":").map(|e| e + 1).unwrap_or(seg.len());
        let obj = &seg[..end];
        if let (Some(id), Some(pid)) = (num_field(obj, "id"), num_field(obj, "pid")) {
            v.push((id as u64, pid as u64));
        }
        rest = &seg[end..];
    }
    v
}

pub fn pid_alive(pid: u64) -> bool {
    Path::new(&format!("/proc/{}", pid)).exists()
}

pub fn signal(pid: u64, sig: i32) {
    // SAFETY: plain kill(2) on a pid this tool recorded; no pattern matching involved.
    unsafe {
        libc::kill(pid as i32, sig);
    }
}

fn is_safe_profile_dir(repo: &Path, dir: &Path) -> bool {
    let want = repo.join("scratch").join("xflash02").join("profiles");
    if !dir.starts_with(&want) || dir == want {
        return false;
    }
    // `findmnt --mountpoint` succeeds only for a mount point; a profile directory is not one.
    let mounted = Command::new("findmnt")
        .args(["--mountpoint"])
        .arg(dir)
        .stdout(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(true);
    !mounted
}

/// Whether `pid` belongs to the private sandbox this batch runs in. `psh` exports `PSH_SB=<dir>` into
/// every process of the sandbox. Without `PSH_SB` in our own environment (a batch outside `psh`)
/// every process counts, as before. X11-FLASH-03: an editor the maintainer opens in the LIVE session
/// has no `PSH_SB`, and must not stop a batch (the scan is by program name).
fn in_our_sandbox(pid: u32) -> bool {
    let Ok(sb) = std::env::var("PSH_SB") else { return true };
    let Ok(env) = fs::read(format!("/proc/{}/environ", pid)) else { return false };
    let want = format!("PSH_SB={}", sb);
    env.split(|b| *b == 0).any(|kv| kv == want.as_bytes())
}

/// Read-only scan of /proc for leftovers of an earlier batch: an observer, a CPU burner, a test app.
fn stray_processes(own: u32) -> Vec<String> {
    let mut found = Vec::new();
    let Ok(rd) = fs::read_dir("/proc") else { return found };
    for e in rd.flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        let Ok(pid) = name.parse::<u32>() else { continue };
        if pid == own {
            continue;
        }
        let Ok(raw) = fs::read(format!("/proc/{}/cmdline", pid)) else { continue };
        let args: Vec<String> = raw
            .split(|b| *b == 0)
            .filter(|a| !a.is_empty())
            .map(|a| String::from_utf8_lossy(a).into_owned())
            .collect();
        let Some(first) = args.first() else { continue };
        let base = first.rsplit('/').next().unwrap_or(first);
        let ours = (base == "xflash")
            || (base == "xtask" && args.iter().any(|a| a == "burn-cpu"))
            || args.iter().any(|a| a.contains("scratch/xflash02/profiles"))
            || (matches!(base, "xterm" | "gnome-text-editor" | "eog") && in_our_sandbox(pid));
        if ours {
            found.push(format!("{} {}", pid, args.iter().take(3).cloned().collect::<Vec<_>>().join(" ")));
        }
    }
    found
}

struct Progress {
    file: File,
}

impl Progress {
    fn event(&mut self, ev: &str, extra: &str) {
        let line = format!(
            "{{\"ev\":\"{}\",\"t_mono\":{},\"t_wall\":{}{}}}\n",
            ev,
            mono_ns(),
            js(&sh_out("date", &["-Is"])),
            if extra.is_empty() { String::new() } else { format!(",{}", extra) }
        );
        let _ = self.file.write_all(line.as_bytes());
        let _ = self.file.flush();
        let _ = self.file.sync_data();
    }
}

fn done_keys(path: &Path) -> HashSet<String> {
    let mut s = HashSet::new();
    if let Ok(text) = fs::read_to_string(path) {
        for l in text.lines() {
            if str_field(l, "ev").as_deref() == Some("run") {
                if let Some(k) = str_field(l, "key") {
                    s.insert(k);
                }
            }
        }
    }
    s
}

pub fn run(args: &[String]) -> Result<(), String> {
    let o = parse(args)?;
    let _ = SHELL_PID_FILE.set(o.shell_pid_file.clone());
    // Private session: the extension's lines are read from the Shell's own stderr file, from the
    // size it has now (the batch's start), not from the journal.
    let log_off: u64 = o.shell_log.as_ref().and_then(|p| fs::metadata(p).ok()).map(|m| m.len()).unwrap_or(0);
    let all_apps = apps();
    let app = all_apps
        .iter()
        .find(|a| a.id == o.app)
        .ok_or_else(|| format!("unknown app {}", o.app))?;
    let repo = std::env::current_dir().map_err(|e| e.to_string())?;
    fs::create_dir_all(&o.out).map_err(|e| format!("{}: {e}", o.out.display()))?;
    let prof_root = repo.join("scratch").join("xflash02").join("profiles");
    fs::create_dir_all(&prof_root).map_err(|e| e.to_string())?;
    let progress_path = o.out.join("progress.jsonl");
    let mut prog = Progress {
        file: OpenOptions::new()
            .create(true)
            .append(true)
            .open(&progress_path)
            .map_err(|e| e.to_string())?,
    };

    // ---- startup checks ----
    let h0 = health();
    let mut last: Option<Health> = fs::read_to_string(&o.state).ok().and_then(|s| parse_health(&s));
    // The progress file of this batch also carries the last health seen.
    if let Ok(text) = fs::read_to_string(&progress_path) {
        for l in text.lines().rev() {
            if let Some(h) = raw_field(l, "health").and_then(parse_health) {
                last = Some(h);
                break;
            }
        }
    }
    let strays = stray_processes(std::process::id());
    if !strays.is_empty() {
        prog.event("stop", &format!("\"reason\":\"stray processes\",\"pids\":{}", js(&strays.join("; "))));
        return Err(format!("stray processes, not touching them: {}", strays.join("; ")));
    }
    if h0.shell_pid.is_empty() {
        return Err("gnome-shell not found".into());
    }
    if let Some(l) = &last {
        if *l != h0 {
            prog.event(
                "crash",
                &format!(
                    "\"note\":\"session changed since the last recorded run (shell restart or reboot)\",\"before\":{},\"now\":{},\"acknowledged\":{}",
                    health_json(l),
                    health_json(&h0),
                    o.ack_restart
                ),
            );
            if !o.ack_restart {
                return Err("the Shell restarted since the last run: CRASH event written, stopping. Review, then pass --ack-restart.".into());
            }
        }
    }
    let _ = fs::write(&o.state, health_json(&h0));
    let baseline_json = windows_json();
    let baseline: HashSet<u64> = all_ids(&baseline_json).into_iter().map(|(i, _)| i).collect();
    let baseline_pids: HashSet<u64> = all_ids(&baseline_json).into_iter().map(|(_, p)| p).collect();

    // ---- plan (deterministic from the arguments and the seed) ----
    let mut plan: Vec<Planned> = Vec::new();
    let reps = if o.probe { 1 } else { o.reps };
    for case in ["small", "large"] {
        for _ in 0..reps {
            plan.push(Planned { wayland: false, size_case: if case == "small" { "small" } else { "large" } });
        }
    }
    if app.wayland && o.ctl_reps > 0 {
        for k in 0..o.ctl_reps {
            plan.push(Planned { wayland: true, size_case: if k % 2 == 0 { "small" } else { "large" } });
        }
    }
    if o.probe && app.wayland {
        plan.push(Planned { wayland: true, size_case: "small" });
    }
    let mut rng = Rng(o.seed.max(1));
    rng.shuffle(&mut plan);
    if let Some(m) = o.max_runs {
        plan.truncate(m);
    }
    let done = done_keys(&progress_path);
    let key_of = |n: usize, p: &Planned| format!("{}:{}:{}:{}", n, app.id, if p.wayland { "wayland" } else { "x11" }, p.size_case);
    let todo = plan.iter().enumerate().filter(|(n, p)| !done.contains(&key_of(*n, p))).count();

    let resumed = !done.is_empty() || o.out.join("meta.json").exists();
    if !o.out.join("meta.json").exists() {
        let meta = format!(
            "{{\"started\":{},\"started_epoch\":{},\"app\":{},\"cond\":{},\"reps\":{},\"ctl_reps\":{},\"seed\":{},\"planned_runs\":{},\"probe\":{},\"loadavg_start\":{},\"gnome_shell\":{},\"spawn_at\":{},\"session\":{},\"warmup_s\":{},\"burn_threads\":{},\"health\":{},\"no_focus\":true}}\n",
            js(&sh_out("date", &["-Is"])),
            sh_out("date", &["+%s"]),
            js(app.id),
            js(&o.cond),
            o.reps,
            o.ctl_reps,
            o.seed,
            plan.len(),
            o.probe,
            js(&read_loadavg()),
            js(&sh_out("gnome-shell", &["--version"])),
            js(&sh_out("spawn-at", &["--version"])),
            js(&std::env::var("XDG_SESSION_TYPE").unwrap_or_default()),
            o.warmup_s,
            o.burn_threads,
            health_json(&h0)
        );
        fs::write(o.out.join("meta.json"), meta).map_err(|e| e.to_string())?;
    }
    let since_epoch: String = fs::read_to_string(o.out.join("meta.json"))
        .ok()
        .and_then(|m| num_field(&m, "started_epoch"))
        .map(|v| format!("{}", v as u64))
        .unwrap_or_else(|| sh_out("date", &["+%s"]));
    prog.event(
        if resumed { "resume" } else { "start" },
        &format!("\"health\":{},\"todo\":{},\"planned\":{}", health_json(&h0), todo, plan.len()),
    );
    if todo == 0 {
        prog.event("stop", "\"reason\":\"nothing left to do\"");
        return Ok(());
    }

    let mut runs = OpenOptions::new().create(true).append(true).open(o.out.join("runs.jsonl")).map_err(|e| e.to_string())?;
    let mut calib = OpenOptions::new().create(true).append(true).open(o.out.join("calib.jsonl")).map_err(|e| e.to_string())?;
    if !set_logging(true) {
        prog.event("stop", "\"reason\":\"SetLogging failed\"");
        return Err("SetLogging(true) failed: is the extension loaded?".into());
    }
    std::thread::sleep(Duration::from_millis(200));

    // ---- observer ----
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let obs_path = o.out.join(if resumed { format!("xobs-{}.jsonl", mono_ns() / 1_000_000_000) } else { "xobs.jsonl".to_string() });
    let mut observer = Command::new(&exe)
        .arg("observe")
        .arg("--out")
        .arg(&obs_path)
        .stdin(Stdio::null())
        .spawn()
        .map_err(|e| format!("observer: {e}"))?;
    let observer_pid = observer.id() as u64;
    std::thread::sleep(Duration::from_millis(1500));

    // ---- load ----
    let mut burner: Option<u64> = None;
    let mut burn_child = None;
    if o.cond == "loaded" {
        let secs = (todo as u64) * 16 + o.warmup_s + 120;
        let c = Command::new(&o.burn)
            .args(["burn-cpu", "--threads", &o.burn_threads.to_string(), "--seconds", &secs.to_string()])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("burn-cpu: {e}"))?;
        burner = Some(c.id() as u64);
        burn_child = Some(c);
        eprintln!("warming up {} s under load ...", o.warmup_s);
        std::thread::sleep(Duration::from_secs(o.warmup_s));
    }
    calibrate(&mut calib, if resumed { "resume" } else { "start" }, 30);

    // ---- runs ----
    let mut stopped = String::new();
    let mut ran = 0usize;
    for (n, p) in plan.iter().enumerate() {
        let key = key_of(n, p);
        if done.contains(&key) {
            continue;
        }
        if health() != h0 {
            stopped = format!("CRASH: gnome-shell or boot id changed before run {n}");
            prog.event("crash", &format!("\"key\":{},\"now\":{}", js(&key), health_json(&health())));
            break;
        }
        // One window at a time: nothing may exist that was not there at the start.
        let mut clear = false;
        for _ in 0..50 {
            let cur: Vec<u64> = all_ids(&windows_json()).into_iter().map(|(i, _)| i).collect();
            if cur.iter().all(|i| baseline.contains(i)) {
                clear = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(200));
        }
        if !clear {
            stopped = format!("a window that is not in the baseline still exists before run {n}");
            prog.event("stop", &format!("\"reason\":{}", js(&stopped)));
            break;
        }

        let size = match o.sizes {
            Some((s, l)) => if p.size_case == "small" { s } else { l },
            None => if p.size_case == "small" { app.small } else { app.large },
        };
        let settle = o.settle_override.unwrap_or(app.settle_ms);
        let pre_pids = baseline_pids.clone();
        let profile = if app.profile {
            let d = prof_root.join(format!("{}-{}", o.out.file_name().and_then(|s| s.to_str()).unwrap_or("b"), n));
            let _ = fs::create_dir_all(&d);
            Some(d)
        } else {
            None
        };
        let ozone = if p.wayland { "wayland" } else { "x11" };
        let argv: Vec<String> = app
            .argv
            .iter()
            .map(|a| {
                a.replace("{OZONE}", ozone)
                    .replace("{PROFILE}", &profile.as_ref().map(|d| d.display().to_string()).unwrap_or_default())
            })
            .collect();
        let class = if p.wayland { app.class_wl } else { app.class_x11 };
        let out_f = o.out.join("tmp_stdout.txt");
        let err_f = o.out.join("tmp_stderr.txt");
        let sf = File::create(&out_f).map_err(|e| e.to_string())?;
        let ef = File::create(&err_f).map_err(|e| e.to_string())?;
        let mut cmd = Command::new("spawn-at");
        cmd.arg("spawn");
        if !class.is_empty() {
            cmd.args(["-c", class]);
        }
        cmd.args(["--no-focus", "--anchor", "bottom-right", "--size", &size.0.to_string(), &size.1.to_string(), "-m", "24", "--json", "--"]);
        cmd.args(&argv);
        for (k, v) in if p.wayland { app.env_wl } else { app.env_x11 } {
            cmd.env(k, v);
        }
        cmd.stdin(Stdio::null()).stdout(sf).stderr(ef);
        let load = read_loadavg();
        let log_before: u64 = o.shell_log.as_ref().and_then(|p| fs::metadata(p).ok()).map(|m| m.len()).unwrap_or(0);
        let t_start = mono_ns();
        let mut child = cmd.spawn().map_err(|e| format!("spawn-at: {e}"))?;
        let deadline = Instant::now() + Duration::from_secs(25);
        let mut exit: Option<i32> = None;
        loop {
            match child.try_wait() {
                Ok(Some(st)) => {
                    exit = st.code();
                    break;
                }
                Ok(None) => {
                    if Instant::now() > deadline {
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(5));
                }
                Err(_) => break,
            }
        }
        let t_ret = mono_ns();
        let timed_out = exit.is_none();
        if timed_out {
            // Our own `spawn-at spawn` child, pid recorded by std.
            signal(child.id() as u64, libc::SIGTERM);
            let _ = child.wait();
        }
        let stdout = fs::read_to_string(&out_f).unwrap_or_default();
        let stderr = fs::read_to_string(&err_f).unwrap_or_default();
        let wid = num_field(&stdout, "window_id").map(|v| v as u64);

        std::thread::sleep(Duration::from_millis(settle));
        let post = windows_json();
        let (entry, wpid) = match wid.and_then(|w| window_entry(&post, w)) {
            Some(e) => {
                let pid = num_field(&e, "pid").map(|v| v as u64);
                (Some(e), pid)
            }
            None => (None, None),
        };
        let focused_at_close = entry.as_deref().map(|e| raw_field(e, "focused").unwrap_or("null").to_string()).unwrap_or_else(|| "null".into());
        let t_settled = mono_ns();
        // What the app really got: DESKTOP_STARTUP_ID and XDG_ACTIVATION_TOKEN from its environment.
        let app_env = wpid
            .and_then(|p| fs::read(format!("/proc/{}/environ", p)).ok())
            .map(|raw| {
                let get = |k: &str| -> String {
                    raw.split(|b| *b == 0)
                        .filter_map(|e| std::str::from_utf8(e).ok())
                        .find_map(|e| e.strip_prefix(&format!("{}=", k)).map(|v| js(v)))
                        .unwrap_or_else(|| "null".into())
                };
                format!("{{\"DESKTOP_STARTUP_ID\":{},\"XDG_ACTIVATION_TOKEN\":{}}}", get("DESKTOP_STARTUP_ID"), get("XDG_ACTIVATION_TOKEN"))
            })
            .unwrap_or_else(|| "null".into());

        // ---- graceful close; a signal only to a pid whose window is already gone ----
        let mut closed = false;
        let mut killed = String::new();
        if let Some(w) = wid {
            for attempt in 0..2 {
                let _ = Command::new("spawn-at").args(["close", "--id", &w.to_string()]).stdout(Stdio::null()).stderr(Stdio::null()).status();
                let until = Instant::now() + Duration::from_secs(5);
                while Instant::now() < until {
                    if window_entry(&windows_json(), w).is_none() {
                        closed = true;
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(200));
                }
                if closed {
                    break;
                }
                let _ = attempt;
            }
            if closed {
                let user_pid = wpid.map(|p| pre_pids.contains(&p)).unwrap_or(true);
                if let (Some(pid), true) = (wpid, app.kill_ok && !user_pid) {
                    let until = Instant::now() + Duration::from_secs(5);
                    while pid_alive(pid) && Instant::now() < until {
                        std::thread::sleep(Duration::from_millis(100));
                    }
                    if pid_alive(pid) {
                        signal(pid, libc::SIGTERM);
                        killed.push_str("TERM ");
                        let until = Instant::now() + Duration::from_secs(5);
                        while pid_alive(pid) && Instant::now() < until {
                            std::thread::sleep(Duration::from_millis(100));
                        }
                        if pid_alive(pid) {
                            signal(pid, libc::SIGKILL);
                            killed.push_str("KILL");
                        }
                    }
                }
            } else {
                stopped = format!("window {w} did not close after two graceful requests; not signalling a process whose window still exists");
            }
        }
        if let Some(d) = &profile {
            for _ in 0..3 {
                if is_safe_profile_dir(&repo, d) {
                    let _ = fs::remove_dir_all(d);
                } else {
                    eprintln!("refusing to remove {} (not a plain profile directory)", d.display());
                    break;
                }
                if !d.exists() {
                    break;
                }
                std::thread::sleep(Duration::from_millis(500));
            }
        }
        let t_end = mono_ns();
        // The Shell's own warnings during this run (private session only; the log has no stamps,
        // so the slice is taken by file offset).
        let shell_warn = match &o.shell_log {
            Some(p) => {
                let all = fs::read(p).unwrap_or_default();
                let from = (log_before as usize).min(all.len());
                let text = String::from_utf8_lossy(&all[from..]).into_owned();
                let w: Vec<&str> = text
                    .lines()
                    .filter(|l| !l.contains("[spawn-at-time]") && (l.contains("warning") || l.contains("assertion") || l.contains("Bail out") || l.contains("crashed") || l.contains("Buggy") || l.contains("ERROR")))
                    .collect();
                js(&w.join(" | "))
            }
            None => "null".into(),
        };
        let status = if wid.is_none() { "no_window" } else if !closed { "not_closed" } else if timed_out { "timed_out" } else { "ok" };
        let rec = format!(
            "{{\"run\":{},\"key\":{},\"app\":{},\"toolkit\":{},\"mode\":{},\"size_case\":{},\"cond\":{},\"req_w\":{},\"req_h\":{},\"window_id\":{},\"win_pid\":{},\"exit\":{},\"timed_out\":{},\"t_start\":{},\"t_return\":{},\"t_settled\":{},\"t_end\":{},\"loadavg\":{},\"closed\":{},\"focused_at_close\":{},\"killed\":{},\"app_env\":{},\"shell_warn\":{},\"final\":{},\"spawn_json\":{},\"stderr\":{}}}\n",
            n,
            js(&key),
            js(app.id),
            js(app.toolkit),
            js(if p.wayland { "wayland" } else { "x11" }),
            js(p.size_case),
            js(&o.cond),
            size.0,
            size.1,
            wid.map(|v| v.to_string()).unwrap_or_else(|| "null".into()),
            wpid.map(|v| v.to_string()).unwrap_or_else(|| "null".into()),
            exit.map(|v| v.to_string()).unwrap_or_else(|| "null".into()),
            timed_out,
            t_start,
            t_ret,
            t_settled,
            t_end,
            js(&load),
            closed,
            focused_at_close,
            js(&killed),
            app_env,
            shell_warn,
            entry
                .as_deref()
                .map(|e| format!(
                    "{{\"x\":{},\"y\":{},\"w\":{},\"h\":{},\"state\":{},\"class\":{}}}",
                    raw_field(e, "x").unwrap_or("null"),
                    raw_field(e, "y").unwrap_or("null"),
                    raw_field(e, "w").unwrap_or("null"),
                    raw_field(e, "h").unwrap_or("null"),
                    js(&str_field(e, "state").unwrap_or_default()),
                    js(&str_field(e, "class").unwrap_or_default())
                ))
                .unwrap_or_else(|| "null".into()),
            if stdout.trim().is_empty() { "null".to_string() } else { stdout.trim().to_string() },
            js(stderr.trim())
        );
        let _ = runs.write_all(rec.as_bytes());
        let _ = runs.flush();
        prog.event("run", &format!("\"key\":{},\"status\":{},\"window_id\":{},\"health\":{}", js(&key), js(status), wid.map(|v| v.to_string()).unwrap_or_else(|| "null".into()), health_json(&health())));
        ran += 1;
        eprintln!("[{}/{}] {} {} {}x{} win={:?} status={}", n + 1, plan.len(), app.id, if p.wayland { "wayland" } else { "x11" }, size.0, size.1, wid, status);
        if !stopped.is_empty() {
            prog.event("stop", &format!("\"reason\":{}", js(&stopped)));
            break;
        }
        std::thread::sleep(Duration::from_millis(3000)); // settle: the next spawn starts 3 s after the window is gone
    }
    eprintln!("runs done in this invocation: {ran}");

    if stopped.is_empty() {
        calibrate(&mut calib, "end", 30);
    }

    // ---- teardown: only pids recorded above ----
    if let Some(b) = burner {
        signal(b, libc::SIGTERM);
    }
    if let Some(mut c) = burn_child {
        let _ = c.wait();
    }
    signal(observer_pid, libc::SIGTERM);
    let _ = observer.wait();
    std::thread::sleep(Duration::from_secs(3)); // the extension flushes its log at idle priority

    let journal = match &o.shell_log {
        Some(p) => {
            let all = fs::read(p).unwrap_or_default();
            let from = (log_off as usize).min(all.len());
            String::from_utf8_lossy(&all[from..]).into_owned()
        }
        None => Command::new("journalctl")
            .args(["--user", "-b", "_COMM=gnome-shell", "--since", &format!("@{}", since_epoch), "-o", "cat", "--no-pager"])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
            .unwrap_or_default(),
    };
    // A private run appends (several invocations share one Shell log); the live path rewrites.
    let append = o.shell_log.is_some();
    let open = |name: &str| -> Result<File, String> {
        OpenOptions::new().create(true).write(true).append(append).truncate(!append).open(o.out.join(name)).map_err(|e| e.to_string())
    };
    let mut ext = open("ext.log")?;
    let mut other = open("journal_other.txt")?;
    for line in journal.lines() {
        // Stderr lines may carry a prefix; the extension's text starts at its tag.
        if let Some(i) = line.find("[spawn-at-time]") {
            let _ = writeln!(ext, "{}", &line[i..]);
        } else if !line.trim().is_empty() {
            let _ = writeln!(other, "{}", line);
        }
    }
    let h1 = health();
    let end = format!(
        "{{\"ended\":{},\"stopped\":{},\"health_start\":{},\"health_end\":{},\"loadavg_end\":{},\"ran_in_this_invocation\":{}}}\n",
        js(&sh_out("date", &["-Is"])),
        js(&stopped),
        health_json(&h0),
        health_json(&h1),
        js(&read_loadavg()),
        ran
    );
    fs::write(o.out.join("end.json"), end).map_err(|e| e.to_string())?;
    let _ = fs::remove_file(o.out.join("tmp_stdout.txt"));
    let _ = fs::remove_file(o.out.join("tmp_stderr.txt"));
    if h1 == h0 {
        let _ = fs::write(&o.state, health_json(&h1));
    } else {
        prog.event("crash", &format!("\"note\":\"session changed during the batch\",\"now\":{}", health_json(&h1)));
        if stopped.is_empty() {
            stopped = "the Shell restarted during the batch".into();
        }
    }
    if !stopped.is_empty() {
        return Err(stopped);
    }
    Ok(())
}
