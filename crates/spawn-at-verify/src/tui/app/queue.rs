//! Execution queue processing, test running, and human review verdict recording.

use super::state::{ModalState, Pending, TuiApp};
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
        // The synchronous form of a verdict key: record it (`apply_verdict`), then run the
        // teardown and the rest of the queue at once. The event loop does the same in steps,
        // drawing between them.
        self.apply_verdict(verdict);
        self.run_all_pending();
    }
}

// ---------------------------------------------------------------------------------------------
// Deferred work: a key press puts its effect on screen first, the slow step runs after the
// next redraw (`Pending`, run by `run_pending` from the event loop).
// ---------------------------------------------------------------------------------------------

impl TuiApp {
    /// True when the event loop has a slow step to run after the redraw it just did.
    pub fn has_pending(&self) -> bool {
        !self.pending.is_empty()
    }

    /// Runs the next pending step, if any. The event loop calls it after drawing, so the state a
    /// key press set (a "running" row, a recorded verdict, "closing window...") is on screen
    /// before the step starts.
    pub fn run_pending(&mut self) {
        match self.pending.pop_front() {
            Some(Pending::Teardown { continue_queue }) => {
                let id = self.closing_for.take();
                let lines = self.close_test_windows_noted();
                if let Some(id) = id {
                    for line in lines {
                        self.tracker.notes.add(&id, line);
                    }
                }
                if continue_queue {
                    self.schedule_next_queued();
                }
            }
            Some(Pending::RunQueueStep) => self.run_one_queued(),
            None => {}
        }
    }

    /// Runs every pending step, including the ones they add (what the synchronous entry points
    /// and the tests use).
    pub fn run_all_pending(&mut self) {
        while self.has_pending() {
            self.run_pending();
        }
    }

    /// A result that shows a test as running, for the frame drawn before it really runs.
    fn running_placeholder(test: &TestDefinition) -> TestResult {
        TestResult {
            id: test.id.clone(),
            title: test.title.clone(),
            kind: test.kind,
            required: test.required,
            auto_status: AutoStatus::Running,
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
            flash_info: Default::default(),
            wait_info: Default::default(),
        }
    }

    /// If a test is queued and nothing waits for a person, shows it as running and schedules it.
    fn schedule_next_queued(&mut self) {
        if self.waiting_for_human.is_some() {
            return;
        }
        let Some(id) = self.queue.front().cloned() else {
            self.status_message = Some("All tests finished.".into());
            return;
        };
        if let Some(test) = self.registry.get_test(&id).cloned() {
            if let Some(pos) = self.filtered_tests().iter().position(|t| t.id == id) {
                self.selected_index = pos;
            }
            self.tracker.notes.clear(&id);
            self.results
                .insert(id.clone(), Self::running_placeholder(&test));
            self.pending.push_back(Pending::RunQueueStep);
        } else {
            self.queue.pop_front();
            self.schedule_next_queued();
        }
    }

    /// Runs the test at the front of the queue (it was shown as running), then schedules the next.
    fn run_one_queued(&mut self) {
        let Some(test_id) = self.queue.pop_front() else {
            return;
        };
        if let Some(test) = self.registry.get_test(&test_id).cloned() {
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
                    return; // the queue waits for the person
                }
            }
        }
        self.schedule_next_queued();
    }

    /// `r` on the selected test: shows it as running now, runs it after the next redraw.
    pub fn schedule_run_selected(&mut self) {
        if let Some(test) = self.selected_test().cloned() {
            self.queue.clear();
            self.queue.push_back(test.id);
            self.schedule_next_queued();
        }
    }

    /// `c` / Ctrl+R: the same for every test of the selected category.
    pub fn schedule_run_selected_category(&mut self) {
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
            self.queue.extend(ids);
            self.status_message = Some(format!("Running {} tests in category '{}'...", count, cat));
            self.schedule_next_queued();
        }
    }

    /// `a`: the same for every test matching the filters.
    pub fn schedule_run_all_filtered(&mut self) {
        self.queue.clear();
        let ids: Vec<String> = self.filtered_tests().iter().map(|t| t.id.clone()).collect();
        self.queue.extend(ids);
        self.schedule_next_queued();
    }

    /// A verdict key (`p`, `x`, `s`, `k`): records the person's answer and updates the row and
    /// the result box in the very next redraw. Closing the test window is a step for later
    /// (`Pending::Teardown`); nothing it reports can change this verdict.
    pub fn apply_verdict(&mut self, verdict: HumanVerdictKind) {
        let target = self.waiting_for_human.clone().or_else(|| {
            if let ModalState::HumanVerdict { test_id } = &self.modal {
                Some(test_id.clone())
            } else {
                self.selected_test().map(|t| t.id.clone())
            }
        });
        if let Some(test_id) = target {
            if let Some(test) = self.registry.get_test(&test_id).cloned() {
                let mut result = self
                    .results
                    .get(&test.id)
                    .cloned()
                    .unwrap_or_else(|| Self::running_placeholder(&test));
                if result.auto_status == AutoStatus::Running {
                    result.auto_status = AutoStatus::Pending;
                }
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
                if self.has_open_test_windows() {
                    self.closing_for = Some(test.id.clone());
                }
            }
        }
        self.waiting_for_human = None;
        self.modal = ModalState::None;
        self.pending.push_back(Pending::Teardown {
            continue_queue: true,
        });
    }

    /// True when a test left a window open that a teardown would close.
    fn has_open_test_windows(&self) -> bool {
        !self.tracker.owned.is_empty() || !self.tracker.tracked_test_windows.is_empty()
    }

    /// Esc while waiting for a verdict: records nothing; the window is closed after the redraw.
    pub fn cancel_review(&mut self) {
        self.closing_for = self.waiting_for_human.take();
        self.status_message = Some("Visual review cancelled.".into());
        self.pending.push_back(Pending::Teardown {
            continue_queue: false,
        });
    }

    /// `r` while waiting for a verdict: the row shows running again at once; the open window is
    /// closed and the test runs again after the redraw.
    pub fn schedule_rerun_waiting(&mut self) {
        let Some(id) = self.waiting_for_human.take() else {
            return;
        };
        self.closing_for = Some(id.clone());
        self.queue.clear();
        self.queue.push_back(id.clone());
        if let Some(test) = self.registry.get_test(&id).cloned() {
            self.results
                .insert(id.clone(), Self::running_placeholder(&test));
        }
        self.tracker.notes.clear(&id);
        self.pending.push_back(Pending::Teardown {
            continue_queue: true,
        });
    }
}
