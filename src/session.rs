//! The end of a run: the question about logging out (or reloading the X11 Shell), the advice
//! about it, and the log-out itself.
//!
//! A log-out ends the user's session, so it must be the very last thing a run does, and the
//! question about it must be the very last prompt: after every other prompt, write and message.
//! The steps of a run therefore only *record* what they know ([`record`]); [`finish`] is called
//! once, as the last statement of the run, and then asks (in a terminal), prints the advice, and
//! only then performs the action. A step that fails ends the run (`exit(1)` or an `Err`) without
//! calling [`finish`], so a failure never asks and never logs the user out.
//!
//! The decision is a pure function of what was recorded and the answer ([`outcome`]); the
//! asking, printing and performing go through [`SessionOps`] so tests can record the order.

use std::sync::Mutex;

/// What to do to the session once everything else is done.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Log out (Wayland). `announce` is printed right before.
    LogOut { announce: &'static str },
    /// Reload GNOME Shell in place (X11).
    ReloadShellX11,
}

/// The advice printed at the end instead of an action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Advice {
    /// Green: the running Shell already loaded exactly these files.
    NoLogoutNeeded,
    /// Yellow warning to log out later. `flag_note` adds the line naming the flag (headless runs).
    LogOutLater { flag_note: bool },
    /// X11 install without a reload: how to reload by hand.
    ReloadLater,
    /// Uninstall on Wayland without a log-out.
    LogoutSkipped,
    /// Uninstall on X11 without a reload.
    ReloadSkipped,
}

/// The sentence of the yellow warning (kept word for word).
pub const LOGOUT_WARNING: &str =
    "If the extension is not immediately active, log out and back in to complete loading.";

/// Which flow recorded the decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flow {
    /// `not_needed`: the facts of logout.rs say the running Shell already loaded these files.
    Install {
        not_needed: bool,
        headless: bool,
    },
    Uninstall,
    Update,
}

/// How the yes/no is obtained.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    /// Ask at the end (interactive run, no explicit flag).
    Ask,
    /// Decided by flags (headless run, `--no-action`, an explicit log-out flag): no question.
    Flag(bool),
}

/// What a flow recorded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pending {
    pub flow: Flow,
    pub is_wayland: bool,
    pub decision: Decision,
}

/// The question asked at the end: two items, the second element says whether picking it means
/// "yes, log out / reload".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Question {
    pub prompt: &'static str,
    /// Plain text printed above the question (the explanation that does not fit in it).
    pub note: &'static str,
    pub items: [(&'static str, bool); 2],
    pub default: usize,
}

/// The question for a flow and session type. Wayland: default No (a log-out closes every app).
/// X11: default Yes (the reload keeps the apps open). The texts are in `prompts.rs`.
pub fn question(flow: Flow, is_wayland: bool) -> Question {
    use crate::prompts::*;
    if is_wayland {
        Question {
            prompt: PROMPT_LOGOUT,
            note: if flow == Flow::Update {
                NOTE_LOGOUT_UPDATE
            } else {
                NOTE_LOGOUT
            },
            items: [(ITEMS_LOGOUT[0], false), (ITEMS_LOGOUT[1], true)],
            default: 0,
        }
    } else {
        Question {
            prompt: PROMPT_RELOAD,
            note: NOTE_RELOAD,
            items: [(ITEMS_RELOAD[0], true), (ITEMS_RELOAD[1], false)],
            default: 0,
        }
    }
}

/// How the question is asked, the advice printed and the action performed.
pub trait SessionOps {
    /// Starts a block of output (the blank line between blocks).
    fn block_start(&mut self);
    fn say(&mut self, text: &str);
    fn success(&mut self, text: &str);
    fn warning(&mut self, text: &str);
    /// Shows the question; `None` when it cannot be shown (then the answer is No).
    fn ask(&mut self, question: &Question) -> Option<bool>;
    fn log_out(&mut self);
    fn reload_shell_x11(&mut self);
}

/// The session outcome of `install`: `restart` is the answer or flag (log out on Wayland,
/// reload on X11), `logout_not_needed` the result of the facts in `logout.rs`.
pub fn install_outcome(
    is_wayland: bool,
    restart: bool,
    logout_not_needed: bool,
    headless: bool,
) -> (Option<Action>, Option<Advice>) {
    if restart {
        let action = if is_wayland {
            Action::LogOut {
                announce: "Logging out to complete GNOME Shell extension initialization...",
            }
        } else {
            Action::ReloadShellX11
        };
        return (Some(action), None);
    }
    let advice = if !is_wayland {
        Advice::ReloadLater
    } else if logout_not_needed {
        Advice::NoLogoutNeeded
    } else {
        Advice::LogOutLater {
            flag_note: headless,
        }
    };
    (None, Some(advice))
}

/// The session outcome of `uninstall`.
pub fn uninstall_outcome(is_wayland: bool, restart: bool) -> (Option<Action>, Option<Advice>) {
    if restart {
        let action = if is_wayland {
            Action::LogOut {
                announce: "Logging out to complete uninstallation...",
            }
        } else {
            Action::ReloadShellX11
        };
        return (Some(action), None);
    }
    (
        None,
        Some(if is_wayland {
            Advice::LogoutSkipped
        } else {
            Advice::ReloadSkipped
        }),
    )
}

/// The session outcome of the upgrade done by `update`: an action only when asked for.
pub fn update_outcome(is_wayland: bool, restart: bool) -> (Option<Action>, Option<Advice>) {
    let action = restart.then_some(if is_wayland {
        Action::LogOut {
            announce: "Logging out to complete GNOME Shell extension upgrade...",
        }
    } else {
        Action::ReloadShellX11
    });
    (action, None)
}

/// How the yes/no of a flow is obtained. `flag_restart` is `should_restart(is_wayland)` of the
/// install or uninstall flags.
///
/// - not interactive: the flags decide, no question;
/// - interactive with `--no-action`: No, no question;
/// - interactive on Wayland with `--gnome-wayland-logout`: Yes, no question (the default of that
///   flag is false, so true was asked for; on X11 the reload flag defaults to true and says
///   nothing about what the user wants, so the question is asked);
/// - otherwise: asked at the end.
pub fn decision(
    interactive: bool,
    is_wayland: bool,
    no_action: bool,
    wayland_logout_flag: bool,
    flag_restart: bool,
) -> Decision {
    if !interactive {
        Decision::Flag(flag_restart)
    } else if no_action {
        Decision::Flag(false)
    } else if is_wayland && wayland_logout_flag {
        Decision::Flag(true)
    } else {
        Decision::Ask
    }
}

/// What `uninstall` records. The removal phase of a clean install (and `--no-action`) records
/// nothing, so it can neither ask nor say anything: the install that follows decides, once.
pub fn uninstall_pending(is_wayland: bool, decision: Decision, no_action: bool) -> Option<Pending> {
    (!no_action).then_some(Pending {
        flow: Flow::Uninstall,
        is_wayland,
        decision,
    })
}

/// Pending end-of-run state.
#[derive(Debug, Default)]
pub struct Session {
    pending: Option<Pending>,
}

impl Session {
    pub const fn new() -> Self {
        Session { pending: None }
    }

    pub fn record(&mut self, pending: Pending) {
        self.pending = Some(pending);
    }

    /// Asks the question if one is due (the last prompt of the run), prints the advice, then
    /// performs the action, which is the last thing that happens. Forgets what was recorded, so
    /// a second call does nothing.
    pub fn finish(&mut self, ops: &mut dyn SessionOps) {
        let Some(p) = self.pending.take() else {
            return;
        };
        // Feature 1: the running Shell already loaded these files, so nothing is asked.
        let not_needed = matches!(
            p.flow,
            Flow::Install {
                not_needed: true,
                ..
            }
        ) && p.is_wayland;
        let restart = match p.decision {
            Decision::Flag(b) => b,
            Decision::Ask if not_needed => false,
            Decision::Ask => {
                ops.block_start();
                let q = question(p.flow, p.is_wayland);
                ops.say(q.note);
                ops.ask(&q).unwrap_or(false)
            }
        };
        let (action, advice) = match p.flow {
            Flow::Install {
                not_needed,
                headless,
            } => install_outcome(p.is_wayland, restart, not_needed, headless),
            Flow::Uninstall => uninstall_outcome(p.is_wayland, restart),
            Flow::Update => update_outcome(p.is_wayland, restart),
        };
        if let Some(advice) = advice {
            ops.block_start();
            match advice {
                Advice::NoLogoutNeeded => ops.success(
                    "No log-out needed: the extension files are unchanged and the running GNOME Shell has already loaded them.",
                ),
                Advice::LogOutLater { flag_note } => {
                    if flag_note {
                        ops.say("Wayland session logout skipped (--gnome-wayland-logout=false).");
                    }
                    ops.warning(LOGOUT_WARNING);
                }
                Advice::ReloadLater => {
                    ops.say("GNOME Shell reload skipped (--gnome-x11-reload=false).");
                    ops.say("You can reload the shell anytime on X11 by pressing Alt+F2, typing 'r', and pressing Enter.");
                }
                Advice::LogoutSkipped => {
                    ops.say("Wayland session logout skipped (--gnome-wayland-logout=false).")
                }
                Advice::ReloadSkipped => {
                    ops.say("GNOME Shell reload skipped (--gnome-x11-reload=false).")
                }
            }
        }
        match action {
            Some(Action::LogOut { announce }) => {
                ops.block_start();
                ops.say(announce);
                ops.log_out();
            }
            Some(Action::ReloadShellX11) => {
                ops.block_start();
                ops.reload_shell_x11();
            }
            None => {}
        }
    }
}

static SESSION: Mutex<Session> = Mutex::new(Session::new());

/// Records what a flow knows, for [`finish`]. Does not ask, print or act.
pub fn record(pending: Pending) {
    SESSION
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .record(pending);
}

/// Asks (if due), prints the advice and performs the action. Call it once, as the last
/// statement of the run.
pub fn finish() {
    let mut s = SESSION.lock().unwrap_or_else(|e| e.into_inner());
    s.finish(&mut RealOps);
}

/// The real prompt, output and the real log-out / reload.
struct RealOps;

impl SessionOps for RealOps {
    fn block_start(&mut self) {
        crate::ui::block_start();
    }
    fn say(&mut self, text: &str) {
        println!("{text}");
    }
    fn success(&mut self, text: &str) {
        crate::ui::println_success(text);
    }
    fn warning(&mut self, text: &str) {
        crate::ui::println_warning(text);
    }
    fn ask(&mut self, q: &Question) -> Option<bool> {
        let labels: Vec<&str> = q.items.iter().map(|i| i.0).collect();
        let picked = dialoguer::Select::new()
            .with_prompt(q.prompt)
            .items(&labels)
            .default(q.default)
            .interact()
            .ok()?;
        Some(q.items[picked].1)
    }
    fn log_out(&mut self) {
        #[cfg(target_os = "linux")]
        crate::platform::linux::gnome::log_out_wayland();
    }
    fn reload_shell_x11(&mut self) {
        #[cfg(target_os = "linux")]
        crate::platform::linux::gnome::restart_gnome_shell_x11();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::Layout;
    use std::io::Write;

    /// Writes through the real layout rule into a buffer; records steps, prompts, answers and
    /// performed actions in one list so their order can be asserted.
    struct Recorder {
        layout: Layout,
        out: Vec<u8>,
        events: Vec<String>,
        answer: Option<bool>,
    }

    impl Recorder {
        fn new(answer: Option<bool>) -> Self {
            Recorder {
                layout: Layout::new(),
                out: Vec::new(),
                events: Vec::new(),
                answer,
            }
        }
        fn step(&mut self, name: &str) {
            self.layout.block_start(&mut self.out).unwrap();
            writeln!(self.out, "step {name}").unwrap();
            self.events.push(format!("step {name}"));
        }
        /// A prompt of the run (not the log-out question).
        fn prompt(&mut self, name: &str) {
            self.layout.block_start(&mut self.out).unwrap();
            writeln!(self.out, "? {name}").unwrap();
            self.events.push(format!("PROMPT {name}"));
        }
        fn text(&self) -> String {
            String::from_utf8(self.out.clone()).unwrap()
        }
        fn last_line(&self) -> String {
            self.text()
                .lines()
                .rev()
                .find(|l| !l.trim().is_empty())
                .unwrap()
                .to_string()
        }
        fn count(&self, e: &str) -> usize {
            self.events.iter().filter(|x| x.as_str() == e).count()
        }
        fn last(&self) -> &str {
            self.events.last().map(String::as_str).unwrap()
        }
    }

    impl SessionOps for Recorder {
        fn block_start(&mut self) {
            self.layout.block_start(&mut self.out).unwrap();
        }
        fn say(&mut self, text: &str) {
            writeln!(self.out, "{text}").unwrap();
            self.events.push(format!("say {text}"));
        }
        fn success(&mut self, text: &str) {
            writeln!(self.out, "{text}").unwrap();
            self.events.push("success".into());
        }
        fn warning(&mut self, text: &str) {
            writeln!(self.out, "{text}").unwrap();
            self.events.push("warning".into());
        }
        fn ask(&mut self, q: &Question) -> Option<bool> {
            writeln!(self.out, "? {}", q.prompt).unwrap();
            self.events.push("ASK".into());
            self.answer
        }
        fn log_out(&mut self) {
            self.events.push("LOG OUT".into());
        }
        fn reload_shell_x11(&mut self) {
            self.events.push("RELOAD X11".into());
        }
    }

    /// The steps of an install, in order: the driver only records; the Update Preferences
    /// prompts and the settings come after it; then the end of the run.
    fn install_run(
        is_wayland: bool,
        decision: Decision,
        not_needed: bool,
        headless: bool,
        answer: Option<bool>,
    ) -> Recorder {
        let mut rec = Recorder::new(answer);
        let mut session = Session::new();
        rec.step("binary installed");
        rec.step("extension deployed");
        session.record(Pending {
            flow: Flow::Install {
                not_needed,
                headless,
            },
            is_wayland,
            decision,
        });
        rec.prompt("update channel");
        rec.prompt("update notifications");
        rec.step("settings saved");
        rec.step("autostart entry written");
        session.finish(&mut rec);
        rec
    }

    /// The log-out question is asked once, after every step and every other prompt.
    fn assert_question_is_last_prompt(rec: &Recorder) {
        assert_eq!(rec.count("ASK"), 1, "{:?}", rec.events);
        let ask = rec.events.iter().position(|e| e == "ASK").unwrap();
        let before = &rec.events[..ask];
        assert_eq!(before.iter().filter(|e| e.starts_with("PROMPT")).count(), 2);
        assert_eq!(before.iter().filter(|e| e.starts_with("step")).count(), 4);
        assert!(rec.events[ask + 1..]
            .iter()
            .all(|e| !e.starts_with("PROMPT") && !e.starts_with("step")));
    }

    fn no_double_blank(rec: &Recorder) {
        let t = rec.text();
        assert!(
            !t.contains("\n\n\n") && !t.starts_with('\n') && !t.ends_with("\n\n"),
            "{t:?}"
        );
    }

    /// Interactive install, Wayland, answer Yes: the question comes after Update Preferences and
    /// the settings, the announce line follows, and the log-out is the last event.
    #[test]
    fn test_interactive_install_yes_asks_last_and_logs_out_last() {
        let rec = install_run(true, Decision::Ask, false, false, Some(true));
        assert_question_is_last_prompt(&rec);
        assert_eq!(rec.last(), "LOG OUT");
        assert_eq!(rec.count("LOG OUT"), 1);
        assert!(rec.events[rec.events.len() - 2].starts_with("say Logging out to complete"));
        no_double_blank(&rec);
    }

    /// Answer No (or a question that cannot be shown): no log-out, and the yellow warning is
    /// the last non-blank line.
    #[test]
    fn test_interactive_install_no_leaves_the_warning_last() {
        for answer in [Some(false), None] {
            let rec = install_run(true, Decision::Ask, false, false, answer);
            assert_question_is_last_prompt(&rec);
            assert_eq!(rec.count("LOG OUT"), 0);
            assert_eq!(rec.last(), "warning");
            assert_eq!(rec.last_line(), LOGOUT_WARNING);
            no_double_blank(&rec);
        }
    }

    /// Feature 1: nothing is asked when no log-out is needed; the green line is last.
    #[test]
    fn test_skip_case_asks_nothing() {
        let rec = install_run(true, Decision::Ask, true, false, Some(true));
        assert_eq!(rec.count("ASK"), 0);
        assert_eq!(rec.count("LOG OUT"), 0);
        assert_eq!(rec.last(), "success");
        assert!(rec.last_line().starts_with("No log-out needed"));
        no_double_blank(&rec);
    }

    /// Headless runs ask nothing: the flag decides, and a log-out is still last.
    #[test]
    fn test_headless_runs_ask_nothing() {
        let yes = install_run(true, Decision::Flag(true), false, true, Some(false));
        assert_eq!(yes.count("ASK"), 0);
        assert_eq!(yes.last(), "LOG OUT");
        let no = install_run(true, Decision::Flag(false), false, true, Some(true));
        assert_eq!(no.count("ASK"), 0);
        assert_eq!(no.last_line(), LOGOUT_WARNING);
        assert!(no.text().contains("--gnome-wayland-logout=false"));
        no_double_blank(&no);
    }

    /// An explicit log-out request skips the question and acts at the end, even interactively;
    /// it also beats "not needed".
    #[test]
    fn test_explicit_request_skips_the_question() {
        assert_eq!(
            decision(true, true, false, true, true),
            Decision::Flag(true)
        );
        for not_needed in [false, true] {
            let rec = install_run(true, Decision::Flag(true), not_needed, false, None);
            assert_eq!(rec.count("ASK"), 0);
            assert_eq!(rec.last(), "LOG OUT");
        }
    }

    /// How flags and the terminal choose between asking and not asking.
    #[test]
    fn test_decision_table() {
        // (interactive, wayland, no_action, wayland_logout flag, flag_restart)
        assert_eq!(
            decision(false, true, false, true, true),
            Decision::Flag(true)
        );
        assert_eq!(
            decision(false, true, false, false, false),
            Decision::Flag(false)
        );
        assert_eq!(
            decision(false, false, false, false, true),
            Decision::Flag(true)
        );
        assert_eq!(
            decision(true, true, true, true, false),
            Decision::Flag(false)
        );
        assert_eq!(decision(true, true, false, false, false), Decision::Ask);
        // X11: the reload flag defaults to true and says nothing, so the question is asked
        assert_eq!(decision(true, false, false, false, true), Decision::Ask);
    }

    /// X11: the reload question is also asked last; Yes reloads last, No leaves the hint last.
    #[test]
    fn test_x11_reload_question_is_last() {
        let yes = install_run(false, Decision::Ask, false, false, Some(true));
        assert_question_is_last_prompt(&yes);
        assert_eq!(yes.last(), "RELOAD X11");
        let no = install_run(false, Decision::Ask, false, false, Some(false));
        assert_question_is_last_prompt(&no);
        assert!(no.text().trim_end().ends_with("pressing Enter."));
        assert_eq!(no.count("RELOAD X11"), 0);
    }

    /// The Wayland question defaults to No (a log-out closes every app); X11 defaults to Yes.
    #[test]
    fn test_question_defaults() {
        for flow in [
            Flow::Uninstall,
            Flow::Update,
            Flow::Install {
                not_needed: false,
                headless: false,
            },
        ] {
            let w = question(flow, true);
            assert!(!w.items[w.default].1, "Wayland default must be No");
            let x = question(flow, false);
            assert!(
                x.items[x.default].1,
                "X11 default is the non-destructive reload"
            );
        }
    }

    /// Clean install, answer Yes: the removal phase records nothing (so it asks and says
    /// nothing), the install records once, and the one question and the one log-out come last.
    #[test]
    fn test_clean_install_asks_once_at_the_end() {
        assert_eq!(uninstall_pending(true, Decision::Ask, true), None);
        assert_eq!(uninstall_pending(true, Decision::Flag(true), true), None);
        let mut rec = Recorder::new(Some(true));
        let mut session = Session::new();
        rec.step("removal phase");
        if let Some(p) = uninstall_pending(true, Decision::Ask, true) {
            session.record(p);
        }
        rec.step("install steps");
        session.record(Pending {
            flow: Flow::Install {
                not_needed: false,
                headless: false,
            },
            is_wayland: true,
            decision: Decision::Ask,
        });
        rec.prompt("update channel");
        rec.prompt("update notifications");
        rec.step("settings saved");
        session.finish(&mut rec);
        assert_eq!(rec.count("ASK"), 1);
        assert_eq!(rec.last(), "LOG OUT");
        let ask = rec.events.iter().position(|e| e == "ASK").unwrap();
        assert!(
            rec.events[..ask]
                .iter()
                .filter(|e| e.starts_with("PROMPT"))
                .count()
                == 2
        );
        assert!(!rec.text().contains("logout skipped"));
        no_double_blank(&rec);
    }

    /// Standalone uninstall: the question is last; Yes logs out last; No prints the skipped line.
    #[test]
    fn test_standalone_uninstall() {
        for (answer, last) in [
            (Some(true), "LOG OUT"),
            (
                Some(false),
                "say Wayland session logout skipped (--gnome-wayland-logout=false).",
            ),
        ] {
            let mut rec = Recorder::new(answer);
            let mut session = Session::new();
            rec.prompt("delete extension files");
            rec.step("binary removed");
            rec.step("extension removed");
            session.record(uninstall_pending(true, Decision::Ask, false).unwrap());
            rec.step("autostart removed");
            session.finish(&mut rec);
            assert_eq!(rec.count("ASK"), 1);
            let ask = rec.events.iter().position(|e| e == "ASK").unwrap();
            assert!(
                rec.events[..ask]
                    .iter()
                    .filter(|e| e.starts_with("step"))
                    .count()
                    == 3
            );
            assert_eq!(rec.last(), last);
        }
    }

    /// The update flow: the question comes after the upgrade steps; Yes logs out last, with the
    /// upgrade announce line; No performs nothing and prints nothing more.
    #[test]
    fn test_update_flow_asks_last() {
        for (answer, logs_out) in [(Some(true), true), (Some(false), false)] {
            let mut rec = Recorder::new(answer);
            let mut session = Session::new();
            rec.step("binary replaced");
            session.record(Pending {
                flow: Flow::Update,
                is_wayland: true,
                decision: Decision::Ask,
            });
            rec.step("cached versions saved");
            rec.step("success message");
            session.finish(&mut rec);
            assert_eq!(rec.count("ASK"), 1);
            assert_eq!(rec.count("LOG OUT") == 1, logs_out);
            if logs_out {
                assert_eq!(rec.last(), "LOG OUT");
                assert!(rec.events[rec.events.len() - 2].contains("upgrade"));
            } else {
                assert_eq!(rec.last(), "ASK");
            }
        }
    }

    /// A step that fails ends the run without calling finish(): nothing is asked and nobody is
    /// logged out, because only finish() asks and performs.
    #[test]
    fn test_failing_step_asks_and_does_nothing() {
        let mut rec = Recorder::new(Some(true));
        let mut session = Session::new();
        rec.step("extension deployed");
        session.record(Pending {
            flow: Flow::Install {
                not_needed: false,
                headless: false,
            },
            is_wayland: true,
            decision: Decision::Ask,
        });
        rec.step("settings saved FAILED");
        // the run exits here; finish() is never called
        assert_eq!(rec.count("ASK"), 0);
        assert_eq!(rec.count("LOG OUT"), 0);
        assert!(!rec.text().contains("log out"));
    }

    /// The end-of-run question fits (prompt, items, note-free) in every flow and session type,
    /// has no trailing colon, and carries the explanation as a note above it.
    #[test]
    fn test_end_question_fits_the_prompt_limits() {
        for flow in [
            Flow::Uninstall,
            Flow::Update,
            Flow::Install {
                not_needed: false,
                headless: false,
            },
        ] {
            for is_wayland in [true, false] {
                let q = question(flow, is_wayland);
                let items: Vec<&str> = q.items.iter().map(|i| i.0).collect();
                crate::prompts::check(q.prompt, &items).unwrap();
                assert!(!q.note.is_empty());
            }
        }
    }

    /// finish() forgets what it did: a second call asks and does nothing.
    #[test]
    fn test_finish_runs_once() {
        let mut rec = Recorder::new(Some(true));
        let mut session = Session::new();
        session.record(Pending {
            flow: Flow::Update,
            is_wayland: true,
            decision: Decision::Ask,
        });
        session.finish(&mut rec);
        session.finish(&mut rec);
        assert_eq!(rec.count("ASK"), 1);
        assert_eq!(rec.count("LOG OUT"), 1);
    }

    /// The log-out question text is in finish()'s module only, so no flow can ask it in the
    /// middle; the log-out is performed in one place; the old mid-run "skipped" lines are gone;
    /// each flow calls finish() as its last statement.
    #[test]
    fn test_source_guards() {
        fn all_rs(dir: &std::path::Path, out: &mut Vec<(String, String)>) {
            for e in std::fs::read_dir(dir).unwrap().flatten() {
                let p = e.path();
                if p.is_dir() {
                    all_rs(&p, out);
                } else if p.extension().is_some_and(|x| x == "rs") {
                    out.push((
                        p.display().to_string(),
                        std::fs::read_to_string(&p).unwrap(),
                    ));
                }
            }
        }
        let mut files = Vec::new();
        all_rs(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
            &mut files,
        );
        let non_test = |s: &str| s.split("#[cfg(test)]").next().unwrap().to_string();
        let mut quits = 0;
        for (p, s) in &files {
            let s = non_test(s);
            if p.ends_with("session.rs") || p.ends_with("prompts.rs") {
                continue;
            }
            quits += s.matches("\"gnome-session-quit\"").count();
            for forbidden in [
                "Log out of your session now",
                "Reload GNOME Shell now",
                "Wayland session logout skipped",
                "GNOME Shell reload skipped",
                "restart_gnome_shell_x11();",
            ] {
                assert!(
                    !s.contains(forbidden),
                    "{p} contains {forbidden:?} outside session.rs"
                );
            }
        }
        assert_eq!(
            quits, 1,
            "gnome-session-quit must be run by log_out_wayland only"
        );
        let session = files
            .iter()
            .find(|(p, _)| p.ends_with("src/session.rs"))
            .unwrap();
        let session = non_test(&session.1);
        assert!(!session.contains("Log out of your session now"));
        let prompts = files
            .iter()
            .find(|(p, _)| p.ends_with("src/prompts.rs"))
            .unwrap();
        assert_eq!(
            prompts
                .1
                .clone()
                .matches("Log out of your session now")
                .count(),
            1
        );
        let main = files
            .iter()
            .find(|(p, _)| p.ends_with("src/main.rs"))
            .unwrap();
        assert_eq!(main.1.matches("crate::session::finish();").count(), 2);
        let update = files
            .iter()
            .find(|(p, _)| p.ends_with("src/update.rs"))
            .unwrap();
        assert_eq!(
            non_test(&update.1)
                .matches("crate::session::finish();")
                .count(),
            1
        );
    }
}
