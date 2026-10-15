//! CLI binary entrypoint for spawn-at-verify.

#[cfg(unix)]
use clap::Parser;
#[cfg(unix)]
use spawn_at_verify::cli::{handle_command, Cli};

/// Parses CLI arguments and dispatches to the requested subcommand handler.
#[cfg(unix)]
fn main() {
    let cli = Cli::parse();
    handle_command(cli.command);
}

/// Off unix the library is empty; say so instead of failing to compile.
#[cfg(not(unix))]
fn main() {
    eprintln!("spawn-at-verify needs a unix system (it drives a GNOME session)");
    std::process::exit(1);
}
