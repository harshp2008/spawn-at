//! Execution queue processing, test running, and human review verdict recording.

use super::state::{ModalState, TuiApp};
use crate::preflight::PreflightReport;
use crate::probe::EnvironmentInfo;
use crate::runner::{
    resolve_final_status, AutoStatus, FinalStatus, HumanVerdict, HumanVerdictKind, TestResult,
    TestRunner,
};
use crate::schema::{TestDefinition, TestKind};

impl TuiApp {
    /// Queues all tests in the currently selected category for execution.
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

    /// Queues the currently selected individual test for execution.
    pub fn run_selected(&mut self) {
        if let Some(test) = self.selected_test().cloned() {
            self.queue.clear();
            self.queue.push_back(test.id);
            self.process_queue_step();
        }
    }

    /// Queues all tests matching current category and mode filters for execution.
    pub fn run_all_filtered(&mut self) {
        self.queue.clear();
        let ids: Vec<String> = self.filtered_tests().iter().map(|t| t.id.clone()).collect();
        for id in ids {
            self.queue.push_back(id);
        }
        self.process_queue_step();
    }

    /// Processes a single step in the execution queue, pausing on human review cases.
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

    /// Executes a single test case through the runner and records its result.
    pub(crate) fn execute_test(&mut self, test: &TestDefinition) {
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
            independent_workarea: None,
        });

        let load_start = crate::loadavg::read_loadavg();
        let result = runner.execute_single_test(test, &preflight, &env_info, &mut self.tracker);
        self.run_load = Some(super::state::RunLoad {
            start: load_start,
            end: crate::loadavg::read_loadavg(),
            cpus: crate::loadavg::cpu_count(),
        });
        self.status_message = Some(format!(
            "Ran {}: {:?} ({}ms)",
            test.id, result.final_status, result.duration_ms
        ));
        self.results.insert(test.id.clone(), result);
    }

    /// Displays the human review modal overlay for recording verdicts.
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

    /// Records human verdict for a test and resumes the execution queue.
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
                            trace_note: None,
                            final_rect: None,
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
}
