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
pub mod update;

use clap::Parser;
use cli::{Cli, Commands};
use dialoguer::Confirm;
use platform::{init_backend, InstallScope};
use spawn_at_core::driver::{Batch, Entry, FocusIntent, Reveal, Urgency};
use spawn_at_core::geometry::{
    check_geometry_diagnostics, resolve_workarea, Anchor, GeometryDiagnostic, PlacementParams,
};
use std::io::IsTerminal;
use std::time::Duration;

#[tokio::main]
async fn main() {
    let raw_args: Vec<String> = std::env::args().collect();
    if raw_args.iter().any(|a| a == "-V" || a == "--version") {
        println!("{}", crate::update::get_version_display());
        return;
    }

    let cli = Cli::parse();

    if let Commands::Update(update_args) = cli.command {
        if let Err(e) = crate::update::run_update(update_args) {
            eprintln!("\x1b[1;31mUpdate Error\x1b[0m: {}", e);
            std::process::exit(1);
        }
        return;
    }

    let suppress_update_notice = matches!(cli.command, Commands::Update(_))
        || match &cli.command {
            Commands::Query { cmd } => match cmd {
                crate::cli::QueryCommands::Layout { json } => *json,
                crate::cli::QueryCommands::Windows { json } => *json,
                crate::cli::QueryCommands::Pointer => false,
            },
            _ => false,
        };

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

            // Configure update settings
            let (channel, notify) = if !args.headless && std::io::stdout().is_terminal() {
                let ch = if let Some(ref c) = args.update_channel {
                    c.clone()
                } else {
                    println!("\x1b[1;36m=== Update Preferences ===\x1b[0m\n");
                    let channel_options = &[
                        "Official releases only (stable) [Recommended]",
                        "Include beta/pre-releases (all)",
                    ];
                    let ch_sel = dialoguer::Select::new()
                        .with_prompt("Select update channel:")
                        .items(channel_options)
                        .default(0)
                        .interact();
                    match ch_sel {
                        Ok(1) => "all".to_string(),
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
                (
                    args.update_channel.unwrap_or_else(|| "stable".to_string()),
                    args.update_notify.unwrap_or(false),
                )
            };

            let mut cfg = crate::config::Config::load_or_default();
            cfg.update.auto_check = true;
            cfg.update.channel = channel;
            cfg.update.notify = notify;
            if let Err(e) = cfg.save() {
                eprintln!("\x1b[1;33mWarning\x1b[0m: Failed to save update configuration: {}", e);
            }

            if let Err(e) = crate::config::install_login_autostart_entry() {
                eprintln!("\x1b[1;33mWarning\x1b[0m: Failed to set up login update check: {}", e);
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

            crate::config::uninstall_login_autostart_entry();
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

            // 1. Calculate target geometry parameters
            let is_cursor_anchor = spawn_args.geometry.anchor == Some(Anchor::Cursor);
            let (cursor_x, cursor_y) = driver.get_cursor_position().await.unwrap_or((0, 0));
            let workareas = driver.get_workareas().await.unwrap_or_default();

            let target_workarea = if is_cursor_anchor || spawn_args.geometry.monitor.eq_ignore_ascii_case("cursor") {
                resolve_workarea(&workareas, (cursor_x, cursor_y), "cursor").unwrap_or_default()
            } else {
                resolve_workarea(&workareas, (cursor_x, cursor_y), &spawn_args.geometry.monitor).unwrap_or_default()
            };

            let pos = spawn_args.geometry.pos.as_ref().map(|p| (p[0], p[1]));
            let size = spawn_args.geometry.size.as_ref().and_then(|s| {
                if s.len() == 2 {
                    let w = s[0].parse::<u32>().ok()?;
                    let h = s[1].parse::<u32>().ok()?;
                    Some((w, h))
                } else {
                    None
                }
            });

            let params = PlacementParams {
                pos,
                offset: None,
                size,
                anchor: spawn_args.geometry.anchor,
                pivot: spawn_args.geometry.pivot,
                margin: spawn_args.geometry.margin,
                margin_top: spawn_args.geometry.margin_top,
                margin_bottom: spawn_args.geometry.margin_bottom,
                margin_left: spawn_args.geometry.margin_left,
                margin_right: spawn_args.geometry.margin_right,
                area: Some(spawn_args.geometry.area),
                cursor_pos: Some((cursor_x, cursor_y)),
                workarea: target_workarea,
            };

            for diag in check_geometry_diagnostics(&params) {
                match diag {
                    GeometryDiagnostic::Oversized { w, h } => {
                        eprintln!(
                            "\n\x1b[1;33m[spawn-at] WARN:\x1b[0m Window is oversized ({w}x{h}); bottom and right margins ignored."
                        );
                    }
                    GeometryDiagnostic::SubMinimumSize { w, h } => {
                        eprintln!(
                            "\n\x1b[1;36m[spawn-at] INFO:\x1b[0m Requested size ({w}x{h}) is below toolkit minimums; window will expand."
                        );
                    }
                    _ => {}
                }
            }

            // 2. Generate unique activation token / entry key conforming to freedesktop startup notification spec
            let ts_ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis();
            let entry_key = format!(
                "spawn-at-{}-{}_TIME{}",
                std::process::id(),
                ts_ms,
                ts_ms
            );

            let app_hint = spawn_args.class.clone().unwrap_or_else(|| {
                driver.resolve_id(&spawn_args.command, None)
            });

            let batch = Batch {
                id: 1,
                entries: vec![Entry {
                    key: entry_key.clone(),
                    app_hint: app_hint.clone(),
                    placement: params,
                }],
                reveal: Reveal::Together,
                focus: FocusIntent::Exclusive,
                urgency: Urgency::Normal,
                deadline: Duration::from_millis(15000),
            };

            // Pre-record existing window IDs so wait_for_spawn captures the newly spawned window accurately
            let pre_existing_ids: std::collections::HashSet<u64> = driver
                .get_windows()
                .await
                .unwrap_or_default()
                .into_iter()
                .filter_map(|w| w.id)
                .collect();

            // 3. Arm the driver with the declarative batch intent
            let armed = match driver.arm(batch.clone()).await {
                Ok(a) => a,
                Err(e) => {
                    eprintln!(
                        "\x1b[1;31mPlacement Error\x1b[0m [driver: {}]: {}",
                        driver.name(),
                        e
                    );
                    std::process::exit(1);
                }
            };

            // 4. Launch child process with armed environment variables
            let mut cmd = std::process::Command::new(&spawn_args.command[0]);
            cmd.args(&spawn_args.command[1..]);
            for (k, v) in armed.launch_env {
                cmd.env(k, v);
            }

            let child = match cmd.spawn() {
                Ok(child) => child,
                Err(e) => {
                    eprintln!(
                        "\x1b[1;31mExecution Error\x1b[0m: Failed to spawn command '{}': {}",
                        spawn_args.command[0], e
                    );
                    std::process::exit(1);
                }
            };

            // If we are on X11, intercept the window natively
            if driver.name() == "X11" {
                eprintln!("[spawn-at-main] Detected X11 backend. Triggering native X11 post_spawn hook...");
                if let Err(e) = driver.post_spawn(child.id(), &batch).await {
                    eprintln!("\x1b[1;33m[spawn-at] Warning:\x1b[0m X11 window interception failed: {}", e);
                }
            }

            if !cli.no_wait {
                match commands::wait_for_spawn(
                    driver.as_ref(),
                    child.id(),
                    &app_hint,
                    &entry_key,
                    &pre_existing_ids,
                    Duration::from_millis(2000),
                )
                .await
                {
                    Ok(win) => {
                        eprintln!(
                            "[spawn-at] INFO: Window mapped at final size ({}x{}) and positioned.",
                            win.w, win.h
                        );
                    }
                    Err(e) => {
                        eprintln!(
                            "\x1b[1;33m[spawn-at] Warning:\x1b[0m Timed out waiting for window to map: {}",
                            e
                        );
                    }
                }
            }
        }
        Commands::Transform(transform_args) => {
            if let Err(e) = commands::run_transform(driver.as_ref(), transform_args, cli.no_wait).await {
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
        Commands::Focus(args) => {
            if let Err(e) = commands::run_focus(driver.as_ref(), args, cli.no_wait).await {
                eprintln!("\x1b[1;31mError\x1b[0m: {}", e);
                std::process::exit(1);
            }
        }
        Commands::Defocus(args) => {
            if let Err(e) = commands::run_defocus(driver.as_ref(), args, cli.no_wait).await {
                eprintln!("\x1b[1;31mError\x1b[0m: {}", e);
                std::process::exit(1);
            }
        }
        Commands::Maximize(args) => {
            if let Err(e) = commands::run_maximize(driver.as_ref(), args, cli.no_wait).await {
                eprintln!("\x1b[1;31mError\x1b[0m: {}", e);
                std::process::exit(1);
            }
        }
        Commands::Minimize(args) => {
            if let Err(e) = commands::run_minimize(driver.as_ref(), args, cli.no_wait).await {
                eprintln!("\x1b[1;31mError\x1b[0m: {}", e);
                std::process::exit(1);
            }
        }
        Commands::Restore(args) => {
            if let Err(e) = commands::run_restore(driver.as_ref(), args, cli.no_wait).await {
                eprintln!("\x1b[1;31mError\x1b[0m: {}", e);
                std::process::exit(1);
            }
        }
        Commands::Update(_) => unreachable!(),
    }

    if !suppress_update_notice {
        crate::update::render_update_notice_if_available();
    }
}
