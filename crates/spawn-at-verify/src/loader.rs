//! Test loader and suite registry for spawn-at-verify.

use crate::schema::{SuiteDefinition, TestDefinition, TestKind};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

/// Combinable filter criteria for test selection.
#[derive(Debug, Clone, Default)]
pub struct TestFilter {
    pub suite: Option<String>,
    pub category_path: Option<String>,
    pub tags: Vec<String>,
    pub kind: Option<TestKind>,
    pub required: Option<bool>,
    pub id_glob: Option<String>,
}

/// In-memory catalog of all discovered tests and suites.
#[derive(Debug, Clone, Default)]
pub struct TestRegistry {
    pub tests: Vec<TestDefinition>,
    pub suites: HashMap<String, SuiteDefinition>,
}

impl TestRegistry {
    /// Loads all test definitions and suites from the specified `verify/` root directory.
    pub fn load_from_dir<P: AsRef<Path>>(verify_root: P) -> Result<Self, String> {
        let root = verify_root.as_ref();
        if !root.exists() {
            return Err(format!(
                "Verify directory '{}' does not exist",
                root.display()
            ));
        }

        let mut tests = Vec::new();
        let mut seen_ids = HashSet::new();

        // 1. Discover all test toml files
        let test_files = find_test_files(root)?;

        for file_path in test_files {
            let content = fs::read_to_string(&file_path)
                .map_err(|e| format!("Failed to read {}: {}", file_path.display(), e))?;

            let parsed: TestDefinition = toml::from_str(&content)
                .map_err(|e| format!("Failed to parse TOML in {}: {}", file_path.display(), e))?;

            // Compute expected relative base ID
            let rel_path = file_path.strip_prefix(root).unwrap_or(&file_path);
            let expected_base_id = rel_path
                .with_extension("")
                .to_string_lossy()
                .trim_start_matches('/')
                .to_string();

            if parsed.id != expected_base_id {
                return Err(format!(
                    "Test ID mismatch in {}: declared id '{}', expected id '{}'",
                    file_path.display(),
                    parsed.id,
                    expected_base_id
                ));
            }

            // Expand matrix if present
            let expanded_tests = parsed.expand_matrix()?;

            for test in expanded_tests {
                if !seen_ids.insert(test.id.clone()) {
                    return Err(format!("Duplicate test ID detected: '{}'", test.id));
                }
                tests.push(test);
            }
        }

        // 2. Discover suites in verify/suites/
        let mut suites = HashMap::new();
        let suites_dir = root.join("suites");
        if suites_dir.is_dir() {
            for entry in fs::read_dir(&suites_dir).map_err(|e| e.to_string())? {
                let entry = entry.map_err(|e| e.to_string())?;
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("toml") {
                    let content = fs::read_to_string(&path)
                        .map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;
                    let suite: SuiteDefinition = toml::from_str(&content)
                        .map_err(|e| format!("Failed to parse suite {}: {}", path.display(), e))?;
                    suites.insert(suite.name.clone(), suite);
                }
            }
        }

        Ok(Self { tests, suites })
    }

    pub fn get_test(&self, id: &str) -> Option<&TestDefinition> {
        self.tests.iter().find(|t| t.id == id)
    }

    pub fn get_suite(&self, name: &str) -> Option<&SuiteDefinition> {
        self.suites.get(name)
    }

    /// Selects tests by combining multiple filtering criteria (AND semantics).
    pub fn filter(&self, filter: &TestFilter) -> Result<Vec<TestDefinition>, String> {
        let mut suite_def = None;
        if let Some(suite_name) = &filter.suite {
            suite_def = Some(
                self.get_suite(suite_name)
                    .ok_or_else(|| format!("Suite '{}' not found", suite_name))?,
            );
        }

        let filtered: Vec<TestDefinition> = self
            .tests
            .iter()
            .filter(|test| {
                // 1. Suite filter
                if let Some(suite) = suite_def {
                    if !suite.matches(test) {
                        return false;
                    }
                }

                // 2. Category / path filter
                if let Some(cat_path) = &filter.category_path {
                    let clean_cat = cat_path.trim_end_matches('/');
                    if !test.id.starts_with(clean_cat) {
                        return false;
                    }
                }

                // 3. Tags filter (supports multiple & negation)
                let eff_tags = test.effective_tags();
                for tag in &filter.tags {
                    if let Some(negated) = tag.strip_prefix('!') {
                        if eff_tags.iter().any(|t| t == negated) {
                            return false;
                        }
                    } else if !eff_tags.iter().any(|t| t == tag) {
                        return false;
                    }
                }

                // 4. Kind filter
                if let Some(kind) = filter.kind {
                    if test.kind != kind {
                        return false;
                    }
                }

                // 5. Required filter
                if let Some(req) = filter.required {
                    if test.required != req {
                        return false;
                    }
                }

                // 6. ID glob filter
                if let Some(glob) = &filter.id_glob {
                    if !matches_glob(glob, &test.id) && !matches_glob(glob, &test.title) {
                        return false;
                    }
                }

                true
            })
            .cloned()
            .collect();

        Ok(filtered)
    }

    /// Selects tests matching a specific suite name.
    pub fn filter_by_suite(&self, suite_name: &str) -> Result<Vec<TestDefinition>, String> {
        self.filter(&TestFilter {
            suite: Some(suite_name.to_string()),
            ..Default::default()
        })
    }

    /// Selects tests by ID query, title query, or regex.
    pub fn filter_by_query(&self, query: &str) -> Result<Vec<TestDefinition>, String> {
        self.filter(&TestFilter {
            id_glob: Some(query.to_string()),
            ..Default::default()
        })
    }
}

/// Matches simple glob patterns with `*` and `?`.
pub fn matches_glob(pattern: &str, text: &str) -> bool {
    let mut regex_str = String::from("^");
    for ch in pattern.chars() {
        match ch {
            '*' => regex_str.push_str(".*"),
            '?' => regex_str.push('.'),
            '.' => regex_str.push_str("\\."),
            '+' => regex_str.push_str("\\+"),
            '(' => regex_str.push_str("\\("),
            ')' => regex_str.push_str("\\)"),
            '[' => regex_str.push_str("\\["),
            ']' => regex_str.push_str("\\]"),
            c => regex_str.push(c),
        }
    }
    regex_str.push('$');
    regex::Regex::new(&regex_str)
        .map(|r| r.is_match(text))
        .unwrap_or(false)
}

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

/// Helper to recursively discover test TOMLs, excluding suites and README files.
fn find_test_files(dir: &Path) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();
    find_test_files_recursive(dir, dir, &mut files)?;
    files.sort();
    Ok(files)
}

fn find_test_files_recursive(
    root: &Path,
    current: &Path,
    results: &mut Vec<PathBuf>,
) -> Result<(), String> {
    for entry in fs::read_dir(current).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();

        if path.is_dir() {
            let rel = path.strip_prefix(root).unwrap_or(&path);
            if rel.starts_with("suites") {
                continue;
            }
            find_test_files_recursive(root, &path, results)?;
        } else if path.extension().and_then(|e| e.to_str()) == Some("toml") {
            results.push(path);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_matches_glob() {
        assert!(matches_glob("placement/*", "placement/bottom-right"));
        assert!(matches_glob("*bottom*", "placement/bottom-right"));
        assert!(!matches_glob("cloak/*", "placement/bottom-right"));
    }

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
        assert!(filtered_comb.len() >= 5); // bottom-right + delayed-paint matrix
    }

    #[test]
    fn test_format_test_tree() {
        let dummy = vec![
            TestDefinition {
                id: "placement/bottom-right".into(),
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
        assert!(tree.contains("preflight/ (1 test)"));
        assert!(tree.contains("Summary: 2 tests (1 auto, 0 human, 1 both, 2 required)"));
    }
}
