//! Test registry discovery, loading, and collection management.

use crate::loader::filter::{matches_glob, TestFilter};
use crate::schema::{SuiteDefinition, TestDefinition};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

/// In-memory catalog of all discovered tests and suites.
#[derive(Debug, Clone, Default)]
pub struct TestRegistry {
    /// All loaded individual test specifications.
    pub tests: Vec<TestDefinition>,
    /// Predefined test suite collections mapped by name.
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

            // The oracle assumes margin 0 when `[expect.rect]` states none, but `spawn-at spawn`
            // defaults to 16 (`src/cli/args.rs:134`), so an implicit margin would be judged wrong.
            if let Some(rect) = &parsed.expect.rect {
                let all_sides = rect.margin_top.is_some()
                    && rect.margin_bottom.is_some()
                    && rect.margin_left.is_some()
                    && rect.margin_right.is_some();
                if rect.margin.is_none() && !all_sides {
                    return Err(format!(
                        "Spec '{}' ({}): [expect.rect] has no explicit margin; set `margin` (or all four per-side margins), because the oracle assumes 0 while spawn-at defaults to 16",
                        parsed.id,
                        file_path.display()
                    ));
                }
            }

            if !crate::schema::is_valid_category(&parsed.category) {
                return Err(format!(
                    "Invalid category '{}' in {}. Valid categories are: {:?}",
                    parsed.category,
                    file_path.display(),
                    crate::schema::VALID_CATEGORIES
                ));
            }

            // Version 2 specs (and the refusal of version 2 keys in a version 1 spec).
            crate::schema::v2::validate(&parsed)?;

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

    /// Retrieves a test specification by its unique identifier.
    pub fn get_test(&self, id: &str) -> Option<&TestDefinition> {
        self.tests.iter().find(|t| t.id == id)
    }

    /// Retrieves a predefined test suite definition by its name.
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
                    let cat_matches = test.category() == clean_cat
                        || (clean_cat == "preflight" && test.category() == "basics")
                        || (clean_cat == "cloak" && test.category() == "animation-and-cloak")
                        || (clean_cat == "transform" && test.category() == "transforms")
                        || (clean_cat == "query" && test.category() == "queries")
                        || (clean_cat == "resilience" && test.category() == "robustness")
                        || test.id.starts_with(clean_cat);
                    if !cat_matches {
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
}

/// Helper to recursively discover test TOMLs, excluding suites and README files.
fn find_test_files(dir: &Path) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();
    find_test_files_recursive(dir, dir, &mut files)?;
    files.sort();
    Ok(files)
}

/// Recursively scans directory for TOML test definitions.
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
