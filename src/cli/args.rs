//! # CLI Argument Structs and Definitions

use clap::{Args, Subcommand};
use spawn_at_core::geometry::{Anchor, Area, Pivot};

#[derive(Subcommand, Debug, Clone, PartialEq, Eq)]
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

/// Shared spatial geometry and layout arguments for positioning windows.
#[derive(Args, Debug, Clone, Default, PartialEq, Eq)]
pub struct GeometryArgs {
    /// Screen anchor target or cursor
    #[arg(long, value_enum)]
    pub anchor: Option<Anchor>,

    /// Window alignment pivot point relative to target
    #[arg(long, value_enum)]
    pub pivot: Option<Pivot>,

    /// Absolute target screen coordinates [X, Y]
    #[arg(
        short = 'p',
        long,
        num_args = 2,
        value_names = ["X", "Y"],
        allow_hyphen_values = true
    )]
    pub pos: Option<Vec<i32>>,

    /// Target window dimensions [W, H]
    #[arg(
        short = 's',
        long,
        num_args = 2,
        value_names = ["W", "H"],
        allow_hyphen_values = true
    )]
    pub size: Option<Vec<String>>,

    /// Target monitor selector (<INDEX>, 'cursor', or 'primary')
    #[arg(long, default_value = "primary")]
    pub monitor: String,

    /// Boundary reference area ('workarea' or 'screen')
    #[arg(long, value_enum, default_value_t = Area::Workarea)]
    pub area: Area,

    /// Universal margin in pixels applied to all boundaries
    #[arg(short = 'm', long, default_value_t = 16)]
    pub margin: i32,

    /// Top boundary margin in pixels
    #[arg(long = "margin-top", alias = "mt")]
    pub margin_top: Option<i32>,

    /// Bottom boundary margin in pixels
    #[arg(long = "margin-bottom", alias = "mb")]
    pub margin_bottom: Option<i32>,

    /// Left boundary margin in pixels
    #[arg(long = "margin-left", alias = "ml")]
    pub margin_left: Option<i32>,

    /// Right boundary margin in pixels
    #[arg(long = "margin-right", alias = "mr")]
    pub margin_right: Option<i32>,

    /// Constrain window strictly within monitor/workarea bounds
    #[arg(
        long,
        default_value = "true",
        default_missing_value = "true",
        num_args = 0..=1,
        action = clap::ArgAction::Set
    )]
    pub clamp: bool,
}

impl GeometryArgs {
    pub fn validate(&self) -> Result<(), String> {
        if let Some(ref s) = self.size {
            if s.len() != 2 {
                return Err(
                    "Invalid size: must specify exactly 2 values [WIDTH, HEIGHT].".to_string(),
                );
            }
            for val_str in s {
                if let Ok(val) = val_str.parse::<i32>() {
                    if val <= 0 {
                        return Err(
                            "Invalid size: width and height must be strictly positive integers (> 0). Got negative or zero dimensions.".to_string(),
                        );
                    }
                }
            }
        }

        if self.anchor.is_some() && self.pos.is_some() {
            return Err(
                "Conflicting arguments: Cannot specify both --pos and --anchor.".to_string(),
            );
        }

        Ok(())
    }
}

#[derive(Args, Debug, Clone)]
pub struct SpawnArgs {
    /// Explicit Wayland App ID or WM_CLASS override (e.g. org.gnome.TextEditor or '*')
    #[arg(short = 'c', long)]
    pub class: Option<String>,

    #[command(flatten)]
    pub geometry: GeometryArgs,

    #[command(flatten)]
    pub focus_modifiers: FocusModifierArgs,

    /// Command to spawn along with any trailing flags/arguments
    #[arg(trailing_var_arg = true, allow_hyphen_values = true, required = true)]
    pub command: Vec<String>,
}

impl SpawnArgs {
    pub fn validate(&self) -> Result<(), String> {
        self.geometry.validate()
    }
}

#[derive(Args, Debug, Clone, Default, PartialEq, Eq)]
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

    #[command(flatten)]
    pub geometry: GeometryArgs,

    // Focus Management
    #[command(flatten)]
    pub focus_modifiers: FocusModifierArgs,
}

impl TransformArgs {
    pub fn validate(&self) -> Result<(), String> {
        if self.class.is_none() && self.title.is_none() && self.pid.is_none() && !self.focused {
            return Err(
                "At least one window selector must be specified: --class (-c), --title (-t), --pid, or --focused".to_string(),
            );
        }

        self.geometry.validate()?;

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

    /// Yield focus to the desktop/shell
    #[arg(long = "to-desktop", alias = "desktop", conflicts_with = "to_window")]
    pub to_desktop: bool,

    /// Transfer focus to a specific window by title, class, or id
    #[arg(long = "to-window", alias = "to", conflicts_with = "to_desktop")]
    pub to_window: Option<String>,
}

impl DefocusArgs {
    pub fn validate(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn mode_and_destination(&self) -> (&'static str, &str) {
        if self.to_desktop {
            ("desktop", "")
        } else if let Some(ref win) = self.to_window {
            ("window", win.as_str())
        } else {
            ("mru", "")
        }
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
