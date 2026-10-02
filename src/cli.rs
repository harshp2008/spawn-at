//! # Command Line Interface Definitions
//!
//! Defines the CLI arguments and subcommands for `spawn-at`.

use crate::platform::{InstallArgs, UninstallArgs};
use crate::core::geometry::{Anchor, Pivot};
use clap::{Args, Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(
    name = "spawn-at",
    version,
    about = "Zero-flicker modular window manager & placement engine",
    long_about = "A high-performance Linux window positioning engine supporting mathematically \
                  perfect, zero-flicker cold-starts and dynamic transformations under GNOME Wayland."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Install the embedded GNOME Shell extension and binary to $PATH
    Install(InstallArgs),
    /// Uninstall the GNOME Shell extension and binary from $PATH
    Uninstall(UninstallArgs),
    /// Spawn an application at a specific target screen geometry
    Spawn(SpawnArgs),
    /// Transform, reposition, or resize an existing window
    Transform(TransformArgs),
    /// Query active state from the compositor
    Query {
        #[command(subcommand)]
        cmd: QueryCommands,
    },
    /// Move an existing window by its class identifier
    Move(MoveArgs),
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
}

#[derive(Subcommand, Debug)]
pub enum QueryCommands {
    /// Fetch and print layout of all work areas
    Layout {
        /// Output raw JSON instead of human-readable table
        #[arg(long)]
        json: bool,
    },
    /// Fetch and print current pointer coordinates
    Pointer,
    /// Fetch and print list of all open windows with metadata
    Windows {
        /// Output raw JSON instead of human-readable table
        #[arg(long)]
        json: bool,
    },
}

#[derive(Args, Debug, Clone)]
pub struct MoveArgs {
    /// Explicit Wayland App ID or WM_CLASS override
    #[arg(short = 'c', long)]
    pub class: String,

    /// Target screen coordinates [X, Y]
    #[arg(
        short,
        long,
        num_args = 2,
        value_names = ["X", "Y"],
        allow_hyphen_values = true
    )]
    pub pos: Vec<i32>,
}

#[derive(Args, Debug, Clone)]
pub struct SpawnArgs {
    /// Absolute target screen coordinates [X, Y]
    #[arg(
        short,
        long,
        num_args = 2,
        value_names = ["X", "Y"],
        conflicts_with = "offset",
        allow_hyphen_values = true
    )]
    pub pos: Option<Vec<i32>>,

    /// Relative offset from mouse cursor [X, Y] (defaults to 0 0 if no pos is given)
    #[arg(
        short,
        long,
        num_args = 2,
        value_names = ["X", "Y"],
        conflicts_with = "pos",
        allow_hyphen_values = true
    )]
    pub offset: Option<Vec<i32>>,

    /// Target window dimensions [WIDTH, HEIGHT]
    #[arg(
        short,
        long,
        num_args = 2,
        value_names = ["WIDTH", "HEIGHT"],
        allow_hyphen_values = true
    )]
    pub size: Option<Vec<i32>>,

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

impl SpawnArgs {
    pub fn validate(&self) -> Result<(), String> {
        if let Some(ref s) = self.size {
            if s.len() != 2 || s[0] <= 0 || s[1] <= 0 {
                return Err(
                    "Invalid size: width and height must be strictly positive integers (> 0)."
                        .to_string(),
                );
            }
        }

        if self.margin.map_or(false, |m| m < 0)
            || self.bound_top.map_or(false, |m| m < 0)
            || self.bound_bottom.map_or(false, |m| m < 0)
            || self.bound_left.map_or(false, |m| m < 0)
            || self.bound_right.map_or(false, |m| m < 0)
        {
            return Err(
                "Invalid margin or boundary constraint: values cannot be negative.".to_string(),
            );
        }

        Ok(())
    }
}

#[derive(Args, Debug, Clone)]
pub struct TransformArgs {
    // Selectors (at least one required)
    #[arg(short = 'c', long)]
    pub class: Option<String>,

    #[arg(short = 't', long)]
    pub title: Option<String>,

    #[arg(long)]
    pub pid: Option<u32>,

    #[arg(long)]
    pub focused: bool,

    // Positioning
    #[arg(
        short = 'p',
        long,
        num_args = 2,
        value_names = ["X", "Y"],
        allow_hyphen_values = true
    )]
    pub pos: Option<Vec<i32>>,

    #[arg(short = 'a', long, value_enum)]
    pub anchor: Option<Anchor>,

    #[arg(long)]
    pub cursor: bool,

    #[arg(long, value_enum, default_value_t = Pivot::TopLeft)]
    pub pivot: Pivot,

    // Sizing
    #[arg(
        short = 's',
        long,
        num_args = 2,
        value_names = ["W", "H"],
        allow_hyphen_values = true
    )]
    pub size: Option<Vec<i32>>,

    // Spatial Context & Clamping
    #[arg(short = 'm', long, default_value = "primary")]
    pub monitor: String,

    #[arg(long, default_value_t = 16)]
    pub margin: i32,

    #[arg(
        long,
        default_value = "true",
        default_missing_value = "true",
        num_args = 0..=1,
        action = clap::ArgAction::Set
    )]
    pub clamp: bool,

    // Focus Management
    #[command(flatten)]
    pub focus_modifiers: FocusModifierArgs,
}

impl Default for TransformArgs {
    fn default() -> Self {
        Self {
            class: None,
            title: None,
            pid: None,
            focused: false,
            pos: None,
            anchor: None,
            cursor: false,
            pivot: Pivot::TopLeft,
            size: None,
            monitor: "primary".to_string(),
            margin: 16,
            clamp: true,
            focus_modifiers: FocusModifierArgs::default(),
        }
    }
}

impl TransformArgs {
    pub fn validate(&self) -> Result<(), String> {
        if self.class.is_none() && self.title.is_none() && self.pid.is_none() && !self.focused {
            return Err(
                "At least one window selector must be specified: --class (-c), --title (-t), --pid, or --focused".to_string(),
            );
        }

        if let Some(ref s) = self.size {
            if s.len() != 2 || s[0] <= 0 || s[1] <= 0 {
                return Err(
                    "Invalid size: width and height must be strictly positive integers (> 0). Got negative or zero dimensions.".to_string(),
                );
            }
        }

        if self.anchor.is_some() && self.pos.is_some() {
            return Err(
                "Conflicting arguments: Cannot specify both --pos and --anchor.".to_string(),
            );
        }

        if self.anchor.is_some() && self.cursor {
            return Err(
                "Conflicting arguments: Cannot specify both --cursor and --anchor.".to_string(),
            );
        }

        if self.anchor.is_some() && self.pivot != Pivot::TopLeft {
            return Err(
                "Conflicting arguments: Cannot specify both --anchor and --pivot.".to_string(),
            );
        }

        if self.margin < 0 {
            return Err("Invalid margin: margin cannot be negative.".to_string());
        }

        Ok(())
    }
}

/// Mutually exclusive focus modifier arguments for window action commands.
#[derive(Debug, Args, Clone, Default, PartialEq, Eq)]
pub struct FocusModifierArgs {
    /// Explicitly focus the target window (default behavior for action commands)
    #[arg(long, conflicts_with_all = ["no_focus", "defocus"])]
    pub focus: bool,

    /// Passive: do not alter focus state; leave active input where it currently is
    #[arg(long, conflicts_with_all = ["focus", "defocus"])]
    pub no_focus: bool,

    /// Active: forcibly strip/yield focus from the target window to the previous window
    #[arg(long, conflicts_with_all = ["focus", "no_focus"])]
    pub defocus: bool,
}

/// Arguments targeting a window selector for focus, state management, or query operations.
#[derive(Args, Debug, Clone, Default, PartialEq, Eq)]
pub struct WindowTargetArgs {
    /// Target window by class / application ID
    #[arg(short = 'c', long)]
    pub class: Option<String>,

    /// Target window by title substring
    #[arg(short = 't', long)]
    pub title: Option<String>,

    /// Target window by process ID (PID)
    #[arg(long)]
    pub pid: Option<u32>,

    /// Target currently focused window
    #[arg(long)]
    pub focused: bool,
}

impl WindowTargetArgs {
    pub fn validate(&self) -> Result<(), String> {
        if self.class.is_none() && self.title.is_none() && self.pid.is_none() && !self.focused {
            return Err(
                "At least one window selector must be specified: --class (-c), --title (-t), --pid, or --focused".to_string(),
            );
        }
        Ok(())
    }
}

impl From<&WindowTargetArgs> for crate::target::WindowSelector {
    fn from(args: &WindowTargetArgs) -> Self {
        Self {
            class: args.class.clone(),
            title: args.title.clone(),
            pid: args.pid,
            focused: args.focused,
        }
    }
}

impl From<WindowTargetArgs> for crate::target::WindowSelector {
    fn from(args: WindowTargetArgs) -> Self {
        Self {
            class: args.class,
            title: args.title,
            pid: args.pid,
            focused: args.focused,
        }
    }
}

/// Arguments for maximizing a target window with focus policies.
#[derive(Args, Debug, Clone, Default, PartialEq, Eq)]
pub struct MaximizeArgs {
    #[command(flatten)]
    pub target: WindowTargetArgs,

    #[command(flatten)]
    pub focus_modifiers: FocusModifierArgs,
}

impl MaximizeArgs {
    pub fn validate(&self) -> Result<(), String> {
        self.target.validate()
    }
}

impl From<&MaximizeArgs> for crate::target::WindowSelector {
    fn from(args: &MaximizeArgs) -> Self {
        (&args.target).into()
    }
}

/// Arguments for restoring a target window with focus policies.
#[derive(Args, Debug, Clone, Default, PartialEq, Eq)]
pub struct RestoreArgs {
    #[command(flatten)]
    pub target: WindowTargetArgs,

    #[command(flatten)]
    pub focus_modifiers: FocusModifierArgs,
}

impl RestoreArgs {
    pub fn validate(&self) -> Result<(), String> {
        self.target.validate()
    }
}

impl From<&RestoreArgs> for crate::target::WindowSelector {
    fn from(args: &RestoreArgs) -> Self {
        (&args.target).into()
    }
}

/// Arguments for minimizing a target window.
#[derive(Args, Debug, Clone, Default, PartialEq, Eq)]
pub struct MinimizeArgs {
    #[command(flatten)]
    pub target: WindowTargetArgs,
}

impl MinimizeArgs {
    pub fn validate(&self) -> Result<(), String> {
        self.target.validate()
    }
}

impl From<&MinimizeArgs> for crate::target::WindowSelector {
    fn from(args: &MinimizeArgs) -> Self {
        (&args.target).into()
    }
}

/// Arguments for defocusing a window with optional target selectors and handover destination.
#[derive(Args, Debug, Clone, Default, PartialEq, Eq)]
pub struct DefocusArgs {
    #[command(flatten)]
    pub target: WindowTargetArgs,

    /// Destination for focus relinquishment ('prev' \[default\] or 'desktop')
    #[arg(long, default_value = "prev", value_parser = ["prev", "desktop"])]
    pub to: String,
}

impl DefocusArgs {
    pub fn validate(&self) -> Result<(), String> {
        if self.to != "prev" && self.to != "desktop" {
            return Err("Invalid --to destination: expected 'prev' or 'desktop'.".to_string());
        }
        Ok(())
    }

    pub fn get_selector(&self) -> crate::target::WindowSelector {
        if self.target.class.is_none()
            && self.target.title.is_none()
            && self.target.pid.is_none()
            && !self.target.focused
        {
            // Default to targeting the currently focused window if no explicit selector is specified
            crate::target::WindowSelector {
                focused: true,
                ..Default::default()
            }
        } else {
            crate::target::WindowSelector {
                class: self.target.class.clone(),
                title: self.target.title.clone(),
                pid: self.target.pid,
                focused: self.target.focused,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::{CommandFactory, Parser};

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
            assert_eq!(t_args.pos, Some(vec![-500, -200]));
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
            assert_eq!(t_args.size, Some(vec![-100, 200]));
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
        let args_default = vec![
            "spawn-at",
            "transform",
            "--class",
            "org.gnome.Calculator",
        ];
        let cli = Cli::try_parse_from(args_default).expect("Parsing should succeed");
        if let Commands::Transform(t_args) = cli.command {
            assert!(t_args.clamp);
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
            assert!(t_args.clamp);
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
            assert!(!t_args.clamp);
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
            assert_eq!(t_args.pivot, Pivot::Center);
            assert!(t_args.validate().is_ok());
        } else {
            panic!("Expected Transform command variant");
        }

        // Anchor + Pivot conflict
        let args_conflict = vec![
            "spawn-at",
            "transform",
            "--class",
            "org.gnome.Calculator",
            "--anchor",
            "top-left",
            "--pivot",
            "center",
        ];
        let cli = Cli::try_parse_from(args_conflict).expect("Parsing should succeed");
        if let Commands::Transform(t_args) = cli.command {
            let err = t_args.validate().unwrap_err();
            assert!(err.contains("Conflicting arguments: Cannot specify both --anchor and --pivot."));
        } else {
            panic!("Expected Transform command variant");
        }

        // Anchor + Cursor conflict
        let args_cursor_conflict = vec![
            "spawn-at",
            "transform",
            "--class",
            "org.gnome.Calculator",
            "--anchor",
            "top-left",
            "--cursor",
        ];
        let cli = Cli::try_parse_from(args_cursor_conflict).expect("Parsing should succeed");
        if let Commands::Transform(t_args) = cli.command {
            let err = t_args.validate().unwrap_err();
            assert!(err.contains("Conflicting arguments: Cannot specify both --cursor and --anchor."));
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
        assert!(matches!(cli.command, Commands::Focus(args) if args.title.as_deref() == Some("editor")));

        let cli = Cli::try_parse_from(["spawn-at", "activate", "--pid", "1234"]).unwrap();
        assert!(matches!(cli.command, Commands::Focus(args) if args.pid == Some(1234)));

        // defocus with aliases and --to
        let cli = Cli::try_parse_from(["spawn-at", "defocus", "--focused"]).unwrap();
        if let Commands::Defocus(args) = cli.command {
            assert!(args.target.focused);
            assert_eq!(args.to, "prev");
        } else {
            panic!("Expected Defocus variant");
        }

        let cli = Cli::try_parse_from(["spawn-at", "unfocus", "--to", "desktop"]).unwrap();
        if let Commands::Defocus(args) = cli.command {
            assert_eq!(args.to, "desktop");
            // Default selector targets focused window
            let sel = args.get_selector();
            assert!(sel.focused);
        } else {
            panic!("Expected Defocus variant");
        }

        let cli = Cli::try_parse_from(["spawn-at", "blur", "-c", "code"]).unwrap();
        if let Commands::Defocus(args) = cli.command {
            assert_eq!(args.target.class.as_deref(), Some("code"));
            assert_eq!(args.to, "prev");
        } else {
            panic!("Expected Defocus variant");
        }

        // maximize with British alias 'maximise' and focus modifiers
        let cli = Cli::try_parse_from(["spawn-at", "maximize", "-c", "terminal", "--no-focus"]).unwrap();
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
        assert!(matches!(cli.command, Commands::Minimize(ref args) if args.target.class.as_deref() == Some("terminal")));

        let cli = Cli::try_parse_from(["spawn-at", "minimise", "-c", "terminal"]).unwrap();
        assert!(matches!(cli.command, Commands::Minimize(ref args) if args.target.class.as_deref() == Some("terminal")));

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

        let cli = Cli::try_parse_from(["spawn-at", "unmaximize", "-c", "terminal", "--no-focus"]).unwrap();
        if let Commands::Restore(args) = cli.command {
            assert_eq!(args.target.class.as_deref(), Some("terminal"));
            assert!(args.focus_modifiers.no_focus);
        } else {
            panic!("Expected Restore variant via unmaximize");
        }

        let cli = Cli::try_parse_from(["spawn-at", "unmaximise", "-c", "terminal"]).unwrap();
        assert!(matches!(cli.command, Commands::Restore(ref args) if args.target.class.as_deref() == Some("terminal")));

        let cli = Cli::try_parse_from(["spawn-at", "unminimize", "-c", "terminal"]).unwrap();
        assert!(matches!(cli.command, Commands::Restore(ref args) if args.target.class.as_deref() == Some("terminal")));

        let cli = Cli::try_parse_from(["spawn-at", "unminimise", "-c", "terminal"]).unwrap();
        assert!(matches!(cli.command, Commands::Restore(ref args) if args.target.class.as_deref() == Some("terminal")));

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

        let focused_args = WindowTargetArgs {
            focused: true,
            ..Default::default()
        };
        assert!(focused_args.validate().is_ok());
    }

    #[test]
    fn test_clap_parser_configuration() {
        Cli::command().debug_assert();
    }
}
