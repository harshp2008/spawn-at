//! Hierarchical formatting and summary generation for test definitions.

use crate::schema::{TestDefinition, TestKind};
use std::collections::BTreeMap;

/// Formats a list of tests as a tree structure grouped by category with counts.
pub fn format_test_tree(tests: &[TestDefinition]) -> String {
    let mut tree: BTreeMap<&str, Vec<&TestDefinition>> = BTreeMap::new();
    let mut auto_count = 0;
    let mut human_count = 0;
    let mut both_count = 0;
    let mut req_count = 0;

    for test in tests {
        tree.entry(test.category()).or_default().push(test);
        match test.kind {
            TestKind::Auto => auto_count += 1,
            TestKind::Human => human_count += 1,
            TestKind::Both => both_count += 1,
        }
        if test.required {
            req_count += 1;
        }
    }

    let mut out = format!(
        "Verify Test Library ({} tests across {} categories)\n",
        tests.len(),
        tree.len()
    );

    let cat_keys: Vec<&str> = tree.keys().copied().collect();
    for (i, &cat) in cat_keys.iter().enumerate() {
        let is_last_cat = i == cat_keys.len() - 1;
        let cat_branch = if is_last_cat {
            "└──"
        } else {
            "├──"
        };
        let cat_tests = &tree[cat];

        out.push_str(&format!(
            "{} {}/ ({} test{})\n",
            cat_branch,
            cat,
            cat_tests.len(),
            if cat_tests.len() == 1 { "" } else { "s" }
        ));

        let indent = if is_last_cat { "    " } else { "│   " };
        for (j, t) in cat_tests.iter().enumerate() {
            let is_last_test = j == cat_tests.len() - 1;
            let test_branch = if is_last_test {
                "└──"
            } else {
                "├──"
            };
            let req_badge = if t.required { " [REQUIRED]" } else { "" };
            out.push_str(&format!(
                "{}{} {} [{:?}]{}\n",
                indent, test_branch, t.id, t.kind, req_badge
            ));
        }
    }

    out.push_str(&format!(
        "\nSummary: {} tests ({} auto, {} human, {} both, {} required)\n",
        tests.len(),
        auto_count,
        human_count,
        both_count,
        req_count
    ));

    out
}
