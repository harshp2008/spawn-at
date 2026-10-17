//! The clap definitions of the `spawn-at` command line: the `Cli` root and the `Commands` enum.
//! The per-subcommand argument structs and their `validate` methods live in `args`, and are
//! re-exported from here.
//!
//! Parsing and validation only: this module runs nothing and touches no compositor. The help text
//! comes from the `///` comments and `#[command]`/`#[arg]` attributes in this module and in
//! `args`, so changing those changes `--help`.

pub mod args;

#[cfg(test)]
mod tests;

pub use args::*;

use crate::platform::{InstallArgs, UninstallArgs};
use clap::{Parser, Subcommand};

// Root of the command line. `disable_version_flag` removes clap's `-V`; `main` answers `-V` and
// `--version` itself before parsing. `no_wait` is global, so it is accepted after any subcommand;
// `main` passes it only to the spawn, transform, focus, defocus, maximize, minimize, restore and
// close handlers.
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
    #[arg(long, global = true, help_heading = "Behavior", display_order = 5)]
    pub no_wait: bool,

    #[command(subcommand)]
    pub command: Commands,
}

impl Cli {
    /// Runs the `validate` of the chosen subcommand's arguments and returns its error message.
    /// Only `spawn`, `transform`, `focus`, `defocus`, `maximize`, `minimize`, `restore` and `close`
    /// have checks; every other subcommand returns `Ok(())`.
    pub fn validate(&self) -> Result<(), String> {
        match &self.command {
            Commands::Spawn(s) => s.validate(),
            Commands::Transform(t) => t.validate(),
            Commands::Focus(f) => f.validate(),
            Commands::Defocus(d) => d.validate(),
            Commands::Maximize(m) => m.validate(),
            Commands::Minimize(m) => m.validate(),
            Commands::Restore(r) => r.validate(),
            Commands::Close(c) => c.validate(),
            _ => Ok(()),
        }
    }
}

// One variant per subcommand. The `///` on each variant is its line in `spawn-at --help`.
// `Install`, `Uninstall` and `Update` take their argument structs from `platform` and `update`,
// not from `args`.
#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Install the embedded GNOME Shell extension and binary to $PATH
    Install(InstallArgs),
    /// Uninstall the GNOME Shell extension and binary from $PATH
    Uninstall(UninstallArgs),
    /// Show or change spawn-at settings (wait cap, wait strategy, window timeout)
    Config(ConfigArgs),
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
    /// Request graceful closure of a target window
    Close(CloseArgs),
}
