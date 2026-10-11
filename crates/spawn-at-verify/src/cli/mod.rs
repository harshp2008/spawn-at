//! CLI argument models and subcommand handlers.

pub mod args;
pub mod check;
pub mod list;
pub mod report;
pub mod run;
pub mod smoke_eval;
pub mod tui;

pub use args::*;
pub use check::*;
pub use list::*;
pub use report::*;
pub use run::*;
pub use smoke_eval::*;
pub use tui::*;

/// Dispatches parsed CLI commands to their respective handlers.
pub fn handle_command(cmd: Commands) {
    match cmd {
        Commands::Check(args) => handle_check(args),
        Commands::Run(args) => handle_run(args),
        Commands::List(args) => handle_list(args),
        Commands::Report(args) => handle_report(args),
        Commands::Tui(args) => handle_tui(args),
        Commands::SmokeEval(cmd) => smoke_eval::execute(cmd),
    }
}
