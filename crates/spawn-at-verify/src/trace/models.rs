//! Trace data models, events, and violation structures.

/// Scope limitation notice required across all trace verification documentation and reports.
pub const SCOPE_LIMITATION_NOTICE: &str =
    "The trace analyser verifies the extension's own logged opacity and frame geometry at logged discrete sampling points only. It cannot prove absence of flicker between samples or verify what was actually rendered on the display hardware; real visual flicker evidence is a V6 / human-review concern.";

/// 2D rectangle in compositor coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TraceRect {
    /// Horizontal coordinate of the top-left corner.
    pub x: i32,
    /// Vertical coordinate of the top-left corner.
    pub y: i32,
    /// Width in pixels.
    pub w: u32,
    /// Height in pixels.
    pub h: u32,
}

/// Window metadata parsed from trace log entries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowMeta {
    /// Compositor or X11 window identifier.
    pub id: u64,
    /// Owning process ID if known.
    pub pid: Option<u64>,
    /// Window class name.
    pub class: String,
    /// Application identifier.
    pub app: String,
    /// Window type classification (e.g. normal, dialog).
    pub window_type: String,
    /// Window protocol ("wayland" or "x11").
    pub proto: String,
}

/// A parsed trace event from `[spawn-at-time]` or `session.log`.
#[derive(Debug, Clone, PartialEq)]
pub struct TraceEvent {
    /// Relative elapsed timestamp in milliseconds from start.
    pub elapsed_ms: f64,
    /// Event type tag (e.g. CLOAK, UNCLOAK, COMMIT).
    pub tag: String,
    /// Window ID associated with the event.
    pub window_id: Option<u64>,
    /// Window metadata if provided in the event log.
    pub meta: Option<WindowMeta>,
    /// Window opacity level (0 = transparent, 255 = opaque).
    pub opacity: Option<i32>,
    /// Recorded window geometry bounds at this sample.
    pub frame: Option<TraceRect>,
    /// Additional parsed key-value payload attributes.
    pub extra: String,
    /// Full original raw log entry text.
    pub raw: String,
}

/// Category of verification violation detected during analysis.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ViolationKind {
    /// Window became visible before reaching target geometry.
    OffTargetVisibility,
    /// Lifecycle transition ordering violated constraints.
    OrderConstraintViolation,
    /// Multiple uncloak or reveal events were emitted.
    MultipleReveal,
    /// Cloaking persisted past the maximum allowable timeout.
    LingeringCloak,
    /// Verification watchdog deadline expired before completion.
    WatchdogExpiration,
    /// A required lifecycle synchronization event was omitted.
    MissingRequiredEvent,
}

/// A specific failure detected by the analyser.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceViolation {
    /// Category of the detected violation.
    pub kind: ViolationKind,
    /// Detailed diagnostic message describing the failure.
    pub message: String,
    /// Offending raw log line triggering the violation.
    pub line: Option<String>,
}

/// A discrete placement lifecycle (from pre-placement/cloak through uncloak reveal).
#[derive(Debug, Clone, PartialEq)]
pub struct PlacementLifecycle {
    /// Index of this lifecycle within the test execution sequence.
    pub index: usize,
    /// Event recording initial window cloaking/opacity suppression.
    pub cloak_event: Option<TraceEvent>,
    /// Event recording window uncloaking and reveal.
    pub uncloak_event: Option<TraceEvent>,
    /// Final measured geometry frame after placement settled.
    pub final_frame: Option<TraceRect>,
    /// Total duration window remained cloaked in milliseconds.
    pub cloak_duration_ms: Option<f64>,
    /// All discrete trace events recorded during this lifecycle.
    pub events: Vec<TraceEvent>,
    /// Verification violations detected during this lifecycle.
    pub violations: Vec<TraceViolation>,
    /// Non-fatal diagnostic warnings emitted during this lifecycle.
    pub warnings: Vec<String>,
}

impl PlacementLifecycle {
    /// Returns true if no verification violations were encountered.
    pub fn is_pass(&self) -> bool {
        self.violations.is_empty()
    }
}
