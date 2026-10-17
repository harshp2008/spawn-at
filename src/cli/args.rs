//! # CLI Argument Structs and Definitions

// Clap-derived structs and enums for the subcommands, plus their `validate` checks that clap
// cannot express. The `///` lines on structs, enums, fields and variants in this file are the
// `--help` text, so they are the contract and are not edited in a comment-only change; notes
// here are plain `//`. The `validate` methods only check the arguments and print nothing.
// `Cli::validate` (src/cli/mod.rs) calls them for spawn, transform, focus, defocus, maximize,
// minimize, restore and close; `main` prints the `Err` text and exits with status 2 (main.rs).
// install, uninstall, config, update and query have no check here. No code in this file talks
// to a backend or touches the file system.

use clap::{Args, Subcommand};
use spawn_at_core::geometry::{Anchor, Area, Pivot};

#[derive(Subcommand, Debug, Clone, PartialEq, Eq)]
pub enum QueryCommands {
    /// Fetch and print layout of monitors, work areas, and insets
    Layout {
        /// Output raw JSON instead of human-readable table
        #[arg(long)]
        json: bool,
    },
    /// Fetch and print current pointer coordinates
    Pointer,
    /// Fetch and print list of all open windows with metadata.
    ///
    /// GEOMETRY is reported in screen coordinates.
    /// REL is the position relative to the monitor's work area (default), or relative to the area chosen by --area (screen or unbounded; unbounded repeats GEOMETRY position).
    Windows {
        /// Output raw JSON instead of human-readable table
        #[arg(long)]
        json: bool,

        /// Coordinate space for REL column (workarea, screen, or unbounded)
        #[arg(long, value_enum, default_value = "workarea")]
        area: Area,
    },
}

/// Shared spatial geometry and layout arguments for positioning windows.
#[derive(Args, Debug, Clone, Default, PartialEq, Eq)]
pub struct GeometryArgs {
    /// Boundary reference area ('workarea', 'screen', or 'unbounded')
    #[arg(
        long,
        value_enum,
        default_value_t = Area::Workarea,
        help_heading = "Reference area"
    )]
    pub area: Area,

    /// Target monitor selector (<INDEX>, 'cursor', or 'primary')
    #[arg(long, help_heading = "Reference area")]
    pub monitor: Option<String>,

    /// Screen anchor target or cursor (defaults to top-left pivot when --pivot is omitted)
    #[arg(long, value_enum, help_heading = "Anchor and pivot")]
    pub anchor: Option<Anchor>,

    /// Window alignment pivot point relative to target
    #[arg(long, value_enum, help_heading = "Anchor and pivot")]
    pub pivot: Option<Pivot>,

    /// Absolute target screen coordinates [X, Y], or anchor offset when --anchor is specified
    #[arg(
        short = 'p',
        long,
        num_args = 2,
        value_names = ["X", "Y"],
        allow_hyphen_values = true,
        action = clap::ArgAction::Append,
        help_heading = "Position and size"
    )]
    pub pos: Option<Vec<i32>>,

    /// Absolute target screen coordinates [X, Y], or anchor offset when --anchor is specified (alias for --pos / -p)
    #[arg(
        short = 'o',
        long,
        num_args = 2,
        value_names = ["X", "Y"],
        allow_hyphen_values = true,
        action = clap::ArgAction::Append,
        help_heading = "Position and size"
    )]
    pub offset: Option<Vec<i32>>,

    /// Target window dimensions [W, H]
    #[arg(
        short = 's',
        long,
        num_args = 2,
        value_names = ["W", "H"],
        allow_hyphen_values = true,
        help_heading = "Position and size"
    )]
    pub size: Option<Vec<String>>,

    /// Keep window strictly inside the chosen area minus the margin (--clamp false allows absolute coordinates outside)
    #[arg(
        long,
        default_missing_value = "true",
        num_args = 0..=1,
        action = clap::ArgAction::Set,
        help_heading = "Bounds"
    )]
    pub clamp: Option<bool>,

    /// Universal margin in pixels applied to all boundaries
    #[arg(short = 'm', long, help_heading = "Bounds")]
    pub margin: Option<i32>,

    /// Top boundary margin in pixels
    #[arg(long = "margin-top", alias = "mt", help_heading = "Bounds")]
    pub margin_top: Option<i32>,

    /// Bottom boundary margin in pixels
    #[arg(long = "margin-bottom", alias = "mb", help_heading = "Bounds")]
    pub margin_bottom: Option<i32>,

    /// Left boundary margin in pixels
    #[arg(long = "margin-left", alias = "ml", help_heading = "Bounds")]
    pub margin_left: Option<i32>,

    /// Right boundary margin in pixels
    #[arg(long = "margin-right", alias = "mr", help_heading = "Bounds")]
    pub margin_right: Option<i32>,
}

impl GeometryArgs {
    // The effective clamp: always false for `--area unbounded`, otherwise `--clamp` if given,
    // else true.
    pub fn clamp(&self) -> bool {
        if self.area == Area::Unbounded {
            false
        } else {
            self.clamp.unwrap_or(true)
        }
    }

    // The effective universal margin in pixels: 0 for `--area unbounded`, otherwise `--margin`
    // if given, else 16. The per-side margins are not looked at here; transform.rs falls back to
    // this value for a side without its own margin.
    pub fn margin(&self) -> i32 {
        if self.area == Area::Unbounded {
            0
        } else {
            self.margin.unwrap_or(16)
        }
    }

    // Rejects: with `--area unbounded`, `--monitor`, `--clamp`, any margin flag and any `--anchor`
    // except `cursor`; a `--size` without exactly 2 values or with a value that parses as an
    // integer <= 0 (a value that does not parse as an integer is not rejected here); `--pos`
    // together with `--offset`; and `--pos` or `--offset` given more than once (more than 2
    // values collected).
    pub fn validate(&self) -> Result<(), String> {
        if self.area == Area::Unbounded {
            if self.monitor.is_some() {
                return Err(
                    "Cannot specify --monitor with --area unbounded: unbounded mode uses global desktop coordinates.".to_string(),
                );
            }
            if self.clamp.is_some() {
                return Err(
                    "Cannot specify --clamp with --area unbounded: unbounded mode is never clamped.".to_string(),
                );
            }
            if self.margin.is_some()
                || self.margin_top.is_some()
                || self.margin_bottom.is_some()
                || self.margin_left.is_some()
                || self.margin_right.is_some()
            {
                return Err(
                    "Cannot specify margins (-m, --mt, --mb, --ml, --mr) with --area unbounded: unbounded mode has no margins.".to_string(),
                );
            }
            if let Some(anchor) = self.anchor {
                if anchor != Anchor::Cursor {
                    return Err(
                        "Cannot specify --anchor (except --anchor cursor) with --area unbounded."
                            .to_string(),
                    );
                }
            }
        }

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

        if self.pos.is_some() && self.offset.is_some() {
            return Err(
                "Conflicting arguments: Cannot specify both --pos (-p) and --offset (-o); they are aliases for the same option.".to_string(),
            );
        }

        if let Some(ref p) = self.pos {
            if p.len() > 2 {
                return Err("Cannot specify --pos (-p) multiple times.".to_string());
            }
        }

        if let Some(ref o) = self.offset {
            if o.len() > 2 {
                return Err("Cannot specify --offset (-o) multiple times.".to_string());
            }
        }

        Ok(())
    }

    /// Returns resolved (X, Y) coordinate pair from either --pos or --offset if specified.
    pub fn pos_or_offset(&self) -> Option<(i32, i32)> {
        if let Some(ref p) = self.pos {
            if p.len() == 2 {
                return Some((p[0], p[1]));
            }
        }
        if let Some(ref o) = self.offset {
            if o.len() == 2 {
                return Some((o[0], o[1]));
            }
        }
        None
    }
}

#[derive(Args, Debug, Clone, Default)]
pub struct SpawnArgs {
    #[command(flatten)]
    pub geometry: GeometryArgs,

    #[command(flatten)]
    pub focus_modifiers: FocusModifierArgs,

    /// Output single structured JSON object on stdout upon mapping
    #[arg(long, help_heading = "Behavior", display_order = 4)]
    pub json: bool,

    /// Explicit Wayland App ID or WM_CLASS override (e.g. org.gnome.TextEditor or '*')
    #[arg(short = 'c', long)]
    pub class: Option<String>,

    /// Developer switch for same-login A/B runs, hidden from --help: which rule waits for the
    /// resized window (auto, legacy or ready-wait), for this spawn only. Kept as text so a bad
    /// value can be ignored with a warning instead of failing the spawn.
    #[arg(long, hide = true, value_name = "auto|legacy|ready-wait")]
    pub wait_strategy: Option<String>,

    /// Developer switch, hidden from --help: cap of the ready-wait in milliseconds (1 to 1500),
    /// for this spawn only. Kept as text for the same reason as --wait-strategy.
    #[arg(long, hide = true, value_name = "MS")]
    pub wait_cap_ms: Option<String>,

    /// Command to spawn along with any trailing flags/arguments
    #[arg(
        trailing_var_arg = true,
        allow_hyphen_values = true,
        required = true,
        help_heading = "Command"
    )]
    pub command: Vec<String>,

    /// Whether the command was preceded by '--' in the raw invocation
    #[arg(skip)]
    pub after_double_dash: bool,
}

// Edit distance between `a` and `b` counted in bytes (not characters), so a non-ASCII character
// counts as several edits. Insertion, deletion and substitution each cost 1.
fn levenshtein(a: &str, b: &str) -> usize {
    let a_bytes = a.as_bytes();
    let b_bytes = b.as_bytes();
    let mut prev = (0..=b_bytes.len()).collect::<Vec<_>>();
    let mut curr = vec![0; b_bytes.len() + 1];

    for (i, &ca) in a_bytes.iter().enumerate() {
        curr[0] = i + 1;
        for (j, &cb) in b_bytes.iter().enumerate() {
            let cost = if ca == cb { 0 } else { 1 };
            curr[j + 1] = (prev[j + 1] + 1).min(curr[j] + 1).min(prev[j] + cost);
        }
        prev.copy_from_slice(&curr);
    }
    prev[b_bytes.len()]
}

// The "Did you mean" text for a mistyped option in the command position of `spawn`. First a fixed
// table of flags people guess (`--no-clamp` and `--clamp` give `--clamp false`, `--no-margin`
// gives `--margin 0`, `--screen`, `--workarea`, `--cursor`, `--center` and the four corner
// names give the `--area` or `--anchor` form). Otherwise the closest entry of `KNOWN_OPTIONS`
// within edit distance 3, the first one listed on a tie; `None` when none is that close.
// `KNOWN_OPTIONS` does not list `--wait-strategy`, `--wait-cap-ms` or `--help`. The only
// caller found by grep is `SpawnArgs::validate` below.
pub fn find_option_suggestion(token: &str) -> Option<&'static str> {
    match token {
        "--no-clamp" | "-no-clamp" => Some("--clamp false"),
        "--clamp" => Some("--clamp false"),
        "--no-margin" => Some("--margin 0"),
        "--screen" => Some("--area screen"),
        "--workarea" => Some("--area workarea"),
        "--cursor" => Some("--anchor cursor"),
        "--center" => Some("--anchor center"),
        "--top-left" => Some("--anchor top-left"),
        "--top-right" => Some("--anchor top-right"),
        "--bottom-left" => Some("--anchor bottom-left"),
        "--bottom-right" => Some("--anchor bottom-right"),
        _ => {
            const KNOWN_OPTIONS: &[&str] = &[
                "--class",
                "--json",
                "--pos",
                "--offset",
                "--size",
                "--anchor",
                "--pivot",
                "--margin",
                "--margin-top",
                "--margin-bottom",
                "--margin-left",
                "--margin-right",
                "--area",
                "--clamp",
                "--monitor",
                "--focus",
                "--no-focus",
                "--defocus",
                "--no-wait",
            ];
            let mut best_match = None;
            let mut min_dist = usize::MAX;
            for &opt in KNOWN_OPTIONS {
                let dist = levenshtein(token, opt);
                if dist <= 3 && dist < min_dist {
                    min_dist = dist;
                    best_match = Some(opt);
                }
            }
            best_match
        }
    }
}

impl SpawnArgs {
    // The geometry checks, then: a command whose first word starts with `-` is an error ("unknown
    // option ...", plus a suggestion when `find_option_suggestion` has one) unless `--` was in the
    // raw arguments (`after_double_dash`, set by `main` before this runs, never by clap). An empty
    // `command` is not checked here: clap's `required = true` does that.
    pub fn validate(&self) -> Result<(), String> {
        self.geometry.validate()?;

        if let Some(cmd) = self.command.first() {
            if cmd.starts_with('-') && !self.after_double_dash {
                let suggestion = find_option_suggestion(cmd);
                let mut err = format!(
                    "unknown option '{cmd}'; to run a command that starts with '-', put it after --"
                );
                if let Some(sugg) = suggestion {
                    err.push_str(&format!(". Did you mean '{sugg}'?"));
                }
                return Err(err);
            }
        }

        Ok(())
    }
}

#[derive(Args, Debug, Clone, Default, PartialEq, Eq)]
pub struct TransformArgs {
    // Selectors (at least one required)
    /// Target window by numeric window ID (from 'query windows')
    #[arg(long)]
    pub id: Option<u64>,

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

    #[command(flatten)]
    pub geometry: GeometryArgs,

    // Focus Management
    #[command(flatten)]
    pub focus_modifiers: FocusModifierArgs,
}

impl TransformArgs {
    // Needs at least one selector, valid geometry, and at least one of a size, a position or
    // offset, or an anchor. `--pivot` alone does not count as something to change, although the
    // error text mentions "--pivot with --size".
    pub fn validate(&self) -> Result<(), String> {
        if self.id.is_none()
            && self.class.is_none()
            && self.title.is_none()
            && self.pid.is_none()
            && !self.focused
        {
            return Err(
                "At least one window selector must be specified: --id, --class (-c), --title (-t), --pid, or --focused".to_string(),
            );
        }

        self.geometry.validate()?;

        let has_size = self.geometry.size.is_some();
        let has_pos = self.geometry.pos.is_some() || self.geometry.offset.is_some();
        let has_anchor = self.geometry.anchor.is_some();

        if !has_size && !has_pos && !has_anchor {
            return Err(
                "nothing to change: give --size, -p/-o, --anchor, or --pivot with --size"
                    .to_string(),
            );
        }

        Ok(())
    }
}

/// Mutually exclusive focus modifier arguments for window action commands.
#[derive(Debug, Args, Clone, Default, PartialEq, Eq)]
pub struct FocusModifierArgs {
    /// Explicitly focus the target window (default behavior for action commands)
    #[arg(
        long,
        conflicts_with_all = ["no_focus", "defocus"],
        help_heading = "Behavior",
        display_order = 1
    )]
    pub focus: bool,

    /// Passive: do not alter focus state; leave active input where it currently is
    #[arg(
        long,
        conflicts_with_all = ["focus", "defocus"],
        help_heading = "Behavior",
        display_order = 2
    )]
    pub no_focus: bool,

    /// Active: forcibly strip/yield focus from the target window to the previous window
    #[arg(
        long,
        conflicts_with_all = ["focus", "no_focus"],
        help_heading = "Behavior",
        display_order = 3
    )]
    pub defocus: bool,
}

/// Arguments targeting a window selector for focus, state management, or query operations.
#[derive(Args, Debug, Clone, Default, PartialEq, Eq)]
pub struct WindowTargetArgs {
    /// Target window by numeric window ID (from 'query windows')
    #[arg(long)]
    pub id: Option<u64>,

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
    // Needs at least one of `--id`, `--class`, `--title`, `--pid`, `--focused`. The selectors
    // are not checked against each other or against the open windows here.
    pub fn validate(&self) -> Result<(), String> {
        if self.id.is_none()
            && self.class.is_none()
            && self.title.is_none()
            && self.pid.is_none()
            && !self.focused
        {
            return Err(
                "At least one window selector must be specified: --id, --class (-c), --title (-t), --pid, or --focused".to_string(),
            );
        }
        Ok(())
    }
}

impl From<&WindowTargetArgs> for crate::target::WindowSelector {
    fn from(args: &WindowTargetArgs) -> Self {
        Self {
            id: args.id,
            class: args.class.clone(),
            title: args.title.clone(),
            pid: args.pid,
            focused: args.focused,
        }
    }
}

/// Arguments targeting a window for graceful closure.
#[derive(Args, Debug, Clone, Default, PartialEq, Eq)]
pub struct CloseArgs {
    #[command(flatten)]
    pub target: WindowTargetArgs,
}

impl CloseArgs {
    pub fn validate(&self) -> Result<(), String> {
        self.target.validate()
    }
}

impl From<WindowTargetArgs> for crate::target::WindowSelector {
    fn from(args: WindowTargetArgs) -> Self {
        Self {
            id: args.id,
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
    // Always `Ok`: defocus needs no selector (`get_selector` defaults to the focused window), and
    // clap already rejects `--to-desktop` together with `--to-window`.
    pub fn validate(&self) -> Result<(), String> {
        Ok(())
    }

    // The pair handed to `defocus_window`: `("desktop", "")`, `("window", <the value of
    // --to-window>)`, or `("mru", "")` when neither flag is given. What each driver does with
    // "mru" is not verified here.
    pub fn mode_and_destination(&self) -> (&'static str, &str) {
        if self.to_desktop {
            ("desktop", "")
        } else if let Some(ref win) = self.to_window {
            ("window", win.as_str())
        } else {
            ("mru", "")
        }
    }

    // The selector for the window to defocus: the given selectors, or the focused window when none
    // of `--id`, `--class`, `--title`, `--pid`, `--focused` is set.
    pub fn get_selector(&self) -> crate::target::WindowSelector {
        if self.target.id.is_none()
            && self.target.class.is_none()
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
            crate::target::WindowSelector::from(&self.target)
        }
    }
}

/// What `spawn-at config` does; without one it is `show`.
#[derive(Subcommand, Debug, Clone, PartialEq, Eq)]
pub enum ConfigCommands {
    /// Print every setting with its effective value and where the value comes from (default)
    Show,
    /// Print the effective value of one setting
    Get {
        /// Setting name, e.g. wait.cap_ms
        key: String,
    },
    /// Write a setting to the config file (other keys in the file are kept)
    Set {
        /// Setting name, e.g. wait.cap_ms
        key: String,
        /// New value
        value: String,
    },
    /// Remove a setting from the config file so its default applies again
    Unset {
        /// Setting name, e.g. wait.cap_ms
        key: String,
    },
    /// Print the path of the config file
    Path,
    /// Remove every spawn-at setting from the config file (update preferences and unknown keys stay)
    Reset,
}

/// Arguments of `spawn-at config`.
#[derive(Args, Debug, Clone, PartialEq, Eq, Default)]
pub struct ConfigArgs {
    #[command(subcommand)]
    pub action: Option<ConfigCommands>,

    /// Show what a spawn with `--wait-strategy` would use (flag layer for show and get), hidden
    #[arg(long, hide = true, value_name = "auto|legacy|ready-wait")]
    pub wait_strategy: Option<String>,

    /// Show what a spawn with `--wait-cap-ms` would use (flag layer for show and get), hidden
    #[arg(long, hide = true, value_name = "MS")]
    pub wait_cap_ms: Option<String>,
}
