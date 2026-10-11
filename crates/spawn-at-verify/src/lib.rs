//! # spawn-at-verify
//! Verification Engine & Test Harness for `spawn-at`.

#![warn(missing_docs)]

pub mod cli;
pub mod loader;
pub mod log_source;
pub mod oracle;
pub mod preflight;
pub mod probe;
pub mod process;
pub mod report;
pub mod runner;
pub mod schema;
pub mod trace;
pub mod tracker;

#[cfg(feature = "tui")]
pub mod tui;
