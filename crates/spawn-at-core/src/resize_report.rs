//! # Resize report
//!
//! How the outcome of a size request leaves the extension (the `ResizeOutcome` D-Bus signal of
//! protocol 6) and appears in the CLI's `--json` output as the `resize` object. The kinds are the
//! four of [`crate::ready_wait::Outcome`], spelled as the golden scripts spell them.
//!
//! A caller builds a [`ResizeReport`] from the signal's fields and may add the requested size.
//! This module only converts and serialises: it does no I/O and does not run the wait itself.
//!
//! A reader must tolerate a kind it does not know: [`ResizeKind::from_wire`] maps it to
//! [`ResizeKind::Unknown`] instead of failing.

// Callers at this commit: src/platform/linux/gnome/driver.rs builds the report from the signal and
// src/commands/spawn.rs adds the requested size and prints it.

use serde::{Deserialize, Serialize};

/// Width and height in pixels, serialised as `{"w": .., "h": ..}`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Dim {
    /// Width in pixels.
    pub w: u32,
    /// Height in pixels.
    pub h: u32,
}

/// How the wait ended: the four outcomes of [`crate::ready_wait::Outcome`], plus `Unknown` for a
/// kind this build does not recognise. Serialises in kebab-case (`timed-out`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum ResizeKind {
    /// The window reached the requested size.
    Reached,
    /// The window answered with another size and stayed there.
    Adjusted,
    /// No answer within the cap, or the hard limit ended the wait.
    TimedOut,
    /// No wait was attempted (not resizable, fullscreen, tiled).
    Skipped,
    /// A kind this build does not know (a newer extension).
    Unknown,
}

impl ResizeKind {
    /// Parses the `outcome` string of the signal; anything unrecognised is [`ResizeKind::Unknown`].
    pub fn from_wire(outcome: &str) -> Self {
        match outcome {
            "reached" => Self::Reached,
            "adjusted" => Self::Adjusted,
            "timed-out" => Self::TimedOut,
            "skipped" => Self::Skipped,
            _ => Self::Unknown,
        }
    }
}

/// The `resize` object of the JSON output.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResizeReport {
    /// How the wait ended.
    pub outcome: ResizeKind,
    /// The size the CLI asked for; absent when the signal carried no request to compare with.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requested: Option<Dim>,
    /// The window's frame size when the wait ended (for a skipped wait, its size at the request).
    #[serde(rename = "final")]
    pub final_size: Dim,
    /// Milliseconds as sent by the extension; copied, not measured here. The 0 for a skipped wait
    /// is not verified from this crate.
    pub waited_ms: u32,
    /// Why the wait ended: `at-target`, `quiet`, `cap`, `hard`, or the skip reason.
    pub reason: String,
}

impl ResizeReport {
    /// Builds a report from the fields of the `ResizeOutcome` signal. `requested` is left `None`;
    /// add it with [`ResizeReport::with_requested`]. An unknown `outcome` text becomes
    /// [`ResizeKind::Unknown`]; the other fields are copied as given.
    pub fn from_wire(
        outcome: &str,
        final_w: u32,
        final_h: u32,
        waited_ms: u32,
        reason: &str,
    ) -> Self {
        Self {
            outcome: ResizeKind::from_wire(outcome),
            requested: None,
            final_size: Dim {
                w: final_w,
                h: final_h,
            },
            waited_ms,
            reason: reason.to_string(),
        }
    }

    /// The same report with `requested` set to `w` x `h` pixels, the size the CLI asked for.
    pub fn with_requested(mut self, w: u32, h: u32) -> Self {
        self.requested = Some(Dim { w, h });
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The four kinds round-trip through their wire spelling, which is also their JSON spelling.
    #[test]
    fn kinds_use_the_golden_script_spelling() {
        for (wire, kind) in [
            ("reached", ResizeKind::Reached),
            ("adjusted", ResizeKind::Adjusted),
            ("timed-out", ResizeKind::TimedOut),
            ("skipped", ResizeKind::Skipped),
        ] {
            assert_eq!(ResizeKind::from_wire(wire), kind);
            assert_eq!(serde_json::to_string(&kind).unwrap(), format!("\"{wire}\""));
        }
    }

    /// A kind from a newer extension is kept as `unknown`, not an error.
    #[test]
    fn unknown_kind_is_tolerated() {
        assert_eq!(ResizeKind::from_wire("probed"), ResizeKind::Unknown);
        assert_eq!(ResizeKind::from_wire(""), ResizeKind::Unknown);
    }

    /// The JSON object has exactly the keys outcome, requested, final, waited_ms, reason, with
    /// `final` (a Rust keyword) as the key name.
    #[test]
    fn json_shape() {
        let report =
            ResizeReport::from_wire("adjusted", 494, 292, 41, "quiet").with_requested(500, 300);
        let value = serde_json::to_value(&report).unwrap();
        assert_eq!(
            value,
            serde_json::json!({
                "outcome": "adjusted",
                "requested": {"w": 500, "h": 300},
                "final": {"w": 494, "h": 292},
                "waited_ms": 41,
                "reason": "quiet"
            })
        );
        let back: ResizeReport = serde_json::from_value(value).unwrap();
        assert_eq!(back, report);
    }

    /// Without a request the `requested` key is left out rather than written as null.
    #[test]
    fn requested_is_omitted_when_unknown() {
        let value =
            serde_json::to_value(ResizeReport::from_wire("skipped", 400, 300, 0, "tiled")).unwrap();
        assert!(value.get("requested").is_none());
    }
}
