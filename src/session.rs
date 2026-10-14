//! The end of a run: the log-out (or the X11 Shell reload) and the advice about it.
//!
//! A log-out ends the user's session, so it must be the very last thing a run does: after every
//! prompt, every write and every message. The steps of a run therefore only *request* a session
//! action and *record* the advice; [`finish`] is called once, as the last statement of the run,
//! prints the advice, and only then performs the action. A step that fails ends the run
//! (`exit(1)` or an `Err`) without calling [`finish`], so a failure never logs the user out.
//!
//! The decision of each flow is a pure function ([`install_outcome`], [`uninstall_outcome`]);
//! the performing goes through [`SessionOps`] so tests can record the order.

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

/// How the advice is printed and the action performed.
pub trait SessionOps {
    /// Starts a block of output (the blank line between blocks).
    fn block_start(&mut self);
    fn say(&mut self, text: &str);
    fn success(&mut self, text: &str);
    fn warning(&mut self, text: &str);
    fn log_out(&mut self);
    fn reload_shell_x11(&mut self);
}

/// Pending end-of-run state.
#[derive(Debug, Default)]
pub struct Session {
    action: Option<Action>,
    advice: Option<Advice>,
}

impl Session {
    pub const fn new() -> Self {
        Session {
            action: None,
            advice: None,
        }
    }

    pub fn request(&mut self, action: Action) {
        self.action = Some(action);
    }

    pub fn set_advice(&mut self, advice: Advice) {
        self.advice = Some(advice);
    }

    /// Prints the advice, then performs the action, then forgets both. The advice comes first
    /// and the action is the last thing that happens.
    pub fn finish(&mut self, ops: &mut dyn SessionOps) {
        if let Some(advice) = self.advice.take() {
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
        match self.action.take() {
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

/// The session outcome of `uninstall`. With `no_action` (the removal phase of a clean install,
/// and `--no-action`) nothing is requested and nothing is said: the install that follows
/// decides once, at its own end.
pub fn uninstall_outcome(
    is_wayland: bool,
    restart: bool,
    no_action: bool,
) -> (Option<Action>, Option<Advice>) {
    if no_action {
        return (None, None);
    }
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

static SESSION: Mutex<Session> = Mutex::new(Session::new());

/// Records the outcome of a flow for [`finish`].
pub fn record(outcome: (Option<Action>, Option<Advice>)) {
    let mut s = SESSION.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(a) = outcome.0 {
        s.request(a);
    }
    if let Some(a) = outcome.1 {
        s.set_advice(a);
    }
}

/// Requests an action (the update flow).
pub fn request(action: Action) {
    record((Some(action), None));
}

/// Prints the advice and performs the action. Call it once, as the last statement of the run.
pub fn finish() {
    let mut s = SESSION.lock().unwrap_or_else(|e| e.into_inner());
    s.finish(&mut RealOps);
}

/// The real output and the real log-out / reload.
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

    /// Writes through the real layout rule into a buffer and records the performed actions in
    /// the same list as the steps of the simulated run, so their order can be asserted.
    struct Recorder {
        layout: Layout,
        out: Vec<u8>,
        events: Vec<String>,
    }

    impl Recorder {
        fn new() -> Self {
            Recorder {
                layout: Layout::new(),
                out: Vec::new(),
                events: Vec::new(),
            }
        }
        /// A step of the run: prints a block and records the step.
        fn step(&mut self, name: &str) {
            self.layout.block_start(&mut self.out).unwrap();
            writeln!(self.out, "step {name}").unwrap();
            self.events.push(format!("step {name}"));
        }
        fn text(&self) -> String {
            String::from_utf8(self.out.clone()).unwrap()
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
        fn log_out(&mut self) {
            self.events.push("LOG OUT".into());
        }
        fn reload_shell_x11(&mut self) {
            self.events.push("RELOAD X11".into());
        }
    }

    /// The steps of an install, in order, then the end of the run.
    fn install_run(is_wayland: bool, restart: bool, not_needed: bool, headless: bool) -> Recorder {
        let mut rec = Recorder::new();
        let mut session = Session::new();
        rec.step("binary installed");
        rec.step("extension deployed");
        // the GNOME driver only records its outcome:
        let (a, adv) = install_outcome(is_wayland, restart, not_needed, headless);
        if let Some(a) = a {
            session.request(a);
        }
        if let Some(a) = adv {
            session.set_advice(a);
        }
        rec.step("update preferences");
        rec.step("settings saved");
        rec.step("autostart entry written");
        session.finish(&mut rec);
        rec
    }

    fn assert_log_out_is_last(rec: &Recorder) {
        assert_eq!(
            rec.events.last().map(String::as_str),
            Some("LOG OUT"),
            "{:?}",
            rec.events
        );
        assert_eq!(
            rec.events.iter().filter(|e| *e == "LOG OUT").count(),
            1,
            "{:?}",
            rec.events
        );
        let at = rec.events.iter().position(|e| e == "LOG OUT").unwrap();
        assert!(
            rec.events[..at]
                .iter()
                .filter(|e| e.starts_with("step"))
                .count()
                == 5,
            "every step runs before the log-out: {:?}",
            rec.events
        );
    }

    /// Interactive install, answer Yes: nothing happens until every step is done; the announce
    /// line comes right before the log-out, which is the last event.
    #[test]
    fn test_interactive_yes_logs_out_last() {
        let rec = install_run(true, true, false, false);
        assert_log_out_is_last(&rec);
        let n = rec.events.len();
        assert!(rec.events[n - 2].starts_with("say Logging out to complete"));
    }

    /// Headless --gnome-wayland-logout=true: the same, the log-out is last.
    #[test]
    fn test_headless_flag_true_logs_out_last() {
        let (a, adv) = install_outcome(true, true, false, true);
        assert_eq!(adv, None);
        assert!(matches!(a, Some(Action::LogOut { .. })));
        assert_log_out_is_last(&install_run(true, true, true, true));
    }

    /// Headless without the flag (and interactive No): no log-out; the yellow warning is the
    /// last line printed, after update preferences and the other steps.
    #[test]
    fn test_headless_false_only_advises_and_advice_is_last() {
        let rec = install_run(true, false, false, true);
        assert!(!rec.events.contains(&"LOG OUT".to_string()));
        assert_eq!(rec.events.last().map(String::as_str), Some("warning"));
        let text = rec.text();
        let last = text.lines().rev().find(|l| !l.trim().is_empty()).unwrap();
        assert_eq!(last, LOGOUT_WARNING);
        assert!(text.contains("Wayland session logout skipped (--gnome-wayland-logout=false)."));
        assert!(!text.contains("\n\n\n") && !text.starts_with('\n') && !text.ends_with("\n\n"));
    }

    /// Feature 1: the green line is the last line printed when no log-out is needed, and no
    /// log-out is performed.
    #[test]
    fn test_no_logout_needed_line_is_last() {
        let rec = install_run(true, false, true, false);
        assert_eq!(rec.events.last().map(String::as_str), Some("success"));
        let text = rec.text();
        let last = text.lines().rev().find(|l| !l.trim().is_empty()).unwrap();
        assert!(last.starts_with("No log-out needed"), "{last}");
        assert!(!text.contains(LOGOUT_WARNING));
        assert!(!rec.events.contains(&"LOG OUT".to_string()));
    }

    /// An explicit log-out request wins over "not needed" (the user or the flag asked for it),
    /// and then no advice is printed.
    #[test]
    fn test_requested_logout_beats_not_needed() {
        let (a, adv) = install_outcome(true, true, true, false);
        assert!(matches!(a, Some(Action::LogOut { .. })) && adv.is_none());
    }

    /// X11: the reload is also the last event, or the hint is the last text.
    #[test]
    fn test_x11_reload_is_last_and_hint_is_last() {
        let rec = install_run(false, true, false, false);
        assert_eq!(rec.events.last().map(String::as_str), Some("RELOAD X11"));
        let rec = install_run(false, false, false, true);
        let text = rec.text();
        assert!(text.trim_end().ends_with("pressing Enter."), "{text}");
    }

    /// Clean install with Yes: the removal phase (uninstall with no_action) requests nothing and
    /// says nothing; the single log-out comes from the install, at the end.
    #[test]
    fn test_clean_install_removal_phase_requests_no_logout() {
        for is_wayland in [true, false] {
            for restart_flag in [true, false] {
                // the clean-install removal runs with no_action=true; whatever the flags say
                assert_eq!(
                    uninstall_outcome(is_wayland, restart_flag, true),
                    (None, None)
                );
            }
        }
        let mut rec = Recorder::new();
        let mut session = Session::new();
        rec.step("removal phase");
        let (a, adv) = uninstall_outcome(true, false, true);
        assert!(a.is_none() && adv.is_none());
        rec.step("install steps");
        let (a, _) = install_outcome(true, true, false, false);
        session.request(a.unwrap());
        rec.step("update preferences");
        session.finish(&mut rec);
        assert_eq!(rec.events.iter().filter(|e| *e == "LOG OUT").count(), 1);
        assert_eq!(rec.events.last().map(String::as_str), Some("LOG OUT"));
        assert!(!rec.text().contains("logout skipped"));
    }

    /// Standalone uninstall: Yes logs out after its steps; No prints the skipped line last.
    #[test]
    fn test_standalone_uninstall() {
        let mut rec = Recorder::new();
        let mut session = Session::new();
        rec.step("binary removed");
        rec.step("extension removed");
        rec.step("autostart removed");
        let (a, _) = uninstall_outcome(true, true, false);
        session.request(a.unwrap());
        session.finish(&mut rec);
        assert_eq!(rec.events.last().map(String::as_str), Some("LOG OUT"));
        assert!(rec.events[rec.events.len() - 2]
            .starts_with("say Logging out to complete uninstallation"));

        let (a, adv) = uninstall_outcome(true, false, false);
        assert_eq!((a, adv), (None, Some(Advice::LogoutSkipped)));
    }

    /// A step that fails ends the run without calling finish(): a requested action is never
    /// performed, because only finish() performs it.
    #[test]
    fn test_failure_before_finish_performs_nothing() {
        let mut rec = Recorder::new();
        let mut session = Session::new();
        rec.step("extension deployed");
        session.request(Action::LogOut { announce: "x" });
        rec.step("settings saved FAILED");
        // the run returns/exits here; finish() is never called, so `session` is just dropped
        assert!(!rec.events.contains(&"LOG OUT".to_string()));
        assert!(!rec.text().contains("Logging out"));
    }

    /// finish() forgets what it did: a second call performs nothing.
    #[test]
    fn test_finish_runs_once() {
        let mut rec = Recorder::new();
        let mut session = Session::new();
        session.request(Action::LogOut { announce: "bye" });
        session.finish(&mut rec);
        session.finish(&mut rec);
        assert_eq!(rec.events.iter().filter(|e| *e == "LOG OUT").count(), 1);
    }

    /// The log-out is performed in one place only (`log_out_wayland`), the old "skipped" lines
    /// are not printed mid-run any more, and each flow calls finish() as its last statement.
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
        let quits: usize = files
            .iter()
            .filter(|(p, _)| !p.ends_with("session.rs"))
            .map(|(_, s)| non_test(s).matches("\"gnome-session-quit\"").count())
            .sum();
        assert_eq!(
            quits, 1,
            "gnome-session-quit must be run by log_out_wayland only"
        );
        for (p, s) in &files {
            if p.ends_with("session.rs") {
                continue;
            }
            let s = non_test(s);
            assert!(
                !s.contains("Wayland session logout skipped")
                    && !s.contains("GNOME Shell reload skipped")
                    && !s.contains("restart_gnome_shell_x11();"),
                "{p} prints or performs session advice mid-run"
            );
        }
        let main = files
            .iter()
            .find(|(p, _)| p.ends_with("src/main.rs"))
            .unwrap();
        assert_eq!(main.1.matches("crate::session::finish();").count(), 2);
    }
}
