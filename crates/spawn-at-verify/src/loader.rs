//! Test loader and suite registry for spawn-at-verify.

use crate::schema::{SuiteDefinition, TestDefinition};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

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

    /// Selects tests matching a specific suite name.
    pub fn filter_by_suite(&self, suite_name: &str) -> Result<Vec<TestDefinition>, String> {
        let suite = self
            .get_suite(suite_name)
            .ok_or_else(|| format!("Suite '{}' not found", suite_name))?;

        let filtered: Vec<TestDefinition> = self
            .tests
            .iter()
            .filter(|t| suite.matches(t))
            .cloned()
            .collect();

        Ok(filtered)
    }

    /// Selects tests by ID query, title query, or regex.
    pub fn filter_by_query(&self, query: &str) -> Result<Vec<TestDefinition>, String> {
        let regex =
            regex::Regex::new(query).map_err(|e| format!("Invalid regex '{}': {}", query, e))?;

        let matched: Vec<TestDefinition> = self
            .tests
            .iter()
            .filter(|t| regex.is_match(&t.id) || regex.is_match(&t.title))
            .cloned()
            .collect();

        Ok(matched)
    }

    /// Selects tests by category.
    pub fn filter_by_category(&self, category: &str) -> Vec<TestDefinition> {
        self.tests
            .iter()
            .filter(|t| t.category() == category)
            .cloned()
            .collect()
    }

    /// Selects tests by tag (matching effective derived tags).
    pub fn filter_by_tag(&self, tag: &str) -> Vec<TestDefinition> {
        self.tests
            .iter()
            .filter(|t| t.effective_tags().iter().any(|tg| tg == tag))
            .cloned()
            .collect()
    }
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
            // Exclude suites directory from individual test definitions
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
    fn test_load_workspace_verify_directory() {
        // Points to real verify directory in the repo
        let verify_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("verify");

        if !verify_path.exists() {
            return;
        }

        let registry =
            TestRegistry::load_from_dir(&verify_path).expect("Failed to load verify registry");

        // We expect at least preflight/binary-head, placement/bottom-right, and expanded delayed-paint tests
        assert!(registry.tests.len() >= 6);

        assert!(registry.get_test("preflight/binary-head").is_some());
        assert!(registry.get_test("placement/bottom-right").is_some());
        assert!(registry.get_test("cloak/delayed-paint/delay-0").is_some());
        assert!(registry.get_test("cloak/delayed-paint/delay-150").is_some());

        // Suites check
        assert!(registry.get_suite("quick").is_some());
        assert!(registry.get_suite("release-gate").is_some());

        // Filter by suite quick
        let quick_tests = registry
            .filter_by_suite("quick")
            .expect("quick suite filter failed");
        assert!(!quick_tests.is_empty());
    }
}
