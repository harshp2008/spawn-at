//! State Machine and Event Handling for spawn-at-verify TUI.

use crate::loader::TestRegistry;
use crate::preflight::{inspect_binary, PreflightReport};
use crate::probe::EnvironmentInfo;
use crate::runner::{
    resolve_final_status, AutoStatus, FinalStatus, HumanVerdict, HumanVerdictKind, RunOptions,
    TestResult, TestRunner,
};
use crate::schema::{TestDefinition, TestKind};
use crate::tracker::WindowTracker;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
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

    pub fn run_selected_category(&mut self) {
        if let Some(test) = self.selected_test() {
            let cat = test.category().to_string();
            self.queue.clear();
            let ids: Vec<String> = self
                .filtered_tests()
                .iter()
                .filter(|t| t.category() == cat)
                .map(|t| t.id.clone())
                .collect();
            let count = ids.len();
            for id in ids {
                self.queue.push_back(id);
            }
            self.status_message = Some(format!("Running {} tests in category '{}'...", count, cat));
            self.process_queue_step();
        }
    }

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

    pub fn run_selected(&mut self) {
        if let Some(test) = self.selected_test().cloned() {
            self.queue.clear();
            self.queue.push_back(test.id);
            self.process_queue_step();
        }
    }

    pub fn run_all_filtered(&mut self) {
        self.queue.clear();
        let ids: Vec<String> = self.filtered_tests().iter().map(|t| t.id.clone()).collect();
        for id in ids {
            self.queue.push_back(id);
        }
        self.process_queue_step();
    }

    pub fn process_queue_step(&mut self) {
        if self.waiting_for_human.is_some() {
            return;
        }

        while let Some(test_id) = self.queue.pop_front() {
            if let Some(test) = self.registry.get_test(&test_id).cloned() {
                if let Some(pos) = self.filtered_tests().iter().position(|t| t.id == test_id) {
                    self.selected_index = pos;
                }

                self.execute_test(&test);

                match test.kind {
                    TestKind::Auto => {
                        self.close_test_windows();
                    }
                    TestKind::Human | TestKind::Both => {
                        self.waiting_for_human = Some(test.id.clone());
                        self.status_message = Some(format!(
                            "Waiting for your verdict on '{}': press 'p' (Pass), 'x' (Fail), 's' (Skip), 'k' (Flaky)",
                            test.id
                        ));
                        return; // Sequential queue blocks on human cases!
                    }
                }
            }
        }

        self.status_message = Some("All tests finished.".into());
    }

    fn execute_test(&mut self, test: &TestDefinition) {
        let runner = TestRunner::new(self.registry.clone(), self.options.clone());
        let preflight = self.preflight.clone().unwrap_or_else(|| PreflightReport {
            detected_commit: None,
            binary_version_output: String::new(),
            repo_head: None,
            repo_dirty: false,
            is_dirty_build: false,
            verdict: crate::preflight::PreflightVerdict::Pass,
        });
        let env_info = self.env_info.clone().unwrap_or_else(|| EnvironmentInfo {
            session_type: "unknown".into(),
            desktop: "unknown".into(),
            gnome_version: None,
            monitors_count: 1,
            primary_workarea: None,
        });

        let result = runner.execute_single_test(test, &preflight, &env_info, &mut self.tracker);
        self.status_message = Some(format!(
            "Ran {}: {:?} ({}ms)",
            test.id, result.final_status, result.duration_ms
        ));
        self.results.insert(test.id.clone(), result);
    }

    pub fn open_verdict_modal(&mut self) {
        if let Some(test) = self.selected_test() {
            if test.kind == TestKind::Auto {
                self.status_message =
                    Some("Test is automated only (kind = 'auto'); no visual review prompt.".into());
                return;
            }
            self.modal = ModalState::HumanVerdict {
                test_id: test.id.clone(),
            };
        }
    }

    pub fn record_verdict(&mut self, verdict: HumanVerdictKind) {
        let target_test_id = self.waiting_for_human.clone().or_else(|| {
            if let ModalState::HumanVerdict { test_id } = &self.modal {
                Some(test_id.clone())
            } else {
                self.selected_test().map(|t| t.id.clone())
            }
        });

        if let Some(test_id) = target_test_id {
            if let Some(test) = self.registry.get_test(&test_id).cloned() {
                let mut result =
                    self.results
                        .get(&test.id)
                        .cloned()
                        .unwrap_or_else(|| TestResult {
                            id: test.id.clone(),
                            title: test.title.clone(),
                            kind: test.kind,
                            required: test.required,
                            auto_status: AutoStatus::Pending,
                            human_verdict: HumanVerdict::default(),
                            final_status: FinalStatus::Pending,
                            disagreement: false,
                            duration_ms: 0,
                            exit_code: None,
                            stdout: String::new(),
                            stderr: String::new(),
                            message: None,
                            log_slice: None,
                        });

                result.human_verdict = HumanVerdict {
                    verdict,
                    note: None,
                };
                let (final_status, disagreement) =
                    resolve_final_status(test.kind, result.auto_status, &result.human_verdict);
                result.final_status = final_status;
                result.disagreement = disagreement;
                self.results.insert(test.id.clone(), result);
                self.status_message =
                    Some(format!("Recorded verdict {:?} for {}", verdict, test.id));
            }
        }

        self.close_test_windows();
        self.waiting_for_human = None;
        self.modal = ModalState::None;

        // Resume queue!
        self.process_queue_step();
    }

    pub fn close_modal(&mut self) {
        self.modal = ModalState::None;
    }

    pub fn handle_key(&mut self, key: KeyEvent) {
        // 1. Raw mode Ctrl+C backstop: closes test windows and terminates immediately
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.close_test_windows();
            self.should_quit = true;
            return;
        }

        // 2. Modal interactions
        match &self.modal {
            ModalState::Help => {
                match key.code {
                    KeyCode::Esc | KeyCode::Char('?') | KeyCode::Char('q') | KeyCode::Char('Q') => {
                        self.close_modal();
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        self.detail_scroll = self.detail_scroll.saturating_add(1);
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        self.detail_scroll = self.detail_scroll.saturating_sub(1);
                    }
                    KeyCode::PageDown => {
                        self.detail_scroll = self.detail_scroll.saturating_add(8);
                    }
                    KeyCode::PageUp => {
                        self.detail_scroll = self.detail_scroll.saturating_sub(8);
                    }
                    KeyCode::Home | KeyCode::Char('g') => {
                        self.detail_scroll = 0;
                    }
                    KeyCode::Char('G') => {
                        self.detail_scroll = u16::MAX;
                    }
                    _ => {}
                }
                return;
            }
            ModalState::HumanVerdict { .. } => {
                match key.code {
                    KeyCode::Char('p') | KeyCode::Char('P') => {
                        self.record_verdict(HumanVerdictKind::Pass);
                    }
                    KeyCode::Char('x') | KeyCode::Char('X') => {
                        self.record_verdict(HumanVerdictKind::Fail);
                    }
                    KeyCode::Char('s') | KeyCode::Char('S') => {
                        self.record_verdict(HumanVerdictKind::Skip);
                    }
                    KeyCode::Char('k') | KeyCode::Char('K') => {
                        self.record_verdict(HumanVerdictKind::Flaky);
                    }
                    KeyCode::Esc => {
                        self.close_modal();
                    }
                    _ => {}
                }
                return;
            }
            ModalState::None => {}
        }

        // 3. Detail drawer mode keybindings (takes precedence while open)
        if self.detail_drawer_open {
            match key.code {
                KeyCode::Esc | KeyCode::Enter | KeyCode::Char('d') => {
                    self.detail_drawer_open = false;
                }
                KeyCode::Char('?') => {
                    self.modal = ModalState::Help;
                }
                KeyCode::Char('y') => {
                    self.copy_drawer_text();
                }
                KeyCode::Char('w') => {
                    let _ = self.save_drawer_text();
                }
                KeyCode::Char('t') => {
                    self.drawer_raw_view = !self.drawer_raw_view;
                }
                KeyCode::Char('q') | KeyCode::Char('Q') => {
                    self.close_test_windows();
                    self.should_quit = true;
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    self.detail_scroll = self.detail_scroll.saturating_add(1);
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    self.detail_scroll = self.detail_scroll.saturating_sub(1);
                }
                KeyCode::PageDown => {
                    self.detail_scroll = self.detail_scroll.saturating_add(8);
                }
                KeyCode::PageUp => {
                    self.detail_scroll = self.detail_scroll.saturating_sub(8);
                }
                KeyCode::Home | KeyCode::Char('g') => {
                    self.detail_scroll = 0;
                }
                KeyCode::Char('G') => {
                    self.detail_scroll = u16::MAX;
                }
                _ => {}
            }
            return;
        }

        // 4. Waiting for human verdict (sequential run queue blocked on human review)
        if self.waiting_for_human.is_some() {
            match key.code {
                KeyCode::Char('d') | KeyCode::Enter => {
                    self.detail_drawer_open = true;
                    self.detail_scroll = 0;
                }
                KeyCode::Char('?') => {
                    self.modal = ModalState::Help;
                }
                KeyCode::Char('p') | KeyCode::Char('P') => {
                    self.record_verdict(HumanVerdictKind::Pass);
                }
                KeyCode::Char('x') | KeyCode::Char('X') => {
                    self.record_verdict(HumanVerdictKind::Fail);
                }
                KeyCode::Char('s') | KeyCode::Char('S') => {
                    self.record_verdict(HumanVerdictKind::Skip);
                }
                KeyCode::Char('k') | KeyCode::Char('K') => {
                    self.record_verdict(HumanVerdictKind::Flaky);
                }
                KeyCode::Char('r') | KeyCode::Char('R') => {
                    if let Some(test_id) = self.waiting_for_human.clone() {
                        if let Some(test) = self.registry.get_test(&test_id).cloned() {
                            self.close_test_windows();
                            self.execute_test(&test);
                        }
                    }
                }
                KeyCode::Char('q') | KeyCode::Char('Q') => {
                    self.close_test_windows();
                    self.should_quit = true;
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    self.detail_scroll = self.detail_scroll.saturating_add(1);
                }
                KeyCode::Up => {
                    self.detail_scroll = self.detail_scroll.saturating_sub(1);
                }
                KeyCode::PageDown => {
                    self.detail_scroll = self.detail_scroll.saturating_add(8);
                }
                KeyCode::PageUp => {
                    self.detail_scroll = self.detail_scroll.saturating_sub(8);
                }
                KeyCode::Home | KeyCode::Char('g') => {
                    self.detail_scroll = 0;
                }
                KeyCode::Char('G') => {
                    self.detail_scroll = u16::MAX;
                }
                KeyCode::Esc => {
                    self.close_test_windows();
                    self.waiting_for_human = None;
                    self.status_message = Some("Visual review cancelled.".into());
                }
                _ => {}
            }
            return;
        }

        // 5. Normal view keybindings
        match key.code {
            KeyCode::Char('?') => {
                self.modal = ModalState::Help;
            }
            KeyCode::Char('q') | KeyCode::Char('Q') => {
                self.close_test_windows();
                self.should_quit = true;
            }
            KeyCode::Enter | KeyCode::Char('d') => {
                self.detail_drawer_open = true;
                self.detail_scroll = 0;
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.active_pane == ActivePane::Details {
                    self.detail_scroll = self.detail_scroll.saturating_add(1);
                } else {
                    self.next();
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if self.active_pane == ActivePane::Details {
                    self.detail_scroll = self.detail_scroll.saturating_sub(1);
                } else {
                    self.previous();
                }
            }
            KeyCode::PageDown | KeyCode::Char(']') => {
                self.detail_scroll = self.detail_scroll.saturating_add(6);
            }
            KeyCode::PageUp | KeyCode::Char('[') => {
                self.detail_scroll = self.detail_scroll.saturating_sub(6);
            }
            KeyCode::Home | KeyCode::Char('g') => {
                if self.active_pane == ActivePane::Details {
                    self.detail_scroll = 0;
                } else {
                    self.selected_index = 0;
                    self.detail_scroll = 0;
                }
            }
            KeyCode::Char('G') => {
                if self.active_pane == ActivePane::Details {
                    self.detail_scroll = u16::MAX;
                } else {
                    let count = self.filtered_tests().len();
                    if count > 0 {
                        self.selected_index = count - 1;
                        self.detail_scroll = 0;
                    }
                }
            }
            KeyCode::Char('f') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.cycle_category_filter();
            }
            KeyCode::Char('f') => {
                self.cycle_filter();
            }
            KeyCode::Char(' ') => {
                self.toggle_collapse_selected_category();
            }
            KeyCode::Tab | KeyCode::BackTab => {
                self.active_pane = match self.active_pane {
                    ActivePane::List => ActivePane::Details,
                    ActivePane::Details => ActivePane::List,
                };
            }
            KeyCode::Char('r') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.run_selected_category();
            }
            KeyCode::Char('r') => {
                self.run_selected();
            }
            KeyCode::Char('c') if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.run_selected_category();
            }
            KeyCode::Char('a') => {
                self.run_all_filtered();
            }
            KeyCode::Char('v') => {
                self.open_verdict_modal();
            }
            _ => {}
        }
    }

    pub fn copy_drawer_text(&mut self) {
        if let Some(test) = self.selected_test().cloned() {
            let (_lines, plain) =
                super::views::build_drawer_content(self, &test, 80, !self.drawer_raw_view);
            let line_count = plain.lines().count();
            let res = if let Some(cb) = &self.clipboard_fn {
                cb(&plain)
            } else {
                system_copy_to_clipboard(&plain)
            };
            match res {
                Ok(()) => {
                    self.drawer_status = Some((
                        format!("Copied {} lines", line_count),
                        std::time::Instant::now(),
                    ));
                }
                Err(e) => {
                    self.drawer_status = Some((
                        format!("Copy failed: {}; use w", e),
                        std::time::Instant::now(),
                    ));
                }
            }
        }
    }

    pub fn save_drawer_text(&mut self) -> Result<PathBuf, String> {
        if let Some(test) = self.selected_test().cloned() {
            let (_lines, plain) =
                super::views::build_drawer_content(self, &test, 80, !self.drawer_raw_view);
            let safe_id = test.id.replace('/', "-");
            let file_name = format!("{}.log.txt", safe_id);
            let path = self.report_dir.join(file_name);
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            std::fs::write(&path, &plain)
                .map_err(|e| format!("Failed to write {}: {}", path.display(), e))?;
            self.drawer_status = Some((path.display().to_string(), std::time::Instant::now()));
            Ok(path)
        } else {
            Err("No test selected".into())
        }
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

pub fn system_copy_to_clipboard(text: &str) -> Result<(), String> {
    // 1. Try wl-copy
    if let Ok(mut child) = std::process::Command::new("wl-copy")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
    {
        if let Some(mut stdin) = child.stdin.take() {
            use std::io::Write;
            let _ = stdin.write_all(text.as_bytes());
        }
        if let Ok(status) = child.wait() {
            if status.success() {
                return Ok(());
            }
        }
    }

    // 2. Try xclip (-selection clipboard)
    if let Ok(mut child) = std::process::Command::new("xclip")
        .args(["-selection", "clipboard"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
    {
        if let Some(mut stdin) = child.stdin.take() {
            use std::io::Write;
            let _ = stdin.write_all(text.as_bytes());
        }
        if let Ok(status) = child.wait() {
            if status.success() {
                return Ok(());
            }
        }
    }

    // 3. Try xsel (--clipboard --input)
    if let Ok(mut child) = std::process::Command::new("xsel")
        .args(["--clipboard", "--input"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
    {
        if let Some(mut stdin) = child.stdin.take() {
            use std::io::Write;
            let _ = stdin.write_all(text.as_bytes());
        }
        if let Ok(status) = child.wait() {
            if status.success() {
                return Ok(());
            }
        }
    }

    // 4. Try OSC 52
    if copy_osc52(text).is_ok() {
        return Ok(());
    }

    Err("no clipboard provider (wl-copy, xclip, xsel, OSC 52) succeeded".into())
}

fn copy_osc52(text: &str) -> Result<(), String> {
    use std::io::Write;
    let b64 = base64_encode(text.as_bytes());
    let osc = format!("\x1b]52;c;{}\x07", b64);
    let mut stdout = std::io::stdout();
    stdout
        .write_all(osc.as_bytes())
        .map_err(|e| e.to_string())?;
    stdout.flush().map_err(|e| e.to_string())?;
    Ok(())
}

fn base64_encode(data: &[u8]) -> String {
    const CHARSET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0];
        let b1 = chunk.get(1).copied().unwrap_or(0);
        let b2 = chunk.get(2).copied().unwrap_or(0);
        out.push(CHARSET[(b0 >> 2) as usize] as char);
        out.push(CHARSET[(((b0 & 3) << 4) | (b1 >> 4)) as usize] as char);
        if chunk.len() > 1 {
            out.push(CHARSET[(((b1 & 0x0f) << 2) | (b2 >> 6)) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(CHARSET[(b2 & 0x3f) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}
