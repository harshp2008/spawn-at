//! Unit tests for schema models and matrix expansion rules.

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
