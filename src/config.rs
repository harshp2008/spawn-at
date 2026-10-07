use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum WindowMode {
    SpawnOnly,
    ClampOnChange,
    PinnedBounds,
}

#[derive(Debug, Deserialize, Clone)]
pub struct AppRule {
    pub mode: WindowMode,
    pub margin: Option<u32>,
    pub avoid_panels: Option<bool>,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct Config {
    #[serde(default)]
    pub apps: HashMap<String, AppRule>,
}

impl Config {
    pub fn load() -> Option<Self> {
        let config_dir = std::env::var("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                let home = std::env::var("HOME").unwrap_or_else(|_| "".into());
                PathBuf::from(home).join(".config")
            });
            
        let config_path = config_dir.join("spawn-at").join("config.toml");
        
        if config_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&config_path) {
                if let Ok(config) = toml::from_str(&content) {
                    return Some(config);
                }
            }
        }
        
        None
    }
}
