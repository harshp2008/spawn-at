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

/// A discrete placement lifecycle (from pre-placement/cloak through uncloak reveal).
#[derive(Debug, Clone, PartialEq)]
pub struct PlacementLifecycle {
    pub index: usize,
    pub cloak_event: Option<TraceEvent>,
    pub uncloak_event: Option<TraceEvent>,
    pub final_frame: Option<TraceRect>,
    pub cloak_duration_ms: Option<f64>,
    pub events: Vec<TraceEvent>,
    pub violations: Vec<TraceViolation>,
    pub warnings: Vec<String>,
}

impl PlacementLifecycle {
    pub fn is_pass(&self) -> bool {
        self.violations.is_empty()
    }
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
    pub lifecycles: Vec<PlacementLifecycle>,
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
        s.push_str(&format!("Lifecycles analyzed: {}\n", self.lifecycles.len()));
        if let Some(dur) = self.cloak_duration_ms {
            s.push_str(&format!("Cloak duration: {:.2}ms\n", dur));
        }
        for lc in &self.lifecycles {
            if let Some(dur) = lc.cloak_duration_ms {
                s.push_str(&format!("  Lifecycle #{}: duration={:.2}ms", lc.index, dur));
                if let Some(ref f) = lc.final_frame {
                    s.push_str(&format!(" final_frame=({},{},{}x{})", f.x, f.y, f.w, f.h));
                }
                s.push('\n');
            }
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
        let mut global_violations = Vec::new();

        // Check global watchdog deadline expiration
        for event in &events {
            if event.tag == "WATCHDOG_EXPIRED" {
                global_violations.push(TraceViolation {
                    kind: ViolationKind::WatchdogExpiration,
                    message: "Watchdog deadline reached; actor was forcefully uncloaked".into(),
                    line: Some(event.raw.clone()),
                });
            }
        }

        // Partition trace into discrete placement lifecycles.
        // A new lifecycle begins when an initialization or cloaking step arrives after a previous reveal.
        let mut lifecycle_slices: Vec<Vec<TraceEvent>> = Vec::new();
        let mut current_slice: Vec<TraceEvent> = Vec::new();
        let mut current_slice_had_uncloak = false;

        for event in &events {
            let is_lifecycle_start = event.tag == "CLOAK"
                || event.tag == "SNAPSHOT_CREATED"
                || event.tag == "SNAPSHOT_SKIPPED"
                || event.tag == "DISPATCH"
                || (event.tag == "BATCH_STEP"
                    && (event.extra.contains("inst=Snapshot")
                        || event.extra.contains("inst=Cloak")));

            if current_slice_had_uncloak && is_lifecycle_start {
                lifecycle_slices.push(current_slice);
                current_slice = Vec::new();
                current_slice_had_uncloak = false;
            }

            if event.tag == "UNCLOAK" {
                current_slice_had_uncloak = true;
            }

            current_slice.push(event.clone());
        }
        if !current_slice.is_empty() {
            lifecycle_slices.push(current_slice);
        }

        let mut lifecycles = Vec::new();
        let mut all_violations = global_violations;
        let mut all_warnings = Vec::new();

        for (lc_idx, lc_events) in lifecycle_slices.into_iter().enumerate() {
            let mut lc_violations = Vec::new();
            let mut lc_warnings = Vec::new();

            let mut cloak_event = None;
            let mut uncloak_event = None;
            let mut cloak_idx = None;
            let mut uncloak_idx = None;
            let mut uncloak_count = 0;
            let mut anchor_released = false;
            let mut final_frame = None;

            let mut first_idx: HashMap<String, usize> = HashMap::new();

            for (idx, event) in lc_events.iter().enumerate() {
                first_idx.entry(event.tag.clone()).or_insert(idx);

                if event.tag == "CLOAK" && cloak_idx.is_none() {
                    cloak_idx = Some(idx);
                    cloak_event = Some(event.clone());
                }

                if event.tag == "UNCLOAK" {
                    uncloak_count += 1;
                    if uncloak_idx.is_none() {
                        uncloak_idx = Some(idx);
                        uncloak_event = Some(event.clone());
                        final_frame = event.frame;
                    }
                }

                if event.tag == "ANCHOR_RELEASED" {
                    anchor_released = true;
                }
            }

            // 1. Single Reveal Invariant per lifecycle
            if uncloak_count > 1 {
                lc_violations.push(TraceViolation {
                    kind: ViolationKind::MultipleReveal,
                    message: format!(
                        "Multiple UNCLOAK events detected in lifecycle #{} ({})",
                        lc_idx, uncloak_count
                    ),
                    line: None,
                });
            }

            // 2. Lingering Cloak Invariant
            if cloak_idx.is_some() && uncloak_idx.is_none() && !anchor_released {
                lc_violations.push(TraceViolation {
                    kind: ViolationKind::LingeringCloak,
                    message: format!(
                        "Window was cloaked in lifecycle #{} but never uncloaked or disarmed",
                        lc_idx
                    ),
                    line: None,
                });
            }

            // 3. Partial Order Constraints within this lifecycle
            // Pairwise relative ordering tolerating any extra unrelated intervening events.
            // Size and commit constraints apply ONLY when those events occur in the trace, never required.
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
                        lc_violations.push(TraceViolation {
                            kind: ViolationKind::OrderConstraintViolation,
                            message: format!(
                                "Order violation in lifecycle #{}: {} (at #{}) occurred after {} (at #{})",
                                lc_idx, tag_a, idx_a, tag_b, idx_b
                            ),
                            line: None,
                        });
                    }
                }
            }

            // Optional check: PRE_REVEAL_UNMINIMIZE required for minimized windows
            if self.require_pre_reveal_unminimize
                && !first_idx.contains_key("PRE_REVEAL_UNMINIMIZE")
            {
                lc_violations.push(TraceViolation {
                    kind: ViolationKind::MissingRequiredEvent,
                    message: format!(
                        "Expected PRE_REVEAL_UNMINIMIZE event in lifecycle #{}, but none was logged",
                        lc_idx
                    ),
                    line: None,
                });
            }

            // 4. Off-Target Invisibility: Opacity must be 0 for all samples between CLOAK and UNCLOAK
            if let (Some(c_idx), Some(u_idx)) = (cloak_idx, uncloak_idx) {
                for (idx, event) in lc_events.iter().enumerate() {
                    if idx > c_idx && idx < u_idx {
                        if let Some(op) = event.opacity {
                            if op > 0 {
                                if let (Some(target), Some(frame)) =
                                    (self.target_geometry, event.frame)
                                {
                                    if frame != target {
                                        lc_violations.push(TraceViolation {
                                            kind: ViolationKind::OffTargetVisibility,
                                            message: format!(
                                                "Window visible (opacity={}) at off-target frame ({},{},{}x{}) while cloaked in lifecycle #{}; expected target is ({},{},{}x{})",
                                                op, frame.x, frame.y, frame.w, frame.h, lc_idx, target.x, target.y, target.w, target.h
                                            ),
                                            line: Some(event.raw.clone()),
                                        });
                                    }
                                } else {
                                    lc_violations.push(TraceViolation {
                                        kind: ViolationKind::OffTargetVisibility,
                                        message: format!(
                                            "Window visible (opacity={}) while cloaked in lifecycle #{}",
                                            op, lc_idx
                                        ),
                                        line: Some(event.raw.clone()),
                                    });
                                }
                            }
                        }
                    }
                }
            }

            // 5. Timing Budget per lifecycle (Warnings only, not hard failures)
            let cloak_duration_ms = match (&cloak_event, &uncloak_event) {
                (Some(c), Some(u)) => {
                    let dur = (u.elapsed_ms - c.elapsed_ms).max(0.0);
                    if dur > self.warning_budget_ms {
                        let w = format!(
                            "Lifecycle #{} cloak duration {:.2}ms exceeded warning budget of {:.2}ms",
                            lc_idx, dur, self.warning_budget_ms
                        );
                        lc_warnings.push(w.clone());
                        all_warnings.push(w);
                    }
                    Some(dur)
                }
                _ => None,
            };

            all_violations.extend(lc_violations.clone());

            lifecycles.push(PlacementLifecycle {
                index: lc_idx,
                cloak_event,
                uncloak_event,
                final_frame,
                cloak_duration_ms,
                events: lc_events,
                violations: lc_violations,
                warnings: lc_warnings,
            });
        }

        let cloak_duration_ms = lifecycles
            .iter()
            .filter_map(|l| l.cloak_duration_ms)
            .fold(None, |acc, d| Some(acc.map_or(d, |a: f64| a.max(d))));

        TraceReport {
            events,
            lifecycles,
            violations: all_violations,
            warnings: all_warnings,
            cloak_duration_ms,
        }
    }
}
