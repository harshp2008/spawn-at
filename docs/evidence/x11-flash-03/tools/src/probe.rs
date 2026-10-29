//! `xflash probe-analyze`: turns the output of the compositor-side probe (tools/probe/xflash-probe@local:
//! `probe.jsonl` plus `px/*.png`) into bad frames per run.
//!
//! A *frame* is one stage `after-paint`. A frame is *visible* when the window actor and all its
//! ancestors are shown and its paint opacity is above 0. A visible frame is *good* when it shows the
//! window's final size with real content; every visible frame before the first good one is *bad*.
//! Two readings are computed side by side:
//!
//! - texture: good = frame rect size and surface-texture size both equal the final ones (the last
//!   visible frame of the window). This is the weaker proxy: it says the compositor holds a buffer
//!   of the final size, not what the pixels are.
//! - pixels: good = the PNG captured for that frame (Shell.Screenshot, same main-loop turn) has the
//!   final frame size, the frame's surface texture has the final size, and the image is not blank
//!   (at least half transparent, or mostly black). The reference is the final size, not the last
//!   image, because content such as a shell prompt legitimately arrives after the reveal. A frame
//!   failing the size test is `old_size`, one failing the blank test `blank`. A visible frame
//!   with no capture (shot cap) falls back to the texture reading and is counted `noshot`.
//!
//! The headless Shell has no vblank, so painted frames are not 60 Hz: `bad_ms` (time the bad state
//! was the latest painted state) and `p60 = min(1, bad_ms / 16.667)` (chance that a 60 Hz refresh at
//! a random phase lands inside it) are the display-independent readings.

use crate::util::median;
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

const FRAME_MS: f64 = 1000.0 / 60.0;

#[derive(Clone, Debug)]
struct Frame {
    t: f64,
    n: u64,
    vis: bool,
    fr: Option<[i64; 4]>,
    bf: Option<[i64; 4]>,
    tex: Option<[i64; 2]>,
}

/// One `e` line: a window or actor signal with the state at that moment.
#[derive(Clone, Debug)]
struct Ev {
    t: f64,
    name: String,
    op: Option<i64>,
    tex: Option<[i64; 2]>,
}

#[derive(Clone, Debug)]
struct Shot {
    n: u64,
    file: String,
    ok: bool,
}

#[derive(Default, Debug)]
struct Win {
    cls: String,
    created: f64,
    frames: Vec<Frame>,
    events: Vec<Ev>,
    shots: Vec<Shot>,
    first_frame_ev: Option<f64>,
    end_reason: String,
}

struct Img {
    w: usize,
    h: usize,
    rgba: Vec<u8>,
}

fn arr4(v: &Value) -> Option<[i64; 4]> {
    let a = v.as_array()?;
    if a.len() != 4 {
        return None;
    }
    Some([a[0].as_i64()?, a[1].as_i64()?, a[2].as_i64()?, a[3].as_i64()?])
}

fn arr2(v: &Value) -> Option<[i64; 2]> {
    let a = v.as_array()?;
    if a.len() != 2 {
        return None;
    }
    Some([a[0].as_i64()?, a[1].as_i64()?])
}

fn read_probe(path: &Path) -> Result<BTreeMap<u64, Win>, String> {
    let text = fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut wins: BTreeMap<u64, Win> = BTreeMap::new();
    for line in text.lines() {
        let Ok(v) = serde_json::from_str::<Value>(line) else { continue };
        let Some(k) = v["k"].as_str() else { continue };
        let Some(w) = v["w"].as_u64() else { continue };
        let win = wins.entry(w).or_default();
        match k {
            "win" => {
                win.cls = v["cls"].as_str().unwrap_or("").to_string();
                win.created = v["t"].as_f64().unwrap_or(0.0);
            }
            "f" => win.frames.push(Frame {
                t: v["t"].as_f64().unwrap_or(0.0),
                n: v["n"].as_u64().unwrap_or(0),
                vis: v["vis"].as_bool().unwrap_or(false),
                fr: arr4(&v["fr"]),
                bf: arr4(&v["bf"]),
                tex: arr2(&v["tex"]),
            }),
            "px" => win.shots.push(Shot {
                n: v["n"].as_u64().unwrap_or(0),
                file: v["file"].as_str().unwrap_or("").to_string(),
                ok: v.get("err").is_none() && v.get("bytes").is_some(),
            }),
            "e" => {
                let name = v["ev"].as_str().unwrap_or("").to_string();
                if name == "first-frame" {
                    win.first_frame_ev = v["t"].as_f64();
                }
                win.events.push(Ev {
                    t: v["t"].as_f64().unwrap_or(0.0),
                    name,
                    op: v["op"].as_i64(),
                    tex: arr2(&v["tex"]),
                });
            }
            "end" => win.end_reason = v["reason"].as_str().unwrap_or("").to_string(),
            _ => {}
        }
    }
    for w in wins.values_mut() {
        w.frames.sort_by(|a, b| a.t.partial_cmp(&b.t).unwrap());
    }
    Ok(wins)
}

fn load_png(path: &Path) -> Option<Img> {
    let file = fs::File::open(path).ok()?;
    let mut dec = png::Decoder::new(file);
    dec.set_transformations(png::Transformations::EXPAND);
    let mut reader = dec.read_info().ok()?;
    let mut buf = vec![0u8; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).ok()?;
    let (w, h) = (info.width as usize, info.height as usize);
    let rgba = match info.color_type {
        png::ColorType::Rgba => buf[..w * h * 4].to_vec(),
        png::ColorType::Rgb => buf[..w * h * 3]
            .chunks(3)
            .flat_map(|p| [p[0], p[1], p[2], 255])
            .collect(),
        _ => return None,
    };
    Some(Img { w, h, rgba })
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Class {
    Real,
    OldSize,
    Blank,
    NoShot,
}

/// True when the image's most common colour (every 3rd pixel, low 3 bits dropped) is black.
fn is_black(a: &Img) -> bool {
    let mut counts: HashMap<[u8; 3], u64> = HashMap::new();
    let mut y = 0;
    while y < a.h {
        let mut x = 0;
        while x < a.w {
            let i = (y * a.w + x) * 4;
            *counts.entry([a.rgba[i] & 0xF8, a.rgba[i + 1] & 0xF8, a.rgba[i + 2] & 0xF8]).or_insert(0) += 1;
            x += 3;
        }
        y += 3;
    }
    counts.into_iter().max_by_key(|(_, n)| *n).map(|(c, _)| c == [0, 0, 0]).unwrap_or(false)
}

/// (dominant colour fraction, transparent fraction), sampled every 3rd pixel in both directions.
fn flatness(a: &Img) -> (f64, f64) {
    let mut counts: HashMap<[u8; 4], u64> = HashMap::new();
    let (mut n, mut transp) = (0u64, 0u64);
    let mut y = 0;
    while y < a.h {
        let mut x = 0;
        while x < a.w {
            let i = (y * a.w + x) * 4;
            let px = [a.rgba[i] & 0xF8, a.rgba[i + 1] & 0xF8, a.rgba[i + 2] & 0xF8, a.rgba[i + 3] & 0xF8];
            *counts.entry(px).or_insert(0) += 1;
            if a.rgba[i + 3] < 250 {
                transp += 1;
            }
            n += 1;
            x += 3;
        }
        y += 3;
    }
    let dom = counts.values().copied().max().unwrap_or(0);
    if n == 0 { (0.0, 0.0) } else { (dom as f64 / n as f64, transp as f64 / n as f64) }
}

#[derive(Default, Clone, Debug)]
pub struct Row {
    pub wid: u64,
    pub label: String,
    pub vis_frames: usize,
    pub bad_tex: usize,
    pub bad_ms_tex: f64,
    pub first_good_tex_ms: Option<f64>,
    pub shots: usize,
    pub bad_px: usize,
    pub hard_px: usize,
    pub old_size: usize,
    pub blank: usize,
    pub bad_ms_px: f64,
    pub first_good_px_ms: Option<f64>,
    pub regress_px: usize,
    pub first_vis_rect: String,
    pub first_vis_tex: String,
    pub final_rect: String,
    pub final_tex: String,
    pub frame_gap_med_ms: Option<f64>,
    /// Reveal = the first `actor-opacity` event that sets opacity 255 (the extension's uncloak).
    pub reveal_to_first_frame_ms: Option<f64>,
    /// Time from the reveal to the first moment the surface texture had its final size (0 if it
    /// already had it).
    pub reveal_to_tex_final_ms: Option<f64>,
    /// `win-size-changed` / `win-position-changed` signals after the reveal.
    pub size_changed_after: usize,
    pub pos_changed_after: usize,
    /// At the first visible frame: texture size equals the buffer rect size ("a texture gate would
    /// let the reveal through").
    pub gate_open_first: Option<bool>,
    pub note: String,
}

fn sz(r: &[i64; 4]) -> [i64; 2] {
    [r[2], r[3]]
}

fn analyze_window(wid: u64, win: &Win, dir: &Path) -> Row {
    let mut row = Row { wid, ..Default::default() };
    let vis: Vec<&Frame> = win.frames.iter().filter(|f| f.vis).collect();
    row.vis_frames = vis.len();
    if vis.is_empty() {
        row.note = "never visible".into();
        return row;
    }
    let gaps: Vec<f64> = win.frames.windows(2).map(|p| p[1].t - p[0].t).collect();
    row.frame_gap_med_ms = median(&gaps);
    let fin = *vis.last().unwrap();
    let (Some(fin_fr), fin_tex) = (fin.fr, fin.tex) else {
        row.note = "no final rect".into();
        return row;
    };
    let t0 = vis[0].t;
    row.first_vis_rect = vis[0].fr.map(|r| format!("{}x{}", r[2], r[3])).unwrap_or_default();
    row.first_vis_tex = vis[0].tex.map(|t| format!("{}x{}", t[0], t[1])).unwrap_or_else(|| "none".into());
    row.final_rect = format!("{}x{}", fin_fr[2], fin_fr[3]);
    row.final_tex = fin_tex.map(|t| format!("{}x{}", t[0], t[1])).unwrap_or_else(|| "none".into());

    // timing around the reveal
    let reveal = win
        .events
        .iter()
        .filter(|e| e.name == "actor-opacity" && e.op == Some(255))
        .map(|e| e.t)
        .fold(f64::NAN, f64::min);
    if reveal.is_finite() {
        row.reveal_to_first_frame_ms = Some(t0 - reveal);
        let tex_final = win
            .events
            .iter()
            .filter(|e| e.t >= reveal && e.tex == fin_tex)
            .map(|e| e.t)
            .chain(win.frames.iter().filter(|f| f.t >= reveal && f.tex == fin_tex).map(|f| f.t))
            .fold(f64::NAN, f64::min);
        if tex_final.is_finite() && fin_tex.is_some() {
            row.reveal_to_tex_final_ms = Some(tex_final - reveal);
        }
        row.size_changed_after = win.events.iter().filter(|e| e.name == "win-size-changed" && e.t > reveal).count();
        row.pos_changed_after = win.events.iter().filter(|e| e.name == "win-position-changed" && e.t > reveal).count();
    }
    row.gate_open_first = match (vis[0].bf, vis[0].tex) {
        (Some(b), Some(t)) => Some(b[2] == t[0] && b[3] == t[1]),
        _ => None,
    };

    // texture reading
    let good_tex =|f: &Frame| f.fr.map(|r| sz(&r)) == Some(sz(&fin_fr)) && f.tex == fin_tex && fin_tex.is_some();
    let ftex = vis.iter().position(|f| good_tex(f));
    match ftex {
        Some(i) => {
            row.bad_tex = i;
            row.bad_ms_tex = vis[i].t - t0;
            row.first_good_tex_ms = Some(vis[i].t - t0);
        }
        None => {
            row.bad_tex = vis.len();
            row.bad_ms_tex = vis.last().unwrap().t - t0;
            row.note = "no good frame (texture)".into();
        }
    }

    // pixel reading
    let shot_for: HashMap<u64, &Shot> = win.shots.iter().filter(|s| s.ok).map(|s| (s.n, s)).collect();
    if shot_for.is_empty() {
        if row.note.is_empty() {
            row.note = "no pixels".into();
        }
        return row;
    }
    // The reference is the final size (frame rect of the last visible frame), not the last image:
    // content such as a shell prompt legitimately arrives long after the reveal.
    let mut classes: Vec<(f64, Class)> = Vec::new();
    for f in &vis {
        let class = match shot_for.get(&f.n).and_then(|s| load_png(&dir.join(&s.file))) {
            // No capture for this frame (shot cap): the texture reading decides.
            None => {
                if good_tex(f) { Class::NoShot } else { Class::OldSize }
            }
            Some(img) => {
                let (dom, transp) = flatness(&img);
                if transp >= 0.5 || (dom >= 0.98 && is_black(&img)) {
                    Class::Blank
                } else if img.w as i64 != fin_fr[2] || img.h as i64 != fin_fr[3] || f.tex != fin_tex {
                    Class::OldSize
                } else {
                    Class::Real
                }
            }
        };
        classes.push((f.t, class));
    }
    row.shots = classes.iter().filter(|(_, c)| *c != Class::NoShot).count();
    let first_good = classes.iter().position(|(_, c)| matches!(c, Class::Real | Class::NoShot));
    // Every visible frame before the first good one is bad. With no good frame in the observed
    // window (the probe stops 300 ms after the first visible frame) all of them are bad and the
    // duration is only a lower bound: the note says `censored`.
    let end = first_good.unwrap_or(classes.len());
    let mut ms = 0.0;
    for j in 0..end {
        row.bad_px += 1;
        match classes[j].1 {
            Class::OldSize => {
                row.old_size += 1;
                row.hard_px += 1;
            }
            Class::Blank => {
                row.blank += 1;
                row.hard_px += 1;
            }
            _ => {}
        }
        if j + 1 < classes.len() {
            ms += classes[j + 1].0 - classes[j].0;
        }
    }
    row.bad_ms_px = ms;
    match first_good {
        Some(i) => {
            row.first_good_px_ms = Some(classes[i].0 - t0);
            row.regress_px = classes[i..]
                .iter()
                .filter(|(_, c)| matches!(c, Class::OldSize | Class::Blank))
                .count();
        }
        None => row.note = "no good frame (pixels), bad_ms is a lower bound: censored".into(),
    }
    row
}

fn label_of(rec: &Value) -> String {
    format!(
        "{}|{}|{}|{}",
        rec["app"].as_str().unwrap_or("?"),
        rec["mode"].as_str().unwrap_or("?"),
        rec["cond"].as_str().unwrap_or("?"),
        rec["size_case"].as_str().unwrap_or("?")
    )
}

fn read_runs_csv(path: &Path) -> HashMap<u64, HashMap<String, String>> {
    let mut out = HashMap::new();
    let Ok(text) = fs::read_to_string(path) else { return out };
    let mut lines = text.lines();
    let Some(head) = lines.next() else { return out };
    let cols: Vec<&str> = head.split(',').collect();
    for l in lines {
        let vals: Vec<&str> = l.split(',').collect();
        let mut m = HashMap::new();
        for (c, v) in cols.iter().zip(vals.iter()) {
            m.insert((*c).to_string(), (*v).to_string());
        }
        if let Some(w) = m.get("wid").and_then(|w| w.parse::<u64>().ok()) {
            out.insert(w, m);
        }
    }
    out
}

fn f1(v: Option<f64>) -> String {
    v.map(|x| format!("{x:.1}")).unwrap_or_else(|| "-".into())
}

pub fn run(args: &[String]) -> Result<(), String> {
    let mut probe_dir = PathBuf::new();
    let mut runs: Option<PathBuf> = None;
    let mut runs_csv: Option<PathBuf> = None;
    let mut out = PathBuf::new();
    let mut i = 0;
    while i < args.len() {
        let v = args.get(i + 1).ok_or_else(|| format!("{} needs a value", args[i]))?;
        match args[i].as_str() {
            "--probe-dir" => probe_dir = PathBuf::from(v),
            "--runs" => runs = Some(PathBuf::from(v)),
            "--runs-csv" => runs_csv = Some(PathBuf::from(v)),
            "--out" => out = PathBuf::from(v),
            o => return Err(format!("probe-analyze: unknown argument {o}")),
        }
        i += 2;
    }
    if probe_dir.as_os_str().is_empty() || out.as_os_str().is_empty() {
        return Err("probe-analyze: --probe-dir DIR and --out DIR are required".into());
    }
    let wins = read_probe(&probe_dir.join("probe.jsonl"))?;
    let csv = runs_csv.as_deref().map(read_runs_csv).unwrap_or_default();

    // Which windows, with which label: every run record's window_id; else every window seen.
    let mut targets: Vec<(u64, String)> = Vec::new();
    if let Some(r) = &runs {
        let text = fs::read_to_string(r).map_err(|e| format!("{}: {e}", r.display()))?;
        for line in text.lines() {
            let Ok(v) = serde_json::from_str::<Value>(line) else { continue };
            let Some(w) = v["window_id"].as_u64() else { continue };
            targets.push((w, label_of(&v)));
        }
    } else {
        for w in wins.keys() {
            targets.push((*w, "all".into()));
        }
    }

    let mut rows: Vec<Row> = Vec::new();
    for (wid, label) in &targets {
        let mut row = match wins.get(wid) {
            Some(w) => analyze_window(*wid, w, &probe_dir),
            None => Row { wid: *wid, note: "no probe record".into(), ..Default::default() },
        };
        row.label = label.clone();
        rows.push(row);
    }

    fs::create_dir_all(&out).map_err(|e| e.to_string())?;
    // per-run csv
    let mut c = String::from(
        "label,wid,vis_frames,frame_gap_med_ms,first_vis_rect,first_vis_tex,final_rect,final_tex,bad_tex,bad_ms_tex,\
shots,bad_px,hard_px,old_size,blank,bad_ms_px,first_good_px_ms,regress_px,R,D,A,L_last_ms,R_first_ms,\
reveal_to_first_frame_ms,reveal_to_tex_final_ms,size_changed_after,pos_changed_after,gate_open_first,note\n",
    );
    for r in &rows {
        let m = csv.get(&r.wid);
        let g = |k: &str| m.and_then(|m| m.get(k)).cloned().unwrap_or_default();
        let _ = writeln!(
            c,
            "{},{},{},{},{},{},{},{},{},{:.2},{},{},{},{},{},{:.2},{},{},{},{},{},{},{},{},{},{},{},{},{}",
            r.label, r.wid, r.vis_frames, f1(r.frame_gap_med_ms), r.first_vis_rect, r.first_vis_tex,
            r.final_rect, r.final_tex, r.bad_tex, r.bad_ms_tex, r.shots, r.bad_px, r.hard_px, r.old_size,
            r.blank, r.bad_ms_px, f1(r.first_good_px_ms), r.regress_px,
            g("R"), g("D"), g("A"), g("L_last_ms"), g("R_first_ms"),
            f1(r.reveal_to_first_frame_ms), f1(r.reveal_to_tex_final_ms), r.size_changed_after,
            r.pos_changed_after,
            r.gate_open_first.map(|b| b.to_string()).unwrap_or_default(), r.note
        );
    }
    fs::write(out.join("probe-runs.csv"), c).map_err(|e| e.to_string())?;

    // summary per label
    let mut groups: BTreeMap<String, Vec<&Row>> = BTreeMap::new();
    for r in &rows {
        groups.entry(r.label.clone()).or_default().push(r);
    }
    let mut s = String::new();
    let _ = writeln!(s, "| cell (app/mode/cond/size) | n | seen | runs bad tex | runs bad px | runs hard px | bad frames px med/max | bad_ms px med/max | p60 med | first good px ms med/max | R | D | A>0 | L med ms |");
    let _ = writeln!(s, "| :-- | --: | --: | --: | --: | --: | :-- | :-- | --: | :-- | --: | --: | --: | --: |");
    for (label, rs) in &groups {
        let seen: Vec<&&Row> = rs.iter().filter(|r| r.vis_frames > 0).collect();
        let bad_tex = seen.iter().filter(|r| r.bad_tex > 0).count();
        let bad_px = seen.iter().filter(|r| r.bad_px > 0).count();
        let hard = seen.iter().filter(|r| r.hard_px > 0).count();
        let bf: Vec<f64> = seen.iter().map(|r| r.bad_px as f64).collect();
        let bm: Vec<f64> = seen.iter().map(|r| r.bad_ms_px).collect();
        let p60: Vec<f64> = bm.iter().map(|m| (m / FRAME_MS).min(1.0)).collect();
        let fg: Vec<f64> = seen.iter().filter_map(|r| r.first_good_px_ms).collect();
        let mx = |v: &[f64]| v.iter().cloned().fold(f64::NAN, f64::max);
        let cnt = |k: &str, pos: bool| {
            seen.iter()
                .filter(|r| {
                    csv.get(&r.wid)
                        .and_then(|m| m.get(k))
                        .and_then(|v| v.parse::<f64>().ok())
                        .map(|v| v > 0.0 || !pos)
                        .unwrap_or(false)
                })
                .count()
        };
        let lv: Vec<f64> = seen
            .iter()
            .filter_map(|r| csv.get(&r.wid).and_then(|m| m.get("L_last_ms")).and_then(|v| v.parse::<f64>().ok()))
            .collect();
        let _ = writeln!(
            s,
            "| {} | {} | {} | {} | {} | {} | {} / {} | {} / {} | {} | {} / {} | {} | {} | {} | {} |",
            label.replace('|', " / "), rs.len(), seen.len(), bad_tex, bad_px, hard,
            f1(median(&bf)), if bf.is_empty() { "-".into() } else { format!("{:.0}", mx(&bf)) },
            f1(median(&bm)), if bm.is_empty() { "-".into() } else { format!("{:.1}", mx(&bm)) },
            f1(median(&p60).map(|p| p * 100.0)) + "%",
            f1(median(&fg)), if fg.is_empty() { "-".into() } else { format!("{:.1}", mx(&fg)) },
            cnt("R", true), cnt("D", true), cnt("A", true), f1(median(&lv)),
        );
    }
    fs::write(out.join("probe-summary.md"), &s).map_err(|e| e.to_string())?;
    print!("{s}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_png(path: &Path, w: u32, h: u32, px: impl Fn(u32, u32) -> [u8; 4]) {
        let file = fs::File::create(path).unwrap();
        let mut enc = png::Encoder::new(file, w, h);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        let mut wr = enc.write_header().unwrap();
        let mut data = Vec::new();
        for y in 0..h {
            for x in 0..w {
                data.extend_from_slice(&px(x, y));
            }
        }
        wr.write_image_data(&data).unwrap();
    }

    fn tmpdir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("xflash-probe-test-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(d.join("px")).unwrap();
        d
    }

    #[test]
    fn black_frame_before_a_good_frame_is_one_blank_bad_frame() {
        let d = tmpdir("black");
        write_png(&d.join("px/1-1.png"), 40, 30, |_, _| [0, 0, 0, 255]);
        write_png(&d.join("px/1-2.png"), 40, 30, |x, _| if x < 20 { [250, 250, 250, 255] } else { [30, 90, 200, 255] });
        let log = [
            r#"{"k":"win","w":1,"t":0,"cls":"X"}"#,
            r#"{"k":"f","w":1,"t":10.0,"n":1,"fr":[0,0,40,30],"tex":[50,40],"vis":true}"#,
            r#"{"k":"px","w":1,"n":1,"file":"px/1-1.png","bytes":10}"#,
            r#"{"k":"f","w":1,"t":30.0,"n":2,"fr":[0,0,40,30],"tex":[40,30],"vis":true}"#,
            r#"{"k":"px","w":1,"n":2,"file":"px/1-2.png","bytes":10}"#,
        ]
        .join("\n");
        fs::write(d.join("probe.jsonl"), log).unwrap();
        let wins = read_probe(&d.join("probe.jsonl")).unwrap();
        let row = analyze_window(1, &wins[&1], &d);
        assert_eq!((row.vis_frames, row.bad_tex, row.bad_px, row.blank, row.old_size), (2, 1, 1, 1, 0));
        assert!((row.bad_ms_px - 20.0).abs() < 1e-9);
        assert_eq!(row.first_good_px_ms, Some(20.0));
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn right_size_first_frame_is_zero_bad_frames_even_if_content_changes_later() {
        let d = tmpdir("good");
        write_png(&d.join("px/2-1.png"), 40, 30, |_, y| if y < 5 { [200, 200, 200, 255] } else { [255, 255, 255, 255] });
        write_png(&d.join("px/2-2.png"), 40, 30, |_, y| if y < 15 { [200, 200, 200, 255] } else { [20, 20, 20, 255] });
        let log = [
            r#"{"k":"f","w":2,"t":5.0,"n":1,"fr":[0,0,40,30],"tex":[40,30],"vis":true}"#,
            r#"{"k":"px","w":2,"n":1,"file":"px/2-1.png","bytes":10}"#,
            r#"{"k":"f","w":2,"t":105.0,"n":2,"fr":[0,0,40,30],"tex":[40,30],"vis":true}"#,
            r#"{"k":"px","w":2,"n":2,"file":"px/2-2.png","bytes":10}"#,
        ]
        .join("\n");
        fs::write(d.join("probe.jsonl"), log).unwrap();
        let wins = read_probe(&d.join("probe.jsonl")).unwrap();
        let row = analyze_window(2, &wins[&2], &d);
        assert_eq!((row.bad_tex, row.bad_px), (0, 0));
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn old_size_texture_is_bad_and_cloaked_frames_are_not_counted() {
        let d = tmpdir("old");
        write_png(&d.join("px/3-1.png"), 40, 30, |x, y| [(x * 5) as u8, (y * 7) as u8, 90, 255]);
        write_png(&d.join("px/3-2.png"), 40, 30, |x, y| [(x * 5) as u8, (y * 7) as u8, 90, 255]);
        let log = [
            r#"{"k":"f","w":3,"t":1.0,"n":1,"fr":[0,0,40,30],"tex":[10,10],"vis":false}"#,
            r#"{"k":"f","w":3,"t":2.0,"n":2,"fr":[0,0,40,30],"tex":[10,10],"vis":true}"#,
            r#"{"k":"px","w":3,"n":2,"file":"px/3-1.png","bytes":10}"#,
            r#"{"k":"f","w":3,"t":18.0,"n":3,"fr":[0,0,40,30],"tex":[40,30],"vis":true}"#,
            r#"{"k":"px","w":3,"n":3,"file":"px/3-2.png","bytes":10}"#,
        ]
        .join("\n");
        fs::write(d.join("probe.jsonl"), log).unwrap();
        let wins = read_probe(&d.join("probe.jsonl")).unwrap();
        let row = analyze_window(3, &wins[&3], &d);
        assert_eq!((row.vis_frames, row.bad_tex, row.bad_px, row.old_size), (2, 1, 1, 1));
        assert!((row.bad_ms_px - 16.0).abs() < 1e-9);
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn timing_around_the_reveal_and_the_texture_gate() {
        let d = tmpdir("timing");
        let log = [
            r#"{"k":"e","w":5,"t":100.0,"ev":"actor-opacity","op":255,"tex":[10,10]}"#,
            r#"{"k":"f","w":5,"t":103.0,"n":1,"fr":[0,0,40,30],"bf":[0,0,50,40],"tex":[10,10],"vis":true}"#,
            r#"{"k":"e","w":5,"t":111.0,"ev":"damaged","op":255,"tex":[50,40]}"#,
            r#"{"k":"f","w":5,"t":112.0,"n":2,"fr":[0,0,40,30],"bf":[0,0,50,40],"tex":[50,40],"vis":true}"#,
            r#"{"k":"e","w":5,"t":150.0,"ev":"win-size-changed","op":255,"tex":[50,40]}"#,
            r#"{"k":"e","w":5,"t":151.0,"ev":"win-position-changed","op":255,"tex":[50,40]}"#,
        ]
        .join("\n");
        fs::write(d.join("probe.jsonl"), log).unwrap();
        let wins = read_probe(&d.join("probe.jsonl")).unwrap();
        let row = analyze_window(5, &wins[&5], &d);
        assert_eq!(row.reveal_to_first_frame_ms, Some(3.0));
        assert_eq!(row.reveal_to_tex_final_ms, Some(11.0));
        assert_eq!((row.size_changed_after, row.pos_changed_after), (1, 1));
        assert_eq!(row.gate_open_first, Some(false));
        assert_eq!(row.bad_tex, 1);
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn a_window_that_stays_black_is_all_bad_and_censored() {
        let d = tmpdir("censored");
        for n in 1..=3 {
            write_png(&d.join(format!("px/4-{n}.png")), 40, 30, |_, _| [0, 0, 0, 255]);
        }
        let log = [
            r#"{"k":"f","w":4,"t":10.0,"n":1,"fr":[0,0,40,30],"tex":[40,30],"vis":true}"#,
            r#"{"k":"px","w":4,"n":1,"file":"px/4-1.png","bytes":10}"#,
            r#"{"k":"f","w":4,"t":40.0,"n":2,"fr":[0,0,40,30],"tex":[40,30],"vis":true}"#,
            r#"{"k":"px","w":4,"n":2,"file":"px/4-2.png","bytes":10}"#,
            r#"{"k":"f","w":4,"t":90.0,"n":3,"fr":[0,0,40,30],"tex":[40,30],"vis":true}"#,
            r#"{"k":"px","w":4,"n":3,"file":"px/4-3.png","bytes":10}"#,
        ]
        .join("\n");
        fs::write(d.join("probe.jsonl"), log).unwrap();
        let wins = read_probe(&d.join("probe.jsonl")).unwrap();
        let row = analyze_window(4, &wins[&4], &d);
        assert_eq!((row.bad_tex, row.bad_px, row.blank, row.hard_px), (0, 3, 3, 3));
        assert!((row.bad_ms_px - 80.0).abs() < 1e-9);
        assert!(row.first_good_px_ms.is_none() && row.note.contains("censored"));
        let _ = fs::remove_dir_all(&d);
    }
}
