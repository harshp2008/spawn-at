//! # spawn-at-core
//!
//! Pure mathematical geometry engine, declarative scheduling types, and driver abstractions for `spawn-at`.

pub mod driver;
pub mod geometry;

pub use driver::{Armed, Batch, Driver, DriverError, Entry, FocusIntent, Reveal, Urgency};
pub use geometry::{
    apply_anchor, apply_pivot, calculate_rect_from_placement, check_geometry_diagnostics,
    clamp_to_bounds, compute_screen_anchor, resolve_workarea, Anchor, Area, GeometryDiagnostic,
    Pivot, PlacementParams, Rect,
};
