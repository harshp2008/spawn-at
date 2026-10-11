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
use std::collections::{HashMap, VecDeque};
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModalState {
    None,
    HumanVerdict { test_id: String },
}

pub struct TuiApp {
    pub registry: TestRegistry,
    pub selected_index: usize,
    pub filter_mode: FilterMode,
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
        })
    }

    pub fn filtered_tests(&self) -> Vec<&TestDefinition> {
        self.registry
            .tests
            .iter()
            .filter(|test| match self.filter_mode {
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
            })
            .collect()
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
                            "Waiting for your verdict on '{}': press 'p' (Pass), 'f' (Fail), 's' (Skip)",
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

        // 2. Waiting for human verdict (sequential run queue blocked on human review)
        if self.waiting_for_human.is_some() {
            match key.code {
                KeyCode::Char('p') | KeyCode::Char('P') => {
                    self.record_verdict(HumanVerdictKind::Pass);
                }
                KeyCode::Char('f') | KeyCode::Char('F') => {
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
                KeyCode::Esc => {
                    self.close_test_windows();
                    self.waiting_for_human = None;
                    self.status_message = Some("Visual review cancelled.".into());
                }
                _ => {}
            }
            return;
        }

        // 3. Modal interactions
        match &self.modal {
            ModalState::HumanVerdict { .. } => {
                match key.code {
                    KeyCode::Char('p') | KeyCode::Char('P') => {
                        self.record_verdict(HumanVerdictKind::Pass);
                    }
                    KeyCode::Char('f') | KeyCode::Char('F') => {
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

        // 4. Detail drawer mode keybindings (full scrollable view)
        if self.detail_drawer_open {
            match key.code {
                KeyCode::Char('q') => {
                    self.close_test_windows();
                    self.should_quit = true;
                }
                KeyCode::Esc | KeyCode::Enter | KeyCode::Char('d') => {
                    self.detail_drawer_open = false;
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
                KeyCode::Home => {
                    self.detail_scroll = 0;
                }
                KeyCode::Char('r') => {
                    self.run_selected();
                }
                KeyCode::Char('v') => {
                    self.open_verdict_modal();
                }
                _ => {}
            }
            return;
        }

        // 5. Normal view keybindings
        match key.code {
            KeyCode::Char('q') => {
                self.close_test_windows();
                self.should_quit = true;
            }
            KeyCode::Enter | KeyCode::Char('d') => {
                self.detail_drawer_open = true;
                self.detail_scroll = 0;
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.next();
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.previous();
            }
            KeyCode::PageDown | KeyCode::Char(']') => {
                self.detail_scroll = self.detail_scroll.saturating_add(6);
            }
            KeyCode::PageUp | KeyCode::Char('[') => {
                self.detail_scroll = self.detail_scroll.saturating_sub(6);
            }
            KeyCode::Home => {
                self.detail_scroll = 0;
            }
            KeyCode::Char('f') => {
                self.cycle_filter();
            }
            KeyCode::Char('r') => {
                self.run_selected();
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
}
