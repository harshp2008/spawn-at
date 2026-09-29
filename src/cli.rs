//! # Command Line Interface Definitions
//!
//! Defines the CLI arguments and subcommands for `spawn-at`.

use crate::drivers::{InstallArgs, UninstallArgs};
use crate::geom::Anchor;
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
    /// Background daemon to enforce window constraints
    Daemon,
    /// Query active state from the compositor
    Query {
        #[command(subcommand)]
        cmd: QueryCommands,
    },
    /// Move an existing window by its class identifier
    Move(MoveArgs),
}

#[derive(Subcommand, Debug)]
pub enum QueryCommands {
    /// Fetch and print JSON layout of all work areas
    Layout,
    /// Fetch and print current pointer coordinates
    Pointer,
    /// Fetch and print JSON list of all open windows with metadata
    Windows,
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

    #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
    pub clamp: bool,
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

        if self.margin < 0 {
            return Err("Invalid margin: margin cannot be negative.".to_string());
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

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
}
