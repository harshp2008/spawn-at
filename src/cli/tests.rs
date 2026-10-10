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
fn test_anchor_with_pos_or_offset_succeeds() {
    let args1 = vec![
        "spawn-at",
        "transform",
        "--class",
        "org.gnome.Calculator",
        "--anchor",
        "center",
        "--pos",
        "10",
        "20",
    ];
    let cli1 = Cli::try_parse_from(args1).expect("Parsing anchor with pos should succeed");
    if let Commands::Transform(t_args) = cli1.command {
        assert!(t_args.validate().is_ok());
        assert_eq!(t_args.geometry.pos_or_offset(), Some((10, 20)));
    } else {
        panic!("Expected Transform command variant");
    }

    let args2 = vec![
        "spawn-at",
        "transform",
        "--class",
        "org.gnome.Calculator",
        "--anchor",
        "center",
        "-o",
        "15",
        "-25",
    ];
    let cli2 = Cli::try_parse_from(args2).expect("Parsing anchor with offset should succeed");
    if let Commands::Transform(t_args) = cli2.command {
        assert!(t_args.validate().is_ok());
        assert_eq!(t_args.geometry.pos_or_offset(), Some((15, -25)));
    } else {
        panic!("Expected Transform command variant");
    }
}

#[test]
fn test_pos_and_offset_alias_conflict_fails_validation() {
    let args = vec![
        "spawn-at",
        "transform",
        "--class",
        "org.gnome.Calculator",
        "--pos",
        "10",
        "10",
        "--offset",
        "20",
        "20",
    ];
    let cli = Cli::try_parse_from(args).expect("Parsing should succeed");
    if let Commands::Transform(t_args) = cli.command {
        let err = t_args.validate().unwrap_err();
        assert!(
            err.contains("Conflicting arguments: Cannot specify both --pos (-p) and --offset (-o)")
        );
    } else {
        panic!("Expected Transform command variant");
    }
}

#[test]
fn test_pos_specified_multiple_times_fails() {
    let args = vec![
        "spawn-at",
        "transform",
        "--class",
        "org.gnome.Calculator",
        "-p",
        "10",
        "10",
        "--pos",
        "20",
        "20",
    ];
    let cli = Cli::try_parse_from(args).expect("Parsing should succeed");
    if let Commands::Transform(t_args) = cli.command {
        let err = t_args.validate().unwrap_err();
        assert!(err.contains("Cannot specify --pos (-p) multiple times."));
    } else {
        panic!("Expected Transform command variant");
    }
}

#[test]
fn test_offset_specified_multiple_times_fails() {
    let args = vec![
        "spawn-at",
        "transform",
        "--class",
        "org.gnome.Calculator",
        "-o",
        "10",
        "10",
        "--offset",
        "20",
        "20",
    ];
    let cli = Cli::try_parse_from(args).expect("Parsing should succeed");
    if let Commands::Transform(t_args) = cli.command {
        let err = t_args.validate().unwrap_err();
        assert!(err.contains("Cannot specify --offset (-o) multiple times."));
    } else {
        panic!("Expected Transform command variant");
    }
}

#[test]
fn test_clamp_flag_parsing() {
    // 1. Default (omitted) -> clamp is None, helper returns true
    let args_default = vec!["spawn-at", "transform", "--class", "org.gnome.Calculator"];
    let cli = Cli::try_parse_from(args_default).expect("Parsing should succeed");
    if let Commands::Transform(t_args) = cli.command {
        assert_eq!(t_args.geometry.clamp, None);
        assert!(t_args.geometry.clamp());
    } else {
        panic!("Expected Transform command variant");
    }

    // 2. Bare flag --clamp -> clamp is Some(true)
    let args_bare = vec![
        "spawn-at",
        "transform",
        "--class",
        "org.gnome.Calculator",
        "--clamp",
    ];
    let cli = Cli::try_parse_from(args_bare).expect("Parsing bare --clamp should succeed");
    if let Commands::Transform(t_args) = cli.command {
        assert_eq!(t_args.geometry.clamp, Some(true));
        assert!(t_args.geometry.clamp());
    } else {
        panic!("Expected Transform command variant");
    }

    // 3. Explicit --clamp=false -> clamp is Some(false)
    let args_false = vec![
        "spawn-at",
        "transform",
        "--class",
        "org.gnome.Calculator",
        "--clamp=false",
    ];
    let cli = Cli::try_parse_from(args_false).expect("Parsing --clamp=false should succeed");
    if let Commands::Transform(t_args) = cli.command {
        assert_eq!(t_args.geometry.clamp, Some(false));
        assert!(!t_args.geometry.clamp());
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
        let err = t_args.validate().unwrap_err();
        assert!(
            err.contains("nothing to change: give --size, -p/-o, --anchor, or --pivot with --size")
        );
    } else {
        panic!("Expected Transform command variant");
    }

    let args_pivot_with_size = vec![
        "spawn-at",
        "transform",
        "--class",
        "org.gnome.Calculator",
        "--pivot",
        "center",
        "--size",
        "500",
        "400",
    ];
    let cli_ps = Cli::try_parse_from(args_pivot_with_size).expect("Parsing should succeed");
    if let Commands::Transform(t_args) = cli_ps.command {
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
fn test_transform_nothing_to_change_fails_validation() {
    // 1. Selector only, no geometry changes
    let args_empty = vec!["spawn-at", "transform", "--class", "org.gnome.Calculator"];
    let cli_empty = Cli::try_parse_from(args_empty).expect("Parsing should succeed");
    if let Commands::Transform(t_args) = cli_empty.command {
        let err = t_args.validate().unwrap_err();
        assert_eq!(
            err,
            "nothing to change: give --size, -p/-o, --anchor, or --pivot with --size"
        );
    } else {
        panic!("Expected Transform command variant");
    }

    // 2. Only --pivot center without --size
    let args_pivot_only = vec!["spawn-at", "transform", "--id", "123", "--pivot", "center"];
    let cli_po = Cli::try_parse_from(args_pivot_only).expect("Parsing should succeed");
    if let Commands::Transform(t_args) = cli_po.command {
        let err = t_args.validate().unwrap_err();
        assert_eq!(
            err,
            "nothing to change: give --size, -p/-o, --anchor, or --pivot with --size"
        );
    } else {
        panic!("Expected Transform command variant");
    }

    // 3. With --size only -> valid
    let args_size = vec![
        "spawn-at",
        "transform",
        "--id",
        "123",
        "--size",
        "500",
        "400",
    ];
    let cli_s = Cli::try_parse_from(args_size).expect("Parsing should succeed");
    if let Commands::Transform(t_args) = cli_s.command {
        assert!(t_args.validate().is_ok());
    } else {
        panic!("Expected Transform command variant");
    }

    // 4. With -p only -> valid
    let args_pos = vec!["spawn-at", "transform", "--id", "123", "-p", "100", "200"];
    let cli_p = Cli::try_parse_from(args_pos).expect("Parsing should succeed");
    if let Commands::Transform(t_args) = cli_p.command {
        assert!(t_args.validate().is_ok());
    } else {
        panic!("Expected Transform command variant");
    }

    // 5. With -o only -> valid
    let args_off = vec!["spawn-at", "transform", "--id", "123", "-o", "50", "50"];
    let cli_o = Cli::try_parse_from(args_off).expect("Parsing should succeed");
    if let Commands::Transform(t_args) = cli_o.command {
        assert!(t_args.validate().is_ok());
    } else {
        panic!("Expected Transform command variant");
    }

    // 6. With --anchor only -> valid
    let args_anc = vec!["spawn-at", "transform", "--id", "123", "--anchor", "center"];
    let cli_a = Cli::try_parse_from(args_anc).expect("Parsing should succeed");
    if let Commands::Transform(t_args) = cli_a.command {
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
        assert_eq!(s_args.geometry.margin, Some(16));
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
        "--anchor",
        "center",
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
        assert_eq!(s.geometry.margin, Some(24));
        assert_eq!(s.geometry.margin_top, Some(12));
        assert_eq!(s.geometry.margin_bottom, Some(14));
        assert_eq!(s.geometry.margin_left, Some(16));
        assert_eq!(s.geometry.margin_right, Some(18));
        assert_eq!(s.geometry.monitor.as_deref(), Some("DP-1"));
        assert_eq!(s.geometry.area, Area::Screen);
        assert!(!s.geometry.clamp());
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
            cmd: QueryCommands::Windows {
                json: true,
                area: Area::Workarea,
            }
        }
    ));

    let cli = Cli::try_parse_from(["spawn-at", "query", "windows", "--area", "screen"]).unwrap();
    assert!(matches!(
        cli.command,
        Commands::Query {
            cmd: QueryCommands::Windows {
                json: false,
                area: Area::Screen,
            }
        }
    ));

    let cli = Cli::try_parse_from([
        "spawn-at",
        "query",
        "windows",
        "--area",
        "unbounded",
        "--json",
    ])
    .unwrap();
    assert!(matches!(
        cli.command,
        Commands::Query {
            cmd: QueryCommands::Windows {
                json: true,
                area: Area::Unbounded,
            }
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
    let cli = Cli::try_parse_from(["spawn-at", "spawn", "--json", "--anchor", "center", "gedit"])
        .unwrap();
    if let Commands::Spawn(s) = cli.command {
        assert!(s.json);
        assert_eq!(s.geometry.anchor, Some(Anchor::Center));
        assert_eq!(s.command, vec!["gedit"]);
    } else {
        panic!("Expected Spawn variant");
    }

    let cli_no_json =
        Cli::try_parse_from(["spawn-at", "spawn", "--anchor", "center", "gedit"]).unwrap();
    if let Commands::Spawn(s) = cli_no_json.command {
        assert!(!s.json);
    } else {
        panic!("Expected Spawn variant");
    }
}

#[test]
fn test_spawn_flag_trap_unknown_option_with_suggestion() {
    let args = vec![
        "spawn-at",
        "spawn",
        "-p",
        "0",
        "0",
        "--size",
        "20",
        "30",
        "--area",
        "screen",
        "--no-clamp",
        "gnome-terminal",
    ];
    let cli = Cli::try_parse_from(args).expect("Parsing should succeed before validation");
    if let Commands::Spawn(s) = cli.command {
        let err = s.validate().unwrap_err();
        assert!(err.contains(
            "unknown option '--no-clamp'; to run a command that starts with '-', put it after --"
        ));
        assert!(err.contains("--clamp false"));
    } else {
        panic!("Expected Spawn variant");
    }
}

#[test]
fn test_spawn_dash_command_after_double_dash_succeeds() {
    let args = vec!["spawn-at", "spawn", "--", "-my-tool"];
    let cli = Cli::try_parse_from(args).expect("Parsing should succeed");
    if let Commands::Spawn(mut s) = cli.command {
        s.after_double_dash = true;
        assert!(s.validate().is_ok());
        assert_eq!(s.command, vec!["-my-tool"]);
    } else {
        panic!("Expected Spawn variant");
    }
}

#[test]
fn test_spawn_dash_command_without_suggestion() {
    let args = vec!["spawn-at", "spawn", "--xyz-random-opt", "gnome-terminal"];
    let cli = Cli::try_parse_from(args).expect("Parsing should succeed before validation");
    if let Commands::Spawn(s) = cli.command {
        let err = s.validate().unwrap_err();
        assert_eq!(
            err,
            "unknown option '--xyz-random-opt'; to run a command that starts with '-', put it after --"
        );
    } else {
        panic!("Expected Spawn variant");
    }
}

#[test]
fn test_unbounded_allowed_and_rejected_options() {
    // 1. Allowed: -p, --pivot, --size, --anchor cursor
    let args_ok = vec![
        "spawn-at",
        "spawn",
        "--area",
        "unbounded",
        "-p",
        "700",
        "300",
        "--pivot",
        "center",
        "--size",
        "800",
        "600",
        "app",
    ];
    let cli = Cli::try_parse_from(args_ok).expect("Parsing should succeed");
    if let Commands::Spawn(s) = cli.command {
        assert!(s.validate().is_ok());
    }

    let args_cursor = vec![
        "spawn-at",
        "spawn",
        "--area",
        "unbounded",
        "--anchor",
        "cursor",
        "app",
    ];
    let cli = Cli::try_parse_from(args_cursor).expect("Parsing should succeed");
    if let Commands::Spawn(s) = cli.command {
        assert!(s.validate().is_ok());
    }

    // 2. Rejected: non-cursor anchor
    let args_bad_anchor = vec![
        "spawn-at",
        "spawn",
        "--area",
        "unbounded",
        "--anchor",
        "top-left",
        "app",
    ];
    let cli = Cli::try_parse_from(args_bad_anchor).expect("Parsing should succeed");
    if let Commands::Spawn(s) = cli.command {
        let err = s.validate().unwrap_err();
        assert!(
            err.contains("Cannot specify --anchor (except --anchor cursor) with --area unbounded.")
        );
    }

    // 3. Rejected: --clamp
    let args_clamp = vec![
        "spawn-at",
        "spawn",
        "--area",
        "unbounded",
        "--clamp=true",
        "app",
    ];
    let cli = Cli::try_parse_from(args_clamp).expect("Parsing should succeed");
    if let Commands::Spawn(s) = cli.command {
        let err = s.validate().unwrap_err();
        assert!(err.contains("Cannot specify --clamp with --area unbounded"));
    }

    // 4. Rejected: -m
    let args_margin = vec![
        "spawn-at",
        "spawn",
        "--area",
        "unbounded",
        "-m",
        "16",
        "app",
    ];
    let cli = Cli::try_parse_from(args_margin).expect("Parsing should succeed");
    if let Commands::Spawn(s) = cli.command {
        let err = s.validate().unwrap_err();
        assert!(err
            .contains("Cannot specify margins (-m, --mt, --mb, --ml, --mr) with --area unbounded"));
    }

    // 5. Rejected: --mt
    let args_mt = vec![
        "spawn-at",
        "spawn",
        "--area",
        "unbounded",
        "--mt",
        "16",
        "app",
    ];
    let cli = Cli::try_parse_from(args_mt).expect("Parsing should succeed");
    if let Commands::Spawn(s) = cli.command {
        let err = s.validate().unwrap_err();
        assert!(err
            .contains("Cannot specify margins (-m, --mt, --mb, --ml, --mr) with --area unbounded"));
    }

    // 6. Rejected: --monitor
    let args_monitor = vec![
        "spawn-at",
        "spawn",
        "--area",
        "unbounded",
        "--monitor",
        "0",
        "app",
    ];
    let cli = Cli::try_parse_from(args_monitor).expect("Parsing should succeed");
    if let Commands::Spawn(s) = cli.command {
        let err = s.validate().unwrap_err();
        assert!(err.contains("Cannot specify --monitor with --area unbounded"));
    }
}

#[test]
fn test_help_headings_order_and_grouping_snapshot() {
    let mut cmd = Cli::command();
    cmd.build();
    let spawn_cmd = cmd.find_subcommand_mut("spawn").unwrap();
    let help_text = spawn_cmd.render_help().to_string();

    // Verify presence and order of headings:
    // Reference area -> Anchor and pivot -> Position and size -> Bounds -> Behavior -> Command
    let pos_ref = help_text
        .find("Reference area:")
        .expect("Missing 'Reference area:' heading");
    let pos_anchor = help_text
        .find("Anchor and pivot:")
        .expect("Missing 'Anchor and pivot:' heading");
    let pos_pos = help_text
        .find("Position and size:")
        .expect("Missing 'Position and size:' heading");
    let pos_bounds = help_text
        .find("Bounds:")
        .expect("Missing 'Bounds:' heading");
    let pos_behavior = help_text
        .find("Behavior:")
        .expect("Missing 'Behavior:' heading");
    let pos_cmd = help_text
        .find("Command:")
        .expect("Missing 'Command:' heading");

    assert!(
        pos_ref < pos_anchor
            && pos_anchor < pos_pos
            && pos_pos < pos_bounds
            && pos_bounds < pos_behavior
            && pos_behavior < pos_cmd,
        "Help headings must appear in order: Reference area, Anchor and pivot, Position and size, Bounds, Behavior, Command.\nActual help text:\n{help_text}"
    );

    // Verify Reference area flags
    let ref_section = &help_text[pos_ref..pos_anchor];
    assert!(ref_section.contains("--area"));
    assert!(ref_section.contains("--monitor"));

    // Verify Anchor and pivot flags
    let anchor_section = &help_text[pos_anchor..pos_pos];
    assert!(anchor_section.contains("--anchor"));
    assert!(anchor_section.contains("--pivot"));

    // Verify Position and size flags
    let pos_section = &help_text[pos_pos..pos_bounds];
    assert!(pos_section.contains("--pos") || pos_section.contains("-p"));
    assert!(pos_section.contains("--offset") || pos_section.contains("-o"));
    assert!(pos_section.contains("--size") || pos_section.contains("-s"));

    // Verify Bounds flags
    let bounds_section = &help_text[pos_bounds..pos_behavior];
    assert!(bounds_section.contains("--clamp"));
    assert!(bounds_section.contains("--margin") || bounds_section.contains("-m"));
    assert!(bounds_section.contains("--margin-top") || bounds_section.contains("mt"));
    assert!(bounds_section.contains("--margin-bottom") || bounds_section.contains("mb"));
    assert!(bounds_section.contains("--margin-left") || bounds_section.contains("ml"));
    assert!(bounds_section.contains("--margin-right") || bounds_section.contains("mr"));

    // Verify Behavior flags order: --focus, --no-focus, --defocus, --json, --no-wait
    let behavior_section = &help_text[pos_behavior..pos_cmd];
    let pos_focus = behavior_section.find("--focus").expect("Missing --focus");
    let pos_no_focus = behavior_section
        .find("--no-focus")
        .expect("Missing --no-focus");
    let pos_defocus = behavior_section
        .find("--defocus")
        .expect("Missing --defocus");
    let pos_json = behavior_section.find("--json").expect("Missing --json");
    let pos_no_wait = behavior_section
        .find("--no-wait")
        .expect("Missing --no-wait");

    assert!(
        pos_focus < pos_no_focus
            && pos_no_focus < pos_defocus
            && pos_defocus < pos_json
            && pos_json < pos_no_wait,
        "Behavior flags must be ordered: --focus, --no-focus, --defocus, --json, --no-wait.\nActual section:\n{behavior_section}"
    );

    // Verify transform help output headings
    let mut root_cmd = Cli::command();
    root_cmd.build();
    let transform_cmd = root_cmd.find_subcommand_mut("transform").unwrap();
    let transform_help = transform_cmd.render_help().to_string();

    let t_ref = transform_help
        .find("Reference area:")
        .expect("Transform missing 'Reference area:' heading");
    let t_anchor = transform_help
        .find("Anchor and pivot:")
        .expect("Transform missing 'Anchor and pivot:' heading");
    let t_pos = transform_help
        .find("Position and size:")
        .expect("Transform missing 'Position and size:' heading");
    let t_bounds = transform_help
        .find("Bounds:")
        .expect("Transform missing 'Bounds:' heading");
    let t_behavior = transform_help
        .find("Behavior:")
        .expect("Transform missing 'Behavior:' heading");

    assert!(
        t_ref < t_anchor && t_anchor < t_pos && t_pos < t_bounds && t_bounds < t_behavior,
        "Transform help headings must appear in order: Reference area, Anchor and pivot, Position and size, Bounds, Behavior.\nActual:\n{transform_help}"
    );
}
