//! # spawn-at-core
//!
//! Pure mathematical geometry engine, declarative scheduling types, and driver abstractions for `spawn-at`.
//!
//! The part of `spawn-at` that does no I/O and names no display server: the placement arithmetic
//! ([`geometry`]), the [`Driver`] trait and the [`Batch`] types a compositor driver receives
//! ([`driver`]), the ready-wait state machine ([`ready_wait`]), the settings schema and the
//! `config` actions ([`settings`]), the per-spawn wait override ([`wait_override`]) and the resize
//! report ([`resize_report`]). Nothing in this crate may do I/O or depend on a compositor.

// At this commit the `spawn-at` binary crate is built on this crate and its compositor backends
// live under src/platform/.

pub mod driver;
pub mod geometry;
pub mod ready_wait;
pub mod resize_report;
pub mod settings;
pub mod wait_override;

pub use driver::{Armed, Batch, Driver, DriverError, Entry, FocusIntent, Reveal, Urgency};
pub use geometry::{
    apply_anchor, apply_pivot, calculate_rect_from_placement, check_geometry_diagnostics,
    clamp_to_bounds, compute_screen_anchor, resolve_workarea, Anchor, Area, GeometryDiagnostic,
    Pivot, PlacementParams, Rect,
};
