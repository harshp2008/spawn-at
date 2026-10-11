//! Trace invariant analysis, lifecycle verification, and timing budget checks.

use super::models::*;
use super::parser::TraceAnalyser;
use super::report::TraceReport;
use std::collections::HashMap;

impl TraceAnalyser {
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
