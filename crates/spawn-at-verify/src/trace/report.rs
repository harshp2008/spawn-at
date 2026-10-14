//! Trace analysis summary and outcome reporting models.

use super::models::*;

/// Analysis outcome report.
///
/// NOTE ON VERIFICATION SCOPE: The trace analyser verifies the extension's own
/// logged opacity and frame geometry at logged discrete sampling points only.
/// It cannot prove the absence of visual flicker between samples or verify what
/// was actually rendered on the display hardware; real visual flicker evidence
/// is a human-review / V6 qualification concern.
#[derive(Debug, Clone, Default)]
pub struct TraceReport {
    /// All parsed and deduplicated trace events.
    pub events: Vec<TraceEvent>,
    /// Analyzed discrete placement lifecycles.
    pub lifecycles: Vec<PlacementLifecycle>,
    /// Aggregate verification violations across all lifecycles.
    pub violations: Vec<TraceViolation>,
    /// Aggregate diagnostic warnings across all lifecycles.
    pub warnings: Vec<String>,
    /// Maximum observed cloak duration across all lifecycles.
    pub cloak_duration_ms: Option<f64>,
}

impl TraceReport {
    /// Returns true if no verification violations were detected.
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
