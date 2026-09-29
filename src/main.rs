//! # spawn-at: Modular Zero-Flicker Window Placement Engine
//!
//! ## Architectural Overview
//!
//! `spawn-at` is a standalone utility that enables mathematically perfect cold-starts
//! for desktop applications on Linux, with primary zero-flicker support for GNOME Wayland.
//!
//! ### Core Components:
//! - **CLI Router ([`main`]):** Parses subcommands (`install`, `uninstall`, `spawn`) using `clap`.
//! - **Compositor Drivers ([`drivers`]):** Trait-based abstraction (`WindowManager`) allowing
//!   environment-specific mechanics:
//!     - [`drivers::gnome::GnomeWaylandDriver`]: Interacts with Mutter via an embedded GNOME
//!       extension using D-Bus pre-arming and opacity cloaking.
//!     - [`drivers::x11::X11Driver`]: Fallback baseline driver for legacy X11 sessions.
//! - **Platform Resolver ([`platform`]):** Maps binary command invocations (e.g. `gnome-text-editor`)
//!   to canonical FreeDesktop / Wayland App IDs (e.g. `org.gnome.TextEditor`), supporting Flatpaks and Snaps.
//! - **Geometry Engine ([`geometry`]):** Calculates absolute pixel coordinates, monitors,
//!   and screen clamping.

mod drivers;
mod geometry;
mod platform;

use dialoguer::Confirm;
use std::io::IsTerminal;
use clap::{Args, Parser, Subcommand};
use drivers::{get_active_driver, InstallArgs, InstallScope, UninstallArgs};
use geometry::GeometryParams;

#[derive(Parser, Debug)]
#[command(
    name = "spawn-at",
    version,
    about = "Zero-flicker modular window manager & placement engine",
    long_about = "A high-performance Linux window positioning engine supporting mathematically \
                  perfect, zero-flicker cold-starts under GNOME Wayland via opacity cloaking."
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Install the embedded GNOME Shell extension and binary to $PATH
    Install(InstallArgs),
    /// Uninstall the GNOME Shell extension and binary from $PATH
    Uninstall(UninstallArgs),
    /// Spawn an application at a specific target screen geometry
    Spawn(SpawnArgs),
}

#[derive(Args, Debug, Clone)]
pub struct SpawnArgs {
    /// Absolute target screen coordinates [X, Y]
    #[arg(short, long, num_args = 2, value_names = ["X", "Y"], conflicts_with = "offset")]
    pub pos: Option<Vec<i32>>,

    /// Relative offset from mouse cursor [X, Y] (defaults to 0 0 if no pos is given)
    #[arg(short, long, num_args = 2, value_names = ["X", "Y"], conflicts_with = "pos")]
    pub offset: Option<Vec<i32>>,

    /// Target window dimensions [WIDTH, HEIGHT]
    #[arg(short, long, num_args = 2, value_names = ["WIDTH", "HEIGHT"])]
    pub size: Option<Vec<u32>>,

    /// Explicit Wayland App ID or WM_CLASS override (e.g. org.gnome.TextEditor or '*')
    #[arg(short = 'c', long)]
    pub class: Option<String>,

    /// Top screen boundary margin
    #[arg(short = 't', long)]
    pub bound_top: Option<i32>,

    /// Bottom screen boundary margin
    #[arg(short = 'b', long)]
    pub bound_bottom: Option<i32>,

    /// Left screen boundary margin
    #[arg(short = 'l', long)]
    pub bound_left: Option<i32>,

    /// Right screen boundary margin
    #[arg(short = 'r', long)]
    pub bound_right: Option<i32>,

    /// Global margin applied to all boundaries
    #[arg(short = 'm', long)]
    pub margin: Option<i32>,

    /// Command to spawn along with any trailing flags/arguments
    #[arg(trailing_var_arg = true, allow_hyphen_values = true, required = true)]
    pub command: Vec<String>,
}

fn main() {
    let cli = Cli::parse();
    let driver = get_active_driver();

    match cli.command {
        Commands::Install(mut args) => {
            if !args.headless && std::io::stdout().is_terminal() {
                if !args.skip_bin {
                    println!("\x1b[1;36m=== Binary Installation Setup ===\x1b[0m\n");
                    let choices = &[
                        "Current user only (~/.local/bin) [Recommended]",
                        "System-wide for all users (/usr/local/bin - requires sudo)",
                        "Skip binary installation (compositor setup only)",
                    ];
                    let selection = dialoguer::Select::new()
                        .with_prompt("Where would you like to install the 'spawn-at' executable?")
                        .items(choices)
                        .default(0)
                        .interact();

                    match selection {
                        Ok(0) => args.scope = InstallScope::User,
                        Ok(1) => args.scope = InstallScope::System,
                        Ok(2) => args.skip_bin = true,
                        Err(e) => {
                            eprintln!("\x1b[1;31mError during selection\x1b[0m: {}", e);
                            std::process::exit(1);
                        }
                        _ => {}
                    }
                    println!();
                }
            }

            if !args.skip_bin {
                if let Err(e) = crate::platform::installer::install_binary(args.scope) {
                    let mut recovered = false;
                    if args.scope == InstallScope::System && !args.headless && std::io::stdout().is_terminal() {
                        println!("\x1b[1;31mSystem installation failed\x1b[0m: {}", e);
                        let fallback = Confirm::new()
                            .with_prompt("System installation failed. Would you like to install to your user directory (~/.local/bin) instead?")
                            .default(true)
                            .interact();

                        if let Ok(true) = fallback {
                            println!("\nFalling back to User scope installation...");
                            args.scope = InstallScope::User;
                            if crate::platform::installer::install_binary(args.scope).is_ok() {
                                recovered = true;
                            }
                        }
                    }

                    if !recovered {
                        eprintln!("\x1b[1;31mError during binary installation\x1b[0m: {}", e);
                        std::process::exit(1);
                    }
                }
                println!();
            }

            if let Err(e) = driver.install(&args) {
                eprintln!("\x1b[1;31mError during install ({})\x1b[0m: {}", driver.name(), e);
                std::process::exit(1);
            }
        }
        Commands::Uninstall(mut args) => {
            if !args.headless && std::io::stdout().is_terminal() {
                if !args.skip_bin {
                    println!("\x1b[1;36m=== Binary Uninstallation Setup ===\x1b[0m\n");
                    let choices = &[
                        "Remove binary from current user directory (~/.local/bin) [Recommended]",
                        "Remove binary from system-wide directory (/usr/local/bin - requires sudo)",
                        "Skip binary removal (compositor cleanup only)",
                    ];
                    let selection = dialoguer::Select::new()
                        .with_prompt("Do you want to remove the global 'spawn-at' executable?")
                        .items(choices)
                        .default(0)
                        .interact();

                    match selection {
                        Ok(0) => args.scope = InstallScope::User,
                        Ok(1) => args.scope = InstallScope::System,
                        Ok(2) => args.skip_bin = true,
                        Err(e) => {
                            eprintln!("\x1b[1;31mError during selection\x1b[0m: {}", e);
                            std::process::exit(1);
                        }
                        _ => {}
                    }
                    println!();
                }
            }

            if !args.skip_bin {
                if let Err(e) = crate::platform::installer::uninstall_binary(args.scope) {
                    eprintln!("\x1b[1;31mError during binary uninstallation\x1b[0m: {}", e);
                    std::process::exit(1);
                }
                println!();
            }

            if let Err(e) = driver.uninstall(&args) {
                eprintln!("\x1b[1;31mError during uninstall ({})\x1b[0m: {}", driver.name(), e);
                std::process::exit(1);
            }
        }
        Commands::Spawn(spawn_args) => {
            if spawn_args.command.is_empty() {
                eprintln!("\x1b[1;31mError\x1b[0m: No command specified to spawn.");
                std::process::exit(1);
            }

            // 1. Calculate target geometry
            let pos = spawn_args.pos.as_ref().map(|p| (p[0], p[1]));
            let offset = spawn_args.offset.as_ref().map(|o| (o[0], o[1]));
            let size = spawn_args.size.as_ref().map(|s| (s[0], s[1]));

            let params = GeometryParams {
                pos,
                offset,
                size,
                bound_top: spawn_args.bound_top,
                bound_bottom: spawn_args.bound_bottom,
                bound_left: spawn_args.bound_left,
                bound_right: spawn_args.bound_right,
                margin: spawn_args.margin,
            };

            let cursor = driver.get_cursor_position();
            let monitors = driver.get_monitors();
            let geom = geometry::calculate(&params, cursor, &monitors);

            // 2. Resolve target application identifier via the active driver
            let target_id = driver.resolve_id(&spawn_args.command, spawn_args.class.as_deref());

            // 3. Dispatch to active driver
            if let Err(e) = driver.spawn_at(&target_id, &spawn_args.command, &geom) {
                eprintln!(
                    "\x1b[1;31mPlacement Error\x1b[0m [driver: {}]: {}",
                    driver.name(),
                    e
                );
                std::process::exit(1);
            }
        }
    }
}
