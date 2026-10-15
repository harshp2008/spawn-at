//! TUI state models, interactive app state, and selection navigation.

use crate::loader::TestRegistry;
use crate::preflight::{inspect_binary, PreflightReport};
use crate::probe::EnvironmentInfo;
use crate::runner::{FinalStatus, RunOptions, TestResult};
use crate::schema::{TestDefinition, TestKind};
use crate::tracker::WindowTracker;
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};

/// Test filter modes cycling across review kinds and result statuses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterMode {
    /// Show all loaded test definitions.
    All,
    /// Filter to tests requiring human visual review.
    HumanOnly,
    /// Filter to automated tests only.
    AutoOnly,
    /// Filter to tests that failed verification.
    FailedOnly,
}

impl FilterMode {
    /// Advances to the next filter mode in cyclical sequence.
    pub fn next(self) -> Self {
        match self {
            Self::All => Self::HumanOnly,
            Self::HumanOnly => Self::AutoOnly,
            Self::AutoOnly => Self::FailedOnly,
            Self::FailedOnly => Self::All,
        }
    }

    /// Human-readable label describing the active filter mode.
    pub fn label(self) -> &'static str {
        match self {
            Self::All => "All",
            Self::HumanOnly => "Human (Review Required)",
            Self::AutoOnly => "Auto Only",
            Self::FailedOnly => "Failed Only",
        }
    }
}

/// Identifies the active interactive pane in the TUI window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivePane {
    /// Tests list navigation pane.
    List,
    /// Detailed description and result drawer pane.
    Details,
}

/// Modal dialog overlay states displayed above the main interface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModalState {
    /// No modal dialog active.
    None,
    /// Keyboard shortcut and usage help modal.
    Help,
    /// Human verdict entry dialog for the specified test case.
    HumanVerdict {
        /// Id of the test the verdict is being entered for.
        test_id: String,
    },
}

/// Computes fixed ordering rank for category grouping.
pub fn category_rank(cat_or_id: &str) -> usize {
    if cat_or_id.starts_with("preflight") || cat_or_id == "basics" {
        0
    } else if cat_or_id.starts_with("placement") || cat_or_id == "placement" {
        1
    } else if cat_or_id.starts_with("cloak") || cat_or_id == "animation-and-cloak" {
        2
    } else if cat_or_id.starts_with("transform") || cat_or_id == "transforms" {
        3
    } else if cat_or_id.starts_with("focus") {
        4
    } else if cat_or_id.starts_with("query") || cat_or_id == "queries" {
        5
    } else if cat_or_id.starts_with("robust")
        || cat_or_id.starts_with("resilience")
        || cat_or_id == "robustness"
    {
        6
    } else if cat_or_id.starts_with("platform") {
        7
    } else {
        8
    }
}

/// Function type alias for test clipboard injection and system copy.
pub type ClipboardHelper = std::sync::Arc<dyn Fn(&str) -> Result<(), String> + Send + Sync>;

/// Load averages around one run and the CPU count they are judged against.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RunLoad {
    /// Load when the run started.
    pub start: Option<crate::loadavg::LoadAvg>,
    /// Load when the run ended.
    pub end: Option<crate::loadavg::LoadAvg>,
    /// CPUs available.
    pub cpus: usize,
}

/// Central state machine and event coordinator for the verification Terminal UI.
pub struct TuiApp {
    /// Test definitions loaded from the `verify/` directory.
    pub registry: TestRegistry,
    /// Index of the highlighted test within [`TuiApp::filtered_tests`] (the filtered, sorted
    /// list), not within the registry; clamped when the filter changes.
    pub selected_index: usize,
    /// Which tests the list shows: all, human-reviewed, automatic, or failed.
    pub filter_mode: FilterMode,
    /// Category the list is restricted to, or `None` to show every category; cycled by the
    /// category key through the sorted category names and back to `None`.
    pub category_filter: Option<String>,
    /// Categories folded shut in the tests list.
    pub collapsed_categories: HashSet<String>,
    /// Latest result for each executed test, keyed by test id.
    pub results: HashMap<String, TestResult>,
    /// Modal dialog drawn over the interface (help or human verdict entry), if any.
    pub modal: ModalState,
    /// Whether the `fixture` helper binary exists next to `bin_path`; when false the header
    /// shows a "FIXTURE MISSING" banner.
    pub fixture_exists: bool,
    /// Expected location of the `fixture` binary (the `fixture` file in `bin_path`'s directory).
    pub fixture_path: PathBuf,
    /// Path of the `spawn-at` binary under test.
    pub bin_path: PathBuf,
    /// Options passed to the runner for each test executed from the TUI.
    pub options: RunOptions,
    /// Preflight inspection of `bin_path` taken at startup (`None` if inspection failed);
    /// reused for every test run.
    pub preflight: Option<PreflightReport>,
    /// Session and monitor facts probed at startup; reused for every test run.
    pub env_info: Option<EnvironmentInfo>,
    /// Records pre-existing windows and the windows spawned by tests, so test windows can be
    /// attributed and closed without touching the user's own.
    pub tracker: WindowTracker,
    /// One-line progress or filter message set by the run queue and filter keys. It is stored
    /// but no view currently draws it.
    pub status_message: Option<String>,
    /// Set by the quit keys; the event loop exits when it becomes true.
    pub should_quit: bool,
    /// Rows the detail drawer is scrolled down; reset to 0 when the selection changes.
    pub detail_scroll: u16,
    /// Whether the detail drawer overlay is open.
    pub detail_drawer_open: bool,
    /// Ids of tests waiting to run, in execution order.
    pub queue: VecDeque<String>,
    /// Id of a human-review test that has run and is waiting for a verdict; while set, the
    /// queue does not advance.
    pub waiting_for_human: Option<String>,
    /// Id of the window that was focused when the app started (the terminal), refocused after
    /// test windows are closed; `None` if it could not be determined.
    pub terminal_window_id: Option<u64>,
    /// Pane that receives scroll and navigation keys: the tests list or the details pane.
    pub active_pane: ActivePane,
    /// Directory the drawer's save key writes `<id>.log.txt` files into.
    pub report_dir: PathBuf,
    /// Load averages at the start and end of the last executed test (or of a recorded run) and
    /// the CPU count; the drawer warns when the 1-minute load was above half the CPUs.
    pub run_load: Option<RunLoad>,
    /// Message shown in the drawer title for 3 seconds after a copy or save, with the instant it
    /// was set.
    pub drawer_status: Option<(String, std::time::Instant)>,
    /// True when the drawer shows the raw trace lines instead of the compact timeline.
    pub drawer_raw_view: bool,
    /// Replaces the system clipboard when set (tests inject a capturing closure); `None` uses
    /// [`system_copy_to_clipboard`](super::clipboard::system_copy_to_clipboard).
    pub clipboard_fn: Option<ClipboardHelper>,
    /// Work the event loop does after the next redraw, one item per loop turn: a key press only
    /// records what the person asked for and puts it on screen; the slow part (running a test,
    /// closing its window) happens after that frame has been drawn.
    pub pending: VecDeque<Pending>,
    /// Id of the test whose window is being closed after a verdict, Esc or re-run: the result box
    /// shows "closing window..." for it until the teardown has run.
    pub closing_for: Option<String>,
}

/// A slow step the event loop runs after a redraw (see [`TuiApp::pending`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pending {
    /// Close the windows the last human-reviewed test left open (by recorded id, through the
    /// tracker), note anything the teardown reports, then go on with the queue if asked.
    Teardown {
        /// Start the next queued test afterwards.
        continue_queue: bool,
    },
    /// Run the next queued test (its row already shows "running").
    RunQueueStep,
}

impl TuiApp {
    /// Initializes a new TuiApp instance, discovering binaries and loading test specifications.
    pub fn new<P: AsRef<Path>>(verify_dir: P, options: RunOptions) -> Result<Self, String> {
        let registry = TestRegistry::load_from_dir(verify_dir)?;
        let bin_path = options.bin_path.clone();

        let fixture_path = bin_path
            .parent()
            .unwrap_or_else(|| Path::new("target/debug"))
            .join("fixture");
        let fixture_exists = fixture_path.exists();

        let preflight = inspect_binary(&bin_path).ok();
        let env_info = Some(EnvironmentInfo::probe(Some(&bin_path)));

        let tracker = WindowTracker::snapshot_startup(&bin_path)
            .unwrap_or_else(|_| WindowTracker::new_with_preexisting(&[]));

        let terminal_window_id = {
            let mut cmd = std::process::Command::new(&bin_path);
            cmd.args(["query", "windows", "--json"]);
            crate::process::execute_with_timeout(cmd, std::time::Duration::from_millis(500))
                .ok()
                .and_then(|out| {
                    serde_json::from_slice::<serde_json::Value>(&out.stdout)
                        .ok()
                        .and_then(|v| {
                            v.as_array().and_then(|arr| {
                                for item in arr {
                                    if item.get("focused").and_then(|f| f.as_bool()) == Some(true) {
                                        return item.get("id").and_then(|id| id.as_u64());
                                    }
                                }
                                None
                            })
                        })
                })
        };

        Ok(Self {
            registry,
            selected_index: 0,
            filter_mode: FilterMode::All,
            category_filter: None,
            collapsed_categories: HashSet::new(),
            results: HashMap::new(),
            modal: ModalState::None,
            fixture_exists,
            fixture_path,
            bin_path,
            options,
            preflight,
            env_info,
            tracker,
            status_message: None,
            should_quit: false,
            detail_scroll: 0,
            detail_drawer_open: false,
            queue: VecDeque::new(),
            waiting_for_human: None,
            terminal_window_id,
            active_pane: ActivePane::List,
            report_dir: PathBuf::from("verify-results"),
            run_load: None,
            drawer_status: None,
            drawer_raw_view: false,
            clipboard_fn: None,
            pending: VecDeque::new(),
            closing_for: None,
        })
    }

    /// Returns a sorted slice of test definitions matching active filters and category selections.
    pub fn filtered_tests(&self) -> Vec<&TestDefinition> {
        let mut list: Vec<&TestDefinition> = self
            .registry
            .tests
            .iter()
            .filter(|test| {
                if let Some(cat) = &self.category_filter {
                    if test.category() != cat && !test.id.starts_with(cat) {
                        return false;
                    }
                }
                match self.filter_mode {
                    FilterMode::All => true,
                    FilterMode::HumanOnly => {
                        test.kind == TestKind::Human || test.kind == TestKind::Both
                    }
                    FilterMode::AutoOnly => test.kind == TestKind::Auto,
                    FilterMode::FailedOnly => self
                        .results
                        .get(&test.id)
                        .map(|r| r.final_status == FinalStatus::Fail)
                        .unwrap_or(false),
                }
            })
            .collect();
        list.sort_by_key(|t| (category_rank(t.category()), category_rank(&t.id), &t.id));
        list
    }

    // Navigation and state methods
    /// Toggles folding expansion/collapse for the currently selected test category.
    pub fn toggle_collapse_selected_category(&mut self) {
        if let Some(test) = self.selected_test() {
            let cat = test.category().to_string();
            if self.collapsed_categories.contains(&cat) {
                self.collapsed_categories.remove(&cat);
                self.status_message = Some(format!("Expanded category '{}'", cat));
            } else {
                self.collapsed_categories.insert(cat.clone());
                self.status_message = Some(format!("Collapsed category '{}'", cat));
            }
        }
    }

    /// Cycles category isolation filter across available categories or clears filter.
    pub fn cycle_category_filter(&mut self) {
        let mut cats: Vec<String> = self
            .registry
            .tests
            .iter()
            .map(|t| t.category().to_string())
            .collect();
        cats.sort();
        cats.dedup();

        self.category_filter = match &self.category_filter {
            None => cats.first().cloned(),
            Some(curr) => {
                if let Some(pos) = cats.iter().position(|c| c == curr) {
                    if pos + 1 < cats.len() {
                        Some(cats[pos + 1].clone())
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
        };
        self.clamp_selection();
        self.status_message = Some(match &self.category_filter {
            Some(c) => format!("Filtered to category: {}", c),
            None => "Category filter cleared (showing all)".into(),
        });
    }

    /// Returns the test definition currently highlighted by list selection.
    pub fn selected_test(&self) -> Option<&TestDefinition> {
        let list = self.filtered_tests();
        if list.is_empty() {
            None
        } else {
            let idx = self.selected_index.min(list.len() - 1);
            Some(list[idx])
        }
    }

    /// Moves selection down to the next test item.
    pub fn next(&mut self) {
        let count = self.filtered_tests().len();
        if count > 0 && self.selected_index + 1 < count {
            self.selected_index += 1;
            self.detail_scroll = 0;
        }
    }

    /// Moves selection up to the previous test item.
    pub fn previous(&mut self) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
            self.detail_scroll = 0;
        }
    }

    /// Cycles across global filter modes and clamps selection indices.
    pub fn cycle_filter(&mut self) {
        self.filter_mode = self.filter_mode.next();
        self.clamp_selection();
        self.status_message = Some(format!("Filter switched to: {}", self.filter_mode.label()));
    }

    /// Clamps selected index within current filtered test list bounds.
    pub fn clamp_selection(&mut self) {
        let count = self.filtered_tests().len();
        if count == 0 {
            self.selected_index = 0;
        } else if self.selected_index >= count {
            self.selected_index = count - 1;
        }
    }

    /// Closes any tracked test windows and refocuses the original terminal window.
    pub fn close_test_windows(&mut self) {
        let _ = self.close_test_windows_noted();
    }

    /// [`TuiApp::close_test_windows`] that returns what the teardown reported (a close that
    /// failed, a window left behind, focus not returned), as short lines for the result box.
    /// None of it is a verdict. Closing goes through the tracker only: by recorded id, through
    /// the sessions' own process handles. The terminal is refocused only if it lost focus (a
    /// version 2 session has usually done that already, and a window that has focus needs no
    /// command).
    pub fn close_test_windows_noted(&mut self) -> Vec<String> {
        let mut lines = Vec::new();
        match self.tracker.cleanup_test_windows_reported(&self.bin_path) {
            Ok((failed, reports)) => {
                for r in reports {
                    lines.extend(r.warnings);
                    if !r.left_behind.is_empty() {
                        lines.push(format!(
                            "a test window is still open after closing (window ids {:?})",
                            r.left_behind
                        ));
                    }
                }
                if !failed.is_empty() {
                    lines.push(format!("could not close window ids {failed:?}"));
                }
            }
            Err(e) => lines.push(e),
        }
        if let Some(term_id) = self.terminal_window_id {
            let has_focus =
                crate::runner::owned::Control::windows(&crate::runner::owned::CliControl {
                    bin: self.bin_path.clone(),
                })
                .map(|w| w.iter().any(|w| w.id == term_id && w.focused))
                .unwrap_or(false);
            if !has_focus {
                let mut cmd = std::process::Command::new(&self.bin_path);
                cmd.args(["focus", "--id", &term_id.to_string(), "--no-wait"]);
                let _ = crate::process::execute_with_timeout(
                    cmd,
                    std::time::Duration::from_millis(500),
                );
            }
        }
        lines
    }

    /// Dismisses any currently open modal dialog overlay.
    pub fn close_modal(&mut self) {
        self.modal = ModalState::None;
    }

    /// Computes title header text for the detail drawer, reflecting copy status if recent.
    pub fn get_drawer_title(&self, _test_id: &str) -> String {
        if let Some((msg, time)) = &self.drawer_status {
            if time.elapsed().as_secs() < 3 {
                return format!(" {} ", msg);
            }
        }
        "Details".to_string()
    }
}
