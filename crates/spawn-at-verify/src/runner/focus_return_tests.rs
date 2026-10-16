//! Tests of the focus return (`focus_return.rs`) against an in-memory compositor whose behaviour
//! is scripted: it honours a `focus`, ignores the first ones, or a window grabs focus back.

use super::focus_return::{
    find_focused, return_focus, FocusCommand, FocusReport, FocusTimings, WindowRef,
};
use super::owned::{Control, WindowInfo};
use super::test_support::info;
use std::sync::Mutex;
use std::time::Duration;

fn timings() -> FocusTimings {
    FocusTimings {
        settle: Duration::from_millis(1),
        attempts: 3,
        wait: Duration::from_millis(20),
        poll: Duration::from_millis(5),
    }
}

struct Comp {
    st: Mutex<St>,
}
struct St {
    windows: Vec<WindowInfo>,
    /// `focus` calls to ignore before one works.
    ignore: usize,
    /// Exit code every `focus` reports.
    exit: i32,
    message: String,
    fail_list: bool,
    sent: Vec<u64>,
}

impl Comp {
    /// Window 1 is the terminal, 2 a fixture that has focus.
    fn new() -> Comp {
        let mut term = info(1, Some(10), "term");
        term.class = Some("gnome-terminal".into());
        let mut fx = info(2, Some(20), "verify-fixture");
        fx.focused = true;
        Comp {
            st: Mutex::new(St {
                windows: vec![term, fx],
                ignore: 0,
                exit: 0,
                message: String::new(),
                fail_list: false,
                sent: Vec::new(),
            }),
        }
    }
    fn with(self, f: impl FnOnce(&mut St)) -> Comp {
        f(&mut self.st.lock().unwrap());
        self
    }
    fn sent(&self) -> Vec<u64> {
        self.st.lock().unwrap().sent.clone()
    }
}

impl Control for Comp {
    fn windows(&self) -> Result<Vec<WindowInfo>, String> {
        let st = self.st.lock().unwrap();
        if st.fail_list {
            return Err("bus gone".into());
        }
        Ok(st.windows.clone())
    }
    fn close(&self, _id: u64) -> bool {
        unreachable!("a focus return never closes anything")
    }
    fn focus(&self, id: u64) -> bool {
        self.focus_detail(id).exit_code == Some(0)
    }
    fn focus_detail(&self, id: u64) -> FocusCommand {
        let mut st = self.st.lock().unwrap();
        st.sent.push(id);
        if st.exit == 0 {
            if st.ignore > 0 {
                st.ignore -= 1;
            } else {
                for w in &mut st.windows {
                    w.focused = w.id == id;
                }
            }
        }
        FocusCommand {
            exit_code: Some(st.exit),
            message: st.message.clone(),
        }
    }
}

fn term() -> WindowRef {
    WindowRef {
        id: 1,
        class: "gnome-terminal".into(),
        title: "term".into(),
    }
}

fn run(c: &Comp, rec: Option<&WindowRef>, forbidden: &[u64]) -> FocusReport {
    return_focus(c, rec, "note", forbidden, &timings())
}

/// The window already has focus: nothing is sent, and the report says so.
#[test]
fn test_already_focused_sends_nothing() {
    let c = Comp::new().with(|s| {
        s.windows[0].focused = true;
        s.windows[1].focused = false;
    });
    let r = run(&c, Some(&term()), &[]);
    assert!(r.matches && r.sent.is_empty());
    assert!(c.sent().is_empty());
    assert_eq!(r.lines()[1], "focus command: not sent (already focused)");
}

/// One `focus` is enough: sent once to the terminal's id and only to it, and the list confirms.
#[test]
fn test_focus_is_sent_once_and_confirmed() {
    let c = Comp::new();
    let r = run(&c, Some(&term()), &[]);
    assert!(r.matches);
    assert_eq!(c.sent(), vec![1]);
    let l = r.lines();
    assert_eq!(l[0], "terminal: id 1 class 'gnome-terminal' title 'term'");
    assert_eq!(l[1], "focus command: sent 1x: #1 exit 0");
    assert!(l[2].contains("matches terminal: yes"), "{l:?}");
}

/// A compositor that ignores the first two commands (focus not yet applied) is retried; the third
/// works, and the report shows three sends.
#[test]
fn test_ignored_focus_is_retried() {
    let c = Comp::new().with(|s| s.ignore = 2);
    let r = run(&c, Some(&term()), &[]);
    assert!(r.matches, "{:?}", r.lines());
    assert_eq!(c.sent(), vec![1, 1, 1]);
}

/// A compositor that never honours it: three sends, the failure is stated, with what had focus.
#[test]
fn test_focus_that_never_lands_is_reported_exactly() {
    let c = Comp::new().with(|s| s.ignore = 99);
    let r = run(&c, Some(&term()), &[]);
    assert!(!r.matches);
    assert_eq!(c.sent().len(), 3);
    let l = r.lines();
    assert!(l[1].starts_with("focus command: sent 3x"), "{l:?}");
    assert!(
        l[2].contains("id 2") && l[2].contains("matches terminal: NO"),
        "{l:?}"
    );
}

/// A refused command: its exit code and message are in the report.
#[test]
fn test_refused_command_shows_exit_code_and_message() {
    let c = Comp::new().with(|s| {
        s.exit = 1;
        s.message = "Window not found".into();
    });
    let r = run(&c, Some(&term()), &[]);
    assert!(!r.matches);
    assert_eq!(r.sent[0].exit_code, Some(1));
    assert!(
        r.lines()[1].contains("exit 1 'Window not found'"),
        "{:?}",
        r.lines()
    );
}

/// No terminal recorded: nothing is sent, the reason is shown.
#[test]
fn test_no_terminal_recorded_sends_nothing() {
    let c = Comp::new();
    let r = return_focus(
        &c,
        None,
        "no window reported focused (3 listed)",
        &[],
        &timings(),
    );
    assert!(c.sent().is_empty());
    assert_eq!(
        r.lines()[0],
        "terminal: none found (no window reported focused (3 listed))"
    );
    assert!(r.lines()[1].contains("not sent (no terminal window recorded)"));
}

/// The recorded window is one the runner owns, or is gone: nothing is sent either way.
#[test]
fn test_owned_or_vanished_window_is_never_targeted() {
    let c = Comp::new();
    let r = run(&c, Some(&term()), &[1]);
    assert!(c.sent().is_empty());
    assert!(r.lines()[1].contains("never targeted"), "{:?}", r.lines());
    let c = Comp::new().with(|s| s.windows.retain(|w| w.id != 1));
    let r = run(&c, Some(&term()), &[]);
    assert!(c.sent().is_empty());
    assert!(r.lines()[1].contains("no longer listed"), "{:?}", r.lines());
}

/// A window list that fails is "unknown", never read as "not focused", and nothing is sent.
#[test]
fn test_failed_list_is_unknown_not_a_mismatch() {
    let c = Comp::new().with(|s| s.fail_list = true);
    let r = run(&c, Some(&term()), &[]);
    assert!(c.sent().is_empty());
    assert!(
        r.lines()[2].contains("unknown (window query failed: bus gone)"),
        "{:?}",
        r.lines()
    );
}

/// Recording: the focused window, or the exact reason there is none.
#[test]
fn test_find_focused_names_the_window_or_the_reason() {
    let c = Comp::new();
    assert_eq!(find_focused(&c).unwrap().id, 2);
    let c = Comp::new().with(|s| s.windows.iter_mut().for_each(|w| w.focused = false));
    assert_eq!(
        find_focused(&c).unwrap_err(),
        "no window reported focused (2 listed)"
    );
    let c = Comp::new().with(|s| s.fail_list = true);
    assert_eq!(
        find_focused(&c).unwrap_err(),
        "window query failed: bus gone"
    );
}
