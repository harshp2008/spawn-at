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

    /// Release channel: "stable" (official releases only) or "all" (includes beta/pre-releases)
    #[serde(default = "default_channel")]
    pub channel: String,

    /// Whether to display visual update notifications in interactive terminal sessions
    #[serde(default = "default_true")]
    pub notify: bool,

    /// Unix epoch timestamp (seconds) of the last update check
    #[serde(default)]
    pub last_check_time: u64,

    /// Latest release version tag cached from GitHub
    #[serde(default)]
    pub latest_cached_version: String,
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
            latest_cached_version: String::new(),
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
                if let Ok(cfg) = toml::from_str::<Config>(&content) {
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
        assert!(cfg.update.latest_cached_version.is_empty());
    }

    #[test]
    fn test_config_serialization() {
        let mut cfg = Config::default();
        cfg.update.channel = "all".to_string();
        cfg.update.latest_cached_version = "v0.2.0".to_string();

        let toml_str = toml::to_string_pretty(&cfg).unwrap();
        assert!(toml_str.contains("channel = \"all\""));
        assert!(toml_str.contains("latest_cached_version = \"v0.2.0\""));

        let deserialized: Config = toml::from_str(&toml_str).unwrap();
        assert_eq!(deserialized.update.channel, "all");
        assert_eq!(deserialized.update.latest_cached_version, "v0.2.0");
    }

    #[test]
    fn test_legacy_config_with_apps_table_loads_cleanly() {
        let legacy_toml = r#"
            [update]
            channel = "all"
            auto_check = false

            [apps.alacritty]
            mode = "clamp-on-change"
            margin = 16
            avoid_panels = true
        "#;
        let cfg: Config = toml::from_str(legacy_toml)
            .expect("Old config with legacy [apps] section must load without errors");
        assert_eq!(cfg.update.channel, "all");
        assert!(!cfg.update.auto_check);
    }
}
