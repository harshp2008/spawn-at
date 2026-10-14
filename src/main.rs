//! # spawn-at: Modular Zero-Flicker Window Placement Engine
//!
//! ## Architectural Overview
//!
//! `spawn-at` is a standalone utility that enables mathematically perfect cold-starts
//! and dynamic transformations for desktop applications on Linux, with primary zero-flicker
//! support for GNOME Wayland.
//!
//! ### Core Components:
//! - **CLI Router ([`cli`]):** Parses subcommands (`install`, `uninstall`, `config`, `update`, `spawn`, `transform`, `query`, `focus`, `defocus`, `maximize`, `minimize`, `restore`, `close`) using `clap`.
//! - **Commands ([`commands`]):** Platform-agnostic execution handlers for subcommands.
//! - **Compositor Platform ([`platform`]):** Trait-based abstraction (`CompositorBackend`) allowing
//!   environment-specific mechanics.
//! - **Platform Resolver ([`platform`]):** Maps binary command invocations to canonical FreeDesktop / Wayland App IDs.
//! - **Geometry Engine (`spawn_at_core::geometry`, in the `spawn-at-core` crate):** Calculates absolute pixel coordinates, workareas, anchor placements, and clamping.
//! - **Target Resolver ([`target`]):** Resolves deterministic window targets by PID, class, title, or focus.

mod cli;
mod commands;
mod config;
mod diagnostics;
mod platform;
mod target;
mod update;

use clap::Parser;
use cli::{Cli, Commands};
use dialoguer::Confirm;
use platform::{init_backend, InstallScope};
use std::io::IsTerminal;

#[tokio::main]
async fn main() {
    let raw_args: Vec<String> = std::env::args().collect();
    if raw_args.iter().any(|a| a == "-V" || a == "--version") {
        println!("{}", crate::update::get_version_display());
        return;
    }

    let mut cli = Cli::parse();
    if raw_args.iter().any(|a| a == "--") {
        if let Commands::Spawn(ref mut s) = cli.command {
            s.after_double_dash = true;
        }
    }

    if let Err(e) = cli.validate() {
        crate::diagnostics::render_error(&e);
        std::process::exit(2);
    }

    if let Commands::Update(update_args) = cli.command {
        if let Err(e) = crate::update::run_update(update_args) {
            crate::diagnostics::render_error(&format!("Update error: {}", e));
            std::process::exit(1);
        }
        return;
    }

    // `config` needs no compositor, no driver and no update notice.
    if let Commands::Config(config_args) = cli.command {
        if let Err(e) = commands::run_config(config_args) {
            crate::diagnostics::render_error(&e);
            std::process::exit(1);
        }
        return;
    }

    let suppress_update_notice = matches!(cli.command, Commands::Update(_))
        || match &cli.command {
            Commands::Query { cmd } => match cmd {
                crate::cli::QueryCommands::Layout { json } => *json,
                crate::cli::QueryCommands::Windows { json, .. } => *json,
                crate::cli::QueryCommands::Pointer => false,
            },
            _ => false,
        };

    let driver = match init_backend().await {
        Ok(d) => d,
        Err(e) => {
            crate::diagnostics::render_error(&format!("Driver initialization error: {}", e));
            std::process::exit(1);
        }
    };

    match cli.command {
        Commands::Install(mut args) => {
            // Before anything is installed: say so when this platform has not been run on, and ask
            // (unless --yes, --headless or there is no terminal; then it only prints).
            let interactive = !args.headless && std::io::stdout().is_terminal();
            if let Some(notice) = driver.install_notice() {
                println!("\x1b[1;33m{notice}\x1b[0m\n");
                let go = crate::platform::install_notice::gate(args.yes, interactive, || {
                    Confirm::new()
                        .with_prompt("Install anyway?")
                        .default(false)
                        .interact()
                        .unwrap_or(false)
                });
                if go == crate::platform::install_notice::Gate::Declined {
                    println!("Nothing was installed.");
                    std::process::exit(1);
                }
            }
            if args.dry_run {
                println!(
                    "Dry run: would install the binary ({:?} scope{}) and the {} extension; nothing was changed.",
                    args.scope,
                    if args.skip_bin { ", skipped" } else { "" },
                    driver.name()
                );
                return;
            }
            if !args.headless && std::io::stdout().is_terminal() && !args.skip_bin {
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
                        crate::diagnostics::render_error(&format!("Error during selection: {}", e));
                        std::process::exit(1);
                    }
                    _ => {}
                }
                println!();
            }

            if !args.skip_bin {
                if let Err(e) = crate::platform::installer::install_binary(args.scope) {
                    let mut recovered = false;
                    if args.scope == InstallScope::System
                        && !args.headless
                        && std::io::stdout().is_terminal()
                    {
                        crate::diagnostics::render_error(&format!(
                            "System installation failed: {}",
                            e
                        ));
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
                        crate::diagnostics::render_error(&format!(
                            "Error during binary installation: {}",
                            e
                        ));
                        std::process::exit(1);
                    }
                }
                println!();
            }

            if let Err(e) = driver.install(&args) {
                crate::diagnostics::render_error(&format!(
                    "Error during install ({}): {}",
                    driver.name(),
                    e
                ));
                std::process::exit(1);
            }

            // Configure update settings
            let (channel, notify) = if !args.headless && std::io::stdout().is_terminal() {
                let ch = if let Some(ref c) = args.update_channel {
                    if crate::update::is_deprecated_channel_alias(c) {
                        println!("{}", crate::update::CHANNEL_ALL_NOTICE);
                    }
                    crate::update::channel_to_write(Some(c)).to_string()
                } else {
                    println!("\x1b[1;36m=== Update Preferences ===\x1b[0m\n");
                    let channel_options = &[
                        "Official releases only (stable) [Recommended]",
                        "Include beta/pre-releases (beta)",
                    ];
                    let ch_sel = dialoguer::Select::new()
                        .with_prompt("Select update channel:")
                        .items(channel_options)
                        .default(0)
                        .interact();
                    match ch_sel {
                        Ok(1) => "beta".to_string(),
                        _ => "stable".to_string(),
                    }
                };

                let notif = if let Some(n) = args.update_notify {
                    n
                } else {
                    let n_sel = dialoguer::Confirm::new()
                        .with_prompt("Enable visual update notifications in terminal when a newer version is available?")
                        .default(true)
                        .interact();
                    n_sel.unwrap_or(true)
                };
                (ch, notif)
            } else {
                if args
                    .update_channel
                    .as_deref()
                    .is_some_and(crate::update::is_deprecated_channel_alias)
                {
                    println!("{}", crate::update::CHANNEL_ALL_NOTICE);
                }
                (
                    crate::update::channel_to_write(args.update_channel.as_deref()).to_string(),
                    args.update_notify.unwrap_or(false),
                )
            };

            let mut cfg = crate::config::Config::load_or_default();
            cfg.update.auto_check = true;
            cfg.update.channel = channel;
            cfg.update.notify = notify;
            if let Err(e) = cfg.save() {
                crate::diagnostics::render_warning(&format!(
                    "Failed to save update configuration: {}",
                    e
                ));
            }

            // The binary this run installed; with the binary step skipped, the one running now.
            let autostart_exe = if args.skip_bin {
                std::env::current_exe().map_err(|e| e.to_string())
            } else {
                crate::platform::installer::installed_binary_path(args.scope)
            };
            if let Err(e) =
                autostart_exe.and_then(|exe| crate::config::install_login_autostart_entry(&exe))
            {
                crate::diagnostics::render_warning(&format!(
                    "Failed to set up login update check: {}",
                    e
                ));
            }
        }
        Commands::Uninstall(mut args) => {
            if !args.headless && std::io::stdout().is_terminal() && !args.skip_bin {
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
                        crate::diagnostics::render_error(&format!("Error during selection: {}", e));
                        std::process::exit(1);
                    }
                    _ => {}
                }
                println!();
            }

            if !args.skip_bin {
                if let Err(e) = crate::platform::installer::uninstall_binary(args.scope) {
                    crate::diagnostics::render_error(&format!(
                        "Error during binary uninstallation: {}",
                        e
                    ));
                    std::process::exit(1);
                }
                println!();
            }

            if let Err(e) = driver.uninstall(&args) {
                crate::diagnostics::render_error(&format!(
                    "Error during uninstall ({}): {}",
                    driver.name(),
                    e
                ));
                std::process::exit(1);
            }

            crate::config::uninstall_login_autostart_entry();
        }
        Commands::Spawn(spawn_args) => {
            if let Err(e) = commands::run_spawn(driver.as_ref(), spawn_args, cli.no_wait).await {
                crate::diagnostics::render_error(&e.to_string());
                std::process::exit(1);
            }
        }
        Commands::Transform(transform_args) => {
            if let Err(e) =
                commands::run_transform(driver.as_ref(), transform_args, cli.no_wait).await
            {
                crate::diagnostics::render_error(&e.to_string());
                std::process::exit(1);
            }
        }
        Commands::Query { cmd } => {
            if let Err(e) = commands::run_query(driver.as_ref(), cmd).await {
                crate::diagnostics::render_error(&e.to_string());
                std::process::exit(1);
            }
        }
        Commands::Focus(args) => {
            if let Err(e) = commands::run_focus(driver.as_ref(), args, cli.no_wait).await {
                crate::diagnostics::render_error(&e.to_string());
                std::process::exit(1);
            }
        }
        Commands::Defocus(args) => {
            if let Err(e) = commands::run_defocus(driver.as_ref(), args, cli.no_wait).await {
                crate::diagnostics::render_error(&e.to_string());
                std::process::exit(1);
            }
        }
        Commands::Maximize(args) => {
            if let Err(e) = commands::run_maximize(driver.as_ref(), args, cli.no_wait).await {
                crate::diagnostics::render_error(&e.to_string());
                std::process::exit(1);
            }
        }
        Commands::Minimize(args) => {
            if let Err(e) = commands::run_minimize(driver.as_ref(), args, cli.no_wait).await {
                crate::diagnostics::render_error(&e.to_string());
                std::process::exit(1);
            }
        }
        Commands::Restore(args) => {
            if let Err(e) = commands::run_restore(driver.as_ref(), args, cli.no_wait).await {
                crate::diagnostics::render_error(&e.to_string());
                std::process::exit(1);
            }
        }
        Commands::Close(args) => {
            if let Err(e) = commands::run_close(driver.as_ref(), args, cli.no_wait).await {
                crate::diagnostics::render_error(&e.to_string());
                std::process::exit(1);
            }
        }
        Commands::Update(_) | Commands::Config(_) => unreachable!(),
    }

    if !suppress_update_notice {
        crate::update::render_update_notice_if_available();
    }
}
