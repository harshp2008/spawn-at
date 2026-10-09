//! # Configuration Manager
//!
//! Manages `~/.config/spawn-at/config.toml` for update preferences, release channels,
//! notification settings, and cached version metadata.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub update: UpdateConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateConfig {
    #[serde(default = "default_true")]
    pub auto_check: bool,

    /// Release channel: "stable" (official releases only) or "beta"
    #[serde(default = "default_channel")]
    pub channel: String,

    /// Whether to display visual update notifications in interactive terminal sessions
    #[serde(default = "default_true")]
    pub notify: bool,

    /// Unix epoch timestamp (seconds) of the last update check
    #[serde(default)]
    pub last_check_time: u64,

    /// Cached newest stable release tag from GitHub
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cached_stable_version: Option<String>,

    /// Cached newest beta/prerelease release tag from GitHub
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cached_beta_version: Option<String>,

    /// Legacy v0.1 cached version field retained for backward-compatible deserialization
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_cached_version: Option<String>,
}

fn default_true() -> bool {
    true
}

fn default_channel() -> String {
    "stable".to_string()
}

impl Default for UpdateConfig {
    fn default() -> Self {
        Self {
            auto_check: true,
            channel: "stable".to_string(),
            notify: true,
            last_check_time: 0,
            cached_stable_version: None,
            cached_beta_version: None,
            latest_cached_version: None,
        }
    }
}

impl UpdateConfig {
    /// Returns the cached version tag for the specified channel ("stable" or "beta").
    pub fn cached_version_for_channel(&self, channel: &str) -> Option<&str> {
        if channel == "beta" {
            // For beta channel, take whichever cached version is higher
            match (&self.cached_beta_version, &self.cached_stable_version) {
                (Some(b), Some(s)) => {
                    let b_ver = crate::update::parse_semver(b);
                    let s_ver = crate::update::parse_semver(s);
                    match (b_ver, s_ver) {
                        (Some(bv), Some(sv)) => {
                            if bv >= sv {
                                Some(b.as_str())
                            } else {
                                Some(s.as_str())
                            }
                        }
                        (Some(_), None) => Some(b.as_str()),
                        (None, Some(_)) => Some(s.as_str()),
                        (None, None) => None,
                    }
                }
                (Some(b), None) => Some(b.as_str()),
                (None, Some(s)) => Some(s.as_str()),
                (None, None) => self.latest_cached_version.as_deref(),
            }
        } else {
            self.cached_stable_version
                .as_deref()
                .or(self.latest_cached_version.as_deref())
        }
    }
}

impl Config {
    /// Returns path to `~/.config/spawn-at/config.toml`.
    pub fn config_path() -> Result<PathBuf, String> {
        let home = std::env::var("HOME").map_err(|_| "Failed to determine HOME directory".to_string())?;
        Ok(PathBuf::from(home).join(".config/spawn-at/config.toml"))
    }

    /// Loads the configuration from disk, returning None if absent or corrupt.
    pub fn load() -> Option<Self> {
        let path = Self::config_path().ok()?;
        if path.exists() {
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(mut cfg) = toml::from_str::<Config>(&content) {
                    // Migrate legacy latest_cached_version if new fields are not populated
                    if cfg.update.cached_stable_version.is_none()
                        && cfg.update.cached_beta_version.is_none()
                    {
                        if let Some(ref legacy) = cfg.update.latest_cached_version {
                            if let Some(v) = crate::update::parse_semver(legacy) {
                                if v.pre.is_empty() {
                                    cfg.update.cached_stable_version = Some(legacy.clone());
                                } else {
                                    cfg.update.cached_beta_version = Some(legacy.clone());
                                }
                            }
                        }
                    }
                    return Some(cfg);
                }
            }
        }
        None
    }

    /// Loads the configuration from disk, falling back to default values.
    pub fn load_or_default() -> Self {
        Self::load().unwrap_or_default()
    }

    /// Persists the configuration to `~/.config/spawn-at/config.toml`.
    pub fn save(&self) -> Result<(), String> {
        let path = Self::config_path()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create config dir '{}': {}", parent.display(), e))?;
        }
        let serialized = toml::to_string_pretty(self)
            .map_err(|e| format!("Failed to serialize config to TOML: {}", e))?;
        fs::write(&path, serialized)
            .map_err(|e| format!("Failed to write config file '{}': {}", path.display(), e))?;
        Ok(())
    }
}

/// Helper returning current timestamp in seconds.
pub fn current_timestamp_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Path to XDG autostart entry: `~/.config/autostart/spawn-at-update-check.desktop`
pub fn autostart_desktop_path() -> Result<PathBuf, String> {
    let home = std::env::var("HOME").map_err(|_| "Failed to determine HOME directory".to_string())?;
    Ok(PathBuf::from(home).join(".config/autostart/spawn-at-update-check.desktop"))
}

/// Deploys an XDG autostart desktop entry so spawn-at performs a silent update check on user login.
pub fn install_login_autostart_entry() -> Result<(), String> {
    let path = autostart_desktop_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let content = r#"[Desktop Entry]
Type=Application
Name=spawn-at Update Check
Exec=spawn-at update --check --headless
Hidden=false
NoDisplay=true
X-GNOME-Autostart-enabled=true
Comment=Check for spawn-at updates silently on login
"#;
    fs::write(&path, content).map_err(|e| e.to_string())?;
    Ok(())
}

/// Removes the XDG autostart desktop entry during uninstallation.
pub fn uninstall_login_autostart_entry() {
    if let Ok(path) = autostart_desktop_path() {
        if path.exists() {
            let _ = fs::remove_file(path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_defaults() {
        let cfg = Config::default();
        assert!(cfg.update.auto_check);
        assert_eq!(cfg.update.channel, "stable");
        assert!(cfg.update.notify);
        assert_eq!(cfg.update.last_check_time, 0);
        assert!(cfg.update.cached_stable_version.is_none());
        assert!(cfg.update.cached_beta_version.is_none());
        assert!(cfg.update.latest_cached_version.is_none());
    }

    #[test]
    fn test_config_serialization() {
        let mut cfg = Config::default();
        cfg.update.channel = "beta".to_string();
        cfg.update.cached_stable_version = Some("v0.2.0".to_string());
        cfg.update.cached_beta_version = Some("v0.2.0-beta.2".to_string());

        let toml_str = toml::to_string_pretty(&cfg).unwrap();
        assert!(toml_str.contains("channel = \"beta\""));
        assert!(toml_str.contains("cached_stable_version = \"v0.2.0\""));
        assert!(toml_str.contains("cached_beta_version = \"v0.2.0-beta.2\""));

        let deserialized: Config = toml::from_str(&toml_str).unwrap();
        assert_eq!(deserialized.update.channel, "beta");
        assert_eq!(deserialized.update.cached_stable_version.as_deref(), Some("v0.2.0"));
        assert_eq!(deserialized.update.cached_beta_version.as_deref(), Some("v0.2.0-beta.2"));
    }

    #[test]
    fn test_old_config_with_latest_cached_version_loads_without_error() {
        let old_toml = r#"
            [update]
            auto_check = true
            channel = "stable"
            notify = true
            last_check_time = 1791541272
            latest_cached_version = "v0.1.0"
        "#;
        let cfg: Config = toml::from_str(old_toml)
            .expect("Old config with latest_cached_version must load without errors");
        assert_eq!(cfg.update.channel, "stable");
        assert_eq!(cfg.update.latest_cached_version.as_deref(), Some("v0.1.0"));
        assert_eq!(cfg.update.cached_version_for_channel("stable"), Some("v0.1.0"));
    }

    #[test]
    fn test_legacy_config_with_apps_table_loads_cleanly() {
        let legacy_toml = r#"
            [update]
            channel = "stable"
            auto_check = false

            [apps.alacritty]
            mode = "clamp-on-change"
            margin = 16
            avoid_panels = true
        "#;
        let cfg: Config = toml::from_str(legacy_toml)
            .expect("Old config with legacy [apps] section must load without errors");
        assert_eq!(cfg.update.channel, "stable");
        assert!(!cfg.update.auto_check);
    }
}
