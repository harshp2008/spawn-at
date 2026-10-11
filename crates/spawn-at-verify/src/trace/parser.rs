//! Trace log parsing, event extraction, and regex tokenization.

use super::models::*;
use regex::Regex;
use std::collections::HashSet;

/// Standalone trace analysis engine.
pub struct TraceAnalyser {
    /// Target window identifier to isolate and analyze.
    pub target_window_id: Option<u64>,
    /// Target geometry expected at final window reveal.
    pub target_geometry: Option<TraceRect>,
    /// Maximum cloaking duration threshold before emitting a warning.
    pub warning_budget_ms: f64,
    /// Whether unminimization must precede buffer reveal.
    pub require_pre_reveal_unminimize: bool,
}

impl Default for TraceAnalyser {
    /// Constructs standard default trace analyzer.
    fn default() -> Self {
        Self {
            target_window_id: None,
            target_geometry: None,
            // PROVISIONAL warning budget: 100.0ms.
            // Operations without resize measure ~3-10ms (from GNOME 46 Wayland fixture trace).
            // However, operations with client buffer resize (-s) require configure round-trips
            // and _waitForCommit frame synchronization, and have NOT yet been measured in a real trace.
            // This 100ms threshold is therefore PROVISIONAL (guessed, not measured for size changes).
            // V6 qualification must capture a real trace with -s to calibrate this budget from empirical data.
            warning_budget_ms: 100.0,
            require_pre_reveal_unminimize: false,
        }
    }
}

impl TraceAnalyser {
    /// Constructs a new  instance.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets target window ID to filter trace analysis events.
    pub fn with_target_window_id(mut self, id: u64) -> Self {
        self.target_window_id = Some(id);
        self
    }

    /// Sets expected target placement geometry.
    pub fn with_target_geometry(mut self, rect: TraceRect) -> Self {
        self.target_geometry = Some(rect);
        self
    }

    /// Sets cloak duration budget before generating warnings.
    pub fn with_warning_budget_ms(mut self, ms: f64) -> Self {
        self.warning_budget_ms = ms;
        self
    }

    /// Enforces unminimization prior to buffer commit and reveal.
    pub fn with_require_pre_reveal_unminimize(mut self, require: bool) -> Self {
        self.require_pre_reveal_unminimize = require;
        self
    }

    /// Parses raw logs into structured trace events, deduplicating identical lines
    /// from journal and session.log, and filtering to the target window id.
    pub fn parse_events(&self, raw_log: &str) -> Vec<TraceEvent> {
        let mut events = Vec::new();
        let mut seen_payloads = HashSet::new();

        let re_line = Regex::new(
            r#"^(\d+(?:\.\d+)?)ms\s*\|\s*([A-Za-z0-9_]+)(?:\s+win=\[([^\]]*)\])?(?:\s+(.*))?$"#,
        )
        .expect("valid regex");

        let re_opacity = Regex::new(r#"opacity=(-?\d+)"#).expect("valid regex");
        let re_frame = Regex::new(r#"frame=\((-?\d+),(-?\d+),(\d+)x(\d+)\)"#).expect("valid regex");
        let re_win_id = Regex::new(r#"id=(\d+)"#).expect("valid regex");
        let re_win_pid = Regex::new(r#"pid=(\d+)"#).expect("valid regex");
        let re_win_class = Regex::new(r#"class="([^"]*)""#).expect("valid regex");
        let re_win_app = Regex::new(r#"app="([^"]*)""#).expect("valid regex");
        let re_win_type = Regex::new(r#"type=([A-Za-z0-9_]+)"#).expect("valid regex");
        let re_win_proto = Regex::new(r#"proto=([A-Za-z0-9_]+)"#).expect("valid regex");

        for line in raw_log.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            // Global watchdog deadline detection across formats
            let is_watchdog = trimmed.contains("Watchdog deadline reached for cloaked actor");

            // Canonical normalization for deduplication between journalctl and session.log:
            // Extract the canonical slice starting at the numeric elapsed time: "<N>ms | <TAG> ..."
            let canonical_slice = if is_watchdog {
                "WATCHDOG_DEADLINE_REACHED"
            } else if let Some(idx) = trimmed.find("ms | ") {
                let before_ms = &trimmed[..idx];
                let num_start = before_ms
                    .rfind(|c: char| !c.is_ascii_digit() && c != '.')
                    .map(|pos| pos + 1)
                    .unwrap_or(0);
                &trimmed[num_start..]
            } else {
                trimmed
            };

            // Deduplication across journalctl and session.log outputs
            if !seen_payloads.insert(canonical_slice.to_string()) {
                continue;
            }

            if is_watchdog {
                events.push(TraceEvent {
                    elapsed_ms: 0.0,
                    tag: "WATCHDOG_EXPIRED".to_string(),
                    window_id: self.target_window_id,
                    meta: None,
                    opacity: None,
                    frame: None,
                    extra: trimmed.to_string(),
                    raw: line.to_string(),
                });
                continue;
            }

            if let Some(caps) = re_line.captures(canonical_slice) {
                let elapsed_ms: f64 = caps
                    .get(1)
                    .and_then(|m| m.as_str().parse().ok())
                    .unwrap_or(0.0);
                let tag = caps
                    .get(2)
                    .map(|m| m.as_str().to_string())
                    .unwrap_or_default();
                let win_str = caps.get(3).map(|m| m.as_str());
                let rest_str = caps.get(4).map(|m| m.as_str()).unwrap_or("");

                let (window_id, meta) = if let Some(w) = win_str {
                    let id = re_win_id
                        .captures(w)
                        .and_then(|c| c.get(1))
                        .and_then(|m| m.as_str().parse::<u64>().ok());
                    let pid = re_win_pid
                        .captures(w)
                        .and_then(|c| c.get(1))
                        .and_then(|m| m.as_str().parse::<u64>().ok());
                    let class = re_win_class
                        .captures(w)
                        .and_then(|c| c.get(1))
                        .map(|m| m.as_str().to_string())
                        .unwrap_or_default();
                    let app = re_win_app
                        .captures(w)
                        .and_then(|c| c.get(1))
                        .map(|m| m.as_str().to_string())
                        .unwrap_or_default();
                    let window_type = re_win_type
                        .captures(w)
                        .and_then(|c| c.get(1))
                        .map(|m| m.as_str().to_string())
                        .unwrap_or_default();
                    let proto = re_win_proto
                        .captures(w)
                        .and_then(|c| c.get(1))
                        .map(|m| m.as_str().to_string())
                        .unwrap_or_default();

                    if let Some(valid_id) = id {
                        (
                            Some(valid_id),
                            Some(WindowMeta {
                                id: valid_id,
                                pid,
                                class,
                                app,
                                window_type,
                                proto,
                            }),
                        )
                    } else {
                        (None, None)
                    }
                } else {
                    (None, None)
                };

                // Filter by target window id if specified
                if let Some(target_id) = self.target_window_id {
                    if let Some(event_id) = window_id {
                        if target_id != event_id {
                            continue;
                        }
                    } else if win_str.is_some() {
                        // Explicit non-matching window entry (e.g. win=[none])
                        continue;
                    }
                }

                let opacity = re_opacity
                    .captures(rest_str)
                    .and_then(|c| c.get(1))
                    .and_then(|m| m.as_str().parse::<i32>().ok());

                let frame = re_frame.captures(rest_str).and_then(|c| {
                    let x = c.get(1)?.as_str().parse::<i32>().ok()?;
                    let y = c.get(2)?.as_str().parse::<i32>().ok()?;
                    let w = c.get(3)?.as_str().parse::<u32>().ok()?;
                    let h = c.get(4)?.as_str().parse::<u32>().ok()?;
                    Some(TraceRect { x, y, w, h })
                });

                events.push(TraceEvent {
                    elapsed_ms,
                    tag,
                    window_id,
                    meta,
                    opacity,
                    frame,
                    extra: rest_str.to_string(),
                    raw: line.to_string(),
                });
            }
        }

        events
    }
}
