//! # spawn-at: Modular Zero-Flicker Window Placement Engine
//!
//! ## Architectural Overview
//!
//! `spawn-at` is a standalone utility that enables mathematically perfect cold-starts
//! and dynamic transformations for desktop applications on Linux, with primary zero-flicker
//! support for GNOME Wayland.
//!
//! ### Core Components:
//! - **CLI Router ([`cli`]):** Parses subcommands (`install`, `uninstall`, `spawn`, `transform`) using `clap`.
//! - **Commands ([`commands`]):** Platform-agnostic execution handlers for subcommands.
//! - **Compositor Platform ([`platform`]):** Trait-based abstraction (`CompositorBackend`) allowing
//!   environment-specific mechanics.
//! - **Platform Resolver ([`platform`]):** Maps binary command invocations to canonical FreeDesktop / Wayland App IDs.
//! - **Geometry Engine ([`core::geometry`]):** Calculates absolute pixel coordinates, workareas, anchor placements, and clamping.
//! - **Target Resolver ([`target`]):** Resolves deterministic window targets by PID, class, title, or focus.

pub mod cli;
pub mod commands;
pub mod config;
pub mod core;
pub mod platform;
pub mod target;

use clap::Parser;
use cli::{Cli, Commands};
use core::geometry::PlacementParams;
use dialoguer::Confirm;
use platform::{init_backend, InstallScope};
use std::io::IsTerminal;

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let driver = match init_backend().await {
        Ok(d) => d,
        Err(e) => {
            eprintln!("\x1b[1;31mDriver Initialization Error\x1b[0m: {}", e);
            std::process::exit(1);
        }
    };

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
                    if args.scope == InstallScope::System
                        && !args.headless
                        && std::io::stdout().is_terminal()
                    {
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
                eprintln!(
                    "\x1b[1;31mError during install ({})\x1b[0m: {}",
                    driver.name(),
                    e
                );
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
                    eprintln!(
                        "\x1b[1;31mError during binary uninstallation\x1b[0m: {}",
                        e
                    );
                    std::process::exit(1);
                }
                println!();
            }

            if let Err(e) = driver.uninstall(&args) {
                eprintln!(
                    "\x1b[1;31mError during uninstall ({})\x1b[0m: {}",
                    driver.name(),
                    e
                );
                std::process::exit(1);
            }
        }
        Commands::Spawn(spawn_args) => {
            if spawn_args.command.is_empty() {
                eprintln!("\x1b[1;31mError\x1b[0m: No command specified to spawn.");
                std::process::exit(1);
            }

            if let Err(e) = spawn_args.validate() {
                eprintln!("\x1b[1;31mError\x1b[0m: {}", e);
                std::process::exit(1);
            }

            // 1. Calculate target geometry
            let pos = spawn_args.pos.as_ref().map(|p| (p[0], p[1]));
            let offset = spawn_args.offset.as_ref().map(|o| (o[0], o[1]));
            let size = spawn_args.size.as_ref().map(|s| (s[0] as u32, s[1] as u32));

            let cursor = driver.get_cursor_position().await.ok();
            let monitors = driver.get_monitors().await.unwrap_or_default();
            
            let params = PlacementParams {
                pos,
                offset,
                size,
                anchor: None,
                pivot: crate::core::geometry::Pivot::TopLeft,
                margin: spawn_args.margin.unwrap_or(0),
                cursor_pos: cursor,
                workarea: monitors.into_iter().next().unwrap_or_default(),
            };

            let payload = core::geometry::calculate_placement(params, 0, 0);
            
            let mut instructions = vec![crate::core::types::Instruction::Cloak];

            if let Some(size) = spawn_args.size {
                instructions.push(crate::core::types::Instruction::SetSize { w: size[0] as u32, h: size[1] as u32 });
                instructions.push(crate::core::types::Instruction::WaitForCommit { timeout_ms: 60 });
            }

            instructions.push(crate::core::types::Instruction::SetPositionAnchored(payload));
            instructions.push(crate::core::types::Instruction::Uncloak);

            // 2. Resolve target application identifier via the active driver
            let target_id = driver.resolve_id(&spawn_args.command, spawn_args.class.as_deref());

            // 3. Dispatch to active driver
            if let Err(e) = driver.spawn_at(&target_id, &spawn_args.command, &instructions).await {
                eprintln!(
                    "\x1b[1;31mPlacement Error\x1b[0m [driver: {}]: {}",
                    driver.name(),
                    e
                );
                std::process::exit(1);
            }
        }
        Commands::Transform(transform_args) => {
            if let Err(e) = commands::run_transform(driver.as_ref(), transform_args).await {
                eprintln!("\x1b[1;31mError\x1b[0m: {}", e);
                std::process::exit(1);
            }
        }
        Commands::Query { cmd } => {
            if let Err(e) = commands::run_query(driver.as_ref(), cmd).await {
                eprintln!("\x1b[1;31mError\x1b[0m: {}", e);
                std::process::exit(1);
            }
        }
        Commands::Move(move_args) => {
            if move_args.pos.len() != 2 {
                eprintln!(
                    "\x1b[1;31mError\x1b[0m: Position must contain exactly X and Y coordinates."
                );
                std::process::exit(1);
            }
            let x = move_args.pos[0];
            let y = move_args.pos[1];

            if let Err(e) = driver.move_window(&move_args.class, x, y).await {
                eprintln!("\x1b[1;31mError\x1b[0m: {}", e);
                std::process::exit(1);
            }
        }
        Commands::Focus(args) => {
            if let Err(e) = commands::run_focus(driver.as_ref(), args).await {
                eprintln!("\x1b[1;31mError\x1b[0m: {}", e);
                std::process::exit(1);
            }
        }
        Commands::Defocus(args) => {
            if let Err(e) = commands::run_defocus(driver.as_ref(), args).await {
                eprintln!("\x1b[1;31mError\x1b[0m: {}", e);
                std::process::exit(1);
            }
        }
        Commands::Maximize(args) => {
            if let Err(e) = commands::run_maximize(driver.as_ref(), args).await {
                eprintln!("\x1b[1;31mError\x1b[0m: {}", e);
                std::process::exit(1);
            }
        }
        Commands::Minimize(args) => {
            if let Err(e) = commands::run_minimize(driver.as_ref(), args).await {
                eprintln!("\x1b[1;31mError\x1b[0m: {}", e);
                std::process::exit(1);
            }
        }
        Commands::Restore(args) => {
            if let Err(e) = commands::run_restore(driver.as_ref(), args).await {
                eprintln!("\x1b[1;31mError\x1b[0m: {}", e);
                std::process::exit(1);
            }
        }
    }
}
