//! Terminal User Interface (TUI) Module for spawn-at-verify.

pub mod app;
pub mod logging;
pub mod render;
pub mod views;

use self::app::TuiApp;
use self::logging::ExtensionLoggingGuard;
use crate::loader::TestFilter;
use crate::runner::RunOptions;
use clap::Args;
use crossterm::{
    cursor::{Hide, Show},
    event::{self, Event},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io::stdout;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// Command-line arguments for the interactive Terminal UI subcommand.
#[derive(Args, Debug, Clone)]
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

/// Executes the interactive Terminal UI.
pub fn run_tui(args: TuiArgs) -> Result<(), String> {
    let verify_dir = if let Some(d) = args.dir {
        d
    } else {
        find_verify_dir().ok_or_else(|| "Could not locate verify/ directory".to_string())?
    };

    let bin_path = args
        .bin
        .unwrap_or_else(|| PathBuf::from("target/debug/spawn-at"));

    let filter = TestFilter {
        suite: args.suite,
        category_path: args.category,
        tags: args.tags,
        ..Default::default()
    };

    let options = RunOptions {
        bin_path,
        filter,
        action_trace: true,
        ..Default::default()
    };

    // 1. Snapshot and manage extension logging state (Requirement 2)
    let mut logging_guard = ExtensionLoggingGuard::init();

    // 2. Set up signal handler for SIGTERM / SIGINT / SIGHUP (Requirement 1)
    let term_flag = Arc::new(AtomicBool::new(false));
    signal_hook::flag::register(signal_hook::consts::SIGTERM, Arc::clone(&term_flag))
        .map_err(|e| format!("Failed to register SIGTERM hook: {}", e))?;
    signal_hook::flag::register(signal_hook::consts::SIGINT, Arc::clone(&term_flag))
        .map_err(|e| format!("Failed to register SIGINT hook: {}", e))?;
    signal_hook::flag::register(signal_hook::consts::SIGHUP, Arc::clone(&term_flag))
        .map_err(|e| format!("Failed to register SIGHUP hook: {}", e))?;

    // 3. Initialize terminal raw mode and alternate screen
    enable_raw_mode().map_err(|e| format!("Failed to enable terminal raw mode: {}", e))?;
    let mut stdout = stdout();
    if let Err(e) = execute!(stdout, EnterAlternateScreen, Hide) {
        let _ = disable_raw_mode();
        return Err(format!(
            "Failed to initialize terminal alternate screen: {}",
            e
        ));
    }

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)
        .map_err(|e| format!("Failed to initialize Ratatui terminal: {}", e))?;

    // 4. Initialize App state
    let mut app = match TuiApp::new(&verify_dir, options) {
        Ok(a) => a,
        Err(err) => {
            let _ = disable_raw_mode();
            let _ = execute!(std::io::stdout(), LeaveAlternateScreen, Show);
            return Err(err);
        }
    };

    // 5. Main event loop
    loop {
        if term_flag.load(Ordering::Relaxed) || app.should_quit {
            break;
        }

        if let Err(e) = terminal.draw(|f| {
            views::draw_ui(f, &app);
        }) {
            eprintln!("Draw error: {}", e);
            break;
        }

        // A key press only recorded what the person asked for and the frame above showed it
        // (a "running" row, a verdict, "closing window..."). Now do the slow step, then loop to
        // draw its result. Nothing is pending when nothing was asked, so an idle loop still just
        // draws and waits for a key.
        if app.has_pending() {
            app.run_pending();
            continue;
        }

        if event::poll(Duration::from_millis(50)).map_err(|e| e.to_string())? {
            if let Event::Key(key) = event::read().map_err(|e| e.to_string())? {
                // Ctrl+C in raw mode is dispatched through handle_key
                app.handle_key(key);
            }
        }
    }

    // 6. Unified Teardown: identical for Ctrl+C, quit, and signals
    unified_teardown(&mut app, &mut logging_guard);

    Ok(())
}

/// Unified cleanup procedure executed on all exit pathways (Ctrl+C, 'q', SIGTERM, SIGHUP).
fn unified_teardown(app: &mut TuiApp, logging_guard: &mut ExtensionLoggingGuard) {
    let _ = disable_raw_mode();
    let _ = execute!(std::io::stdout(), LeaveAlternateScreen, Show);

    // Safely cleanup any remaining test-spawned windows
    let _ = app.tracker.cleanup_test_windows(&app.bin_path);

    // Restore extension logging state recorded at startup
    logging_guard.restore();
}

/// Discovers the root directory containing test definition files across candidate paths.
pub fn find_verify_dir() -> Option<PathBuf> {
    let candidates = [
        PathBuf::from("verify"),
        PathBuf::from("../../verify"),
        PathBuf::from("../verify"),
    ];
    for c in &candidates {
        if c.is_dir() {
            return Some(c.clone());
        }
    }
    None
}
