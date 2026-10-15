//! Headless test verifying category schema, loader validation, and library coverage.

use spawn_at_verify::loader::{TestFilter, TestRegistry};
use spawn_at_verify::schema::{is_valid_category, TestDefinition, VALID_CATEGORIES};
use std::path::Path;

/// Proposed test case in the Part 2 test library plan.
pub struct ProposedCase {
    pub category: &'static str,
    pub id: &'static str,
    pub title: &'static str,
    pub command_or_flag: &'static str,
}

/// Canonical catalog of proposed test cases covering all categories and CLI commands.
pub const PROPOSED_CASES: &[ProposedCase] = &[
    // 1. Basics
    ProposedCase {
        category: "basics",
        id: "basics/preflight-binary-head",
        title: "Binary version matches repository HEAD",
        command_or_flag: "preflight-binary-check",
    },
    ProposedCase {
        category: "basics",
        id: "basics/version-and-help",
        title: "CLI help and version output",
        command_or_flag: "--help, --version",
    },
    // 2. Placement
    ProposedCase {
        category: "placement",
        id: "placement/bottom-right",
        title: "Window placed in bottom-right corner",
        command_or_flag: "spawn --anchor bottom-right --margin",
    },
    ProposedCase {
        category: "placement",
        id: "placement/area-screen-vs-workarea",
        title: "Screen vs Workarea boundary",
        command_or_flag: "spawn --area screen|workarea|unbounded",
    },
    ProposedCase {
        category: "placement",
        id: "placement/clamp-bounds",
        title: "Boundary clamping constraints",
        command_or_flag: "spawn --clamp",
    },
    // 3. Transforms
    ProposedCase {
        category: "transforms",
        id: "transforms/reposition-by-id",
        title: "Reposition existing window by numeric ID",
        command_or_flag: "transform --id --pos",
    },
    ProposedCase {
        category: "transforms",
        id: "transforms/maximize-and-restore",
        title: "Maximize and restore window geometry",
        command_or_flag: "maximize, restore",
    },
    ProposedCase {
        category: "transforms",
        id: "transforms/minimize-and-focus",
        title: "Minimize and unminimize via focus",
        command_or_flag: "minimize, focus",
    },
    // 4. Focus
    ProposedCase {
        category: "focus",
        id: "focus/activate-by-title",
        title: "Activate window by title substring",
        command_or_flag: "focus --title",
    },
    ProposedCase {
        category: "focus",
        id: "focus/defocus-active-window",
        title: "Relinquish keyboard focus",
        command_or_flag: "defocus",
    },
    ProposedCase {
        category: "focus",
        id: "focus/passive-spawn-no-focus",
        title: "Spawn window without stealing focus",
        command_or_flag: "spawn --no-focus",
    },
    // 5. Animation and cloak
    ProposedCase {
        category: "animation-and-cloak",
        id: "cloak/delayed-paint",
        title: "Mask cold-start window paint delay",
        command_or_flag: "spawn (actor trace cloak validation)",
    },
    ProposedCase {
        category: "animation-and-cloak",
        id: "cloak/no-flicker-center",
        title: "Zero off-target flicker when spawning centered",
        command_or_flag: "spawn --anchor center (actor trace)",
    },
    // 6. Queries
    ProposedCase {
        category: "queries",
        id: "queries/query-layout",
        title: "Fetch monitor and workarea layout",
        command_or_flag: "query layout --json",
    },
    ProposedCase {
        category: "queries",
        id: "queries/query-windows",
        title: "List open windows with metadata",
        command_or_flag: "query windows --json",
    },
    ProposedCase {
        category: "queries",
        id: "queries/query-pointer",
        title: "Fetch pointer coordinates",
        command_or_flag: "query pointer",
    },
    // 7. Robustness
    ProposedCase {
        category: "robustness",
        id: "robustness/invalid-anchor-arg",
        title: "Graceful error on invalid anchor argument",
        command_or_flag: "spawn --anchor invalid",
    },
    ProposedCase {
        category: "robustness",
        id: "robustness/app-crash-early-exit",
        title: "Target process crashes during cloak",
        command_or_flag: "spawn (crashed fixture)",
    },
    ProposedCase {
        category: "robustness",
        id: "robustness/nonexistent-window-id",
        title: "Transform nonexistent window ID returns clean error",
        command_or_flag: "transform --id 999999999",
    },
    // 8. Platform
    ProposedCase {
        category: "platform",
        id: "platform/x11-fallback",
        title: "X11 session compatibility check",
        command_or_flag: "session requires = ['x11']",
    },
    ProposedCase {
        category: "platform",
        id: "platform/multi-monitor-secondary",
        title: "Target secondary monitor selector",
        command_or_flag: "spawn --monitor 1",
    },
];

/// Scans all active test definitions in `verify/` directory and validates their category against `VALID_CATEGORIES`.
/// Asserts registry is non-empty and every discovered test belongs to a valid suite category.
/// Fails if a new test case TOML introduces an unclassified or misspelled category folder.
#[test]
fn test_every_active_case_has_valid_category() {
    let verify_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("verify");

    if !verify_path.exists() {
        return;
    }

    let registry = TestRegistry::load_from_dir(&verify_path).expect("failed to load verify dir");
    assert!(
        !registry.tests.is_empty(),
        "test registry must not be empty"
    );

    for test in &registry.tests {
        let cat = test.category();
        assert!(
            is_valid_category(cat),
            "Test '{}' has invalid category '{}'. Valid categories are: {:?}",
            test.id,
            cat,
            VALID_CATEGORIES
        );
    }
}

/// Deserializes a TOML spec lacking the mandatory `category` field and asserts serde returns an error naming "category".
/// Fails if `TestDefinition.category` is made optional or given a default value, allowing unclassified test files.
#[test]
fn test_missing_category_in_toml_is_deserialization_error() {
    let toml_missing_category = r#"
id = "placement/test-no-category"
title = "Missing category test"
description = "Should fail deserialization"
kind = "auto"
required = true

[action]
command = "spawn"
args = []

[expect]
exit_code = 0
"#;

    let res: Result<TestDefinition, _> = toml::from_str(toml_missing_category);
    assert!(
        res.is_err(),
        "TOML missing 'category' must fail deserialization"
    );
    let err = res.unwrap_err().to_string();
    assert!(
        err.contains("category"),
        "Error message should mention missing 'category': {}",
        err
    );
}

/// Parses a TOML spec with `category = "nonexistent-category-foo"` and asserts `is_valid_category` returns false.
/// Fails if `VALID_CATEGORIES` validation allows arbitrary string values instead of known suite directories.
#[test]
fn test_unknown_category_is_validation_error() {
    let toml_invalid_cat = r#"
id = "placement/test-invalid-cat"
category = "nonexistent-category-foo"
title = "Invalid category test"
description = "Should fail validation"
kind = "auto"
required = true

[action]
command = "spawn"
args = []

[expect]
exit_code = 0
"#;

    let parsed: TestDefinition = toml::from_str(toml_invalid_cat).expect("TOML parse succeeded");
    assert!(!is_valid_category(&parsed.category));
}

/// Queries `verify/` registry filtered by categories `placement`, `basics`, and `animation-and-cloak`.
/// Asserts each slice is non-empty and every returned test definition reports that category.
/// Fails if category filtering returns wrong category matches or empty results for standard suites.
#[test]
fn test_category_filtering_in_registry() {
    let verify_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("verify");

    if !verify_path.exists() {
        return;
    }

    let registry = TestRegistry::load_from_dir(&verify_path).expect("failed to load registry");

    // Filter by placement
    let placement_tests = registry
        .filter(&TestFilter {
            category_path: Some("placement".into()),
            ..Default::default()
        })
        .unwrap();
    assert!(!placement_tests.is_empty());
    for t in placement_tests {
        assert_eq!(t.category(), "placement");
    }

    // Filter by basics
    let basics_tests = registry
        .filter(&TestFilter {
            category_path: Some("basics".into()),
            ..Default::default()
        })
        .unwrap();
    assert!(!basics_tests.is_empty());
    for t in basics_tests {
        assert_eq!(t.category(), "basics");
    }

    // Filter by animation-and-cloak
    let cloak_tests = registry
        .filter(&TestFilter {
            category_path: Some("animation-and-cloak".into()),
            ..Default::default()
        })
        .unwrap();
    assert!(!cloak_tests.is_empty());
    for t in cloak_tests {
        assert_eq!(t.category(), "animation-and-cloak");
    }
}

/// The catalog has the nine hostile-app cells of `verify/hostile/` (each with a declared resize
/// outcome and exactly one hostile fixture flag), and 67 specs in all: the 36 earlier ones, these
/// nine, and the 22 version 2 specs of `queries/`, `transforms/`, `focus/` and `robustness/` (18
/// files, the anchor matrix expanding to five). The first 45 are pinned separately by
/// `catalog_golden_test`. Fails when a cell is added or removed without this count being updated.
#[test]
fn test_catalog_count_and_hostile_cells() {
    let verify_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("verify");
    let registry = TestRegistry::load_from_dir(&verify_path).expect("failed to load registry");
    assert_eq!(registry.tests.len(), 67, "catalog size");

    let hostile: Vec<_> = registry
        .tests
        .iter()
        .filter(|t| t.category() == "hostile")
        .collect();
    let mut ids: Vec<&str> = hostile.iter().map(|t| t.id.as_str()).collect();
    ids.sort();
    assert_eq!(
        ids,
        [
            "hostile/clamp-to-min",
            "hostile/default-equals-min",
            "hostile/grid-snap",
            "hostile/hung",
            "hostile/non-resizable",
            "hostile/refuse-grow",
            "hostile/restless",
            "hostile/slow-above-cap",
            "hostile/slow-below-cap",
        ]
    );
    let flags = [
        "--resize-delay-ms",
        "--min-size",
        "--max-size",
        "--resize-increment",
        "--fixed-size",
        "--stall-after-map-ms",
        "--flap-ms",
    ];
    for t in hostile {
        let resize = t
            .expect
            .resize
            .as_ref()
            .unwrap_or_else(|| panic!("{} has no [expect.resize]", t.id));
        assert!(
            ["reached", "adjusted", "timed-out", "skipped"].contains(&resize.outcome.as_str()),
            "{}: {}",
            t.id,
            resize.outcome
        );
        let args = &t.subject.as_ref().expect("subject").args;
        let used = args.iter().filter(|a| flags.contains(&a.as_str())).count();
        assert_eq!(used, 1, "{}: one hostile flag per cell", t.id);
        assert!(!t.required, "{}: hostile cells are not required", t.id);
    }
}
