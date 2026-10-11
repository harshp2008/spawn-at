//! Schema definitions for spawn-at-verify test definitions and suites.

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

impl TestDefinition {
    /// Returns the effective tags for this test, automatically deriving
    /// platform tags (`wayland-only`, `x11-only`, `gnome-only`) from `[requires]`.
    pub fn effective_tags(&self) -> Vec<String> {
        let mut tags = self.tags.clone();

        let is_any_session = self.requires.session.iter().any(|s| s == "any");
        let has_wayland = self.requires.session.iter().any(|s| s == "wayland");
        let has_x11 = self.requires.session.iter().any(|s| s == "x11");

        if !is_any_session {
            if has_wayland && !has_x11 && !tags.contains(&"wayland-only".to_string()) {
                tags.push("wayland-only".to_string());
            }
            if has_x11 && !has_wayland && !tags.contains(&"x11-only".to_string()) {
                tags.push("x11-only".to_string());
            }
        }

        let is_any_backend = self.requires.backend.iter().any(|b| b == "any");
        let has_gnome = self.requires.backend.iter().any(|b| b == "gnome");
        let has_x11_backend = self.requires.backend.iter().any(|b| b == "x11");

        if !is_any_backend {
            if has_gnome && !has_x11_backend && !tags.contains(&"gnome-only".to_string()) {
                tags.push("gnome-only".to_string());
            }
            if has_x11_backend && !has_gnome && !tags.contains(&"x11-only".to_string()) {
                tags.push("x11-only".to_string());
            }
        }

        tags
    }

    /// Returns the required category of this test (e.g. "placement", "basics").
    pub fn category(&self) -> &str {
        &self.category
    }

    /// Expands matrix variations if a `[matrix]` table is present.
    /// If no matrix is defined, returns `vec![self.clone()]`.
    pub fn expand_matrix(&self) -> Result<Vec<TestDefinition>, String> {
        let matrix = match &self.matrix {
            Some(m) if !m.is_empty() => m,
            _ => return Ok(vec![self.clone()]),
        };

        // For V1, we expand key-value combinations.
        let mut keys: Vec<String> = matrix.keys().cloned().collect();
        keys.sort(); // Deterministic ordering

        let mut combinations: Vec<Vec<(String, String)>> = vec![vec![]];

        for key in &keys {
            let values = &matrix[key];
            if values.is_empty() {
                return Err(format!(
                    "Matrix parameter '{}' has empty values array in test '{}'",
                    key, self.id
                ));
            }

            let mut next_combinations = Vec::new();
            for comb in &combinations {
                for val in values {
                    let val_str = match val {
                        toml::Value::String(s) => s.clone(),
                        toml::Value::Integer(i) => i.to_string(),
                        toml::Value::Float(f) => f.to_string(),
                        toml::Value::Boolean(b) => b.to_string(),
                        other => other.to_string(),
                    };
                    let mut new_comb = comb.clone();
                    new_comb.push((key.clone(), val_str));
                    next_combinations.push(new_comb);
                }
            }
            combinations = next_combinations;
        }

        let mut expanded = Vec::with_capacity(combinations.len());

        for comb in combinations {
            let mut clone = self.clone();
            // Build suffix: key-value or key1-val1/key2-val2
            let suffix_parts: Vec<String> =
                comb.iter().map(|(k, v)| format!("{}-{}", k, v)).collect();
            clone.id = format!("{}/{}", self.id, suffix_parts.join("-"));

            // Helper to substitute placeholders like "{delay}" with the value
            let substitute_str = |text: &str| -> String {
                let mut res = text.to_string();
                for (k, v) in &comb {
                    let placeholder = format!("{{{}}}", k);
                    res = res.replace(&placeholder, v);
                }
                res
            };

            clone.title = substitute_str(&clone.title);
            clone.description = substitute_str(&clone.description);

            if let Some(sub) = &mut clone.subject {
                sub.app = substitute_str(&sub.app);
                sub.profile = substitute_str(&sub.profile);
                sub.args = sub.args.iter().map(|a| substitute_str(a)).collect();
            }

            clone.action.command = substitute_str(&clone.action.command);
            clone.action.args = clone
                .action
                .args
                .iter()
                .map(|a| substitute_str(a))
                .collect();

            if let Some(hum) = &mut clone.human {
                hum.watch = substitute_str(&hum.watch);
            }

            // Remove matrix definition on expanded instances so they are treated as concrete tests
            clone.matrix = None;
            expanded.push(clone);
        }

        Ok(expanded)
    }
}

/// Recognized valid test category names.
pub const VALID_CATEGORIES: &[&str] = &[
    "basics",
    "placement",
    "transforms",
    "focus",
    "animation-and-cloak",
    "queries",
    "robustness",
    "platform",
];

/// Returns true if the given category string is in VALID_CATEGORIES.
pub fn is_valid_category(cat: &str) -> bool {
    VALID_CATEGORIES.contains(&cat)
}

/// Suite definition deserialized from `verify/suites/*.toml`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuiteDefinition {
    /// Suite identifier name.
    pub name: String,
    /// Descriptive explanation of the test suite.
    pub description: String,
    /// Categories included in this suite.
    #[serde(default)]
    pub categories: Vec<String>,
    /// Tag filters applied by this suite.
    #[serde(default)]
    pub tags: Vec<String>,
    /// If true, suite filters to required tests only.
    #[serde(default)]
    pub required_only: bool,
}

impl SuiteDefinition {
    /// Checks whether a test matches this suite's criteria.
    pub fn matches(&self, test: &TestDefinition) -> bool {
        // 1. Required only check
        if self.required_only && !test.required {
            return false;
        }

        // 2. Category check: if categories specified, test's category must be in it
        if !self.categories.is_empty() {
            let test_cat = test.category();
            if !self.categories.iter().any(|c| {
                c == test_cat
                    || (c == "preflight" && test_cat == "basics")
                    || (c == "cloak" && test_cat == "animation-and-cloak")
                    || (c == "transform" && test_cat == "transforms")
                    || (c == "query" && test_cat == "queries")
                    || (c == "resilience" && test_cat == "robustness")
            }) {
                return false;
            }
        }

        // 3. Tag check: positive and negated tags
        let eff_tags = test.effective_tags();
        for rule in &self.tags {
            if let Some(negated) = rule.strip_prefix('!') {
                // If test has the negated tag, reject
                if eff_tags.iter().any(|t| t == negated) {
                    return false;
                }
            } else {
                // Positive tag required
                if !eff_tags.iter().any(|t| t == rule) {
                    return false;
                }
            }
        }

        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tests deserialization of bottom-right placement test from TOML specification.
    #[test]
    fn test_deserialize_placement_bottom_right() {
        let toml_str = r#"
id = "placement/bottom-right"
category = "placement"
title = "Bottom-right anchor placement"
description = "Spawns a window at the bottom-right corner."
kind = "both"
required = true
tags = ["visual", "needs-fixture"]

[requires]
session = ["any"]
backend = ["any"]
gnome = "any"
monitors_min = 1
scale = "any"
apps = []

[subject]
app = "fixture"
profile = "plain"
args = ["--title", "Verify-Placement-BottomRight", "--app-id", "verify.placement.bottomright"]

[action]
command = "spawn"
repeatable = false
args = [
    "--anchor", "bottom-right",
    "--size", "400", "300",
    "--margin", "16"
]

[expect]
exit_code = 0
stderr_absent = ["[spawn-at] ERROR:", "[spawn-at] WARNING:"]
stderr_present = []
diagnostics = []
never_visible_off_target = false

[expect.rect]
anchor = "bottom-right"
size = [400, 300]
margin = 16
tolerance_px = 1

[human]
watch = "Verify the window appears precisely in the bottom-right corner."
"#;

        let test: TestDefinition = toml::from_str(toml_str).expect("deserialize failed");
        assert_eq!(test.id, "placement/bottom-right");
        assert_eq!(test.category(), "placement");
        assert_eq!(test.kind, TestKind::Both);
        assert!(test.required);
        assert_eq!(test.action.command, "spawn");
        assert_eq!(test.expect.exit_code, 0);

        let rect = test.expect.rect.unwrap();
        assert_eq!(rect.anchor.as_deref(), Some("bottom-right"));
        assert_eq!(rect.size, Some(vec![400, 300]));
        assert_eq!(rect.margin, Some(16));
        assert_eq!(rect.tolerance_px, 1);
    }

    /// Tests automatic derivation of platform tags from session requirements.
    #[test]
    fn test_derived_platform_tags() {
        let mut test = TestDefinition {
            id: "cloak/test".into(),
            category: "animation-and-cloak".into(),
            subcategory: None,
            title: "Test".into(),
            description: "Test".into(),
            kind: TestKind::Auto,
            required: true,
            tags: vec!["visual".into()],
            matrix: None,
            requires: TestRequires {
                session: vec!["wayland".into()],
                backend: vec!["gnome".into()],
                ..Default::default()
            },
            subject: None,
            action: TestAction {
                command: "spawn".into(),
                repeatable: false,
                args: vec![],
            },
            expect: TestExpect::default(),
            human: None,
            regression: None,
        };

        let tags = test.effective_tags();
        assert!(tags.contains(&"visual".to_string()));
        assert!(tags.contains(&"wayland-only".to_string()));
        assert!(tags.contains(&"gnome-only".to_string()));
        assert!(!tags.contains(&"x11-only".to_string()));

        // When session is ["any"], wayland-only should NOT be added
        test.requires.session = vec!["any".into()];
        let tags2 = test.effective_tags();
        assert!(!tags2.contains(&"wayland-only".to_string()));
    }

    /// Tests matrix expansion generating concrete test instances from parameters.
    #[test]
    fn test_matrix_expansion() {
        let toml_str = r#"
id = "cloak/delayed-paint"
category = "animation-and-cloak"
title = "Delayed paint matrix"
description = "Tests window cold-start masking with delay {delay} ms"
kind = "both"
required = true
tags = ["visual"]

[matrix]
delay = [0, 50, 150]

[subject]
app = "fixture"
profile = "delayed-paint"
args = ["--paint-delay-ms", "{delay}"]

[action]
command = "spawn"
args = ["--anchor", "center"]
"#;

        let test: TestDefinition = toml::from_str(toml_str).unwrap();
        let expanded = test.expand_matrix().unwrap();
        assert_eq!(expanded.len(), 3);
        assert_eq!(expanded[0].id, "cloak/delayed-paint/delay-0");
        assert_eq!(expanded[1].id, "cloak/delayed-paint/delay-50");
        assert_eq!(expanded[2].id, "cloak/delayed-paint/delay-150");

        assert_eq!(
            expanded[1].subject.as_ref().unwrap().args,
            vec!["--paint-delay-ms", "50"]
        );
        assert_eq!(
            expanded[2].description,
            "Tests window cold-start masking with delay 150 ms"
        );
    }

    /// Tests test selection filtering logic defined by suite definitions.
    #[test]
    fn test_suite_filtering() {
        let suite = SuiteDefinition {
            name: "quick".into(),
            description: "Quick tests".into(),
            categories: vec!["preflight".into(), "placement".into()],
            tags: vec!["!slow".into()],
            required_only: false,
        };

        let test_pass = TestDefinition {
            id: "placement/bottom-right".into(),
            category: "placement".into(),
            subcategory: None,
            title: "".into(),
            description: "".into(),
            kind: TestKind::Auto,
            required: false,
            tags: vec!["visual".into()],
            matrix: None,
            requires: TestRequires::default(),
            subject: None,
            action: TestAction {
                command: "".into(),
                repeatable: false,
                args: vec![],
            },
            expect: TestExpect::default(),
            human: None,
            regression: None,
        };

        let test_slow = TestDefinition {
            id: "placement/slow-test".into(),
            category: "placement".into(),
            subcategory: None,
            title: "".into(),
            description: "".into(),
            kind: TestKind::Auto,
            required: false,
            tags: vec!["slow".into()],
            matrix: None,
            requires: TestRequires::default(),
            subject: None,
            action: TestAction {
                command: "".into(),
                repeatable: false,
                args: vec![],
            },
            expect: TestExpect::default(),
            human: None,
            regression: None,
        };

        let test_wrong_cat = TestDefinition {
            id: "cloak/test".into(),
            category: "animation-and-cloak".into(),
            subcategory: None,
            title: "".into(),
            description: "".into(),
            kind: TestKind::Auto,
            required: false,
            tags: vec![],
            matrix: None,
            requires: TestRequires::default(),
            subject: None,
            action: TestAction {
                command: "".into(),
                repeatable: false,
                args: vec![],
            },
            expect: TestExpect::default(),
            human: None,
            regression: None,
        };

        assert!(suite.matches(&test_pass));
        assert!(!suite.matches(&test_slow));
        assert!(!suite.matches(&test_wrong_cat));
    }
}
