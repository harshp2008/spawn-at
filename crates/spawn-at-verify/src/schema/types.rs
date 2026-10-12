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

/// Automated test expectations under `[expect]`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TestExpect {
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
}
