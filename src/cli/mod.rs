//! # Command Line Interface Definitions
//!
//! Defines the CLI arguments and subcommands for `spawn-at`.

pub mod args;

#[cfg(test)]
mod tests;

pub use args::*;

use clap::{Parser, Subcommand};
use crate::platform::{InstallArgs, UninstallArgs};

#[derive(Parser, Debug)]
#[command(
    name = "spawn-at",
    disable_version_flag = true,
    about = "Zero-flicker modular window manager & placement engine",
    long_about = "A high-performance Linux window positioning engine supporting mathematically \
                  perfect, zero-flicker cold-starts and dynamic transformations under GNOME Wayland."
)]
pub struct Cli {
    /// Do not wait for compositor to apply changes before exiting
    #[arg(long, global = true)]
    pub no_wait: bool,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Install the embedded GNOME Shell extension and binary to $PATH
    Install(InstallArgs),
    /// Uninstall the GNOME Shell extension and binary from $PATH
    Uninstall(UninstallArgs),
    /// Check for or install available updates
    Update(crate::update::UpdateArgs),
    /// Spawn an application at a specific target screen geometry
    Spawn(SpawnArgs),
    /// Transform, reposition, or resize an existing window
    Transform(TransformArgs),
    /// Query active state from the compositor
    Query {
        #[command(subcommand)]
        cmd: QueryCommands,
    },
    /// Focus / activate a target window (unhides if minimized)
    #[command(aliases = ["raise", "activate"])]
    Focus(WindowTargetArgs),
    /// Relinquish keyboard focus from a target window
    #[command(aliases = ["unfocus", "blur"])]
    Defocus(DefocusArgs),
    /// Maximize a target window
    #[command(alias = "maximise")]
    Maximize(MaximizeArgs),
    /// Minimize a target window
    #[command(alias = "minimise")]
    Minimize(MinimizeArgs),
    /// Restore a window to its normal floating state (unmaximize/unminimize)
    #[command(aliases = ["unmaximize", "unmaximise", "unminimize", "unminimise"])]
    Restore(RestoreArgs),
}
