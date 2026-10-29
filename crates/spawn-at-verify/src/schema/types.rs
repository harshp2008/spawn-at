//! Core declarative TOML schema data models.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Kind of test: automated assertions, visual human review, or both.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TestKind {
    /// Fully automated assertions without human intervention.
    Auto,
    /// Requires manual human visual verification.
    Human,
    /// Automated assertions combined with visual review prompt.
    Both,
}

/// Environment constraints defined under `[requires]`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestRequires {
    /// Permitted display sessions (e.g. ["wayland"], ["x11"], ["any"]).
    #[serde(default = "default_any_vec")]
    pub session: Vec<String>,
    /// Permitted backends (e.g. ["gnome"], ["x11"], ["any"]).
    #[serde(default = "default_any_vec")]
    pub backend: Vec<String>,
    /// Permitted GNOME Shell version range string.
    #[serde(default = "default_any_str")]
    pub gnome: String,
    /// Minimum connected monitor count required.
    #[serde(default = "default_one_u32")]
    pub monitors_min: u32,
    /// Supported monitor scale mode ("any", "fractional", "1x").
    #[serde(default = "default_any_str")]
    pub scale: String,
    /// Required binary applications on system PATH.
    #[serde(default)]
    pub apps: Vec<String>,
}

/// Default value helper providing `["any"]` vector.
fn default_any_vec() -> Vec<String> {
    vec!["any".to_string()]
}

/// Default value helper providing `"any"` string.
fn default_any_str() -> String {
    "any".to_string()
}

/// Default value helper providing 1 monitor.
fn default_one_u32() -> u32 {
    1
}

impl Default for TestRequires {
    /// Constructs standard default test environment requirements.
    fn default() -> Self {
        Self {
            session: default_any_vec(),
            backend: default_any_vec(),
            gnome: default_any_str(),
            monitors_min: default_one_u32(),
            scale: default_any_str(),
            apps: Vec::new(),
        }
    }
}

/// Target subject application to launch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestSubject {
    /// Application binary name or "fixture".
    pub app: String,
    /// Subject application profile name.
    #[serde(default = "default_plain_str")]
    pub profile: String,
    /// Additional arguments passed to the subject binary.
    #[serde(default)]
    pub args: Vec<String>,
    /// Minimum window width the fixture enforces (`--min-width`). Declaring it tells the oracle
    /// that the final width is `max(requested, min_width)`. Only meaningful for `app = "fixture"`.
    #[serde(default)]
    pub min_width: Option<u32>,
    /// Minimum window height the fixture enforces (`--min-height`); same meaning as `min_width`.
    #[serde(default)]
    pub min_height: Option<u32>,
}

/// Default value helper providing `"plain"` profile string.
fn default_plain_str() -> String {
    "plain".to_string()
}

/// Action to invoke against `spawn-at`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestAction {
    /// Subcommand to invoke on spawn-at (e.g. "spawn").
    pub command: String,
    /// Whether this action supports execution repeat loops.
    #[serde(default)]
    pub repeatable: bool,
    /// Command-line arguments passed to spawn-at.
    #[serde(default)]
    pub args: Vec<String>,
}

/// Geometric bounds expectation for independent arithmetic oracle check.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpectRect {
    /// Expected anchor position label.
    #[serde(default)]
    pub anchor: Option<String>,
    /// Expected pivot alignment label.
    #[serde(default)]
    pub pivot: Option<String>,
    /// Expected width and height dimensions `[w, h]`.
    #[serde(default)]
    pub size: Option<Vec<u32>>,
    /// Uniform margin expectation.
    #[serde(default)]
    pub margin: Option<i32>,
    /// Top margin expectation override.
    #[serde(default)]
    pub margin_top: Option<i32>,
    /// Bottom margin expectation override.
    #[serde(default)]
    pub margin_bottom: Option<i32>,
    /// Left margin expectation override.
    #[serde(default)]
    pub margin_left: Option<i32>,
    /// Right margin expectation override.
    #[serde(default)]
    pub margin_right: Option<i32>,
    /// Maximum allowed delta in pixels before assertion fails.
    #[serde(default = "default_tolerance_px")]
    pub tolerance_px: i32,
}

/// Default value helper providing 1 pixel tolerance.
fn default_tolerance_px() -> i32 {
    1
}

/// What the ready-wait reported for the resize, under `[expect.resize]`: the `resize` object of
/// the CLI's `--json` output. Judged only for a run that used the ready-wait machine (the legacy
/// rule reports no outcome); every field but `outcome` is optional.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpectResize {
    /// `reached`, `adjusted`, `timed-out` or `skipped`.
    pub outcome: String,
    /// Expected `reason`: `at-target`, `quiet`, `cap`, `hard`, `not-resizable`, `fullscreen`, `tiled`.
    #[serde(default)]
    pub reason: Option<String>,
    /// Lowest accepted `waited_ms`.
    #[serde(default)]
    pub waited_ms_min: Option<u32>,
    /// Highest accepted `waited_ms` (for the hard limit: a little over 1500).
    #[serde(default)]
    pub waited_ms_max: Option<u32>,
    /// Expected `final` size `[w, h]`, within `tolerance_px`.
    #[serde(default)]
    pub final_size: Option<Vec<u32>>,
    /// Allowed difference per axis for `final_size`.
    #[serde(default = "default_resize_tolerance_px")]
    pub tolerance_px: u32,
}

/// Default tolerance for `final_size`: 2 pixels, the harness's size tolerance elsewhere.
fn default_resize_tolerance_px() -> u32 {
    2
}

/// Automated test expectations under `[expect]`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TestExpect {
    /// What the ready-wait reported (`[expect.resize]`), judged for ready-wait runs only.
    #[serde(default)]
    pub resize: Option<ExpectResize>,
    /// Expected process exit code.
    #[serde(default)]
    pub exit_code: i32,
    /// String patterns that must NOT appear in standard error.
    #[serde(default)]
    pub stderr_absent: Vec<String>,
    /// String patterns that MUST appear in standard error.
    #[serde(default)]
    pub stderr_present: Vec<String>,
    /// Diagnostic patterns expected in log traces.
    #[serde(default)]
    pub diagnostics: Vec<String>,
    /// Whether window must never be visible off target geometry.
    #[serde(default)]
    pub never_visible_off_target: bool,
    /// Geometric bounds expectation for independent arithmetic oracle check.
    #[serde(default)]
    pub rect: Option<ExpectRect>,
    /// Version 2: substrings that must appear in standard output (`${name}` allowed).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub stdout_present: Vec<String>,
    /// Version 2: standard output is a JSON array holding an entry whose `id` and `title` are
    /// those of this recorded window.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stdout_lists_window: Option<String>,
}

/// Human visual review watch prompt under `[human]`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestHuman {
    /// Prompt instructions instructing the reviewer what behavior to watch.
    pub watch: String,
}

/// Historical regression metadata under `[regression]`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestRegression {
    /// Git commit hash associated with historical regression.
    pub commit: String,
    /// Explanatory notes documenting the bug or historical defect.
    pub note: String,
}

/// Schema version of a spec that uses `[[setup]]`, `[[then]]`, `[[check]]` or the stdout
/// expectations. A spec without `schema_version` is version 1, which is every spec written before
/// `docs/DESIGN_RUNNER_SETUP.md`.
pub const SCHEMA_VERSION_EXISTING_WINDOW: u32 = 2;

/// `[[setup]]`: one fixture window the runner launches itself before the action. The runner picks
/// the title, the app id, the crash backstop and the protocol; the spec supplies only whitelisted
/// fixture flags (`schema::v2::SETUP_FLAGS`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TestSetup {
    /// Name of the window: `${name}` in action arguments is its recorded id.
    pub name: String,
    /// Fixture flags (for example `["--width", "400", "--height", "300"]`).
    #[serde(default)]
    pub args: Vec<String>,
    /// Longest wait, in milliseconds, for the window to show up in `query windows`; the runner
    /// default (5000) applies when absent.
    #[serde(default)]
    pub timeout_ms: Option<u64>,
    /// A real program to start instead of the fixture: its name, then flags (`schema::v2::
    /// SETUP_PROGRAMS`). The window is the one new window that belongs to the process the runner
    /// started. Empty (the default) means the fixture.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub command: Vec<String>,
    /// Environment for `command` (`schema::v2::SETUP_ENV`). Only meaningful with `command`.
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub env: std::collections::BTreeMap<String, String>,
}

/// What a `[[check]]` asserts about a recorded window.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum CheckState {
    /// The window is listed by `query windows`.
    Present,
    /// The window is not listed.
    Gone,
    /// Listed and maximized.
    Maximized,
    /// Listed and minimized.
    Minimized,
    /// Listed, neither maximized nor minimized.
    Normal,
    /// Listed and focused.
    Focused,
    /// Listed and not focused.
    Unfocused,
}

/// `[[check]]`: one assertion about a recorded window, evaluated after an action. Exactly one of
/// `state`, `same_rect_as`, `process_exited` and `shell_alive` must be given.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TestCheck {
    /// Evaluate after this action: 0 is `[action]`, 1 the first `[[then]]`, and so on. Absent
    /// means after the last action.
    #[serde(default)]
    pub after: Option<usize>,
    /// Name of the `[[setup]]` window the check is about. Empty exactly for `shell_alive`.
    #[serde(default)]
    pub window: String,
    /// The window's state in `query windows`.
    #[serde(default)]
    pub state: Option<CheckState>,
    /// `"before"`: the window's rectangle equals (within 2 px) the one read just before `[action]`.
    #[serde(default)]
    pub same_rect_as: Option<String>,
    /// The fixture process this setup launched has exited (within 2 s).
    #[serde(default)]
    pub process_exited: bool,
    /// The GNOME Shell process (pid and start time) is the one that was running when the test
    /// began, so the actions did not make it abort or restart. Takes no `window`.
    #[serde(default, skip_serializing_if = "is_false")]
    pub shell_alive: bool,
}

fn is_false(v: &bool) -> bool {
    !*v
}

/// The keys a version 2 spec may add. Flattened into [`TestDefinition`], so a version 1 spec
/// parses exactly as before and serializes without any of them.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecV2 {
    /// 1 when absent. 2 allows the other keys of this struct.
    #[serde(default = "default_schema_version", skip_serializing_if = "is_v1")]
    pub schema_version: u32,
    /// Fixture windows to launch before the action.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub setup: Vec<TestSetup>,
    /// Actions run in order after `[action]`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub then: Vec<TestAction>,
    /// Assertions on the recorded windows.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub check: Vec<TestCheck>,
}

fn default_schema_version() -> u32 {
    1
}

fn is_v1(v: &u32) -> bool {
    *v == 1
}

impl Default for SpecV2 {
    /// Version 1 with nothing added.
    fn default() -> Self {
        Self {
            schema_version: 1,
            setup: Vec::new(),
            then: Vec::new(),
            check: Vec::new(),
        }
    }
}

impl SpecV2 {
    /// True when the spec uses none of the version 2 keys.
    pub fn is_empty(&self) -> bool {
        self.setup.is_empty() && self.then.is_empty() && self.check.is_empty()
    }
}

/// Test definition deserialized from a `.toml` file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestDefinition {
    /// Unique test case identifier.
    pub id: String,
    /// Category classification name.
    pub category: String,
    /// Optional subcategory grouping.
    #[serde(default)]
    pub subcategory: Option<String>,
    /// Short human-readable title.
    pub title: String,
    /// Detailed description of the test scenario.
    pub description: String,
    /// Test execution kind (auto, human, or both).
    pub kind: TestKind,
    /// True if test is mandatory for qualification.
    #[serde(default)]
    pub required: bool,
    /// Descriptive labels and categorization tags.
    #[serde(default)]
    pub tags: Vec<String>,
    /// Parameter matrix definitions for combinatorial expansion.
    #[serde(default)]
    pub matrix: Option<HashMap<String, Vec<toml::Value>>>,
    /// Host session and platform environment requirements.
    #[serde(default)]
    pub requires: TestRequires,
    /// Target subject process to launch.
    #[serde(default)]
    pub subject: Option<TestSubject>,
    /// Action command to execute with spawn-at.
    pub action: TestAction,
    /// Automated test expectations and assertions.
    #[serde(default)]
    pub expect: TestExpect,
    /// Manual visual review instructions.
    #[serde(default)]
    pub human: Option<TestHuman>,
    /// Associated regression tracking metadata.
    #[serde(default)]
    pub regression: Option<TestRegression>,
    /// Version 2 additions (`schema_version`, `[[setup]]`, `[[then]]`, `[[check]]`).
    #[serde(flatten)]
    pub v2: SpecV2,
}
