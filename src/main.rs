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
//! - **Compositor Drivers ([`drivers`]):** Trait-based abstraction (`WindowManager`) allowing
//!   environment-specific mechanics.
//! - **Platform Resolver ([`platform`]):** Maps binary command invocations to canonical FreeDesktop / Wayland App IDs.
//! - **Geometry Engine ([`geom`]):** Calculates absolute pixel coordinates, workareas, anchor placements, and clamping.
//! - **Target Resolver ([`target`]):** Resolves deterministic window targets by PID, class, title, or focus.

pub mod cli;
pub mod config;
pub mod daemon;
pub mod drivers;
pub mod geom;
pub mod geometry;
pub mod platform;
pub mod target;

use clap::Parser;
use cli::{Cli, Commands, QueryCommands, TransformArgs};
use dialoguer::Confirm;
use drivers::{get_active_driver, InstallScope};
use geometry::GeometryParams;
use std::io::IsTerminal;

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
        Commands::Transform(transform_args) => {
            let rt = tokio::runtime::Runtime::new().unwrap();
            if let Err(e) = rt.block_on(run_transform(transform_args)) {
                eprintln!("\x1b[1;31mError\x1b[0m: {}", e);
                std::process::exit(1);
            }
        }
        Commands::Daemon => {
            let rt = tokio::runtime::Runtime::new().unwrap();
            if let Err(e) = rt.block_on(daemon::run_daemon()) {
                eprintln!("\x1b[1;31mDaemon Error\x1b[0m: {}", e);
                std::process::exit(1);
            }
        }
        Commands::Query { cmd } => {
            let rt = tokio::runtime::Runtime::new().unwrap();
            let res = rt.block_on(async {
                let conn = zbus::Connection::session().await?;
                let proxy = daemon::SpawnAtProxy::new(&conn).await?;
                match cmd {
                    QueryCommands::Layout => {
                        let layout = proxy.get_workareas().await?;
                        println!("{}", layout);
                    }
                    QueryCommands::Pointer => {
                        let (x, y) = proxy.get_pointer().await?;
                        println!("{}, {}", x, y);
                    }
                    QueryCommands::Windows => {
                        let windows = proxy.get_windows().await?;
                        println!("{}", windows);
                    }
                }
                Ok::<(), zbus::Error>(())
            });
            if let Err(e) = res {
                eprintln!("\x1b[1;31mQuery Error\x1b[0m: {}", e);
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

            let rt = tokio::runtime::Runtime::new().unwrap();
            let res = rt.block_on(async {
                let conn = zbus::Connection::session().await?;
                let proxy = daemon::SpawnAtProxy::new(&conn).await?;
                proxy.move_window(&move_args.class, x, y).await
            });
            if let Err(e) = res {
                eprintln!("\x1b[1;31mMove Error\x1b[0m: {}", e);
                std::process::exit(1);
            }
        }
    }
}

async fn run_transform(args: TransformArgs) -> Result<(), Box<dyn std::error::Error>> {
    args.validate()?;

    let conn = zbus::Connection::session().await?;
    let proxy = daemon::SpawnAtProxy::new(&conn).await?;

    // 1. Query active windows via D-Bus and resolve target window using TargetResolver
    let json_windows = proxy.get_windows().await?;
    let windows: Vec<target::WindowMetadata> = serde_json::from_str(&json_windows)
        .map_err(|e| format!("Failed to parse windows from compositor: {}", e))?;

    let selector = target::WindowSelector {
        class: args.class,
        title: args.title,
        pid: args.pid,
        focused: args.focused,
    };
    let target_win = target::resolve_target(&windows, &selector)?;

    // 2. Query active workareas and cursor coordinates
    let json_workareas = proxy.get_workareas().await?;
    let workareas: Vec<geom::Rect> = serde_json::from_str(&json_workareas)
        .map_err(|e| format!("Failed to parse workareas from compositor: {}", e))?;
    let (cursor_x, cursor_y) = proxy.get_pointer().await?;

    // 3. Resolve target workarea from --monitor (or cursor location if --cursor / --monitor cursor)
    let target_workarea = if args.cursor || args.monitor.eq_ignore_ascii_case("cursor") {
        geom::resolve_workarea(&workareas, (cursor_x, cursor_y), "cursor")?
    } else {
        geom::resolve_workarea(&workareas, (cursor_x, cursor_y), &args.monitor)?
    };

    // 4. Determine target size
    let (target_w, target_h) = if let Some(ref s) = args.size {
        if s.len() != 2 {
            return Err("Size argument must contain exactly width and height: --size W H".into());
        }
        (s[0], s[1])
    } else {
        (target_win.w, target_win.h)
    };

    // 5. Determine target position
    let (target_x, target_y) = if let Some(anchor) = args.anchor {
        geom::apply_anchor(target_workarea, target_w, target_h, anchor, args.margin)
    } else if args.cursor {
        (cursor_x - target_w / 2, cursor_y - target_h / 2)
    } else if let Some(ref pos) = args.pos {
        if pos.len() != 2 {
            return Err(
                "Position argument must contain exactly X and Y coordinates: --pos X Y".into(),
            );
        }
        (pos[0], pos[1])
    } else {
        (target_win.x, target_win.y)
    };

    // 6. If clamp is true, run clamp_rect against target workarea and margin
    let (final_x, final_y, final_w, final_h) = if args.clamp {
        let clamped = geom::clamp_rect(
            geom::Rect {
                x: target_x,
                y: target_y,
                w: target_w,
                h: target_h,
            },
            target_workarea,
            args.margin,
        );
        (clamped.x, clamped.y, clamped.w, clamped.h)
    } else {
        (target_x, target_y, target_w, target_h)
    };

    // 7. Call proxy.move_resize_window(target_id_or_pid, final_x, final_y, final_w, final_h)
    let target_id = if let Some(pid) = target_win.pid {
        if pid > 0 {
            pid.to_string()
        } else {
            target_win.class.clone()
        }
    } else {
        target_win.class.clone()
    };

    let success = proxy
        .move_resize_window(&target_id, final_x, final_y, final_w, final_h)
        .await?;

    if !success {
        return Err(format!(
            "Compositor failed to move/resize window '{}' (PID: {:?}, class: '{}')",
            target_win.title, target_win.pid, target_win.class
        )
        .into());
    }

    Ok(())
}
