//! # spawn-at-core
//!
//! Pure mathematical geometry engine, declarative scheduling types, and driver abstractions for `spawn-at`.

pub mod driver;
pub mod geometry;

pub use driver::{Armed, Batch, Driver, DriverError, Entry, FocusIntent, Reveal, Urgency};
pub use geometry::{
    apply_anchor, apply_pivot, calculate, clamp_to_bounds, resolve_workarea, Anchor, Area,
    GeometryParams, Pivot, PlacementParams, Rect, TargetGeometry,
};
