//! Keyboard event handling and navigation dispatch.

use super::state::{ActivePane, ModalState, TuiApp};
use crate::runner::HumanVerdictKind;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

impl TuiApp {
    /// Handles keyboard events, dispatching to raw shortcuts, modals, drawer scrolling, or list navigation.
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
}
