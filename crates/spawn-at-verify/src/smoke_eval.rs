//! Helper CLI utilities for smoke-live.sh JSON evaluation and placement verification.

use clap::Subcommand;
use serde_json::Value;
use std::collections::HashSet;
use std::io::{self, Read};

/// Internal subcommand for evaluating window placement in live smoke tests without python.
#[derive(Subcommand, Debug)]
pub enum SmokeEvalCommand {
    /// Extracts display workarea coordinates (x y w h) from layout JSON on standard input.
    Workarea,
    /// Extracts space-separated active window IDs from windows JSON on standard input.
    WindowIds,
    /// Finds newly mapped window matching hint or app name, outputting id|pid|x|y|w|h.
    FindWindow {
        /// Space-separated window IDs existing before spawn action.
        #[arg(long, default_value = "")]
        before_ids: String,
        /// Case-insensitive window class or title substring to match.
        #[arg(long, default_value = "")]
        class_hint: String,
        /// Application name hint to match.
        #[arg(long, default_value = "")]
        app_name: String,
    },
    /// Verifies window placement geometry against target anchor and margins on workarea.
    VerifyGeom {
        /// Observed window X position.
        #[arg(long)]
        win_x: f64,
        /// Observed window Y position.
        #[arg(long)]
        win_y: f64,
        /// Observed window width.
        #[arg(long)]
        win_w: f64,
        /// Observed window height.
        #[arg(long)]
        win_h: f64,
        /// Target display workarea X origin.
        #[arg(long)]
        wa_x: f64,
        /// Target display workarea Y origin.
        #[arg(long)]
        wa_y: f64,
        /// Target display workarea width.
        #[arg(long)]
        wa_w: f64,
        /// Target display workarea height.
        #[arg(long)]
        wa_h: f64,
        /// Target anchor specification (e.g. "center", "top-right", "bottom-right").
        #[arg(long)]
        anchor: String,
        /// Target outer margin in pixels.
        #[arg(long, default_value = "0")]
        margin: f64,
    },
}

/// Executes the requested smoke evaluation subcommand.
pub fn execute(cmd: SmokeEvalCommand) {
    match cmd {
        SmokeEvalCommand::Workarea => {
            let mut input = String::new();
            io::stdin()
                .read_to_string(&mut input)
                .expect("failed to read stdin");
            let data: Value = serde_json::from_str(&input).expect("failed to parse layout json");
            let wa = if let Some(monitors) = data.get("monitors").and_then(|m| m.as_array()) {
                monitors
                    .first()
                    .and_then(|m| m.get("workarea"))
                    .unwrap_or(&data)
            } else if let Some(first) = data.as_array().and_then(|arr| arr.first()) {
                first
            } else {
                &data
            };
            let x = wa.get("x").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let y = wa.get("y").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let w = wa
                .get("w")
                .or_else(|| wa.get("width"))
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);
            let h = wa
                .get("h")
                .or_else(|| wa.get("height"))
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);
            println!("{:.0} {:.0} {:.0} {:.0}", x, y, w, h);
        }
        SmokeEvalCommand::WindowIds => {
            let mut input = String::new();
            io::stdin()
                .read_to_string(&mut input)
                .expect("failed to read stdin");
            let data: Value = serde_json::from_str(&input).expect("failed to parse windows json");
            let mut ids = Vec::new();
            if let Some(arr) = data.as_array() {
                for item in arr {
                    if let Some(id) = item.get("id") {
                        ids.push(id.to_string());
                    }
                }
            }
            println!("{}", ids.join(" "));
        }
        SmokeEvalCommand::FindWindow {
            before_ids,
            class_hint,
            app_name,
        } => {
            let mut input = String::new();
            io::stdin()
                .read_to_string(&mut input)
                .expect("failed to read stdin");
            let data: Value = serde_json::from_str(&input).expect("failed to parse windows json");
            let before_set: HashSet<String> = before_ids
                .split_whitespace()
                .map(|s| s.to_string())
                .collect();
            let hint = class_hint.to_lowercase();
            let app = app_name.to_lowercase();

            let mut target = None;
            if let Some(arr) = data.as_array() {
                for item in arr {
                    let wid = item.get("id").map(|v| v.to_string()).unwrap_or_default();
                    if !wid.is_empty() && before_set.contains(&wid) {
                        continue;
                    }
                    let w_class = item
                        .get("class")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_lowercase();
                    let w_title = item
                        .get("title")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_lowercase();
                    if (!hint.is_empty() && (w_class.contains(&hint) || w_title.contains(&hint)))
                        || (!app.is_empty() && w_class.contains(&app))
                    {
                        target = Some(item);
                        break;
                    }
                }
            }

            if let Some(item) = target {
                let id = item
                    .get("id")
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "-1".to_string());
                let pid = item
                    .get("pid")
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "-1".to_string());
                let x = item.get("x").and_then(|v| v.as_f64()).unwrap_or(0.0);
                let y = item.get("y").and_then(|v| v.as_f64()).unwrap_or(0.0);
                let w = item.get("w").and_then(|v| v.as_f64()).unwrap_or(0.0);
                let h = item.get("h").and_then(|v| v.as_f64()).unwrap_or(0.0);
                println!("{}|{}|{:.0}|{:.0}|{:.0}|{:.0}", id, pid, x, y, w, h);
            } else {
                println!("NOT_FOUND");
            }
        }
        SmokeEvalCommand::VerifyGeom {
            win_x,
            win_y,
            win_w,
            win_h,
            wa_x,
            wa_y,
            wa_w,
            wa_h,
            anchor,
            margin,
        } => {
            let (exp_x, exp_y) = match anchor.as_str() {
                "center" => (wa_x + (wa_w - win_w) / 2.0, wa_y + (wa_h - win_h) / 2.0),
                "top-right" => (wa_x + wa_w - margin - win_w, wa_y + margin),
                "bottom-right" => (wa_x + wa_w - margin - win_w, wa_y + wa_h - margin - win_h),
                other => {
                    eprintln!("UNKNOWN_ANCHOR: {}", other);
                    std::process::exit(1);
                }
            };

            let err_x = (win_x - exp_x).abs();
            let err_y = (win_y - exp_y).abs();
            let pos_ok = err_x <= 1.01 && err_y <= 1.01;
            let wa_ok = win_x >= wa_x - 1.01
                && win_y >= wa_y - 1.01
                && (win_x + win_w) <= (wa_x + wa_w + 1.01)
                && (win_y + win_h) <= (wa_y + wa_h + 1.01);
            let passed = pos_ok && wa_ok;

            println!(
                "{}|{:.1}|{:.1}|{:.1}|{:.1}|{}",
                if passed { "True" } else { "False" },
                exp_x,
                exp_y,
                err_x,
                err_y,
                if wa_ok { "True" } else { "False" }
            );
        }
    }
}
