//! Consistency between the suites in `verify/suites/*.toml`, the category alias list inside
//! `SuiteDefinition::matches` (`src/schema/suite.rs`) and the specs under `verify/` (audit
//! proposal P9, gap 7 in `docs/TEST_COVERAGE_AUDIT.md`).
//!
//! Without this, a suite can name a category that has no spec and "100% of required tests pass"
//! reads as coverage, and a new spec in a category no suite selects is silently never run.
//!
//! Everything is checked through `SuiteDefinition::matches` itself, so the alias list is not
//! copied here. Tags and `required_only` are cleared before matching: only the category wiring
//! is under test.

use spawn_at_verify::loader::TestRegistry;
use spawn_at_verify::schema::{SuiteDefinition, TestDefinition, VALID_CATEGORIES};
use std::path::Path;

/// Suite category names that match no spec today, each with the reason. A name may only stay here
/// while it is still empty (`test_known_empty_list_has_no_stale_entries`): the day a spec appears
/// for it, the entry must be deleted, which is the point of the list.
///
/// The names `transform`, `query`, `resilience` and `lifecycle` were here until the version 2
/// catalog specs arrived (`transforms/`, `queries/`, `robustness/` and `focus/`); `lifecycle` also
/// needed the alias `lifecycle` -> `focus` that `SuiteDefinition::matches` now has. What is left
/// are directories under `verify/` that hold only a README.
const KNOWN_EMPTY: &[(&str, &str)] = &[
    ("launchers", "verify/launchers/ holds only a README"),
    ("apps", "verify/apps/ holds only a README"),
    ("diagnostics", "verify/diagnostics/ holds only a README"),
    (
        "regression",
        "verify/regression/ holds only a README, and `regression` is not in VALID_CATEGORIES, so the loader would reject a spec in it",
    ),
];

/// Valid categories that no suite can select, whatever tags a spec has. They have no spec today
/// (checked below), so nothing is lost yet; a spec in one of them would never run in any suite.
/// Only `platform` is left: no suite names it and no alias maps to it. (`focus` was here until
/// the alias `lifecycle` -> `focus` was added together with the first focus specs.)
const UNREACHABLE_CATEGORIES: &[&str] = &["platform"];

fn registry() -> TestRegistry {
    let verify = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("verify");
    TestRegistry::load_from_dir(&verify).expect("failed to load the verify/ directory")
}

/// `suite` reduced to one category name and no tag or required-only filter.
fn one_category(suite: &SuiteDefinition, name: &str) -> SuiteDefinition {
    SuiteDefinition {
        categories: vec![name.to_string()],
        tags: Vec::new(),
        required_only: false,
        ..suite.clone()
    }
}

fn category_only(suite: &SuiteDefinition) -> SuiteDefinition {
    SuiteDefinition {
        tags: Vec::new(),
        required_only: false,
        ..suite.clone()
    }
}

fn has_specs(reg: &TestRegistry, suite: &SuiteDefinition, name: &str) -> bool {
    let probe = one_category(suite, name);
    reg.tests.iter().any(|t| probe.matches(t))
}

fn sorted_suites(reg: &TestRegistry) -> Vec<&SuiteDefinition> {
    let mut suites: Vec<_> = reg.suites.values().collect();
    suites.sort_by(|a, b| a.name.cmp(&b.name));
    suites
}

/// The five suites exist, so the checks below cannot pass by looping over nothing.
#[test]
fn test_all_five_suites_are_loaded() {
    let reg = registry();
    let mut names: Vec<_> = reg.suites.keys().map(String::as_str).collect();
    names.sort();
    assert_eq!(
        names,
        ["full", "quick", "regression", "release-gate", "visual"]
    );
}

/// Every category a suite names has at least one spec, or is in KNOWN_EMPTY with a reason.
/// Fails when a suite file gains a category that matches nothing and is not listed.
#[test]
fn test_every_suite_category_has_specs_or_is_a_known_empty() {
    let reg = registry();
    let mut unexplained = Vec::new();
    for suite in sorted_suites(&reg) {
        for name in &suite.categories {
            let known_empty = KNOWN_EMPTY.iter().any(|(n, _)| n == name);
            if !has_specs(&reg, suite, name) && !known_empty {
                unexplained.push(format!("{}: {}", suite.name, name));
            }
        }
    }
    assert!(
        unexplained.is_empty(),
        "suite categories with no spec and no entry in KNOWN_EMPTY: {unexplained:?}"
    );
}

/// KNOWN_EMPTY is exact: each entry is still named by a suite and still has no spec. Fails when
/// a spec is added for a listed name (delete the entry) or when a suite stops naming it.
#[test]
fn test_known_empty_list_has_no_stale_entries() {
    let reg = registry();
    for (name, why) in KNOWN_EMPTY {
        let named_by: Vec<_> = sorted_suites(&reg)
            .into_iter()
            .filter(|s| s.categories.iter().any(|c| c == name))
            .collect();
        assert!(
            !named_by.is_empty(),
            "`{name}` is in KNOWN_EMPTY but no suite names it any more ({why})"
        );
        for suite in named_by {
            assert!(
                !has_specs(&reg, suite, name),
                "`{name}` now matches specs in suite `{}`: remove it from KNOWN_EMPTY ({why})",
                suite.name
            );
        }
    }
}

/// The wiring direction: every spec is selected, by category, by at least one suite. A spec added
/// in a category that no suite names (or aliases) fails here.
#[test]
fn test_every_spec_is_selected_by_some_suite() {
    let reg = registry();
    let unwired: Vec<_> = reg
        .tests
        .iter()
        .filter(|t| !reg.suites.values().any(|s| category_only(s).matches(t)))
        .map(|t| format!("{} (category {})", t.id, t.category()))
        .collect();
    assert!(
        unwired.is_empty(),
        "specs that no suite selects (name the category in a suite file or add an alias): {unwired:?}"
    );
}

/// Pins which valid categories no suite can reach. Today `focus` and `platform`, and neither has a
/// spec. Fails if a spec appears in one of them (it would never run), and also if a suite or alias
/// change makes one reachable (delete it from UNREACHABLE_CATEGORIES and from KNOWN_EMPTY's note).
#[test]
fn test_unreachable_categories_are_exactly_the_listed_empty_ones() {
    let reg = registry();
    let template = reg.tests.first().expect("the catalog is not empty").clone();
    let mut unreachable = Vec::new();
    for cat in VALID_CATEGORIES {
        let probe = TestDefinition {
            category: cat.to_string(),
            ..template.clone()
        };
        if !reg
            .suites
            .values()
            .any(|s| category_only(s).matches(&probe))
        {
            unreachable.push(*cat);
        }
    }
    assert_eq!(unreachable, UNREACHABLE_CATEGORIES);
    for cat in UNREACHABLE_CATEGORIES {
        assert!(
            !reg.tests.iter().any(|t| t.category() == *cat),
            "`{cat}` has specs but no suite can select it"
        );
    }
}

/// A spec tagged `moves-focus` (every spec that launches a window carries it, see
/// `schema::v2`) is left out of `quick` and `release-gate`-style selection by tag only; `full`
/// still runs it. Fails if `quick` stops excluding the tag.
#[test]
fn test_quick_leaves_out_moves_focus_specs_and_full_keeps_them() {
    let reg = registry();
    let quick = &reg.suites["quick"];
    let full = &reg.suites["full"];
    let mut t: TestDefinition = reg
        .tests
        .iter()
        .find(|t| quick.matches(t) && full.matches(t))
        .expect("a spec both suites select")
        .clone();
    assert!(quick.matches(&t) && full.matches(&t));
    t.tags.push("moves-focus".to_string());
    assert!(
        !quick.matches(&t),
        "quick must not select a moves-focus spec"
    );
    assert!(full.matches(&t), "full still selects it");
}
