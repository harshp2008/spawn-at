//! TUI state models, interactive app state, and selection navigation.

use crate::loader::TestRegistry;
use crate::preflight::{inspect_binary, PreflightReport};
use crate::probe::EnvironmentInfo;
use crate::runner::{FinalStatus, RunOptions, TestResult};
use crate::schema::{TestDefinition, TestKind};
use crate::tracker::WindowTracker;
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterMode {
    All,
    HumanOnly,
    AutoOnly,
    FailedOnly,
}

impl FilterMode {
    pub fn next(self) -> Self {
        match self {
            Self::All => Self::HumanOnly,
            Self::HumanOnly => Self::AutoOnly,
            Self::AutoOnly => Self::FailedOnly,
            Self::FailedOnly => Self::All,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::All => "All",
            Self::HumanOnly => "Human (Review Required)",
            Self::AutoOnly => "Auto Only",
            Self::FailedOnly => "Failed Only",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivePane {
    List,
    Details,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModalState {
    None,
    Help,
    HumanVerdict { test_id: String },
}

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

pub type ClipboardHelper = std::sync::Arc<dyn Fn(&str) -> Result<(), String> + Send + Sync>;

pub struct TuiApp {
    pub registry: TestRegistry,
    pub selected_index: usize,
    pub filter_mode: FilterMode,
    pub category_filter: Option<String>,
    pub collapsed_categories: HashSet<String>,
    pub results: HashMap<String, TestResult>,
    pub modal: ModalState,
    pub fixture_exists: bool,
    pub fixture_path: PathBuf,
    pub bin_path: PathBuf,
    pub options: RunOptions,
    pub preflight: Option<PreflightReport>,
    pub env_info: Option<EnvironmentInfo>,
    pub tracker: WindowTracker,
    pub status_message: Option<String>,
    pub should_quit: bool,
    pub detail_scroll: u16,
    pub detail_drawer_open: bool,
    pub queue: VecDeque<String>,
    pub waiting_for_human: Option<String>,
    pub terminal_window_id: Option<u64>,
    pub active_pane: ActivePane,
    pub report_dir: PathBuf,
    pub drawer_status: Option<(String, std::time::Instant)>,
    pub drawer_raw_view: bool,
    pub clipboard_fn: Option<ClipboardHelper>,
}

impl TuiApp {
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
            drawer_status: None,
            drawer_raw_view: false,
            clipboard_fn: None,
        })
    }

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

    pub fn jump_next_category(&mut self) {
        let list = self.filtered_tests();
        if list.is_empty() {
            return;
        }
        let current_cat = list[self.selected_index.min(list.len() - 1)].category();
        if let Some(next_pos) = list
            .iter()
            .enumerate()
            .skip(self.selected_index + 1)
            .find(|(_, t)| t.category() != current_cat)
            .map(|(i, _)| i)
        {
            self.selected_index = next_pos;
            self.detail_scroll = 0;
        } else if let Some(first_pos) = list.iter().position(|t| t.category() != current_cat) {
            self.selected_index = first_pos;
            self.detail_scroll = 0;
        }
    }

    pub fn jump_prev_category(&mut self) {
        let list = self.filtered_tests();
        if list.is_empty() {
            return;
        }
        let current_cat = list[self.selected_index.min(list.len() - 1)].category();
        let mut prev_cat = None;
        for t in list[..self.selected_index].iter().rev() {
            if t.category() != current_cat {
                prev_cat = Some(t.category());
                break;
            }
        }
        if let Some(target_cat) = prev_cat {
            if let Some(pos) = list.iter().position(|t| t.category() == target_cat) {
                self.selected_index = pos;
                self.detail_scroll = 0;
            }
        } else if let Some(last_cat) = list.last().map(|t| t.category()) {
            if last_cat != current_cat {
                if let Some(pos) = list.iter().position(|t| t.category() == last_cat) {
                    self.selected_index = pos;
                    self.detail_scroll = 0;
                }
            }
        }
    }

    // Navigation and state methods
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

    pub fn selected_test(&self) -> Option<&TestDefinition> {
        let list = self.filtered_tests();
        if list.is_empty() {
            None
        } else {
            let idx = self.selected_index.min(list.len() - 1);
            Some(list[idx])
        }
    }

    pub fn next(&mut self) {
        let count = self.filtered_tests().len();
        if count > 0 && self.selected_index + 1 < count {
            self.selected_index += 1;
            self.detail_scroll = 0;
        }
    }

    pub fn previous(&mut self) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
            self.detail_scroll = 0;
        }
    }

    pub fn cycle_filter(&mut self) {
        self.filter_mode = self.filter_mode.next();
        self.clamp_selection();
        self.status_message = Some(format!("Filter switched to: {}", self.filter_mode.label()));
    }

    pub fn clamp_selection(&mut self) {
        let count = self.filtered_tests().len();
        if count == 0 {
            self.selected_index = 0;
        } else if self.selected_index >= count {
            self.selected_index = count - 1;
        }
    }

    pub fn close_test_windows(&mut self) {
        let _ = self.tracker.cleanup_test_windows(&self.bin_path);
        if let Some(term_id) = self.terminal_window_id {
            let mut cmd = std::process::Command::new(&self.bin_path);
            cmd.args(["focus", "--id", &term_id.to_string(), "--no-wait"]);
            let _ =
                crate::process::execute_with_timeout(cmd, std::time::Duration::from_millis(500));
        }
    }

    pub fn close_modal(&mut self) {
        self.modal = ModalState::None;
    }

    pub fn get_drawer_title(&self, _test_id: &str) -> String {
        if let Some((msg, time)) = &self.drawer_status {
            if time.elapsed().as_secs() < 3 {
                return format!(" {} ", msg);
            }
        }
        "Details".to_string()
    }
}
