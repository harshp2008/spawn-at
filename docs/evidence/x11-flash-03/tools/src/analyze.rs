//! `xflash analyze`: puts the extension's log and the X-side observer on one timeline and computes,
//! per run, V, R, D, A and L (see docs/evidence/x11-flash-02/README.md), then evaluates the
//! "client settled" reveal rules offline. Reads `runs.jsonl`, `ext.log`, `xobs*.jsonl` and
//! `calib.jsonl` of one or more batch directories; writes `runs.csv` and `summary.md` next to them.
//!
//! Clock: every `X11_OBS_*` line of the extension carries `mono_ms=`, the CLOCK_MONOTONIC reading
//! taken when the line was made. `mono_ms - elapsed` is the extension's clock origin; the
//! observer and the driver stamp with the same clock.

use crate::util::{fmt_opt, median, num_field, raw_field, str_field};
use std::collections::HashMap;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

/// One frame at 60 Hz, ms. Damage inside this time after the reveal is counted as part of the
/// reveal frame, not as a late paint.
const FRAME_MS: f64 = 16.7;
/// A burst of damage ends when nothing arrives for this long, ms.
const BURST_GAP_MS: f64 = 50.0;
/// The cap a reveal rule would fall back to, ms after the resize request.
const RULE_CAP_MS: f64 = 250.0;

#[derive(Clone, Debug)]
struct Ext {
    ms: f64,
    tag: String,
    wid: Option<u64>,
    extra: String,
}

fn parse_ext(line: &str) -> Option<Ext> {
    let rest = line.strip_prefix("[spawn-at-time] +")?;
    let (ms, rest) = rest.split_once("ms | ")?;
    let ms: f64 = ms.parse().ok()?;
    let (tag, rest) = rest.split_once(' ').unwrap_or((rest, ""));
    let mut wid = None;
    let mut extra = rest.to_string();
    if let Some(r) = rest.strip_prefix("win=[") {
        if let Some((inside, after)) = r.split_once("] ") {
            wid = inside.strip_prefix("id=").and_then(|s| s.split(' ').next()).and_then(|s| s.parse().ok());
            extra = after.to_string();
        } else if let Some(inside) = r.strip_suffix(']') {
            wid = inside.strip_prefix("id=").and_then(|s| s.split(' ').next()).and_then(|s| s.parse().ok());
            extra.clear();
        }
    }
    Some(Ext { ms, tag: tag.to_string(), wid, extra })
}

fn kv<'a>(extra: &'a str, key: &str) -> Option<&'a str> {
    let pat = format!("{}=", key);
    let at = extra.find(&pat)?;
    let rest = &extra[at + pat.len()..];
    Some(rest.split(' ').next().unwrap_or(rest))
}

/// "(WxH)" or "(x,y,WxH)" to the last two numbers.
fn wh(s: &str) -> Option<(i64, i64)> {
    let inner = s.trim_matches(|c| c == '(' || c == ')');
    let last = inner.rsplit(',').next()?;
    let (w, h) = last.split_once('x')?;
    Some((w.parse().ok()?, h.parse().ok()?))
}

#[derive(Default, Clone)]
struct WinInfo {
    class: String,
    pid: u64,
    first_seen: u64,
}

struct XEv {
    t: u64,
    ev: String,
    win: u64,
    w: i64,
    h: i64,
    synthetic: bool,
    parent: u64,
}

#[derive(Default, Clone)]
struct RunRow {
    batch: String,
    run: i64,
    app: String,
    toolkit: String,
    mode: String,
    cond: String,
    size_case: String,
    req: (i64, i64),
    final_wh: Option<(i64, i64)>,
    status: String,
    wid: u64,
    proto: String,
    // extension side, ms since the run's own CLOAK unless noted
    cloak_ms: Option<f64>,
    commit_reason: String,
    ready_outcome: String,
    a_after: usize,
    late_corrected: usize,
    // X side
    x_found: bool,
    v: Option<bool>,
    map_minus_cloak: Option<f64>,
    initial_wh: Option<(i64, i64)>,
    self_resizes_before_set: usize,
    r_count: usize,
    r_first: Option<f64>,
    r_last: Option<f64>,
    d_late: usize,
    d_other: usize,
    d_first: Option<f64>,
    d_in_frame: usize,
    l_cfg: Option<f64>,
    l_d1: Option<f64>,
    l_last: Option<f64>,
    flash_any: bool,
    // rules
    rules: Vec<(String, RuleOut)>,
}

#[derive(Clone, Default)]
struct RuleOut {
    fired: bool,
    capped: bool,
    extra_cloak: Option<f64>,
    needed: bool,
    fixed: bool,
    misfire: bool,
}

fn read_lines(p: &Path) -> Vec<String> {
    fs::read_to_string(p).map(|s| s.lines().map(|l| l.to_string()).collect()).unwrap_or_default()
}

pub fn run(args: &[String]) -> Result<(), String> {
    let mut dirs: Vec<PathBuf> = Vec::new();
    let mut dump: Option<i64> = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--batch" => {
                dirs.push(PathBuf::from(args.get(i + 1).ok_or("--batch DIR")?));
                i += 1;
            }
            "--dump" => {
                dump = args.get(i + 1).and_then(|s| s.parse().ok());
                i += 1;
            }
            other => return Err(format!("analyze: unknown argument {other}")),
        }
        i += 1;
    }
    if dirs.is_empty() {
        return Err("analyze: --batch DIR (repeatable) is required".into());
    }
    let mut all: Vec<RunRow> = Vec::new();
    let mut clock_notes = String::new();
    for d in &dirs {
        let (rows, note) = analyze_batch(d, dump)?;
        let _ = writeln!(clock_notes, "{}", note);
        write_csv(d, &rows)?;
        all.extend(rows);
    }
    let md = summary(&all, &clock_notes);
    for d in &dirs {
        fs::write(d.join("summary.md"), &md).map_err(|e| e.to_string())?;
    }
    println!("{}", md);
    Ok(())
}

fn analyze_batch(dir: &Path, dump: Option<i64>) -> Result<(Vec<RunRow>, String), String> {
    let bname = dir.file_name().and_then(|s| s.to_str()).unwrap_or("batch").to_string();
    // ---- extension log ----
    let ext: Vec<Ext> = read_lines(&dir.join("ext.log")).iter().filter_map(|l| parse_ext(l)).collect();
    let mut by_wid: HashMap<u64, Vec<usize>> = HashMap::new();
    for (i, e) in ext.iter().enumerate() {
        if let Some(w) = e.wid {
            by_wid.entry(w).or_default().push(i);
        }
    }
    // clock origin per window from X11_OBS lines: mono_ms - elapsed (ns, f64)
    let mut origin_w: HashMap<u64, Vec<f64>> = HashMap::new();
    let mut x11_lines: Vec<(usize, f64)> = Vec::new();
    for (i, e) in ext.iter().enumerate() {
        if e.tag.starts_with("X11_OBS_") {
            if let Some(m) = kv(&e.extra, "mono_ms").and_then(|s| s.parse::<f64>().ok()) {
                let o = m * 1e6 - e.ms * 1e6;
                x11_lines.push((i, o));
                if let Some(w) = e.wid {
                    origin_w.entry(w).or_default().push(o);
                }
            }
        }
    }
    let mut origin: HashMap<u64, f64> = HashMap::new();
    let mut spread_max = 0.0f64;
    for (w, v) in &origin_w {
        let mut s = v.clone();
        s.sort_by(|a, b| a.partial_cmp(b).unwrap());
        origin.insert(*w, s[s.len() / 2]);
        spread_max = spread_max.max((s[s.len() - 1] - s[0]) / 1e6);
    }
    let nearest_origin = |idx: usize| -> Option<f64> {
        x11_lines.iter().min_by_key(|(i, _)| (*i as i64 - idx as i64).abs()).map(|(_, o)| *o)
    };
    // ---- observer ----
    let mut xev: Vec<XEv> = Vec::new();
    let mut winfo: HashMap<u64, WinInfo> = HashMap::new();
    let mut files: Vec<PathBuf> = fs::read_dir(dir)
        .map_err(|e| e.to_string())?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.file_name().and_then(|s| s.to_str()).map(|n| n.starts_with("xobs") && n.ends_with(".jsonl")).unwrap_or(false))
        .collect();
    files.sort();
    let mut dropped_damage = 0u64;
    let mut live: HashMap<u64, u64> = HashMap::new();
    let mut next_inst = 0u64;
    for f in &files {
        for l in read_lines(f) {
            let Some(ev) = str_field(&l, "ev") else { continue };
            let t = num_field(&l, "t").unwrap_or(0.0) as u64;
            if ev == "dropped" {
                dropped_damage += num_field(&l, "damage").unwrap_or(0.0) as u64;
                continue;
            }
            // X window ids are reused after a window is destroyed, so every event is attached to a
            // window instance: a CreateNotify starts one, a DestroyNotify ends it.
            let raw_win = num_field(&l, "win").unwrap_or(0.0) as u64;
            if ev == "CreateNotify" || !live.contains_key(&raw_win) {
                next_inst += 1;
                live.insert(raw_win, next_inst);
            }
            let win = live[&raw_win];
            if ev == "DestroyNotify" {
                live.remove(&raw_win);
            }
            if ev == "props" || ev == "snap" || ev == "CreateNotify" {
                let wi = winfo.entry(win).or_insert_with(|| WinInfo { first_seen: t, ..Default::default() });
                if let Some(c) = str_field(&l, "class") {
                    wi.class = c;
                }
                if let Some(p) = num_field(&l, "pid") {
                    wi.pid = p as u64;
                }
            }
            let (w, h) = match ev.as_str() {
                "ConfigureNotify" | "CreateNotify" => (num_field(&l, "w").unwrap_or(-1.0) as i64, num_field(&l, "h").unwrap_or(-1.0) as i64),
                "snap" => (num_field(&l, "w").unwrap_or(-1.0) as i64, num_field(&l, "h").unwrap_or(-1.0) as i64),
                "DamageNotify" => (num_field(&l, "gw").unwrap_or(-1.0) as i64, num_field(&l, "gh").unwrap_or(-1.0) as i64),
                _ => (-1, -1),
            };
            xev.push(XEv {
                t,
                ev,
                win,
                w,
                h,
                synthetic: raw_field(&l, "synthetic") == Some("true"),
                parent: num_field(&l, "parent").unwrap_or(0.0) as u64,
            });
        }
    }
    xev.sort_by_key(|e| e.t);

    // ---- calibration cross-check ----
    let mut note = format!("{}: {} ext lines, {} X11_OBS clock lines, {} windows with an origin, per-window origin spread max {:.3} ms, observer dropped {} damage lines", bname, ext.len(), x11_lines.len(), origin.len(), spread_max, dropped_damage);
    let echoes: Vec<&Ext> = ext.iter().filter(|e| e.tag == "SET_LOGGING").collect();
    let calib: Vec<(String, f64, f64)> = read_lines(&dir.join("calib.jsonl"))
        .iter()
        .filter(|l| str_field(l, "ev").as_deref() == Some("calib"))
        .map(|l| (str_field(l, "phase").unwrap_or_default(), num_field(l, "before").unwrap_or(0.0), num_field(l, "after").unwrap_or(0.0)))
        .collect();
    if !echoes.is_empty() && !calib.is_empty() {
        // Pair the newest echoes with the calibration calls from the end (same number of each).
        let n = echoes.len().min(calib.len());
        let (mut lo, mut hi) = (f64::MIN, f64::MAX);
        for k in 0..n {
            let e = echoes[echoes.len() - 1 - k];
            let c = &calib[calib.len() - 1 - k];
            lo = lo.max(c.1 - e.ms * 1e6 - 5000.0);
            hi = hi.min(c.2 - e.ms * 1e6 + 5000.0);
        }
        let mid_x11 = if origin.is_empty() { None } else { let mut v: Vec<f64> = origin.values().cloned().collect(); v.sort_by(|a, b| a.partial_cmp(b).unwrap()); Some(v[v.len() / 2]) };
        if lo <= hi {
            let _ = write!(note, "; SetLogging brackets ({} pairs, newest-first pairing): origin in an interval {:.2} ms wide", n, (hi - lo) / 1e6);
            if let Some(m) = mid_x11 {
                let _ = write!(note, "; X11_OBS origin {} that interval (offset from its middle {:.2} ms)", if m >= lo && m <= hi { "inside" } else { "OUTSIDE" }, (m - (lo + hi) / 2.0) / 1e6);
            }
        } else {
            let _ = write!(note, "; SetLogging brackets: the {} newest pairs do not overlap ({:.2} ms apart), not used", n, (lo - hi) / 1e6);
        }
    }

    // ---- runs ----
    let mut rows = Vec::new();
    for l in read_lines(&dir.join("runs.jsonl")) {
        let mut r = RunRow { batch: bname.clone(), ..Default::default() };
        r.run = num_field(&l, "run").unwrap_or(-1.0) as i64;
        r.app = str_field(&l, "app").unwrap_or_default();
        r.toolkit = str_field(&l, "toolkit").unwrap_or_default();
        r.mode = str_field(&l, "mode").unwrap_or_default();
        r.cond = str_field(&l, "cond").unwrap_or_default();
        r.size_case = str_field(&l, "size_case").unwrap_or_default();
        r.req = (num_field(&l, "req_w").unwrap_or(0.0) as i64, num_field(&l, "req_h").unwrap_or(0.0) as i64);
        let closed = raw_field(&l, "closed") == Some("true");
        let wid = num_field(&l, "window_id");
        r.status = if wid.is_none() { "no_window".into() } else if !closed { "not_closed".into() } else { "ok".into() };
        if let Some(f) = raw_field(&l, "final") {
            if f != "null" {
                if let (Some(w), Some(h)) = (num_field(f, "w"), num_field(f, "h")) {
                    r.final_wh = Some((w as i64, h as i64));
                }
            }
        }
        let t_start = num_field(&l, "t_start").unwrap_or(0.0);
        let t_end = num_field(&l, "t_end").unwrap_or(0.0);
        let win_pid = num_field(&l, "win_pid").map(|v| v as u64);
        let Some(wid) = wid.map(|v| v as u64) else {
            rows.push(r);
            continue;
        };
        r.wid = wid;
        let idxs = by_wid.get(&wid).cloned().unwrap_or_default();
        let org = origin.get(&wid).cloned().or_else(|| idxs.first().and_then(|i| nearest_origin(*i)));
        let abs = |e: &Ext| org.map(|o| o + e.ms * 1e6);
        let evs: Vec<&Ext> = idxs.iter().map(|i| &ext[*i]).collect();
        let first = |tag: &str| evs.iter().find(|e| e.tag == tag).copied();
        r.proto = String::new();
        let t_cloak = first("CLOAK").and_then(|e| abs(e));
        let t_unc = first("UNCLOAK").and_then(|e| abs(e));
        let t_set = evs.iter().find(|e| e.tag == "X11_OBS_MARK" && e.extra.contains("what=set_size")).and_then(|e| kv(&e.extra, "mono_ms").and_then(|s| s.parse::<f64>().ok())).map(|m| m * 1e6)
            .or_else(|| first("SET_SIZE_REQUEST").and_then(|e| abs(e)));
        if let (Some(c), Some(u)) = (t_cloak, t_unc) {
            r.cloak_ms = Some((u - c) / 1e6);
        }
        r.commit_reason = first("COMMIT_RESOLVED").and_then(|e| kv(&e.extra, "reason").map(|s| s.to_string())).unwrap_or_default();
        r.ready_outcome = evs.iter().rev().find(|e| e.tag == "READY_WAIT_RESOLVED").map(|e| format!("{}:{}", kv(&e.extra, "outcome").unwrap_or("?"), kv(&e.extra, "reason").unwrap_or("?"))).unwrap_or_default();
        if let Some(u) = t_unc {
            r.a_after = evs.iter().filter(|e| e.tag == "ANCHOR_CORRECTION" && abs(e).map(|t| t >= u).unwrap_or(false)).count();
        }
        r.late_corrected = evs.iter().filter(|e| e.tag == "LATE_RESIZE_CORRECTED").count();
        if r.mode != "x11" || t_unc.is_none() {
            if r.mode == "x11" && t_unc.is_none() {
                r.status = format!("{}+no_reveal", r.status);
            }
            rows.push(r);
            continue;
        }
        let t_unc = t_unc.unwrap();

        // client window on the X side
        let mut best: Option<(u64, i64)> = None;
        for (w, wi) in &winfo {
            if wi.class == "mutter-x11-frames" || wi.pid == 0 || Some(wi.pid) != win_pid {
                continue;
            }
            if (wi.first_seen as f64) < t_start - 3e9 || (wi.first_seen as f64) > t_end {
                continue;
            }
            let area: i64 = xev.iter().filter(|e| e.win == *w && e.ev == "ConfigureNotify").map(|e| e.w * e.h).max().unwrap_or(0);
            if best.map(|b| area >= b.1).unwrap_or(true) {
                best = Some((*w, area));
            }
        }
        let Some((cw, _)) = best else {
            rows.push(r);
            continue;
        };
        r.x_found = true;
        let ev_of: Vec<&XEv> = xev.iter().filter(|e| e.win == cw).collect();
        let t_map = ev_of.iter().find(|e| e.ev == "MapNotify").map(|e| e.t as f64);
        // The first cloak is made at WINDOW_CREATED (extension.js _handleWindowCreated); the CLOAK
        // line of the batch comes later, at the map. V compares the X map with the first cloak.
        let t_first_cloak = first("WINDOW_CREATED").and_then(|e| abs(e));
        if let (Some(m), Some(c)) = (t_map, t_first_cloak) {
            r.v = Some(m < c);
            r.map_minus_cloak = Some((m - c) / 1e6);
        }
        // size timeline of the client window
        let mut cur: Option<(i64, i64)> = None;
        let mut changes: Vec<(f64, i64, i64)> = Vec::new(); // (t, w, h) of each size change
        for e in &ev_of {
            if (e.ev == "CreateNotify" || e.ev == "snap") && cur.is_none() && e.w > 0 {
                cur = Some((e.w, e.h));
                r.initial_wh = cur;
            }
            if e.ev == "ConfigureNotify" && e.w > 0 {
                if cur != Some((e.w, e.h)) {
                    changes.push((e.t as f64, e.w, e.h));
                    cur = Some((e.w, e.h));
                }
            }
        }
        let _ = ev_of.iter().any(|e| e.synthetic || e.parent > 0);
        let dmg: Vec<f64> = ev_of.iter().filter(|e| e.ev == "DamageNotify").map(|e| e.t as f64).collect();
        let t_set_f = t_set.unwrap_or(t_unc);
        r.self_resizes_before_set = changes.iter().filter(|c| c.0 < t_set_f).count();
        let late: Vec<&(f64, i64, i64)> = changes.iter().filter(|c| c.0 > t_unc).collect();
        r.r_count = late.len();
        r.r_first = late.first().map(|c| (c.0 - t_unc) / 1e6);
        r.r_last = late.last().map(|c| (c.0 - t_unc) / 1e6);
        let d_in: Vec<&f64> = dmg.iter().filter(|t| **t > t_unc && (**t - t_unc) / 1e6 <= FRAME_MS).collect();
        r.d_in_frame = d_in.len();
        // The paint that answers the last resize is the chain of damage events that starts at the
        // first damage after the last size change (or after the request, if the size never
        // changed) and continues while the gap stays at most BURST_GAP_MS. Late paint (D) is the
        // part of that chain more than one frame after the reveal. Other damage within 1 s of the
        // reveal (cursor blink, unrelated redraws) is counted apart in d_other.
        let anchor_t = changes.last().map(|c| c.0).unwrap_or(t_set_f);
        let after_anchor: Vec<f64> = dmg.iter().cloned().filter(|t| *t >= anchor_t).collect();
        let mut burst: Vec<f64> = Vec::new();
        if let Some(d1) = after_anchor.first() {
            let mut end = *d1;
            for t in &after_anchor {
                if (t - end) / 1e6 <= BURST_GAP_MS {
                    end = *t;
                    burst.push(*t);
                } else {
                    break;
                }
            }
        }
        let d_late_1s: Vec<f64> = burst.iter().cloned().filter(|t| (t - t_unc) / 1e6 > FRAME_MS).collect();
        r.d_late = d_late_1s.len();
        r.d_first = d_late_1s.first().map(|t| (t - t_unc) / 1e6);
        r.d_other = dmg.iter().filter(|t| (**t - t_unc) / 1e6 > FRAME_MS && (**t - t_unc) / 1e6 <= 1000.0 && !burst.contains(t)).count();
        // L: last size change, then the damage burst that follows it
        if let Some(last) = changes.last() {
            r.l_cfg = Some((last.0 - t_unc) / 1e6);
            let after: Vec<f64> = dmg.iter().cloned().filter(|t| *t >= last.0).collect();
            if let Some(d1) = after.first() {
                r.l_d1 = Some((d1 - t_unc) / 1e6);
                let mut end = *d1;
                for t in &after {
                    if (t - end) / 1e6 <= BURST_GAP_MS {
                        end = *t;
                    } else {
                        break;
                    }
                }
                r.l_last = Some((end - t_unc) / 1e6);
            }
        }
        r.flash_any = r.r_count > 0 || r.d_late > 0 || r.a_after > 0;

        // ---- offline rules (never part of the product) ----
        // R1: first damage after the first size change that follows the resize request, plus one frame.
        // R2: end of the first damage burst after that size change (gap > 50 ms), plus one frame.
        let first_cfg_after_set = changes.iter().find(|c| c.0 >= t_set_f);
        let mut late_events: Vec<f64> = late.iter().map(|c| c.0).collect();
        late_events.extend(d_late_1s.iter().cloned());
        let max_late = late_events.iter().cloned().fold(f64::MIN, f64::max);
        let min_late = late_events.iter().cloned().fold(f64::MAX, f64::min);
        for (name, which) in [("R1", 1), ("R2", 2)] {
            let mut out = RuleOut { needed: !late_events.is_empty(), ..Default::default() };
            if let Some(c) = first_cfg_after_set {
                let after: Vec<f64> = dmg.iter().cloned().filter(|t| *t >= c.0).collect();
                let fire = if which == 1 {
                    after.first().map(|t| t + FRAME_MS * 1e6)
                } else {
                    after.first().map(|d1| {
                        let mut end = *d1;
                        for t in &after {
                            if (t - end) / 1e6 <= BURST_GAP_MS { end = *t; } else { break; }
                        }
                        end + FRAME_MS * 1e6
                    })
                };
                if let Some(mut f) = fire {
                    let cap_t = t_set_f + RULE_CAP_MS * 1e6;
                    if f > cap_t {
                        f = cap_t;
                        out.capped = true;
                    }
                    out.fired = true;
                    out.extra_cloak = Some((f - t_unc) / 1e6);
                    if out.needed {
                        out.fixed = f >= max_late;
                        out.misfire = f < max_late;
                        let _ = min_late;
                    }
                }
            }
            r.rules.push((name.into(), out));
        }

        if dump == Some(r.run) {
            eprintln!("--- run {} ({} {} {}) cloak..unc {:?} ms", r.run, r.app, r.mode, r.size_case, r.cloak_ms);
            eprintln!("t_set {:+.2} ms, t_unc 0", (t_set_f - t_unc) / 1e6);
            for c in &changes {
                eprintln!("  cfg {:+.2} ms -> {}x{}", (c.0 - t_unc) / 1e6, c.1, c.2);
            }
            for t in dmg.iter().take(60) {
                eprintln!("  dmg {:+.2} ms", (t - t_unc) / 1e6);
            }
        }
        rows.push(r);
    }
    Ok((rows, note))
}

fn write_csv(dir: &Path, rows: &[RunRow]) -> Result<(), String> {
    let mut s = String::from("batch,run,app,toolkit,mode,cond,size_case,req_w,req_h,final_w,final_h,status,wid,cloak_ms,commit_reason,ready_outcome,A,late_corrected,x_found,V,map_minus_cloak_ms,init_w,init_h,self_resizes_before_set,R,R_first_ms,R_last_ms,D,D_first_ms,D_in_frame,D_other,L_cfg_ms,L_d1_ms,L_last_ms,flash_any,R1_fired,R1_extra_ms,R1_fixed,R1_misfire,R2_fired,R2_extra_ms,R2_fixed,R2_misfire\n");
    for r in rows {
        let f = |o: Option<f64>| o.map(|v| format!("{:.2}", v)).unwrap_or_default();
        let rule = |n: usize| -> String {
            r.rules.get(n).map(|(_, o)| format!("{},{},{},{}", o.fired, f(o.extra_cloak), o.fixed, o.misfire)).unwrap_or_else(|| ",,,".into())
        };
        let _ = writeln!(
            s,
            "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}",
            r.batch, r.run, r.app, r.toolkit.replace(',', ";"), r.mode, r.cond, r.size_case, r.req.0, r.req.1,
            r.final_wh.map(|v| v.0.to_string()).unwrap_or_default(), r.final_wh.map(|v| v.1.to_string()).unwrap_or_default(),
            r.status, r.wid, f(r.cloak_ms), r.commit_reason, r.ready_outcome, r.a_after, r.late_corrected, r.x_found,
            r.v.map(|b| b.to_string()).unwrap_or_default(), f(r.map_minus_cloak),
            r.initial_wh.map(|v| v.0.to_string()).unwrap_or_default(), r.initial_wh.map(|v| v.1.to_string()).unwrap_or_default(),
            r.self_resizes_before_set, r.r_count, f(r.r_first), f(r.r_last), r.d_late, f(r.d_first), r.d_in_frame, r.d_other,
            f(r.l_cfg), f(r.l_d1), f(r.l_last), r.flash_any, rule(0), rule(1)
        );
    }
    fs::write(dir.join("runs.csv"), s).map_err(|e| e.to_string())
}

fn summary(rows: &[RunRow], clock_notes: &str) -> String {
    let mut groups: Vec<((String, String, String, String), Vec<&RunRow>)> = Vec::new();
    for r in rows {
        let k = (r.app.clone(), r.mode.clone(), r.cond.clone(), r.size_case.clone());
        match groups.iter_mut().find(|g| g.0 == k) {
            Some(g) => g.1.push(r),
            None => groups.push((k, vec![r])),
        }
    }
    groups.sort_by(|a, b| a.0.cmp(&b.0));
    let mut s = String::new();
    let _ = writeln!(s, "# xflash analysis\n\nClock: {}\n", clock_notes.trim());
    let _ = writeln!(s, "## X11/XWayland runs\n");
    let _ = writeln!(s, "| app | cond | size | n | valid | V | R | D | A | any | L_last med/max ms | cloak med ms | R1 fixed/needed (never, misfire) | R1 extra med/max ms |");
    let _ = writeln!(s, "| :-- | :-- | :-- | --: | --: | --: | --: | --: | --: | --: | :-- | --: | :-- | :-- |");
    for (k, g) in groups.iter().filter(|g| g.0 .1 == "x11") {
        let valid: Vec<&&RunRow> = g.iter().filter(|r| r.x_found).collect();
        let cnt = |f: &dyn Fn(&RunRow) -> bool| valid.iter().filter(|r| f(r)).count();
        let l: Vec<f64> = valid.iter().filter_map(|r| r.l_last).collect();
        let cloak: Vec<f64> = g.iter().filter_map(|r| r.cloak_ms).collect();
        let r1: Vec<&RuleOut> = valid.iter().filter_map(|r| r.rules.first().map(|x| &x.1)).collect();
        let needed = r1.iter().filter(|o| o.needed).count();
        let fixed = r1.iter().filter(|o| o.needed && o.fixed).count();
        let never = r1.iter().filter(|o| !o.fired).count();
        let mis = r1.iter().filter(|o| o.misfire).count();
        let extra: Vec<f64> = r1.iter().filter_map(|o| o.extra_cloak).collect();
        let _ = writeln!(
            s,
            "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} / {} | {} | {}/{} ({}, {}) | {} / {} |",
            k.0, k.2, k.3, g.len(), valid.len(),
            cnt(&|r| r.v == Some(true)), cnt(&|r| r.r_count > 0), cnt(&|r| r.d_late > 0), cnt(&|r| r.a_after > 0), cnt(&|r| r.flash_any),
            fmt_opt(median(&l)), fmt_opt(l.iter().cloned().fold(None, |a: Option<f64>, v| Some(a.map_or(v, |x| x.max(v))))),
            fmt_opt(median(&cloak)), fixed, needed, never, mis,
            fmt_opt(median(&extra)), fmt_opt(extra.iter().cloned().fold(None, |a: Option<f64>, v| Some(a.map_or(v, |x| x.max(v)))))
        );
    }
    let _ = writeln!(s, "\n## Native Wayland control runs (extension log only)\n");
    let _ = writeln!(s, "| app | cond | size | n | A>0 | cloak med / max ms | ready-wait outcomes | final == requested |");
    let _ = writeln!(s, "| :-- | :-- | :-- | --: | --: | :-- | :-- | --: |");
    for (k, g) in groups.iter().filter(|g| g.0 .1 == "wayland") {
        let cloak: Vec<f64> = g.iter().filter_map(|r| r.cloak_ms).collect();
        let mut outc: HashMap<String, usize> = HashMap::new();
        for r in g.iter() {
            *outc.entry(r.ready_outcome.clone()).or_default() += 1;
        }
        let mut o: Vec<String> = outc.into_iter().map(|(k, v)| format!("{}x {}", v, k)).collect();
        o.sort();
        let exact = g.iter().filter(|r| r.final_wh == Some(r.req)).count();
        let _ = writeln!(
            s,
            "| {} | {} | {} | {} | {} | {} / {} | {} | {} |",
            k.0, k.2, k.3, g.len(), g.iter().filter(|r| r.a_after > 0).count(), fmt_opt(median(&cloak)),
            fmt_opt(cloak.iter().cloned().fold(None, |a: Option<f64>, v| Some(a.map_or(v, |x| x.max(v))))),
            o.join("; "), exact
        );
    }
    s
}
