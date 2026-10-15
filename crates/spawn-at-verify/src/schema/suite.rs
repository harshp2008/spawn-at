//! Test suite definition schema models.

use super::types::*;
use serde::{Deserialize, Serialize};

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
                    || (c == "lifecycle" && test_cat == "focus")
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
