use super::*;
use clap::{CommandFactory, Parser};
use spawn_at_core::geometry::{Anchor, Area, Pivot};

#[test]
fn test_parse_negative_pos_coordinates() {
    let args = vec![
        "spawn-at",
        "transform",
        "--class",
        "org.gnome.Calculator",
        "--pos",
        "-500",
        "-200",
    ];
    let cli = Cli::try_parse_from(args).expect("Failed to parse negative --pos values");
    if let Commands::Transform(t_args) = cli.command {
        assert_eq!(t_args.geometry.pos, Some(vec![-500, -200]));
        assert!(t_args.validate().is_ok());
    } else {
        panic!("Expected Transform command variant");
    }
}

#[test]
fn test_negative_size_fails_validation() {
    let args = vec![
        "spawn-at",
        "transform",
        "--class",
        "org.gnome.Calculator",
        "--size",
        "-100",
        "200",
    ];
    let cli = Cli::try_parse_from(args).expect("Parsing negative --size should succeed");
    if let Commands::Transform(t_args) = cli.command {
        assert_eq!(
            t_args.geometry.size,
            Some(vec!["-100".to_string(), "200".to_string()])
        );
        let err = t_args.validate().unwrap_err();
        assert!(err.contains("Invalid size: width and height must be strictly positive integers"));
    } else {
        panic!("Expected Transform command variant");
    }
}

#[test]
fn test_anchor_and_pos_conflict_fails_validation() {
    let args = vec![
        "spawn-at",
        "transform",
        "--class",
        "org.gnome.Calculator",
        "--anchor",
        "center",
        "--pos",
        "10",
        "10",
    ];
    let cli = Cli::try_parse_from(args).expect("Parsing should succeed");
    if let Commands::Transform(t_args) = cli.command {
        let err = t_args.validate().unwrap_err();
        assert!(err.contains("Conflicting arguments: Cannot specify both --pos and --anchor."));
    } else {
        panic!("Expected Transform command variant");
    }
}

#[test]
fn test_clamp_flag_parsing() {
    // 1. Default (omitted) -> clamp is true
    let args_default = vec!["spawn-at", "transform", "--class", "org.gnome.Calculator"];
    let cli = Cli::try_parse_from(args_default).expect("Parsing should succeed");
    if let Commands::Transform(t_args) = cli.command {
        assert!(t_args.geometry.clamp);
    } else {
        panic!("Expected Transform command variant");
    }

    // 2. Bare flag --clamp -> clamp is true
    let args_bare = vec![
        "spawn-at",
        "transform",
        "--class",
        "org.gnome.Calculator",
        "--clamp",
    ];
    let cli = Cli::try_parse_from(args_bare).expect("Parsing bare --clamp should succeed");
    if let Commands::Transform(t_args) = cli.command {
        assert!(t_args.geometry.clamp);
    } else {
        panic!("Expected Transform command variant");
    }

    // 3. Explicit --clamp=false
    let args_false = vec![
        "spawn-at",
        "transform",
        "--class",
        "org.gnome.Calculator",
        "--clamp=false",
    ];
    let cli = Cli::try_parse_from(args_false).expect("Parsing --clamp=false should succeed");
    if let Commands::Transform(t_args) = cli.command {
        assert!(!t_args.geometry.clamp);
    } else {
        panic!("Expected Transform command variant");
    }
}

#[test]
fn test_pivot_parsing_and_conflicts() {
    let args_pivot = vec![
        "spawn-at",
        "transform",
        "--class",
        "org.gnome.Calculator",
        "--pivot",
        "center",
    ];
    let cli = Cli::try_parse_from(args_pivot).expect("Parsing --pivot should succeed");
    if let Commands::Transform(t_args) = cli.command {
        assert_eq!(t_args.geometry.pivot, Some(Pivot::Center));
        assert!(t_args.validate().is_ok());
    } else {
        panic!("Expected Transform command variant");
    }

    // Anchor + Pivot both specified (e.g. --anchor top-left --pivot center)
    let args_both = vec![
        "spawn-at",
        "transform",
        "--class",
        "org.gnome.Calculator",
        "--anchor",
        "top-left",
        "--pivot",
        "center",
    ];
    let cli = Cli::try_parse_from(args_both).expect("Parsing should succeed");
    if let Commands::Transform(t_args) = cli.command {
        assert_eq!(t_args.geometry.anchor, Some(Anchor::TopLeft));
        assert_eq!(t_args.geometry.pivot, Some(Pivot::Center));
        assert!(t_args.validate().is_ok());
    } else {
        panic!("Expected Transform command variant");
    }

    // Anchor::Cursor + Pivot is valid
    let args_cursor_pivot = vec![
        "spawn-at",
        "transform",
        "--class",
        "org.gnome.Calculator",
        "--anchor",
        "cursor",
        "--pivot",
        "bottom-right",
    ];
    let cli = Cli::try_parse_from(args_cursor_pivot).expect("Parsing should succeed");
    if let Commands::Transform(t_args) = cli.command {
        assert_eq!(t_args.geometry.anchor, Some(Anchor::Cursor));
        assert_eq!(t_args.geometry.pivot, Some(Pivot::BottomRight));
        assert!(t_args.validate().is_ok());
    } else {
        panic!("Expected Transform command variant");
    }
}

#[test]
fn test_area_and_margins_parsing() {
    let args = vec![
        "spawn-at",
        "spawn",
        "--area",
        "screen",
        "--margin-top",
        "10",
        "--mb",
        "20",
        "--margin-left",
        "30",
        "--mr",
        "40",
        "-m",
        "16",
        "gedit",
    ];
    let cli =
        Cli::try_parse_from(args).expect("Parsing spawn with area and margins should succeed");
    if let Commands::Spawn(s_args) = cli.command {
        assert_eq!(s_args.geometry.area, Area::Screen);
        assert_eq!(s_args.geometry.margin_top, Some(10));
        assert_eq!(s_args.geometry.margin_bottom, Some(20));
        assert_eq!(s_args.geometry.margin_left, Some(30));
        assert_eq!(s_args.geometry.margin_right, Some(40));
        assert_eq!(s_args.geometry.margin, 16);
        assert_eq!(s_args.command, vec!["gedit"]);
        assert!(s_args.validate().is_ok());
    } else {
        panic!("Expected Spawn command variant");
    }
}

#[test]
fn test_directional_margin_aliases() {
    let args = vec![
        "spawn-at",
        "transform",
        "--class",
        "kitty",
        "--mt",
        "5",
        "--margin-bottom",
        "15",
        "--ml",
        "25",
        "--margin-right",
        "35",
    ];
    let cli = Cli::try_parse_from(args)
        .expect("Parsing transform with short margin aliases should succeed");
    if let Commands::Transform(t_args) = cli.command {
        assert_eq!(t_args.geometry.margin_top, Some(5));
        assert_eq!(t_args.geometry.margin_bottom, Some(15));
        assert_eq!(t_args.geometry.margin_left, Some(25));
        assert_eq!(t_args.geometry.margin_right, Some(35));
        assert!(t_args.validate().is_ok());
    } else {
        panic!("Expected Transform command variant");
    }
}

#[test]
fn test_transform_focus_defocus_conflict() {
    // 1. --focus alone succeeds
    let args_focus = vec![
        "spawn-at",
        "transform",
        "--class",
        "org.gnome.Calculator",
        "--focus",
    ];
    let cli = Cli::try_parse_from(args_focus).expect("Parsing --focus should succeed");
    if let Commands::Transform(t_args) = cli.command {
        assert!(t_args.focus_modifiers.focus);
        assert!(!t_args.focus_modifiers.defocus);
        assert!(!t_args.focus_modifiers.no_focus);
    } else {
        panic!("Expected Transform variant");
    }

    // 2. --defocus alone succeeds
    let args_defocus = vec![
        "spawn-at",
        "transform",
        "--class",
        "org.gnome.Calculator",
        "--defocus",
    ];
    let cli = Cli::try_parse_from(args_defocus).expect("Parsing --defocus should succeed");
    if let Commands::Transform(t_args) = cli.command {
        assert!(!t_args.focus_modifiers.focus);
        assert!(t_args.focus_modifiers.defocus);
        assert!(!t_args.focus_modifiers.no_focus);
    } else {
        panic!("Expected Transform variant");
    }

    // 3. --no-focus alone succeeds
    let args_no_focus = vec![
        "spawn-at",
        "transform",
        "--class",
        "org.gnome.Calculator",
        "--no-focus",
    ];
    let cli = Cli::try_parse_from(args_no_focus).expect("Parsing --no-focus should succeed");
    if let Commands::Transform(t_args) = cli.command {
        assert!(!t_args.focus_modifiers.focus);
        assert!(!t_args.focus_modifiers.defocus);
        assert!(t_args.focus_modifiers.no_focus);
    } else {
        panic!("Expected Transform variant");
    }

    // 4. Conflicts
    let args_conflict_focus_defocus = vec![
        "spawn-at",
        "transform",
        "--class",
        "org.gnome.Calculator",
        "--focus",
        "--defocus",
    ];
    let err = Cli::try_parse_from(args_conflict_focus_defocus).unwrap_err();
    assert!(err.to_string().contains("cannot be used with"));

    let args_conflict_focus_no_focus = vec![
        "spawn-at",
        "transform",
        "--class",
        "org.gnome.Calculator",
        "--focus",
        "--no-focus",
    ];
    let err = Cli::try_parse_from(args_conflict_focus_no_focus).unwrap_err();
    assert!(err.to_string().contains("cannot be used with"));

    let args_conflict_defocus_no_focus = vec![
        "spawn-at",
        "transform",
        "--class",
        "org.gnome.Calculator",
        "--defocus",
        "--no-focus",
    ];
    let err = Cli::try_parse_from(args_conflict_defocus_no_focus).unwrap_err();
    assert!(err.to_string().contains("cannot be used with"));
}

#[test]
fn test_lifecycle_and_focus_subcommands_parsing() {
    // focus with aliases
    let cli = Cli::try_parse_from(["spawn-at", "focus", "-c", "code"]).unwrap();
    assert!(matches!(cli.command, Commands::Focus(args) if args.class.as_deref() == Some("code")));

    let cli = Cli::try_parse_from(["spawn-at", "raise", "-t", "editor"]).unwrap();
    assert!(
        matches!(cli.command, Commands::Focus(args) if args.title.as_deref() == Some("editor"))
    );

    let cli = Cli::try_parse_from(["spawn-at", "activate", "--pid", "1234"]).unwrap();
    assert!(matches!(cli.command, Commands::Focus(args) if args.pid == Some(1234)));

    // defocus with aliases, --to-desktop/--desktop, and --to-window/--to
    let cli = Cli::try_parse_from(["spawn-at", "defocus", "--focused"]).unwrap();
    if let Commands::Defocus(args) = cli.command {
        assert!(args.target.focused);
        assert!(!args.to_desktop);
        assert_eq!(args.to_window, None);
        assert_eq!(args.mode_and_destination(), ("mru", ""));
    } else {
        panic!("Expected Defocus variant");
    }

    let cli = Cli::try_parse_from(["spawn-at", "unfocus", "--to-desktop"]).unwrap();
    if let Commands::Defocus(args) = cli.command {
        assert!(args.to_desktop);
        assert_eq!(args.to_window, None);
        assert_eq!(args.mode_and_destination(), ("desktop", ""));
        // Default selector targets focused window
        let sel = args.get_selector();
        assert!(sel.focused);
    } else {
        panic!("Expected Defocus variant");
    }

    let cli = Cli::try_parse_from(["spawn-at", "defocus", "--desktop"]).unwrap();
    if let Commands::Defocus(args) = cli.command {
        assert!(args.to_desktop);
        assert_eq!(args.mode_and_destination(), ("desktop", ""));
    } else {
        panic!("Expected Defocus variant");
    }

    let cli = Cli::try_parse_from(["spawn-at", "defocus", "--to-window", "editor"]).unwrap();
    if let Commands::Defocus(args) = cli.command {
        assert!(!args.to_desktop);
        assert_eq!(args.to_window.as_deref(), Some("editor"));
        assert_eq!(args.mode_and_destination(), ("window", "editor"));
    } else {
        panic!("Expected Defocus variant");
    }

    let cli =
        Cli::try_parse_from(["spawn-at", "blur", "-c", "code", "--to", "target_win"]).unwrap();
    if let Commands::Defocus(args) = cli.command {
        assert_eq!(args.target.class.as_deref(), Some("code"));
        assert_eq!(args.to_window.as_deref(), Some("target_win"));
        assert_eq!(args.mode_and_destination(), ("window", "target_win"));
    } else {
        panic!("Expected Defocus variant");
    }

    // Conflict between --to-desktop and --to-window
    let conflict_res = Cli::try_parse_from([
        "spawn-at",
        "defocus",
        "--to-desktop",
        "--to-window",
        "editor",
    ]);
    assert!(conflict_res.is_err());

    // maximize with British alias 'maximise' and focus modifiers
    let cli =
        Cli::try_parse_from(["spawn-at", "maximize", "-c", "terminal", "--no-focus"]).unwrap();
    if let Commands::Maximize(args) = cli.command {
        assert_eq!(args.target.class.as_deref(), Some("terminal"));
        assert!(args.focus_modifiers.no_focus);
        assert!(!args.focus_modifiers.focus);
        assert!(!args.focus_modifiers.defocus);
    } else {
        panic!("Expected Maximize variant");
    }

    let cli = Cli::try_parse_from(["spawn-at", "maximise", "-c", "terminal", "--defocus"]).unwrap();
    if let Commands::Maximize(args) = cli.command {
        assert_eq!(args.target.class.as_deref(), Some("terminal"));
        assert!(args.focus_modifiers.defocus);
    } else {
        panic!("Expected Maximize variant via alias");
    }

    // minimize with British alias 'minimise' (no focus modifiers allowed)
    let cli = Cli::try_parse_from(["spawn-at", "minimize", "-c", "terminal"]).unwrap();
    assert!(
        matches!(cli.command, Commands::Minimize(ref args) if args.target.class.as_deref() == Some("terminal"))
    );

    let cli = Cli::try_parse_from(["spawn-at", "minimise", "-c", "terminal"]).unwrap();
    assert!(
        matches!(cli.command, Commands::Minimize(ref args) if args.target.class.as_deref() == Some("terminal"))
    );

    // minimize rejecting focus modifiers
    assert!(Cli::try_parse_from(["spawn-at", "minimize", "-c", "terminal", "--focus"]).is_err());
    assert!(Cli::try_parse_from(["spawn-at", "minimize", "-c", "terminal", "--no-focus"]).is_err());
    assert!(Cli::try_parse_from(["spawn-at", "minimize", "-c", "terminal", "--defocus"]).is_err());

    // restore and aliases (unmaximize, unmaximise, unminimize, unminimise) with focus modifiers
    let cli = Cli::try_parse_from(["spawn-at", "restore", "-c", "terminal", "--focus"]).unwrap();
    if let Commands::Restore(args) = cli.command {
        assert_eq!(args.target.class.as_deref(), Some("terminal"));
        assert!(args.focus_modifiers.focus);
    } else {
        panic!("Expected Restore variant");
    }

    let cli =
        Cli::try_parse_from(["spawn-at", "unmaximize", "-c", "terminal", "--no-focus"]).unwrap();
    if let Commands::Restore(args) = cli.command {
        assert_eq!(args.target.class.as_deref(), Some("terminal"));
        assert!(args.focus_modifiers.no_focus);
    } else {
        panic!("Expected Restore variant via unmaximize");
    }

    let cli = Cli::try_parse_from(["spawn-at", "unmaximise", "-c", "terminal"]).unwrap();
    assert!(
        matches!(cli.command, Commands::Restore(ref args) if args.target.class.as_deref() == Some("terminal"))
    );

    let cli = Cli::try_parse_from(["spawn-at", "unminimize", "-c", "terminal"]).unwrap();
    assert!(
        matches!(cli.command, Commands::Restore(ref args) if args.target.class.as_deref() == Some("terminal"))
    );

    let cli = Cli::try_parse_from(["spawn-at", "unminimise", "-c", "terminal"]).unwrap();
    assert!(
        matches!(cli.command, Commands::Restore(ref args) if args.target.class.as_deref() == Some("terminal"))
    );

    // Old subcommands/aliases removed
    assert!(Cli::try_parse_from(["spawn-at", "float", "-c", "terminal"]).is_err());
    assert!(Cli::try_parse_from(["spawn-at", "daemon"]).is_err());
}

#[test]
fn test_window_target_args_validation() {
    let empty_args = WindowTargetArgs::default();
    assert!(empty_args.validate().is_err());

    let class_args = WindowTargetArgs {
        class: Some("gedit".to_string()),
        ..Default::default()
    };
    assert!(class_args.validate().is_ok());

    let id_args = WindowTargetArgs {
        id: Some(123),
        ..Default::default()
    };
    assert!(id_args.validate().is_ok());

    let focused_args = WindowTargetArgs {
        focused: true,
        ..Default::default()
    };
    assert!(focused_args.validate().is_ok());
}

#[test]
fn test_close_subcommand_parsing() {
    // Close with --id
    let cli = Cli::try_parse_from(["spawn-at", "close", "--id", "42"]).unwrap();
    if let Commands::Close(args) = cli.command {
        assert_eq!(args.target.id, Some(42));
        assert!(args.validate().is_ok());
    } else {
        panic!("Expected Close variant");
    }

    // Close with -c
    let cli = Cli::try_parse_from(["spawn-at", "close", "-c", "gedit"]).unwrap();
    if let Commands::Close(args) = cli.command {
        assert_eq!(args.target.class.as_deref(), Some("gedit"));
        assert!(args.validate().is_ok());
    } else {
        panic!("Expected Close variant");
    }

    // Close with --pid
    let cli = Cli::try_parse_from(["spawn-at", "close", "--pid", "12345"]).unwrap();
    if let Commands::Close(args) = cli.command {
        assert_eq!(args.target.pid, Some(12345));
        assert!(args.validate().is_ok());
    } else {
        panic!("Expected Close variant");
    }

    // Close with -t
    let cli = Cli::try_parse_from(["spawn-at", "close", "-t", "Editor"]).unwrap();
    if let Commands::Close(args) = cli.command {
        assert_eq!(args.target.title.as_deref(), Some("Editor"));
        assert!(args.validate().is_ok());
    } else {
        panic!("Expected Close variant");
    }

    // Close with --focused
    let cli = Cli::try_parse_from(["spawn-at", "close", "--focused"]).unwrap();
    if let Commands::Close(args) = cli.command {
        assert!(args.target.focused);
        assert!(args.validate().is_ok());
    } else {
        panic!("Expected Close variant");
    }

    // Close without any selector fails validation
    let cli = Cli::try_parse_from(["spawn-at", "close"]).unwrap();
    if let Commands::Close(args) = cli.command {
        assert!(args.validate().is_err());
    } else {
        panic!("Expected Close variant");
    }
}

#[test]
fn test_clap_parser_configuration() {
    Cli::command().debug_assert();
}

#[test]
fn test_global_no_wait_flag_parsing() {
    // Flag after subcommand
    let cli = Cli::try_parse_from(["spawn-at", "maximize", "--focused", "--no-wait"]).unwrap();
    assert!(cli.no_wait);

    // Flag before subcommand
    let cli = Cli::try_parse_from(["spawn-at", "--no-wait", "maximize", "--focused"]).unwrap();
    assert!(cli.no_wait);

    // Default: false
    let cli = Cli::try_parse_from(["spawn-at", "maximize", "--focused"]).unwrap();
    assert!(!cli.no_wait);

    // With spawn command
    let cli = Cli::try_parse_from(["spawn-at", "spawn", "--no-wait", "--", "alacritty"]).unwrap();
    assert!(cli.no_wait);
}

#[test]
fn test_characterization_all_subcommands_parsing() {
    // 1. Spawn with full matrix of flags
    let cli = Cli::try_parse_from([
        "spawn-at",
        "spawn",
        "--anchor",
        "bottom-right",
        "--pivot",
        "center",
        "-p",
        "100",
        "200",
        "-s",
        "800",
        "600",
        "-m",
        "24",
        "--mt",
        "12",
        "--mb",
        "14",
        "--ml",
        "16",
        "--mr",
        "18",
        "--monitor",
        "DP-1",
        "--area",
        "screen",
        "--clamp=false",
        "--no-focus",
        "--no-wait",
        "--",
        "my-app",
        "arg1",
        "--flag",
    ])
    .unwrap();
    assert!(cli.no_wait);
    if let Commands::Spawn(s) = cli.command {
        assert_eq!(s.geometry.anchor, Some(Anchor::BottomRight));
        assert_eq!(s.geometry.pivot, Some(Pivot::Center));
        assert_eq!(s.geometry.pos, Some(vec![100, 200]));
        assert_eq!(
            s.geometry.size,
            Some(vec!["800".to_string(), "600".to_string()])
        );
        assert_eq!(s.geometry.margin, 24);
        assert_eq!(s.geometry.margin_top, Some(12));
        assert_eq!(s.geometry.margin_bottom, Some(14));
        assert_eq!(s.geometry.margin_left, Some(16));
        assert_eq!(s.geometry.margin_right, Some(18));
        assert_eq!(s.geometry.monitor, "DP-1");
        assert_eq!(s.geometry.area, Area::Screen);
        assert!(!s.geometry.clamp);
        assert!(s.focus_modifiers.no_focus);
        assert_eq!(s.command, vec!["my-app", "arg1", "--flag"]);
    } else {
        panic!("Expected Spawn variant");
    }

    // 2. Query subcommands
    let cli = Cli::try_parse_from(["spawn-at", "query", "layout", "--json"]).unwrap();
    assert!(matches!(
        cli.command,
        Commands::Query {
            cmd: QueryCommands::Layout { json: true }
        }
    ));

    let cli = Cli::try_parse_from(["spawn-at", "query", "pointer"]).unwrap();
    assert!(matches!(
        cli.command,
        Commands::Query {
            cmd: QueryCommands::Pointer
        }
    ));

    let cli = Cli::try_parse_from(["spawn-at", "query", "windows", "--json"]).unwrap();
    assert!(matches!(
        cli.command,
        Commands::Query {
            cmd: QueryCommands::Windows { json: true }
        }
    ));

    // 3. Update args
    let cli = Cli::try_parse_from([
        "spawn-at",
        "update",
        "--check",
        "--channel",
        "all",
        "--force",
        "--headless",
    ])
    .unwrap();
    if let Commands::Update(u) = cli.command {
        assert!(u.check);
        assert_eq!(u.channel.as_deref(), Some("all"));
        assert!(u.force);
        assert!(u.headless);
    } else {
        panic!("Expected Update variant");
    }

    // 4. Install & Uninstall
    let cli = Cli::try_parse_from(["spawn-at", "install", "--skip-bin", "--headless"]).unwrap();
    if let Commands::Install(i) = cli.command {
        assert!(i.skip_bin);
        assert!(i.headless);
    } else {
        panic!("Expected Install variant");
    }

    let cli = Cli::try_parse_from(["spawn-at", "uninstall"]).unwrap();
    assert!(matches!(cli.command, Commands::Uninstall(_)));
}

#[test]
fn test_short_a_flag_rejected() {
    // -a short flag is intentionally removed; only --anchor is valid
    let res = Cli::try_parse_from(["spawn-at", "transform", "-a", "center", "-c", "gedit"]);
    assert!(res.is_err());
    let err = res.unwrap_err().to_string();
    assert!(err.contains("unexpected argument '-a'") || err.contains("unexpected argument"));

    // In spawn, -a is not parsed as anchor (it becomes a positional command argument instead)
    let cli = Cli::try_parse_from(["spawn-at", "spawn", "--anchor", "center", "gedit"]).unwrap();
    if let Commands::Spawn(s) = cli.command {
        assert_eq!(s.geometry.anchor, Some(Anchor::Center));
    }
    let cli2 = Cli::try_parse_from(["spawn-at", "spawn", "-a", "center", "gedit"]).unwrap();
    if let Commands::Spawn(s) = cli2.command {
        assert_eq!(s.geometry.anchor, None);
        assert_eq!(s.command, vec!["-a", "center", "gedit"]);
    }
}

#[test]
fn test_spawn_json_flag_parsing() {
    let cli = Cli::try_parse_from(["spawn-at", "spawn", "--json", "--anchor", "center", "gedit"]).unwrap();
    if let Commands::Spawn(s) = cli.command {
        assert!(s.json);
        assert_eq!(s.geometry.anchor, Some(Anchor::Center));
        assert_eq!(s.command, vec!["gedit"]);
    } else {
        panic!("Expected Spawn variant");
    }

    let cli_no_json = Cli::try_parse_from(["spawn-at", "spawn", "--anchor", "center", "gedit"]).unwrap();
    if let Commands::Spawn(s) = cli_no_json.command {
        assert!(!s.json);
    } else {
        panic!("Expected Spawn variant");
    }
}
