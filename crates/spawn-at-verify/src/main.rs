//! CLI binary entrypoint for spawn-at-verify.

use clap::Parser;
use spawn_at_verify::cli::{handle_command, Cli};

/// Parses CLI arguments and dispatches to the requested subcommand handler.
fn main() {
    let cli = Cli::parse();
    handle_command(cli.command);
}
