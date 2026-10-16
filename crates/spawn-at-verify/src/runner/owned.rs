//! Windows and processes a version 2 test launches itself (`docs/DESIGN_RUNNER_SETUP.md` section 3).
//!
//! The rule this module exists to keep: **the runner acts only on windows and processes it created
//! itself in this test, identified by data it recorded itself, and refuses to act on anything
//! else.**
//!
//! - The fixture is started directly by the runner, so the runner holds its [`Child`]. A signal is
//!   only ever sent through that handle, and only while the child has not been reaped (so the pid
//!   cannot have been recycled). A pid read from `query windows`, `/proc` or a title is never a
//!   target.
//! - Every fixture gets a title no other process has: `verify-<run tag>-<name>`. The window is
//!   recorded only when exactly one listed window carries that title and its pid is the child's.
//! - A window is closed by its recorded id, and only if the id still carries that title.
//! - A [`SetupSession`] tears everything down in `Drop`, so a failed setup, a timeout, a panic and
//!   every early return end the same way.

use crate::oracle::Rect;
use crate::schema::v2::{self, Bindings};
use std::os::unix::fs::DirBuilderExt;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Crash backstop passed to every Auto fixture (`--exit-after-ms`): a SIGKILLed runner leaves a
/// fixture that ends itself (same value as `build_action_command_args`).
pub const EXIT_AFTER_AUTO_MS: u64 = 10_000;

/// Crash backstop for tests a human watches (Human, Both): five minutes.
pub const EXIT_AFTER_HUMAN_MS: u64 = 300_000;

/// Counter that makes each session's private directory and run tag unique within the process.
static SESSION_SEQ: AtomicUsize = AtomicUsize::new(0);

/// One window as `spawn-at query windows --json` lists it.
#[derive(Debug, Clone, PartialEq)]
pub struct WindowInfo {
    /// Compositor window id.
    pub id: u64,
    /// Process id the backend reports for the window, when it reports one.
    pub pid: Option<u64>,
    /// Window title.
    pub title: String,
    /// Wayland application id, if any.
    pub app_id: Option<String>,
    /// Window class, if any.
    pub class: Option<String>,
    /// Frame rectangle, when all four numbers are listed.
    pub rect: Option<Rect>,
    /// The window has keyboard focus.
    pub focused: bool,
    /// The window is maximized.
    pub maximized: bool,
    /// The window is minimized.
    pub minimized: bool,
}

/// Parses the JSON array printed by `spawn-at query windows --json`.
pub fn parse_windows(json: &[u8]) -> Result<Vec<WindowInfo>, String> {
    let parsed: serde_json::Value =
        serde_json::from_slice(json).map_err(|e| format!("cannot parse the window list: {e}"))?;
    let list = parsed
        .as_array()
        .ok_or_else(|| "the window list is not a JSON array".to_string())?;
    let mut out = Vec::new();
    for w in list {
        let Some(id) = w.get("id").and_then(|v| v.as_u64()) else {
            continue;
        };
        let num = |k: &str| w.get(k).and_then(|v| v.as_i64());
        let rect = match (num("x"), num("y"), num("w"), num("h")) {
            (Some(x), Some(y), Some(ww), Some(hh)) => Some(Rect::new(
                i32::try_from(x).unwrap_or(0),
                i32::try_from(y).unwrap_or(0),
                u32::try_from(ww).unwrap_or(0),
                u32::try_from(hh).unwrap_or(0),
            )),
            _ => None,
        };
        let text = |k: &str| w.get(k).and_then(|v| v.as_str()).map(str::to_string);
        let flag = |k: &str| w.get(k).and_then(|v| v.as_bool()).unwrap_or(false);
        out.push(WindowInfo {
            id,
            pid: w.get("pid").and_then(|v| v.as_u64()),
            title: text("title").unwrap_or_default(),
            app_id: text("app_id"),
            class: text("class"),
            rect,
            focused: flag("focused"),
            maximized: flag("maximized"),
            minimized: flag("minimized"),
        });
    }
    Ok(out)
}

/// What the session may ask of the compositor: list windows, close one by id. A trait so the
/// tests can use an in-memory fake; the real one runs the `spawn-at` under test.
pub trait Control: Send + Sync {
    /// The current windows. An error is an error: a failed query is never read as "no windows".
    fn windows(&self) -> Result<Vec<WindowInfo>, String>;
    /// Asks the window with this id to close; true when the command succeeded.
    fn close(&self, id: u64) -> bool;
    /// Gives the window with this id keyboard focus; true when the command succeeded. Used only
    /// to put focus back on the window that had it before the test (design 3.6).
    fn focus(&self, id: u64) -> bool;
    /// Like [`Control::focus`], with the exit code and message for the focus-return report. The
    /// default knows only success or failure.
    fn focus_detail(&self, id: u64) -> super::focus_return::FocusCommand {
        let ok = self.focus(id);
        super::focus_return::FocusCommand {
            exit_code: Some(if ok { 0 } else { 1 }),
            message: String::new(),
        }
    }
}

/// Runs one `spawn-at` action of a test. A trait so tests can answer without a binary.
pub trait ActionRunner: Send + Sync {
    /// Runs `spawn-at <args>` and returns its output; an error means it could not be run or timed out.
    fn run(
        &self,
        args: &[String],
        timeout: Duration,
    ) -> Result<crate::process::ProcessOutput, String>;
}

/// [`ActionRunner`] over the `spawn-at` binary under test, with the user's config file kept out.
pub struct CliActions {
    /// Path of the `spawn-at` binary.
    pub bin: PathBuf,
}

impl ActionRunner for CliActions {
    fn run(
        &self,
        args: &[String],
        timeout: Duration,
    ) -> Result<crate::process::ProcessOutput, String> {
        let mut cmd = Command::new(&self.bin);
        cmd.args(args);
        crate::no_config::isolate(&mut cmd);
        crate::process::execute_with_timeout(cmd, timeout)
    }
}

/// [`Control`] over the `spawn-at` binary under test.
pub struct CliControl {
    /// Path of the `spawn-at` binary.
    pub bin: PathBuf,
}

impl Control for CliControl {
    fn windows(&self) -> Result<Vec<WindowInfo>, String> {
        let mut cmd = Command::new(&self.bin);
        cmd.args(["query", "windows", "--json"]);
        crate::no_config::isolate(&mut cmd);
        let out = crate::process::execute_with_timeout(cmd, Duration::from_secs(5))
            .map_err(|e| format!("query windows: {e}"))?;
        if out.exit_code != Some(0) {
            return Err(format!(
                "query windows failed (exit {:?}): {}",
                out.exit_code,
                String::from_utf8_lossy(&out.stderr).trim()
            ));
        }
        parse_windows(&out.stdout)
    }

    fn close(&self, id: u64) -> bool {
        let mut cmd = Command::new(&self.bin);
        cmd.args(["close", "--id", &id.to_string(), "--no-wait"]);
        crate::no_config::isolate(&mut cmd);
        matches!(
            crate::process::execute_with_timeout(cmd, Duration::from_secs(5)),
            Ok(out) if out.exit_code == Some(0)
        )
    }

    fn focus(&self, id: u64) -> bool {
        self.focus_detail(id).exit_code == Some(0)
    }

    fn focus_detail(&self, id: u64) -> super::focus_return::FocusCommand {
        let mut cmd = Command::new(&self.bin);
        cmd.args(["focus", "--id", &id.to_string(), "--no-wait"]);
        crate::no_config::isolate(&mut cmd);
        match crate::process::execute_with_timeout(cmd, Duration::from_millis(1500)) {
            Ok(out) => super::focus_return::FocusCommand {
                exit_code: out.exit_code,
                message: String::from_utf8_lossy(&out.stderr)
                    .lines()
                    .find(|l| !l.trim().is_empty())
                    .unwrap_or("")
                    .trim()
                    .to_string(),
            },
            Err(e) => super::focus_return::FocusCommand {
                exit_code: None,
                message: e,
            },
        }
    }
}

/// Waits and grace periods of a session. The defaults are the design's; tests shorten them.
#[derive(Debug, Clone, Copy)]
pub struct Timings {
    /// How long a setup waits for its window when the spec gives no `timeout_ms`.
    pub setup_default: Duration,
    /// Pause between two looks at `query windows` or at the child.
    pub poll: Duration,
    /// How long teardown waits for the child to exit after the window was closed.
    pub exit_grace: Duration,
    /// How long teardown waits after SIGTERM before SIGKILL.
    pub term_grace: Duration,
    /// How long a `[[check]]` keeps looking for the state it expects (a compositor applies a
    /// state change a little after the command returns).
    pub check_wait: Duration,
    /// How long `[expect.rect]` keeps looking for the window to reach its rectangle (the harness's
    /// `SETTLE_MS`).
    pub settle: Duration,
}

impl Default for Timings {
    /// 5 s setup wait, 100 ms poll, 1 s after the close, 1 s after SIGTERM, 2 s per check,
    /// 1300 ms settle.
    fn default() -> Self {
        Self {
            setup_default: Duration::from_secs(5),
            poll: Duration::from_millis(100),
            exit_grace: Duration::from_secs(1),
            term_grace: Duration::from_secs(1),
            check_wait: Duration::from_secs(2),
            settle: Duration::from_millis(crate::oracle::SETTLE_MS),
        }
    }
}

/// One fixture window to launch.
#[derive(Debug, Clone)]
pub struct SetupRequest {
    /// Setup name (`${name}` in the spec).
    pub name: String,
    /// Whitelisted fixture flags from the spec.
    pub args: Vec<String>,
    /// The spec's own wait limit, if it set one.
    pub timeout: Option<Duration>,
}

/// Why a setup failed. Always an `Error` result, never a `Fail`: nothing about the product was
/// judged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SetupError {
    /// The fixture binary is not there.
    FixtureMissing(PathBuf),
    /// The process could not be started.
    LaunchFailed(String),
    /// The process ended before its window appeared.
    ExitedEarly {
        /// Exit code, or `None` when a signal ended it.
        code: Option<i32>,
        /// What it wrote to standard error.
        stderr: String,
    },
    /// No window with the unique title within the wait limit.
    NoWindow(Duration),
    /// More than one window carries the unique title. None of them is touched.
    Ambiguous(Vec<u64>),
    /// The one window with the title belongs to another process.
    PidMismatch {
        /// The pid of the child the runner started.
        expected: u64,
        /// The pid the window list gives.
        got: u64,
    },
    /// `query windows` failed.
    QueryFailed(String),
    /// The run was interrupted (Ctrl-C, SIGTERM) while waiting.
    Interrupted,
    /// The request itself broke a rule (a flag the runner owns).
    BadRequest(String),
}

impl std::fmt::Display for SetupError {
    /// The text that follows "setup failed: " in the test result.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SetupError::FixtureMissing(p) => write!(
                f,
                "fixture binary not found at {}; run `cargo build -p spawn-at-verify --bin fixture --features fixture`",
                p.display()
            ),
            SetupError::LaunchFailed(e) => write!(f, "could not start the fixture: {e}"),
            SetupError::ExitedEarly { code, stderr } => write!(
                f,
                "the fixture exited (code {code:?}) before its window appeared: {}",
                stderr.trim()
            ),
            SetupError::NoWindow(d) => write!(
                f,
                "no window with the fixture's unique title appeared within {} ms",
                d.as_millis()
            ),
            SetupError::Ambiguous(ids) => write!(
                f,
                "ambiguous: windows {ids:?} all carry the fixture's unique title; none of them was touched"
            ),
            SetupError::PidMismatch { expected, got } => write!(
                f,
                "the window with the fixture's unique title belongs to pid {got}, not to the fixture the runner started (pid {expected}); it was not touched"
            ),
            SetupError::QueryFailed(e) => write!(f, "{e}"),
            SetupError::Interrupted => write!(f, "interrupted"),
            SetupError::BadRequest(e) => write!(f, "{e}"),
        }
    }
}

/// One fixture process and, once found, its window.
struct OwnedFixture {
    name: String,
    title: String,
    window_id: Option<u64>,
    /// The child; kept for the whole session. `reaped` says `try_wait` has returned a status.
    child: Child,
    reaped: bool,
    stderr_path: PathBuf,
}

/// The warning shown when focus did not come back to the window recorded at the start.
pub const FOCUS_NOT_RETURNED: &str = "could not return focus to your terminal";

/// What [`SetupSession::restore_focus`] found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FocusReturn {
    /// No window was recorded at the start, it is gone, or it is one of ours: nothing was done
    /// and nothing needs saying.
    NothingRecorded,
    /// The recorded window already had focus; no command was sent.
    AlreadyFocused,
    /// `focus` was sent and the window list now shows the window focused.
    Returned,
    /// `focus` was sent (or the list could not be read) and the window is not focused.
    NotReturned(String),
}

/// Refuses an action that would do anything but focus the user's own window. `user` is the id
/// recorded by [`SetupSession::remember_focus`]; a command line whose `--id` names it must be a
/// `focus`. (Specs cannot name that id at all; this is the second lock.)
pub fn guard_user_window(user: Option<u64>, args: &[String]) -> Result<(), String> {
    let Some(user) = user else { return Ok(()) };
    let named = args
        .windows(2)
        .any(|w| w[0] == "--id" && w[1] == user.to_string())
        || args.iter().any(|a| a == &format!("--id={user}"));
    if named && args.first().map(String::as_str) != Some("focus") {
        return Err(format!(
            "refusing `{}` on window {user}: it is the window that had focus when the run started, and the runner may only focus it",
            args.first().map(String::as_str).unwrap_or("")
        ));
    }
    Ok(())
}

/// What teardown found worth saying.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TeardownReport {
    /// Things teardown declined to do or could not do.
    pub warnings: Vec<String>,
    /// Recorded ids that `query windows` still lists, with our title, after teardown.
    pub left_behind: Vec<u64>,
}

/// All the fixtures one test launched. Dropping it tears them down.
pub struct SetupSession {
    control: Arc<dyn Control>,
    fixture_bin: PathBuf,
    run_tag: String,
    exit_after_ms: u64,
    timings: Timings,
    interrupted: Arc<AtomicBool>,
    extra_env: Vec<(String, String)>,
    dir: PathBuf,
    fixtures: Vec<OwnedFixture>,
    torn_down: bool,
    report: TeardownReport,
    /// The window that had focus before the first fixture started; focus goes back to it after
    /// teardown if it still exists and lost focus (design 3.6). Never one of our own windows.
    refocus: Option<u64>,
}

/// True once `child` has exited, waiting at most `limit`. A status read by `try_wait` reaps the
/// child, so nothing may be signalled afterwards.
fn wait_exit(child: &mut Child, reaped: &mut bool, limit: Duration, poll: Duration) -> bool {
    if *reaped {
        return true;
    }
    let deadline = Instant::now() + limit;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => {
                *reaped = true;
                return true;
            }
            Ok(None) => {}
            Err(_) => return false,
        }
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(poll.min(Duration::from_millis(20)));
    }
}

impl SetupSession {
    /// Starts an empty session. `run_tag_hint` is mixed into the unique titles (a test id, say);
    /// the process id, the time and a counter make them unique across runs.
    pub fn new(
        control: Arc<dyn Control>,
        fixture_bin: PathBuf,
        run_tag_hint: &str,
        exit_after_ms: u64,
        timings: Timings,
        interrupted: Arc<AtomicBool>,
    ) -> std::io::Result<Self> {
        let seq = SESSION_SEQ.fetch_add(1, Ordering::SeqCst);
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        let hint: String = run_tag_hint
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
            .collect();
        let run_tag = format!("{}-{}-{}-{}", std::process::id(), now_ms, seq, hint);
        let dir = std::env::temp_dir().join(format!(
            "spawn-at-verify-setup-{}-{}",
            std::process::id(),
            seq
        ));
        std::fs::DirBuilder::new().mode(0o700).create(&dir)?;
        Ok(Self {
            control,
            fixture_bin,
            run_tag,
            exit_after_ms,
            timings,
            interrupted,
            extra_env: Vec::new(),
            dir,
            fixtures: Vec::new(),
            torn_down: false,
            report: TeardownReport::default(),
            refocus: None,
        })
    }

    /// Records the id of the window that has focus now, for [`SetupSession::teardown`] to give
    /// focus back to. Call it before the first launch.
    pub fn remember_focus(&mut self) {
        self.refocus = self
            .control
            .windows()
            .ok()
            .and_then(|list| list.iter().find(|w| w.focused).map(|w| w.id));
    }

    /// The id of the window that had focus before the first launch, if one was recorded. It is the
    /// user's own window: the only thing the runner may ever do to it is give it focus.
    pub fn user_window(&self) -> Option<u64> {
        self.refocus
    }

    /// Puts focus back on the window recorded by [`SetupSession::remember_focus`] and checks
    /// the result with a read-only window list. The only command sent to that window is `focus`.
    /// Called once after the automatic phase of a test a person judges (the keys must reach the
    /// TUI's terminal, not the fixture) and again in teardown, where it does nothing if focus is
    /// already there.
    pub fn restore_focus(&self) -> FocusReturn {
        let Some(id) = self.refocus else {
            return FocusReturn::NothingRecorded;
        };
        // Never one of our own windows, whatever the ids say.
        if self.fixtures.iter().any(|f| f.window_id == Some(id)) {
            return FocusReturn::NothingRecorded;
        }
        let focused_now = |this: &Self| -> Option<Option<bool>> {
            let list = this.control.windows().ok()?;
            Some(list.iter().find(|w| w.id == id).map(|w| w.focused))
        };
        match focused_now(self) {
            None => FocusReturn::NotReturned(FOCUS_NOT_RETURNED.into()),
            Some(None) => FocusReturn::NothingRecorded, // the window is gone: nothing to return to
            Some(Some(true)) => FocusReturn::AlreadyFocused,
            Some(Some(false)) => {
                let _ = self.control.focus(id);
                let deadline = Instant::now() + self.timings.check_wait.min(Duration::from_secs(1));
                loop {
                    if focused_now(self) == Some(Some(true)) {
                        return FocusReturn::Returned;
                    }
                    if Instant::now() >= deadline {
                        return FocusReturn::NotReturned(FOCUS_NOT_RETURNED.into());
                    }
                    std::thread::sleep(self.timings.poll);
                }
            }
        }
    }

    /// Adds an environment variable for the fixtures launched from now on.
    pub fn with_env(mut self, key: &str, value: &str) -> Self {
        self.extra_env.push((key.to_string(), value.to_string()));
        self
    }

    /// The unique title of setup `name`.
    pub fn title_of(&self, name: &str) -> String {
        format!("verify-{}-{}", self.run_tag, name)
    }

    fn find(&self, name: &str) -> Option<&OwnedFixture> {
        self.fixtures.iter().find(|f| f.name == name)
    }

    /// The recorded window id of setup `name`, once its window was found.
    pub fn window_id(&self, name: &str) -> Option<u64> {
        self.find(name).and_then(|f| f.window_id)
    }

    /// The pid of the child the runner started for setup `name`.
    pub fn child_pid(&self, name: &str) -> Option<u32> {
        self.find(name).map(|f| f.child.id())
    }

    /// Placeholder values for this session: recorded ids and unique titles only.
    pub fn bindings(&self) -> Bindings {
        let mut b = Bindings::default();
        for f in &self.fixtures {
            if let Some(id) = f.window_id {
                b.ids.insert(f.name.clone(), id);
            }
            b.titles.insert(f.name.clone(), f.title.clone());
        }
        b
    }

    /// Number of fixtures launched so far (found or not).
    pub fn len(&self) -> usize {
        self.fixtures.len()
    }

    /// True when no fixture was launched.
    pub fn is_empty(&self) -> bool {
        self.fixtures.is_empty()
    }

    /// The ids of every recorded window (for the tracker, which closes by id on the old paths).
    pub fn recorded_ids(&self) -> Vec<u64> {
        self.fixtures.iter().filter_map(|f| f.window_id).collect()
    }

    /// Setup name and recorded id of every fixture whose window was found: the windows whose
    /// lines of the extension's log may be kept and shown.
    pub fn named_ids(&self) -> Vec<(String, u64)> {
        self.fixtures
            .iter()
            .filter_map(|f| f.window_id.map(|id| (f.name.clone(), id)))
            .collect()
    }

    /// Launches one fixture and waits for its window. The fixture is part of the session from the
    /// moment it starts, so a failure at any later point still tears it down.
    pub fn launch(&mut self, req: &SetupRequest) -> Result<(), SetupError> {
        if self.find(&req.name).is_some() {
            return Err(SetupError::BadRequest(format!(
                "setup '{}' launched twice",
                req.name
            )));
        }
        // Defence in depth: the loader already refuses these, but this module must hold alone.
        v2::validate_setup_args(&format!("setup '{}'", req.name), &req.args)
            .map_err(SetupError::BadRequest)?;
        if !self.fixture_bin.exists() {
            return Err(SetupError::FixtureMissing(self.fixture_bin.clone()));
        }
        let title = self.title_of(&req.name);
        let stderr_path = self.dir.join(format!("fixture-{}.err", req.name));
        let stderr_file = std::fs::File::create(&stderr_path)
            .map_err(|e| SetupError::LaunchFailed(format!("{}: {e}", stderr_path.display())))?;

        let mut cmd = Command::new(&self.fixture_bin);
        cmd.args([
            "--title",
            &title,
            "--app-id",
            &format!("verify.owned.{}", req.name),
            "--exit-after-ms",
            &self.exit_after_ms.to_string(),
        ]);
        cmd.args(&req.args);
        for (k, v) in &self.extra_env {
            cmd.env(k, v);
        }
        cmd.stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::from(stderr_file));
        // Its own process group, as `execute_with_timeout` does for its children: a terminal
        // Ctrl-C reaches the runner, which tears down, instead of racing the fixture.
        // SAFETY: setpgid(0, 0) is async-signal-safe and touches only the child.
        unsafe {
            cmd.pre_exec(|| {
                libc::setpgid(0, 0);
                Ok(())
            });
        }
        let child = cmd
            .spawn()
            .map_err(|e| SetupError::LaunchFailed(e.to_string()))?;
        self.fixtures.push(OwnedFixture {
            name: req.name.clone(),
            title,
            window_id: None,
            child,
            reaped: false,
            stderr_path,
        });
        let idx = self.fixtures.len() - 1;
        let limit = req.timeout.unwrap_or(self.timings.setup_default);
        self.await_window(idx, limit)
    }

    /// Polls until exactly one window carries the fixture's title and belongs to its child.
    fn await_window(&mut self, idx: usize, limit: Duration) -> Result<(), SetupError> {
        let deadline = Instant::now() + limit;
        loop {
            if self.interrupted.load(Ordering::SeqCst) {
                return Err(SetupError::Interrupted);
            }
            let f = &mut self.fixtures[idx];
            match f.child.try_wait() {
                Ok(Some(status)) => {
                    f.reaped = true;
                    let stderr = std::fs::read_to_string(&f.stderr_path).unwrap_or_default();
                    return Err(SetupError::ExitedEarly {
                        code: status.code(),
                        stderr,
                    });
                }
                Ok(None) => {}
                Err(e) => return Err(SetupError::LaunchFailed(e.to_string())),
            }
            let windows = self.control.windows().map_err(SetupError::QueryFailed)?;
            let f = &mut self.fixtures[idx];
            let ours: Vec<&WindowInfo> = windows.iter().filter(|w| w.title == f.title).collect();
            match ours.as_slice() {
                [] => {}
                [one] => {
                    if let Some(pid) = one.pid {
                        if pid != u64::from(f.child.id()) {
                            return Err(SetupError::PidMismatch {
                                expected: u64::from(f.child.id()),
                                got: pid,
                            });
                        }
                    }
                    f.window_id = Some(one.id);
                    return Ok(());
                }
                many => {
                    return Err(SetupError::Ambiguous(many.iter().map(|w| w.id).collect()));
                }
            }
            if Instant::now() >= deadline {
                return Err(SetupError::NoWindow(limit));
            }
            std::thread::sleep(self.timings.poll);
        }
    }

    /// Re-reads `query windows` and checks that setup `name`'s recorded id is still listed with
    /// its unique title and the child's pid. The window is returned; any difference is an error,
    /// and the caller must then not run the action.
    pub fn verify_target(&self, name: &str) -> Result<WindowInfo, String> {
        let f = self
            .find(name)
            .ok_or_else(|| format!("no setup named '{name}'"))?;
        let id = f
            .window_id
            .ok_or_else(|| format!("setup '{name}' has no recorded window"))?;
        let windows = self.control.windows()?;
        let w = windows.into_iter().find(|w| w.id == id).ok_or_else(|| {
            format!("target changed: window {id} of '{name}' is no longer listed")
        })?;
        if w.title != f.title {
            return Err(format!(
                "target changed: window {id} of '{name}' now has the title '{}', not '{}'",
                w.title, f.title
            ));
        }
        if let Some(pid) = w.pid {
            if pid != u64::from(f.child.id()) {
                return Err(format!(
                    "target changed: window {id} of '{name}' now belongs to pid {pid}, not to the fixture the runner started (pid {})",
                    f.child.id()
                ));
            }
        }
        Ok(w)
    }

    /// Setup `name`'s window as listed now, or `None` when it is not listed. Unlike
    /// [`SetupSession::verify_target`] a missing window is a normal answer (a `close` check).
    pub fn read(&self, name: &str) -> Result<Option<WindowInfo>, String> {
        let id = self
            .window_id(name)
            .ok_or_else(|| format!("setup '{name}' has no recorded window"))?;
        let title = self.find(name).map(|f| f.title.clone()).unwrap_or_default();
        Ok(self
            .control
            .windows()?
            .into_iter()
            .find(|w| w.id == id && w.title == title))
    }

    /// True when the child of setup `name` has exited within `within`.
    pub fn wait_process_exit(&mut self, name: &str, within: Duration) -> bool {
        let poll = self.timings.poll;
        match self.fixtures.iter_mut().find(|f| f.name == name) {
            Some(f) => wait_exit(&mut f.child, &mut f.reaped, within, poll),
            None => false,
        }
    }

    /// An id the runner has just seen is not a window: the highest listed id plus 1,000,000.
    pub fn choose_nonexistent(&self) -> Result<u64, String> {
        let max = self
            .control
            .windows()?
            .iter()
            .map(|w| w.id)
            .max()
            .unwrap_or(0);
        Ok(max + 1_000_000)
    }

    /// Fails if `id` is listed now. Called right before an action that names it.
    pub fn ensure_absent(&self, id: u64) -> Result<(), String> {
        if self.control.windows()?.iter().any(|w| w.id == id) {
            return Err(format!(
                "id {id} was chosen as nonexistent but is listed now; the action was not run"
            ));
        }
        Ok(())
    }

    /// Closes and stops everything this session launched; safe to call twice. Order, per
    /// fixture, in reverse launch order: close the window by id if the id still carries the
    /// fixture's title; wait for the process; SIGTERM through the handle; SIGKILL through the
    /// handle. Then one look at `query windows` for anything left.
    pub fn teardown(&mut self) -> TeardownReport {
        if self.torn_down {
            return self.report.clone();
        }
        self.torn_down = true;
        let mut report = TeardownReport::default();
        let control = Arc::clone(&self.control);
        let timings = self.timings;
        let user_window = self.refocus;
        for f in self.fixtures.iter_mut().rev() {
            // The user's own window is never closed, even if an id were ever confused.
            if let Some(id) = f.window_id.filter(|id| Some(*id) != user_window) {
                match control.windows() {
                    Ok(list) => match list.iter().find(|w| w.id == id) {
                        Some(w) if w.title == f.title => {
                            if !control.close(id) {
                                report
                                    .warnings
                                    .push(format!("close of window {id} ('{}') failed", f.name));
                            }
                        }
                        Some(w) => report.warnings.push(format!(
                            "did not close window {id}: it now carries the title '{}', not '{}'",
                            w.title, f.title
                        )),
                        None => {}
                    },
                    Err(e) => report.warnings.push(format!(
                        "did not close window {id} of '{}': cannot verify its title ({e})",
                        f.name
                    )),
                }
            }
            if !wait_exit(
                &mut f.child,
                &mut f.reaped,
                timings.exit_grace,
                timings.poll,
            ) {
                // Still running and not reaped, so the pid is ours.
                // SAFETY: plain kill(2) on the pid of a child this struct owns and has not reaped.
                unsafe {
                    libc::kill(f.child.id() as libc::pid_t, libc::SIGTERM);
                }
                if !wait_exit(
                    &mut f.child,
                    &mut f.reaped,
                    timings.term_grace,
                    timings.poll,
                ) {
                    let _ = f.child.kill();
                    let _ = f.child.wait();
                    f.reaped = true;
                }
            }
        }
        // One look for what is left of our windows.
        let recorded: Vec<(u64, String)> = self
            .fixtures
            .iter()
            .filter_map(|f| f.window_id.map(|id| (id, f.title.clone())))
            .collect();
        if !recorded.is_empty() {
            match control.windows() {
                Ok(list) => {
                    for (id, title) in recorded {
                        if list.iter().any(|w| w.id == id && w.title == title) {
                            report.left_behind.push(id);
                        }
                    }
                }
                Err(e) => report
                    .warnings
                    .push(format!("could not check for leftover windows: {e}")),
            }
        }
        // Best effort: put focus back where it was. The same mechanism as the return after the
        // automatic phase of a Both test (`restore_focus`): it does nothing when the window still
        // has focus, so the two never conflict.
        if let FocusReturn::NotReturned(why) = self.restore_focus() {
            report.warnings.push(why);
        }
        let _ = std::fs::remove_dir_all(&self.dir);
        self.report = report.clone();
        report
    }

    /// The directory holding the fixtures' standard error files (for the tests).
    pub fn dir(&self) -> &Path {
        &self.dir
    }
}

impl Drop for SetupSession {
    /// Teardown on every path out: normal end, early return, panic.
    fn drop(&mut self) {
        let _ = self.teardown();
    }
}
