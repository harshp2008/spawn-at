//! Unit tests for test loader, filtering, and tree visualization.

use super::*;
use crate::schema::{TestDefinition, TestKind};
use std::path::Path;

/// Tests glob pattern matching against test identifiers.
#[test]
fn test_matches_glob() {
    assert!(matches_glob("placement/*", "placement/bottom-right"));
    assert!(matches_glob("*bottom*", "placement/bottom-right"));
    assert!(!matches_glob("cloak/*", "placement/bottom-right"));
}

/// Tests multi-criteria filtering across suites, kinds, and tags.
#[test]
fn test_combinable_filter() {
    let verify_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("verify");

    if !verify_path.exists() {
        return;
    }

    let reg = TestRegistry::load_from_dir(&verify_path).unwrap();

    // 1. Filter by category path
    let filtered_cat = reg
        .filter(&TestFilter {
            category_path: Some("placement/".into()),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(filtered_cat.len(), 31);
    assert!(filtered_cat
        .iter()
        .any(|t| t.id == "placement/bottom-right"));

    // 2. Filter combining kind and tag
    let filtered_comb = reg
        .filter(&TestFilter {
            kind: Some(TestKind::Both),
            tags: vec!["visual".into()],
            required: Some(true),
            ..Default::default()
        })
        .unwrap();
    // The eight Both placement specs: the four corners, margin-0, center-200x150 and the two
    // bottom-right resize specs (smaller, below-min-both).
    let mut both_ids: Vec<_> = filtered_comb.iter().map(|t| t.id.as_str()).collect();
    both_ids.sort();
    assert_eq!(
        both_ids,
        [
            "placement/bottom-left",
            "placement/bottom-right",
            "placement/bottom-right-below-min-both",
            "placement/bottom-right-smaller",
            "placement/center-200x150",
            "placement/margin-0",
            "placement/top-left",
            "placement/top-right",
        ]
    );

    // 3. Filter Auto cases
    let filtered_auto = reg
        .filter(&TestFilter {
            kind: Some(TestKind::Auto),
            ..Default::default()
        })
        .unwrap();
    assert!(filtered_auto.len() >= 4); // delayed-paint matrix (delay-0, 50, 150, 300)
}

/// Tests hierarchical tree formatting of categorized test listings.
#[test]
fn test_format_test_tree() {
    let dummy = vec![
        TestDefinition {
            id: "placement/bottom-right".into(),
            category: "placement".into(),
            subcategory: None,
            title: "Bottom Right".into(),
            description: "".into(),
            kind: TestKind::Both,
            required: true,
            tags: vec![],
            matrix: None,
            requires: Default::default(),
            subject: None,
            action: crate::schema::TestAction {
                command: "".into(),
                repeatable: false,
                args: vec![],
            },
            expect: Default::default(),
            human: None,
            regression: None,
        },
        TestDefinition {
            id: "preflight/binary-head".into(),
            category: "basics".into(),
            subcategory: None,
            title: "Binary Head".into(),
            description: "".into(),
            kind: TestKind::Auto,
            required: true,
            tags: vec![],
            matrix: None,
            requires: Default::default(),
            subject: None,
            action: crate::schema::TestAction {
                command: "".into(),
                repeatable: false,
                args: vec![],
            },
            expect: Default::default(),
            human: None,
            regression: None,
        },
    ];

    let tree = format_test_tree(&dummy);
    assert!(tree.contains("placement/ (1 test)"));
    assert!(tree.contains("basics/ (1 test)"));
    assert!(tree.contains("Summary: 2 tests (1 auto, 0 human, 1 both, 2 required)"));
}

/// A scratch `verify/`-shaped directory under the system temp dir, removed on drop.
struct TempVerifyDir(std::path::PathBuf);

impl TempVerifyDir {
    /// Creates `<tmp>/spawn-at-verify-loader-<pid>-<name>` with a `placement/` subdirectory.
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "spawn-at-verify-loader-{}-{}",
            std::process::id(),
            name
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("placement")).unwrap();
        Self(dir)
    }
}

impl Drop for TempVerifyDir {
    /// Removes the scratch directory.
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A spec that is not valid TOML next to a valid one makes the whole load fail with an error
/// naming the broken file and "Failed to parse TOML"; the valid sibling is not loaded alone.
/// Fails if the loader skipped the broken file (the load would return Ok with 1 test).
#[test]
fn test_malformed_spec_is_an_error_not_skipped() {
    let dir = TempVerifyDir::new("malformed-spec");
    let good = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../verify/placement/top.toml");
    std::fs::copy(good, dir.0.join("placement/top.toml")).unwrap();
    std::fs::write(
        dir.0.join("placement/broken.toml"),
        "id = \"placement/broken\"\nthis is not toml\n",
    )
    .unwrap();
    let err = TestRegistry::load_from_dir(&dir.0).unwrap_err();
    assert!(err.contains("Failed to parse TOML"), "{}", err);
    assert!(err.contains("placement/broken.toml"), "{}", err);
}

/// A spec whose declared id does not match its path is also an error, not a skipped file.
#[test]
fn test_spec_with_wrong_id_is_an_error_not_skipped() {
    let dir = TempVerifyDir::new("wrong-id");
    let good = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../verify/placement/top.toml"),
    )
    .unwrap();
    std::fs::write(
        dir.0.join("placement/other.toml"),
        good, // still declares id = "placement/top"
    )
    .unwrap();
    let err = TestRegistry::load_from_dir(&dir.0).unwrap_err();
    assert!(err.contains("Test ID mismatch"), "{}", err);
}

/// A malformed file under `suites/` is reported too ("Failed to parse suite").
#[test]
fn test_malformed_suite_is_an_error_not_skipped() {
    let dir = TempVerifyDir::new("malformed-suite");
    std::fs::create_dir_all(dir.0.join("suites")).unwrap();
    std::fs::write(dir.0.join("suites/bad.toml"), "name = \n").unwrap();
    let err = TestRegistry::load_from_dir(&dir.0).unwrap_err();
    assert!(err.contains("Failed to parse suite"), "{}", err);
}

/// A spec whose `[expect.rect]` has no margin is a load error naming the spec id and the file;
/// the same spec with `margin = 16`, or with all four per-side margins, loads. Fails if the
/// oracle's implicit margin 0 is accepted silently.
#[test]
fn test_spec_without_explicit_margin_is_an_error() {
    let top = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../verify/placement/top.toml"),
    )
    .unwrap();
    assert!(top.contains("margin = 16\n"), "fixture spec changed");

    let dir = TempVerifyDir::new("no-margin");
    let without = top.replace("margin = 16\n", "");
    std::fs::write(dir.0.join("placement/top.toml"), &without).unwrap();
    let err = TestRegistry::load_from_dir(&dir.0).unwrap_err();
    assert!(err.contains("placement/top"), "{}", err);
    assert!(err.contains("no explicit margin"), "{}", err);
    assert!(err.contains("placement/top.toml"), "{}", err);

    std::fs::write(dir.0.join("placement/top.toml"), &top).unwrap();
    assert!(TestRegistry::load_from_dir(&dir.0).is_ok());

    let per_side = top.replace(
        "margin = 16\n",
        "margin_top = 16\nmargin_bottom = 16\nmargin_left = 16\nmargin_right = 16\n",
    );
    std::fs::write(dir.0.join("placement/top.toml"), per_side).unwrap();
    assert!(TestRegistry::load_from_dir(&dir.0).is_ok());
}
