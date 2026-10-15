//! # spawn-at-verify
//! Verification Engine & Test Harness for `spawn-at`.

// The harness drives a GNOME session with unix system calls (libc), so off unix the crate is
// empty and its binary only says so (see Cargo.toml).
#![cfg(unix)]
#![warn(missing_docs)]

pub mod cli;
pub mod display;
pub mod ext_logging;
pub mod loadavg;
pub mod loader;
pub mod log_source;
pub mod no_config;
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
