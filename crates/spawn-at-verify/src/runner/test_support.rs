//! Fakes shared by the tests of the setup machinery and of the version 2 executor: a shell script
//! standing in for the fixture, an in-memory `spawn-at query windows` / `close` / `focus`, and an
//! action runner that answers from a closure. The fixtures are real child processes, so "the
//! child is dead" and "the signal came after the close" are observed, not assumed.

use super::owned::{ActionRunner, Control, SetupRequest, SetupSession, Timings, WindowInfo};
use crate::oracle::Rect;
use crate::process::ProcessOutput;
use std::collections::HashMap;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

static SEQ: AtomicUsize = AtomicUsize::new(0);

const SCRIPT: &str = r#"#!/bin/sh
D="$(dirname "$0")"
title=""
while [ $# -gt 0 ]; do
  case "$1" in --title) title="$2"; shift ;; esac
  shift
done
mode="$(cat "$D/mode" 2>/dev/null)"
case "$mode" in
  exit-now) echo "boom" >&2; exit 3 ;;
  no-window) exec sleep 30 ;;
  ignore-term) trap '' TERM; printf '%s %s\n' "$$" "$title" >> "$D/state"; while :; do sleep 0.05; done ;;
  record-term) trap 'echo TERM >> "$D/events"; exit 0' TERM; printf '%s %s\n' "$$" "$title" >> "$D/state"; while :; do sleep 0.05; done ;;
  *) printf '%s %s\n' "$$" "$title" >> "$D/state"; exec sleep 30 ;;
esac
"#;

/// Scratch directory holding the fake fixture and the files it and the fake control share.
pub(super) struct Env {
    pub(super) dir: PathBuf,
}

impl Env {
    pub(super) fn new(mode: &str) -> Env {
        let dir = std::env::temp_dir().join(format!(
            "spawn-at-owned-test-{}-{}",
            std::process::id(),
            SEQ.fetch_add(1, Ordering::SeqCst)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let script = dir.join("fixture");
        std::fs::write(&script, SCRIPT).unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        let env = Env { dir };
        env.set_mode(mode);
        env
    }
    pub(super) fn set_mode(&self, mode: &str) {
        std::fs::write(self.dir.join("mode"), mode).unwrap();
    }
    pub(super) fn fixture(&self) -> PathBuf {
        self.dir.join("fixture")
    }
    pub(super) fn events(&self) -> Vec<String> {
        std::fs::read_to_string(self.dir.join("events"))
            .unwrap_or_default()
            .lines()
            .map(str::to_string)
            .collect()
    }
    /// The pids the fake fixtures have registered, in launch order.
    pub(super) fn fixture_pids(&self) -> Vec<u32> {
        std::fs::read_to_string(self.dir.join("state"))
            .unwrap_or_default()
            .lines()
            .filter_map(|l| l.split_once(' ').and_then(|(p, _)| p.parse().ok()))
            .collect()
    }
}

impl Drop for Env {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// Changes to what a listed window reports.
#[derive(Default, Clone)]
pub(super) struct Patch {
    pub(super) focused: Option<bool>,
    pub(super) maximized: Option<bool>,
    pub(super) minimized: Option<bool>,
    pub(super) rect: Option<Rect>,
}

#[derive(Default)]
pub(super) struct FakeState {
    pub(super) hidden: Vec<u64>,
    pub(super) extra: Vec<WindowInfo>,
    pub(super) retitle: HashMap<u64, String>,
    pub(super) patches: HashMap<u64, Patch>,
    pub(super) fail_query: bool,
    pub(super) ignore_close: bool,
    pub(super) close_calls: Vec<u64>,
    pub(super) focus_calls: Vec<u64>,
    /// List the `extra` windows only once the fake fixture has registered its own, so a test
    /// that wants "our window plus a second one" does not race the fixture's start.
    pub(super) extra_needs_state: bool,
}

/// In-memory `spawn-at`: the windows are the lines the fake fixture wrote (`<pid> <title>`), ids
/// 1000, 1001, ..., plus whatever the test adds.
pub(super) struct Fake {
    pub(super) dir: PathBuf,
    pub(super) st: Mutex<FakeState>,
}

impl Fake {
    pub(super) fn new(env: &Env) -> Arc<Fake> {
        Arc::new(Fake {
            dir: env.dir.clone(),
            st: Mutex::new(FakeState::default()),
        })
    }
    pub(super) fn close_calls(&self) -> Vec<u64> {
        self.st.lock().unwrap().close_calls.clone()
    }
    pub(super) fn focus_calls(&self) -> Vec<u64> {
        self.st.lock().unwrap().focus_calls.clone()
    }
    pub(super) fn patch(&self, id: u64, f: impl FnOnce(&mut Patch)) {
        let mut st = self.st.lock().unwrap();
        f(st.patches.entry(id).or_default());
    }
}

pub(super) fn info(id: u64, pid: Option<u64>, title: &str) -> WindowInfo {
    WindowInfo {
        id,
        pid,
        title: title.to_string(),
        app_id: None,
        class: None,
        rect: None,
        focused: false,
        maximized: false,
        minimized: false,
    }
}

impl Control for Fake {
    fn windows(&self) -> Result<Vec<WindowInfo>, String> {
        let st = self.st.lock().unwrap();
        if st.fail_query {
            return Err("query windows failed (exit Some(1)): no extension".into());
        }
        let text = std::fs::read_to_string(self.dir.join("state")).unwrap_or_default();
        let mut out = Vec::new();
        for (i, line) in text.lines().enumerate() {
            let Some((pid, title)) = line.split_once(' ') else {
                continue; // a line still being written
            };
            let id = 1000 + i as u64;
            if st.hidden.contains(&id) {
                continue;
            }
            let title = st.retitle.get(&id).cloned().unwrap_or(title.to_string());
            out.push(info(id, Some(pid.parse().unwrap()), &title));
        }
        if !st.extra_needs_state || !out.is_empty() {
            out.extend(st.extra.iter().cloned());
        }
        for w in &mut out {
            if let Some(p) = st.patches.get(&w.id) {
                if let Some(v) = p.focused {
                    w.focused = v;
                }
                if let Some(v) = p.maximized {
                    w.maximized = v;
                }
                if let Some(v) = p.minimized {
                    w.minimized = v;
                }
                if p.rect.is_some() {
                    w.rect = p.rect;
                }
            }
        }
        Ok(out)
    }

    fn close(&self, id: u64) -> bool {
        use std::io::Write;
        let mut st = self.st.lock().unwrap();
        st.close_calls.push(id);
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.dir.join("events"))
            .unwrap();
        writeln!(f, "close {id}").unwrap();
        if !st.ignore_close {
            st.hidden.push(id);
        }
        true
    }

    fn focus(&self, id: u64) -> bool {
        self.st.lock().unwrap().focus_calls.push(id);
        true
    }
}

/// An [`ActionRunner`] that records every call and answers from a closure.
pub(super) struct FakeActions {
    pub(super) calls: Mutex<Vec<Vec<String>>>,
    #[allow(clippy::type_complexity)]
    handler: Mutex<Box<dyn FnMut(&[String]) -> ProcessOutput + Send>>,
}

impl FakeActions {
    pub(super) fn new(
        handler: impl FnMut(&[String]) -> ProcessOutput + Send + 'static,
    ) -> Arc<FakeActions> {
        Arc::new(FakeActions {
            calls: Mutex::new(Vec::new()),
            handler: Mutex::new(Box::new(handler)),
        })
    }
    /// An action runner whose every command exits 0 and prints nothing.
    pub(super) fn ok() -> Arc<FakeActions> {
        Self::new(|_| out(0, "", ""))
    }
    pub(super) fn calls(&self) -> Vec<Vec<String>> {
        self.calls.lock().unwrap().clone()
    }
}

impl ActionRunner for FakeActions {
    fn run(&self, args: &[String], _timeout: Duration) -> Result<ProcessOutput, String> {
        self.calls.lock().unwrap().push(args.to_vec());
        Ok((self.handler.lock().unwrap())(args))
    }
}

/// A process result with this exit code, standard output and standard error.
pub(super) fn out(code: i32, stdout: &str, stderr: &str) -> ProcessOutput {
    ProcessOutput {
        exit_code: Some(code),
        stdout: stdout.as_bytes().to_vec(),
        stderr: stderr.as_bytes().to_vec(),
    }
}

pub(super) fn fast() -> Timings {
    Timings {
        setup_default: Duration::from_millis(1500),
        poll: Duration::from_millis(20),
        exit_grace: Duration::from_millis(300),
        term_grace: Duration::from_millis(300),
        check_wait: Duration::from_millis(200),
        settle: Duration::from_millis(200),
    }
}

pub(super) fn session(env: &Env, fake: &Arc<Fake>) -> SetupSession {
    let ctl: Arc<dyn Control> = fake.clone();
    SetupSession::new(
        ctl,
        env.fixture(),
        "t",
        10_000,
        fast(),
        Arc::new(AtomicBool::new(false)),
    )
    .unwrap()
}

pub(super) fn req(name: &str) -> SetupRequest {
    SetupRequest {
        name: name.to_string(),
        args: vec!["--width".into(), "400".into()],
        timeout: Some(Duration::from_millis(400)),
        ..Default::default()
    }
}

/// True while a process with this pid exists (a zombie counts; a reaped child does not).
pub(super) fn alive(pid: u32) -> bool {
    // SAFETY: signal 0 only checks that the pid exists.
    unsafe { libc::kill(pid as libc::pid_t, 0) == 0 }
}
