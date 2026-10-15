//! # Sway backend (EXPERIMENTAL)
//!
//! Design and measurements: `docs/DESIGN_SWAY.md`. Submodules:
//! - `ipc`: the i3-ipc frame format (pure, no sockets).
//! - `types`: the JSON Sway sends and its conversion to `WindowMetadata` and `MonitorLayout`.

// Nothing calls these modules until the driver lands (next commits of this series); the unit
// tests do. Remove this allow with the commit that adds the driver.
#![allow(dead_code)]

pub mod ipc;
pub mod types;
