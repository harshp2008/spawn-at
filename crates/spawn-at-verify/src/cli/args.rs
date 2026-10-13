//! CLI argument parser schemas and subcommand definitions.

use super::smoke_eval;
use crate::schema::TestKind;
use clap::{Args, Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

/// Root CLI parser for spawn-at-verify.
#[derive(Parser, Debug)]
#[command(
    name = "spawn-at-verify",
    about = "Verification Engine & Test Harness for spawn-at",
    version = "0.1.0"
)]
pub struct Cli {
    /// Selected CLI subcommand to execute.
    #[command(subcommand)]
    pub command: Commands,
}

/// Available top-level subcommands.
#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Execute automated verification tests
    Run(RunArgs),
    /// List available tests and suites formatted as a tree with counts
    List(ListArgs),
    /// Validate test definitions and suite schemas
    Check(CheckArgs),
    /// Re-render or inspect past verification reports
    Report(ReportArgs),
    /// Interactive Terminal UI for test execution and human review
    Tui(TuiArgs),
    /// Measure one case's cloak duration over N separate processes and N --repeat runs
    Calibrate(CalibrateArgs),
    /// Print the TUI detail drawer for a recorded real case (headless)
    Render(RenderArgs),
    /// Internal helper for smoke-live.sh JSON evaluation and placement verification
    #[command(subcommand, hide = true)]
    SmokeEval(smoke_eval::SmokeEvalCommand),
}

/// Filter test kind CLI argument.
#[derive(Clone, Copy, Debug, ValueEnum)]
#[allow(missing_docs)]
pub enum KindArg {
    Auto,
    Human,
    Both,
}

impl From<KindArg> for TestKind {
    /// Converts CLI test kind parameter into schema TestKind model.
    fn from(k: KindArg) -> Self {
        match k {
            KindArg::Auto => TestKind::Auto,
            KindArg::Human => TestKind::Human,
            KindArg::Both => TestKind::Both,
        }
    }
}

/// Arguments for the `run` subcommand.
#[derive(Args, Debug)]
#[command(after_help = super::run_summary::EXIT_CODE_HELP)]
pub struct RunArgs {
    /// Path to spawn-at binary under test [default: target/debug/spawn-at]
    #[arg(long)]
    pub bin: Option<PathBuf>,

    /// Path to verify test library directory [default: auto-detected verify/]
    #[arg(long)]
    pub dir: Option<PathBuf>,

    /// Run a predefined test suite (e.g. quick, release-gate, visual)
    #[arg(long)]
    pub suite: Option<String>,

    /// Select by category path prefix (e.g. placement/, cloak/no-flash)
    #[arg(long)]
    pub category: Option<String>,

    /// Select by tag (can be specified multiple times)
    #[arg(long = "tag")]
    pub tags: Vec<String>,

    /// Select by test kind (auto, human, both)
    #[arg(long)]
    pub kind: Option<KindArg>,

    /// Select required tests only
    #[arg(long)]
    pub required: bool,

    /// Filter tests by ID glob pattern (e.g. *bottom*, placement/*)
    #[arg(long)]
    pub glob: Option<String>,

    /// Legacy filter argument (alias for glob)
    #[arg(long)]
    pub filter: Option<String>,

    /// Repeat each test N times (cap 100) for flakiness detection. report.json keeps the first
    /// run in `results` (marked Flaky if outcomes differed) and stores all N runs under
    /// `repeat_runs`
    #[arg(long, default_value = "1")]
    pub repeat: usize,

    /// Allow verification against a dirty build
    #[arg(long)]
    pub allow_dirty: bool,

    /// Dry run: list planned tests without executing actions
    #[arg(long)]
    pub dry_run: bool,

    /// Include real window titles in reports (opt out of default redaction)
    #[arg(long)]
    pub include_titles: bool,

    /// Output results in JSON format
    #[arg(long)]
    pub json: bool,

    /// Fail a test that passed on placement but flashed (the window was shown before the
    /// fixture painted its final size, or its size or position changed after the reveal).
    /// Without it such a test is reported as PASS (flash) and the exit code is unchanged
    #[arg(long)]
    pub strict_flash: bool,

    /// Raise the floor under which a late paint is not counted as a flash, in milliseconds. The
    /// floor is never lower than the measured clock uncertainty (about 2 ms)
    #[arg(long, default_value_t = 0.0)]
    pub flash_threshold_ms: f64,
}

/// Arguments for the `list` subcommand.
#[derive(Args, Debug)]
pub struct ListArgs {
    /// Path to verify test library directory [default: auto-detected verify/]
    #[arg(long)]
    pub dir: Option<PathBuf>,

    /// Filter by suite name
    #[arg(long)]
    pub suite: Option<String>,

    /// Select by category path prefix (e.g. placement/, cloak/no-flash)
    #[arg(long)]
    pub category: Option<String>,

    /// Filter by tag (can be specified multiple times)
    #[arg(long = "tag")]
    pub tags: Vec<String>,

    /// Filter by test kind (auto, human, both)
    #[arg(long)]
    pub kind: Option<KindArg>,

    /// Filter required tests only
    #[arg(long)]
    pub required: bool,

    /// Filter by ID glob pattern
    #[arg(long)]
    pub glob: Option<String>,

    /// Output list in raw JSON format
    #[arg(long)]
    pub json: bool,
}

/// Arguments for the `check` subcommand.
#[derive(Args, Debug)]
pub struct CheckArgs {
    /// Path to verify test library directory [default: auto-detected verify/]
    #[arg(long)]
    pub dir: Option<PathBuf>,
}

/// Arguments for the `report` subcommand.
#[derive(Args, Debug)]
pub struct ReportArgs {
    /// Path to session report directory or report.json [default: latest in verify-results/]
    pub path: Option<PathBuf>,
}

/// Arguments for the `render` subcommand.
#[derive(Args, Debug)]
pub struct RenderArgs {
    /// Recorded case to render: an exact test id or a suffix that is unique (e.g. delay-500)
    #[arg(long)]
    pub case: Option<String>,

    /// List the recorded cases that can be rendered and exit
    #[arg(long)]
    pub list: bool,

    /// Terminal width in columns (minimum 60)
    #[arg(long, default_value = "80")]
    pub width: u16,

    /// Terminal height in rows (minimum 15)
    #[arg(long, default_value = "60")]
    pub height: u16,

    /// Drawer scroll offset in rows
    #[arg(long, default_value = "0")]
    pub scroll: u16,

    /// Path to verify test library directory [default: auto-detected verify/]
    #[arg(long)]
    pub dir: Option<PathBuf>,
}

/// Which measurement mode(s) `calibrate` runs.
#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum CalibrateMode {
    /// N separate `run` processes, then N runs with `--repeat` in one process.
    Both,
    /// Only N separate `run` processes.
    Separate,
    /// Only N runs with `--repeat` in one process.
    Repeat,
}

/// Arguments for the `calibrate` subcommand.
#[derive(Args, Debug)]
pub struct CalibrateArgs {
    /// Exact test id to measure (e.g. cloak/delayed-paint/delay-500)
    #[arg(long)]
    pub case: String,

    /// Runs per mode, capped at 100
    #[arg(long, default_value = "10")]
    pub runs: usize,

    /// Path to spawn-at binary under test [default: target/debug/spawn-at]
    #[arg(long)]
    pub bin: Option<PathBuf>,

    /// Path to verify test library directory [default: auto-detected verify/]
    #[arg(long)]
    pub dir: Option<PathBuf>,

    /// Which mode(s) to measure: both, separate, or repeat
    #[arg(long, value_enum, default_value = "both")]
    pub mode: CalibrateMode,

    /// Allow measuring a dirty build
    #[arg(long)]
    pub allow_dirty: bool,
}

/// Arguments for the `tui` subcommand.
#[derive(Args, Debug)]
pub struct TuiArgs {
    /// Path to spawn-at binary under test [default: target/debug/spawn-at]
    #[arg(long)]
    pub bin: Option<PathBuf>,

    /// Path to verify test library directory [default: auto-detected verify/]
    #[arg(long)]
    pub dir: Option<PathBuf>,

    /// Run a predefined test suite (e.g. quick, release-gate, visual)
    #[arg(long)]
    pub suite: Option<String>,

    /// Select by category path prefix
    #[arg(long)]
    pub category: Option<String>,

    /// Select by tag (can be specified multiple times)
    #[arg(long = "tag")]
    pub tags: Vec<String>,
}
