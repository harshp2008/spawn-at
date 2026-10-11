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
    assert_eq!(filtered_cat.len(), 1);
    assert_eq!(filtered_cat[0].id, "placement/bottom-right");

    // 2. Filter combining kind and tag
    let filtered_comb = reg
        .filter(&TestFilter {
            kind: Some(TestKind::Both),
            tags: vec!["visual".into()],
            required: Some(true),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(filtered_comb.len(), 1); // bottom-right
    assert_eq!(filtered_comb[0].id, "placement/bottom-right");

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
