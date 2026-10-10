//! Trace Analysis Engine (`TraceAnalyser`).
//!
//! NOTE ON VERIFICATION SCOPE: The trace analyser verifies the extension's own
//! logged opacity and frame geometry at logged discrete sampling points only.
//! It cannot prove the absence of visual flicker between samples or verify what
//! was actually rendered on the display hardware; real visual flicker evidence
//! is a human-review / V6 qualification concern.

use regex::Regex;
use std::collections::{HashMap, HashSet};

/// Scope limitation notice required across all trace verification documentation and reports.
pub const SCOPE_LIMITATION_NOTICE: &str =
    "The trace analyser verifies the extension's own logged opacity and frame geometry at logged discrete sampling points only. It cannot prove absence of flicker between samples or verify what was actually rendered on the display hardware; real visual flicker evidence is a V6 / human-review concern.";

/// 2D rectangle in compositor coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TraceRect {
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
}

/// Window metadata parsed from trace log entries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowMeta {
    pub id: u64,
    pub pid: Option<u64>,
    pub class: String,
    pub app: String,
    pub window_type: String,
    pub proto: String,
}

/// A parsed trace event from `[spawn-at-time]` or `session.log`.
#[derive(Debug, Clone, PartialEq)]
pub struct TraceEvent {
    pub elapsed_ms: f64,
    pub tag: String,
    pub window_id: Option<u64>,
    pub meta: Option<WindowMeta>,
    pub opacity: Option<i32>,
    pub frame: Option<TraceRect>,
    pub extra: String,
    pub raw: String,
}

/// Category of verification violation detected during analysis.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ViolationKind {
    OffTargetVisibility,
    OrderConstraintViolation,
    MultipleReveal,
    LingeringCloak,
    WatchdogExpiration,
    MissingRequiredEvent,
}

/// A specific failure detected by the analyser.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceViolation {
    pub kind: ViolationKind,
    pub message: String,
    pub line: Option<String>,
}

/// Analysis outcome report.
///
/// NOTE ON VERIFICATION SCOPE: The trace analyser verifies the extension's own
/// logged opacity and frame geometry at logged discrete sampling points only.
/// It cannot prove the absence of visual flicker between samples or verify what
/// was actually rendered on the display hardware; real visual flicker evidence
/// is a human-review / V6 qualification concern.
#[derive(Debug, Clone, Default)]
pub struct TraceReport {
    pub events: Vec<TraceEvent>,
    pub violations: Vec<TraceViolation>,
    pub warnings: Vec<String>,
    pub cloak_duration_ms: Option<f64>,
}

impl TraceReport {
    pub fn is_pass(&self) -> bool {
        self.violations.is_empty()
    }

    /// Formats a complete human-readable report summary.
    pub fn summary(&self) -> String {
        let mut s = String::new();
        s.push_str("=== Trace Verification Report ===\n");
        s.push_str(&format!("Scope Note: {}\n", SCOPE_LIMITATION_NOTICE));
        s.push_str(&format!(
            "Status: {}\n",
            if self.is_pass() { "PASS" } else { "FAIL" }
        ));
        s.push_str(&format!("Events parsed: {}\n", self.events.len()));
        if let Some(dur) = self.cloak_duration_ms {
            s.push_str(&format!("Cloak duration: {:.2}ms\n", dur));
        }
        if !self.violations.is_empty() {
            s.push_str(&format!("Violations ({}):\n", self.violations.len()));
            for v in &self.violations {
                s.push_str(&format!("  - [{:?}] {}\n", v.kind, v.message));
                if let Some(ref line) = v.line {
                    s.push_str(&format!("    line: {}\n", line));
                }
            }
        }
        if !self.warnings.is_empty() {
            s.push_str(&format!("Warnings ({}):\n", self.warnings.len()));
            for w in &self.warnings {
                s.push_str(&format!("  - {}\n", w));
            }
        }
        s
    }
}

/// Standalone trace analysis engine.
pub struct TraceAnalyser {
    pub target_window_id: Option<u64>,
    pub target_geometry: Option<TraceRect>,
    pub warning_budget_ms: f64,
    pub require_pre_reveal_unminimize: bool,
}

impl Default for TraceAnalyser {
    fn default() -> Self {
        Self {
            target_window_id: None,
            target_geometry: None,
            warning_budget_ms: 250.0,
            require_pre_reveal_unminimize: false,
        }
    }
}

impl TraceAnalyser {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_target_window_id(mut self, id: u64) -> Self {
        self.target_window_id = Some(id);
        self
    }

    pub fn with_target_geometry(mut self, rect: TraceRect) -> Self {
        self.target_geometry = Some(rect);
        self
    }

    pub fn with_warning_budget_ms(mut self, ms: f64) -> Self {
        self.warning_budget_ms = ms;
        self
    }

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

    /// Evaluates invariant assertions against the parsed trace event sequence.
    pub fn analyse(&self, raw_log: &str) -> TraceReport {
        let events = self.parse_events(raw_log);
        let mut violations = Vec::new();
        let mut warnings = Vec::new();

        let mut cloak_idx = None;
        let mut uncloak_idx = None;
        let mut cloak_time = None;
        let mut uncloak_time = None;
        let mut uncloak_count = 0;
        let mut anchor_released = false;

        // Collect indices of first occurrence of each tag for partial-order constraints
        let mut first_idx: HashMap<String, usize> = HashMap::new();

        for (idx, event) in events.iter().enumerate() {
            first_idx.entry(event.tag.clone()).or_insert(idx);

            if event.tag == "WATCHDOG_EXPIRED" {
                violations.push(TraceViolation {
                    kind: ViolationKind::WatchdogExpiration,
                    message: "Watchdog deadline reached; actor was forcefully uncloaked".into(),
                    line: Some(event.raw.clone()),
                });
            }

            if event.tag == "CLOAK" && cloak_idx.is_none() {
                cloak_idx = Some(idx);
                cloak_time = Some(event.elapsed_ms);
            }

            if event.tag == "UNCLOAK" {
                uncloak_count += 1;
                if uncloak_idx.is_none() {
                    uncloak_idx = Some(idx);
                    uncloak_time = Some(event.elapsed_ms);
                }
            }

            if event.tag == "ANCHOR_RELEASED" {
                anchor_released = true;
            }
        }

        // 1. Single Reveal Invariant
        if uncloak_count > 1 {
            violations.push(TraceViolation {
                kind: ViolationKind::MultipleReveal,
                message: format!("Multiple UNCLOAK events detected ({})", uncloak_count),
                line: None,
            });
        }

        // 2. Lingering Cloak Invariant
        if cloak_idx.is_some() && uncloak_idx.is_none() && !anchor_released {
            violations.push(TraceViolation {
                kind: ViolationKind::LingeringCloak,
                message: "Window was cloaked but never uncloaked or disarmed".into(),
                line: None,
            });
        }

        // 3. Partial Order Constraints (A must occur before B if both present)
        // Checks pairwise relative ordering tolerating any extra unrelated intervening events.
        let order_constraints = [
            ("CLOAK", "AFTER_SET_SIZE"),
            ("CLOAK", "COMMIT_RESOLVED"),
            ("CLOAK", "AFTER_SET_POSITION_ANCHORED"),
            ("CLOAK", "PRE_REVEAL_UNMINIMIZE"),
            ("CLOAK", "UNCLOAK"),
            ("AFTER_SET_SIZE", "COMMIT_RESOLVED"),
            ("AFTER_SET_SIZE", "AFTER_SET_POSITION_ANCHORED"),
            ("AFTER_SET_SIZE", "UNCLOAK"),
            ("COMMIT_RESOLVED", "AFTER_SET_POSITION_ANCHORED"),
            ("COMMIT_RESOLVED", "UNCLOAK"),
            ("AFTER_SET_POSITION_ANCHORED", "UNCLOAK"),
            ("PRE_REVEAL_UNMINIMIZE", "UNCLOAK"),
        ];

        for (tag_a, tag_b) in order_constraints {
            if let (Some(&idx_a), Some(&idx_b)) = (first_idx.get(tag_a), first_idx.get(tag_b)) {
                if idx_a >= idx_b {
                    violations.push(TraceViolation {
                        kind: ViolationKind::OrderConstraintViolation,
                        message: format!(
                            "Order violation: {} (at #{}) occurred after {} (at #{})",
                            tag_a, idx_a, tag_b, idx_b
                        ),
                        line: None,
                    });
                }
            }
        }

        // Optional requirement: PRE_REVEAL_UNMINIMIZE must occur for minimized window flows
        if self.require_pre_reveal_unminimize && !first_idx.contains_key("PRE_REVEAL_UNMINIMIZE") {
            violations.push(TraceViolation {
                kind: ViolationKind::MissingRequiredEvent,
                message: "Expected PRE_REVEAL_UNMINIMIZE event in trace, but none was logged"
                    .into(),
                line: None,
            });
        }

        // 4. Off-Target Invisibility: Opacity must be 0 for all samples between CLOAK and UNCLOAK
        if let (Some(c_idx), Some(u_idx)) = (cloak_idx, uncloak_idx) {
            for (idx, event) in events.iter().enumerate() {
                if idx > c_idx && idx < u_idx {
                    if let Some(op) = event.opacity {
                        if op > 0 {
                            if let (Some(target), Some(frame)) = (self.target_geometry, event.frame)
                            {
                                if frame != target {
                                    violations.push(TraceViolation {
                                        kind: ViolationKind::OffTargetVisibility,
                                        message: format!(
                                            "Window visible (opacity={}) at off-target frame ({},{},{}x{}) while cloaked; expected target is ({},{},{}x{})",
                                            op, frame.x, frame.y, frame.w, frame.h, target.x, target.y, target.w, target.h
                                        ),
                                        line: Some(event.raw.clone()),
                                    });
                                }
                            } else {
                                violations.push(TraceViolation {
                                    kind: ViolationKind::OffTargetVisibility,
                                    message: format!(
                                        "Window visible (opacity={}) while cloaked",
                                        op
                                    ),
                                    line: Some(event.raw.clone()),
                                });
                            }
                        }
                    }
                }
            }
        }

        // 5. Timing Budgets (Warnings only, not hard failures)
        let cloak_duration_ms = match (cloak_time, uncloak_time) {
            (Some(c), Some(u)) => {
                let dur = (u - c).max(0.0);
                if dur > self.warning_budget_ms {
                    warnings.push(format!(
                        "Cloak duration {:.2}ms exceeded warning budget of {:.2}ms",
                        dur, self.warning_budget_ms
                    ));
                }
                Some(dur)
            }
            _ => None,
        };

        TraceReport {
            events,
            violations,
            warnings,
            cloak_duration_ms,
        }
    }
}
