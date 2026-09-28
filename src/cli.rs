use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "spawn-at")]
#[command(about = "Standalone CLI Placement Engine", long_about = None)]
pub struct Cli {
    /// Absolute screen coordinates (mutually exclusive with offset)
    #[arg(
        short,
        long,
        num_args = 2,
        value_names = ["X", "Y"],
        conflicts_with = "offset"
    )]
    pub pos: Option<Vec<i32>>,

    /// Offset relative to mouse cursor (defaults to 0 0 if omitted)
    #[arg(
        short,
        long,
        num_args = 2,
        value_names = ["X", "Y"],
        conflicts_with = "pos"
    )]
    pub offset: Option<Vec<i32>>,

    /// Optional target window size override
    #[arg(short, long, num_args = 2, value_names = ["WIDTH", "HEIGHT"])]
    pub size: Option<Vec<i32>>,

    #[arg(short = 't', long)]
    pub bound_top: Option<i32>,

    #[arg(short = 'b', long)]
    pub bound_bottom: Option<i32>,

    #[arg(short = 'l', long)]
    pub bound_left: Option<i32>,

    #[arg(short = 'r', long)]
    pub bound_right: Option<i32>,

    #[arg(short = 'm', long)]
    pub margin: Option<i32>,

    /// Command to spawn along with any trailing flags/arguments
    #[arg(trailing_var_arg = true, allow_hyphen_values = true, required = true)]
    pub command: Vec<String>,
}
