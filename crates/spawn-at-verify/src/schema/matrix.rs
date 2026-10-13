//! Test definition matrix expansion and categorization helpers.

use super::types::*;

impl TestDefinition {
    /// The rectangle expectation the oracle must judge: `expect.rect` with its `size` replaced by
    /// `max(requested, fixture minimum)` per axis when the subject declares `min_width` /
    /// `min_height`. The position is then the anchor for that final size. `None` when the test
    /// has no rectangle expectation; a rectangle without a size is returned unchanged.
    pub fn effective_expect_rect(&self) -> Option<ExpectRect> {
        let mut rect = self.expect.rect.clone()?;
        if let (Some(size), Some(subject)) = (rect.size.as_mut(), self.subject.as_ref()) {
            if size.len() >= 2 {
                let (w, h) = crate::oracle::final_size_with_min(
                    (size[0], size[1]),
                    subject.min_width,
                    subject.min_height,
                );
                size[0] = w;
                size[1] = h;
            }
        }
        Some(rect)
    }

    /// Returns the effective tags for this test, automatically deriving
    /// platform tags (`wayland-only`, `x11-only`, `gnome-only`) from `[requires]`.
    pub fn effective_tags(&self) -> Vec<String> {
        let mut tags = self.tags.clone();

        let is_any_session = self.requires.session.iter().any(|s| s == "any");
        let has_wayland = self.requires.session.iter().any(|s| s == "wayland");
        let has_x11 = self.requires.session.iter().any(|s| s == "x11");

        if !is_any_session {
            if has_wayland && !has_x11 && !tags.contains(&"wayland-only".to_string()) {
                tags.push("wayland-only".to_string());
            }
            if has_x11 && !has_wayland && !tags.contains(&"x11-only".to_string()) {
                tags.push("x11-only".to_string());
            }
        }

        let is_any_backend = self.requires.backend.iter().any(|b| b == "any");
        let has_gnome = self.requires.backend.iter().any(|b| b == "gnome");
        let has_x11_backend = self.requires.backend.iter().any(|b| b == "x11");

        if !is_any_backend {
            if has_gnome && !has_x11_backend && !tags.contains(&"gnome-only".to_string()) {
                tags.push("gnome-only".to_string());
            }
            if has_x11_backend && !has_gnome && !tags.contains(&"x11-only".to_string()) {
                tags.push("x11-only".to_string());
            }
        }

        tags
    }

    /// Returns the required category of this test (e.g. "placement", "basics").
    pub fn category(&self) -> &str {
        &self.category
    }

    /// Expands matrix variations if a `[matrix]` table is present.
    /// If no matrix is defined, returns `vec![self.clone()]`.
    pub fn expand_matrix(&self) -> Result<Vec<TestDefinition>, String> {
        let matrix = match &self.matrix {
            Some(m) if !m.is_empty() => m,
            _ => return Ok(vec![self.clone()]),
        };

        // For V1, we expand key-value combinations.
        let mut keys: Vec<String> = matrix.keys().cloned().collect();
        keys.sort(); // Deterministic ordering

        let mut combinations: Vec<Vec<(String, String)>> = vec![vec![]];

        for key in &keys {
            let values = &matrix[key];
            if values.is_empty() {
                return Err(format!(
                    "Matrix parameter '{}' has empty values array in test '{}'",
                    key, self.id
                ));
            }

            let mut next_combinations = Vec::new();
            for comb in &combinations {
                for val in values {
                    let val_str = match val {
                        toml::Value::String(s) => s.clone(),
                        toml::Value::Integer(i) => i.to_string(),
                        toml::Value::Float(f) => f.to_string(),
                        toml::Value::Boolean(b) => b.to_string(),
                        other => other.to_string(),
                    };
                    let mut new_comb = comb.clone();
                    new_comb.push((key.clone(), val_str));
                    next_combinations.push(new_comb);
                }
            }
            combinations = next_combinations;
        }

        let mut expanded = Vec::with_capacity(combinations.len());

        for comb in combinations {
            let mut clone = self.clone();
            // Build suffix: key-value or key1-val1/key2-val2
            let suffix_parts: Vec<String> =
                comb.iter().map(|(k, v)| format!("{}-{}", k, v)).collect();
            clone.id = format!("{}/{}", self.id, suffix_parts.join("-"));

            // Helper to substitute placeholders like "{delay}" with the value
            let substitute_str = |text: &str| -> String {
                let mut res = text.to_string();
                for (k, v) in &comb {
                    let placeholder = format!("{{{}}}", k);
                    res = res.replace(&placeholder, v);
                }
                res
            };

            clone.title = substitute_str(&clone.title);
            clone.description = substitute_str(&clone.description);

            if let Some(sub) = &mut clone.subject {
                sub.app = substitute_str(&sub.app);
                sub.profile = substitute_str(&sub.profile);
                sub.args = sub.args.iter().map(|a| substitute_str(a)).collect();
            }

            clone.action.command = substitute_str(&clone.action.command);
            clone.action.args = clone
                .action
                .args
                .iter()
                .map(|a| substitute_str(a))
                .collect();

            if let Some(hum) = &mut clone.human {
                hum.watch = substitute_str(&hum.watch);
            }

            // Remove matrix definition on expanded instances so they are treated as concrete tests
            clone.matrix = None;
            expanded.push(clone);
        }

        Ok(expanded)
    }
}

/// Recognized valid test category names.
pub const VALID_CATEGORIES: &[&str] = &[
    "basics",
    "placement",
    "transforms",
    "focus",
    "animation-and-cloak",
    "queries",
    "robustness",
    "platform",
    "hostile",
];

/// Returns true if the given category string is in VALID_CATEGORIES.
pub fn is_valid_category(cat: &str) -> bool {
    VALID_CATEGORIES.contains(&cat)
}
