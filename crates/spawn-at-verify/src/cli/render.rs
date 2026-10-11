//! Handler for `spawn-at-verify render`: prints the TUI drawer for a recorded real case.

use super::args::RenderArgs;
use std::process::exit;

/// Lists the recorded cases, or renders one to stdout.
pub fn handle_render(args: RenderArgs) {
    #[cfg(feature = "tui")]
    {
        use super::check::resolve_verify_dir;
        use crate::tui::render::{find_case, recorded_cases, render_case};

        if args.list {
            for c in recorded_cases() {
                println!("{}    (from tests/fixtures/{})", c.id, c.fixture);
            }
            return;
        }
        let name = args.case.unwrap_or_else(|| {
            eprintln!("Error: --case <name> is required (use --list to see the cases)");
            exit(2)
        });
        if args.width < 60 || args.height < 15 {
            eprintln!(
                "Error: the TUI needs at least 60x15 (got {}x{})",
                args.width, args.height
            );
            exit(2);
        }
        let verify_dir = resolve_verify_dir(args.dir).unwrap_or_else(|e| {
            eprintln!("Error: {}", e);
            exit(1)
        });
        let case = find_case(&name).unwrap_or_else(|e| {
            eprintln!("Error: {}", e);
            exit(1)
        });
        match render_case(&verify_dir, &case, args.width, args.height, args.scroll) {
            Ok(text) => print!("{}", text),
            Err(e) => {
                eprintln!("Render error: {}", e);
                exit(1);
            }
        }
    }
    #[cfg(not(feature = "tui"))]
    {
        let _ = args;
        eprintln!(
            "Error: TUI feature is not compiled in.\nRun with: cargo run -p spawn-at-verify --features tui,fixture -- render --list"
        );
        exit(1);
    }
}
