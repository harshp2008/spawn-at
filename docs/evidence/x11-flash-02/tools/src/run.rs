//! `xflash run`: drives one batch of spawns of real applications and records, for each run, the
//! monotonic stamps and the answers of `spawn-at`. It starts the X-side observer
//! (`xflash observe`) as a child for the whole batch, turns the extension's timing log on, and at
//! the end copies the extension's lines out of the journal.
//!
//! It only ever signals process ids it recorded itself (the observer, the CPU burner and, for the
//! apps marked `kill_ok`, the pid that `spawn-at query windows` reports for the window it
//! spawned). It never matches a process by name.

use crate::util::{js, mono_ns, num_field, raw_field, str_field, Rng};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// A real application the batch can spawn.
pub struct App {
    pub id: &'static str,
    pub toolkit: &'static str,
    /// Value of `spawn-at spawn --class`: the WM_CLASS class (X11) or app id (Wayland) of the window.
    pub class_x11: &'static str,
    pub class_wl: &'static str,
    /// argv; `{PROFILE}` is replaced by a fresh directory, `{OZONE}` by x11 or wayland.
    pub argv: &'static [&'static str],
    pub env_x11: &'static [(&'static str, &'static str)],
    pub env_wl: &'static [(&'static str, &'static str)],
    /// Whether a native Wayland control run exists for it.
    pub wayland: bool,
    /// Needs a throwaway profile directory per run.
    pub profile: bool,
    /// The window's pid is a process of ours and may be signalled after the window is closed.
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
            class_x11: "Google-chrome",
            class_wl: "xflash-chrome",
            argv: &[
                "google-chrome",
                "--user-data-dir={PROFILE}",
                "--no-first-run",
                "--no-default-browser-check",
                "--disable-sync",
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
    ]
}

struct Opts {
    out: PathBuf,
    apps: Vec<String>,
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
}

fn parse(args: &[String]) -> Result<Opts, String> {
    let mut o = Opts {
        out: PathBuf::new(),
        apps: Vec::new(),
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
    };
    let mut i = 0;
    let need = |i: usize| -> Result<&String, String> {
        args.get(i + 1).ok_or_else(|| format!("{} needs a value", args[i]))
    };
    while i < args.len() {
        match args[i].as_str() {
            "--out" => o.out = PathBuf::from(need(i)?),
            "--apps" => o.apps = need(i)?.split(',').map(|s| s.to_string()).collect(),
            "--cond" => o.cond = need(i)?.clone(),
            "--reps" => o.reps = need(i)?.parse().map_err(|_| "--reps N")?,
            "--ctl-reps" => o.ctl_reps = need(i)?.parse().map_err(|_| "--ctl-reps N")?,
            "--seed" => o.seed = need(i)?.parse().map_err(|_| "--seed N")?,
            "--warmup-s" => o.warmup_s = need(i)?.parse().map_err(|_| "--warmup-s N")?,
            "--burn" => o.burn = need(i)?.clone(),
            "--burn-threads" => o.burn_threads = need(i)?.parse().map_err(|_| "--burn-threads N")?,
            "--settle-ms" => o.settle_override = Some(need(i)?.parse().map_err(|_| "--settle-ms N")?),
            "--max-runs" => o.max_runs = Some(need(i)?.parse().map_err(|_| "--max-runs N")?),
            "--sizes" => {
                // WxH,WxH : small, large for every app of the batch
                let v = need(i)?;
                let mut it = v.split(',').map(|s| {
                    let mut p = s.split('x');
                    (
                        p.next().and_then(|x| x.parse().ok()).unwrap_or(0u32),
                        p.next().and_then(|x| x.parse().ok()).unwrap_or(0u32),
                    )
                });
                o.sizes = Some((it.next().unwrap_or((0, 0)), it.next().unwrap_or((0, 0))));
            }
            "--probe" => {
                // A flag with no value: advance by one and skip the value step below.
                o.probe = true;
                i += 1;
                continue;
            }
            other => return Err(format!("run: unknown argument {other}")),
        }
        i += 2;
    }
    if o.out.as_os_str().is_empty() {
        return Err("run: --out DIR is required".into());
    }
    if o.apps.is_empty() {
        return Err("run: --apps a,b,c is required".into());
    }
    Ok(o)
}

#[derive(Clone)]
struct Planned {
    app: usize,
    wayland: bool,
    size_case: &'static str,
}

fn sh_out(prog: &str, args: &[&str]) -> String {
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

fn shell_pid() -> String {
    sh_out("pgrep", &["-x", "gnome-shell"])
}

fn set_logging(on: bool) -> bool {
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
/// the analyser pairs the echoes with these brackets in order (same method as the verify tool's
/// ClockAnchor, crates/spawn-at-verify/src/trace/flash.rs).
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

fn windows_json() -> String {
    Command::new("spawn-at")
        .args(["query", "windows", "--json"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        .unwrap_or_default()
}

/// The object of `spawn-at query windows --json` whose id is `id`.
fn window_entry(all: &str, id: u64) -> Option<String> {
    let pat = format!("{{\"id\":{},", id);
    let start = all.find(&pat)?;
    let rest = &all[start..];
    let end = rest[1..].find("{\"id\":").map(|e| e + 1).unwrap_or(rest.len());
    Some(rest[..end].to_string())
}

fn all_ids(all: &str) -> Vec<(u64, u64)> {
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

fn pid_alive(pid: u64) -> bool {
    Path::new(&format!("/proc/{}", pid)).exists()
}

fn signal(pid: u64, sig: i32) {
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

pub fn run(args: &[String]) -> Result<(), String> {
    let o = parse(args)?;
    let all_apps = apps();
    let repo = std::env::current_dir().map_err(|e| e.to_string())?;
    fs::create_dir_all(&o.out).map_err(|e| format!("{}: {e}", o.out.display()))?;
    let prof_root = repo.join("scratch").join("xflash02").join("profiles");
    fs::create_dir_all(&prof_root).map_err(|e| e.to_string())?;

    // ---- plan ----
    let mut chosen = Vec::new();
    for name in &o.apps {
        let idx = all_apps
            .iter()
            .position(|a| a.id == name)
            .ok_or_else(|| format!("unknown app {name}"))?;
        chosen.push(idx);
    }
    let mut plan: Vec<Planned> = Vec::new();
    let reps = if o.probe { 1 } else { o.reps };
    for &a in &chosen {
        for case in ["small", "large"] {
            for _ in 0..reps {
                plan.push(Planned { app: a, wayland: false, size_case: case });
            }
        }
        if all_apps[a].wayland && o.ctl_reps > 0 {
            for k in 0..o.ctl_reps {
                plan.push(Planned {
                    app: a,
                    wayland: true,
                    size_case: if k % 2 == 0 { "small" } else { "large" },
                });
            }
        }
        if o.probe && all_apps[a].wayland {
            plan.push(Planned { app: a, wayland: true, size_case: "small" });
        }
    }
    let mut rng = Rng(o.seed.max(1));
    rng.shuffle(&mut plan);
    if let Some(m) = o.max_runs {
        plan.truncate(m);
    }

    // ---- files ----
    let mut runs = OpenOptions::new()
        .create(true)
        .append(true)
        .open(o.out.join("runs.jsonl"))
        .map_err(|e| e.to_string())?;
    let start_wall = sh_out("date", &["-Is"]);
    let t_batch0 = mono_ns();
    let meta = format!(
        "{{\"started\":{},\"cond\":{},\"reps\":{},\"ctl_reps\":{},\"seed\":{},\"apps\":{},\"planned_runs\":{},\"probe\":{},\"loadavg_start\":{},\"gnome_shell\":{},\"gnome_shell_pid\":{},\"spawn_at\":{},\"session\":{},\"warmup_s\":{},\"burn_threads\":{}}}\n",
        js(&start_wall),
        js(&o.cond),
        o.reps,
        o.ctl_reps,
        o.seed,
        js(&o.apps.join(",")),
        plan.len(),
        o.probe,
        js(&read_loadavg()),
        js(&sh_out("gnome-shell", &["--version"])),
        js(&shell_pid()),
        js(&sh_out("spawn-at", &["--version"])),
        js(&std::env::var("XDG_SESSION_TYPE").unwrap_or_default()),
        o.warmup_s,
        o.burn_threads
    );
    fs::write(o.out.join("meta.json"), meta).map_err(|e| e.to_string())?;
    let mut calib = File::create(o.out.join("calib.jsonl")).map_err(|e| e.to_string())?;

    let health0 = shell_pid();
    if !set_logging(true) {
        return Err("SetLogging(true) failed: is the extension loaded?".into());
    }
    std::thread::sleep(Duration::from_millis(200));

    // ---- observer ----
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let obs_path = o.out.join("xobs.jsonl");
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
        let secs = (plan.len() as u64) * 14 + o.warmup_s + 120;
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

    calibrate(&mut calib, "start", 40);

    // ---- runs ----
    let mut aborted = String::new();
    for (n, p) in plan.iter().enumerate() {
        if shell_pid() != health0 {
            aborted = format!("gnome-shell pid changed before run {n}");
            break;
        }
        let app = &all_apps[p.app];
        let size = match o.sizes {
            Some((s, l)) => if p.size_case == "small" { s } else { l },
            None => if p.size_case == "small" { app.small } else { app.large },
        };
        let settle = o.settle_override.unwrap_or(app.settle_ms);
        let pre = windows_json();
        let pre_ids = all_ids(&pre);
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
        cmd.args(["--anchor", "bottom-right", "--size", &size.0.to_string(), &size.1.to_string(), "-m", "24", "--json", "--"]);
        cmd.args(&argv);
        for (k, v) in if p.wayland { app.env_wl } else { app.env_x11 } {
            cmd.env(k, v);
        }
        cmd.stdin(Stdio::null()).stdout(sf).stderr(ef);
        let load = read_loadavg();
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
        let spawn_pid = child.id() as u64;
        if timed_out {
            signal(spawn_pid, libc::SIGTERM);
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
        let t_settled = mono_ns();

        // ---- close ----
        let mut closed = false;
        let mut killed = String::new();
        if let Some(w) = wid {
            let user_pid = wpid.map(|p| pre_ids.iter().any(|(_, q)| *q == p)).unwrap_or(true);
            let _ = Command::new("spawn-at").args(["close", "--id", &w.to_string()]).stdout(Stdio::null()).stderr(Stdio::null()).status();
            let until = Instant::now() + Duration::from_secs(6);
            while Instant::now() < until {
                if window_entry(&windows_json(), w).is_none() {
                    closed = true;
                    break;
                }
                std::thread::sleep(Duration::from_millis(200));
            }
            if let (Some(pid), true) = (wpid, app.kill_ok && !user_pid) {
                let until = Instant::now() + Duration::from_secs(5);
                while pid_alive(pid) && Instant::now() < until {
                    std::thread::sleep(Duration::from_millis(100));
                }
                if pid_alive(pid) {
                    signal(pid, libc::SIGTERM);
                    killed.push_str("TERM ");
                    let until = Instant::now() + Duration::from_secs(4);
                    while pid_alive(pid) && Instant::now() < until {
                        std::thread::sleep(Duration::from_millis(100));
                    }
                    if pid_alive(pid) {
                        signal(pid, libc::SIGKILL);
                        killed.push_str("KILL");
                    }
                }
            }
        }
        if let Some(d) = &profile {
            // The profile may hold files a dying browser is still writing; retry once.
            for _ in 0..2 {
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
        let rec = format!(
            "{{\"run\":{},\"app\":{},\"toolkit\":{},\"mode\":{},\"size_case\":{},\"cond\":{},\"req_w\":{},\"req_h\":{},\"window_id\":{},\"win_pid\":{},\"exit\":{},\"timed_out\":{},\"t_start\":{},\"t_return\":{},\"t_settled\":{},\"t_end\":{},\"loadavg\":{},\"closed\":{},\"killed\":{},\"final\":{},\"spawn_json\":{},\"stderr\":{}}}\n",
            n,
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
            js(&killed),
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
        eprintln!(
            "[{}/{}] {} {} {}x{} win={:?} closed={}",
            n + 1,
            plan.len(),
            app.id,
            if p.wayland { "wayland" } else { "x11" },
            size.0,
            size.1,
            wid,
            closed
        );
        std::thread::sleep(Duration::from_millis(900));
    }

    calibrate(&mut calib, "end", 40);
    let t_batch1 = mono_ns();

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

    let secs = (t_batch1 - t_batch0) / 1_000_000_000 + 30;
    let journal = Command::new("journalctl")
        .args(["--user", "-b", "_COMM=gnome-shell", "--since", &format!("-{}s", secs), "-o", "cat", "--no-pager"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        .unwrap_or_default();
    let mut ext = File::create(o.out.join("ext.log")).map_err(|e| e.to_string())?;
    let mut other = File::create(o.out.join("journal_other.txt")).map_err(|e| e.to_string())?;
    for line in journal.lines() {
        if line.starts_with("[spawn-at-time]") {
            let _ = writeln!(ext, "{}", line);
        } else if !line.trim().is_empty() {
            let _ = writeln!(other, "{}", line);
        }
    }
    let health1 = shell_pid();
    let end = format!(
        "{{\"ended\":{},\"aborted\":{},\"gnome_shell_pid_start\":{},\"gnome_shell_pid_end\":{},\"loadavg_end\":{},\"duration_s\":{}}}\n",
        js(&sh_out("date", &["-Is"])),
        js(&aborted),
        js(&health0),
        js(&health1),
        js(&read_loadavg()),
        (t_batch1 - t_batch0) / 1_000_000_000
    );
    fs::write(o.out.join("end.json"), end).map_err(|e| e.to_string())?;
    let _ = fs::remove_file(o.out.join("tmp_stdout.txt"));
    let _ = fs::remove_file(o.out.join("tmp_stderr.txt"));
    if !aborted.is_empty() {
        return Err(aborted);
    }
    if health0 != health1 {
        return Err("gnome-shell pid changed during the batch".into());
    }
    Ok(())
}
