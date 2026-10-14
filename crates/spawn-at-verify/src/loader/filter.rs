//! Test filter criteria and pattern matching.

use crate::schema::TestKind;

/// Combinable filter criteria for test selection.
#[derive(Debug, Clone, Default)]
pub struct TestFilter {
    /// Target suite identifier name.
    pub suite: Option<String>,
    /// Category directory path prefix.
    pub category_path: Option<String>,
    /// Associated tag labels required on matches.
    pub tags: Vec<String>,
    /// Test kind classification filter.
    pub kind: Option<TestKind>,
    /// Whether test is mandatory.
    pub required: Option<bool>,
    /// Identifier glob pattern match.
    pub id_glob: Option<String>,
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
