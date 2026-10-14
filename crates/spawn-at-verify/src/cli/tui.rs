//! Handler for `spawn-at-verify tui` subcommand.

use super::args::TuiArgs;
use std::process::exit;

/// Launches interactive or headless Terminal UI.
pub fn handle_tui(tui_args: TuiArgs) {
    #[cfg(feature = "tui")]
    {
        let tui_args = crate::tui::TuiArgs {
            bin: tui_args.bin,
            dir: tui_args.dir,
            suite: tui_args.suite,
            category: tui_args.category,
            tags: tui_args.tags,
        };
        if let Err(e) = crate::tui::run_tui(tui_args) {
            eprintln!("TUI Error: {}", e);
            exit(1);
        }
    }
    #[cfg(not(feature = "tui"))]
    {
        let _ = tui_args;
        eprintln!(
                    "Error: TUI feature is not compiled in.\nRun with: cargo run -p spawn-at-verify --features tui,fixture -- tui"
                );
        exit(1);
    }
}
