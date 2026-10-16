//! # spawn-at-verify
//! Verification Engine & Test Harness for `spawn-at`.
//!
//! Developer tooling, not part of the release package. It drives the built `spawn-at` binary only
//! through its command line, loads test specs from `verify/` ([`loader`], [`schema`]), runs them
//! against a fixture window ([`runner`]), compares each placement with an independent arithmetic
//! oracle ([`oracle`]), reads the GNOME extension's log to analyse cloak and flash ([`trace`]) and
//! writes the results ([`report`]). The terminal front end, `tui`, is behind the `tui` feature.

// The harness drives a GNOME session with unix system calls (libc), so off unix the crate is
// empty and its binary only says so (see Cargo.toml).
#![cfg(unix)]
#![warn(missing_docs)]

pub mod backend;
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
