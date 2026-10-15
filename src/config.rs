//! # Configuration Manager
//!
//! Manages `~/.config/spawn-at/config.toml` for update preferences, release channels,
//! notification settings, and cached version metadata.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
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
    /// Returns the path of the config file: `$SPAWN_AT_CONFIG`, else
    /// `$XDG_CONFIG_HOME/spawn-at/config.toml`, else `~/.config/spawn-at/config.toml` (the same
    /// file `spawn-at config` edits).
    pub fn config_path() -> Result<PathBuf, String> {
        crate::platform::linux::config_store::LinuxConfigStore::from_env()
            .path()
            .cloned()
            .map_err(|_| "Failed to determine HOME directory".to_string())
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

    /// Persists the update preferences to the config file. Only the `[update]` table is
    /// replaced: the settings written by `spawn-at config` and any other key already in the file
    /// are kept, and the file is replaced atomically. A file that is not valid TOML is
    /// overwritten, as before (the update checker owns what it can read back).
    pub fn save(&self) -> Result<(), String> {
        let store = crate::platform::linux::config_store::LinuxConfigStore::from_env();
        store.path()?;
        self.save_to(&store)
    }

    /// [`Config::save`] into `store`.
    pub fn save_to(
        &self,
        store: &dyn spawn_at_core::settings::SettingsStore,
    ) -> Result<(), String> {
        let mut table = store
            .read()?
            .and_then(|t| t.parse::<toml::Table>().ok())
            .unwrap_or_default();
        let mine = toml::Value::try_from(self)
            .map_err(|e| format!("Failed to serialize config to TOML: {}", e))?;
        if let Some(update) = mine.get("update") {
            table.insert("update".to_string(), update.clone());
        }
        let serialized = toml::to_string_pretty(&table)
            .map_err(|e| format!("Failed to serialize config to TOML: {}", e))?;
        store.write(&serialized)
    }
}

/// Helper returning current timestamp in seconds.
pub fn current_timestamp_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Path to XDG autostart entry: `$XDG_CONFIG_HOME/autostart/spawn-at-update-check.desktop` for an
/// absolute `XDG_CONFIG_HOME`, else `~/.config/autostart/spawn-at-update-check.desktop`.
pub fn autostart_desktop_path() -> Result<PathBuf, String> {
    crate::platform::host().autostart_entry_path()
}

/// The pure part of [`autostart_desktop_path`]. The config directory comes from the same rule as
/// the config file (`config_location`, ignoring `SPAWN_AT_CONFIG`, which names one file).
pub(crate) fn autostart_path_from(
    xdg_config_home: Option<&str>,
    home: Option<&str>,
) -> Result<PathBuf, String> {
    let file = spawn_at_core::settings::config_location(None, xdg_config_home, home)
        .ok_or_else(|| "Failed to determine HOME directory".to_string())?;
    // `<config dir>/spawn-at/config.toml` -> `<config dir>`
    let dir = file
        .parent()
        .and_then(Path::parent)
        .ok_or_else(|| "Failed to determine the config directory".to_string())?;
    Ok(dir.join("autostart/spawn-at-update-check.desktop"))
}

/// Deploys an XDG autostart desktop entry so spawn-at performs a silent update check on user login.
///
/// `exe` is the absolute path of the binary to run: the session that reads the entry at login
/// does not have `~/.local/bin` on its `PATH`, so a bare `spawn-at` is never found.
pub fn install_login_autostart_entry(exe: &Path) -> Result<(), String> {
    if !exe.is_absolute() {
        return Err(format!(
            "autostart needs an absolute binary path, got '{}'",
            exe.display()
        ));
    }
    let path = autostart_desktop_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::write(&path, autostart_entry_content(exe)).map_err(|e| e.to_string())?;
    Ok(())
}

/// Quotes one word of an `Exec=` line as the Desktop Entry specification asks: a word with a
/// space or a reserved character goes in double quotes, and `"`, `` ` ``, `$` and `\` inside
/// are escaped with a backslash. A literal `%` is written `%%`.
fn exec_word(word: &str) -> String {
    let plain = !word.is_empty()
        && word
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "/._-+,:@=".contains(c));
    if plain {
        return word.to_string();
    }
    let mut out = String::from("\"");
    for c in word.chars() {
        match c {
            '"' | '`' | '$' | '\\' => {
                out.push('\\');
                out.push(c);
            }
            '%' => out.push_str("%%"),
            _ => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Text of the autostart entry for the binary at `exe`.
pub fn autostart_entry_content(exe: &Path) -> String {
    format!(
        "[Desktop Entry]\nType=Application\nName=spawn-at Update Check\n\
         Exec={} update --check --headless\nHidden=false\nNoDisplay=true\n\
         X-GNOME-Autostart-enabled=true\nComment=Check for spawn-at updates silently on login\n",
        exec_word(&exe.to_string_lossy())
    )
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

    /// The Exec= line starts with the absolute path, never a bare `spawn-at`, and keeps the
    /// arguments after it.
    #[test]
    fn test_autostart_exec_is_absolute_path() {
        let text = autostart_entry_content(Path::new("/home/u/.local/bin/spawn-at"));
        assert!(
            text.contains("\nExec=/home/u/.local/bin/spawn-at update --check --headless\n"),
            "{text}"
        );
        assert!(!text.contains("Exec=spawn-at"));
    }

    /// A home directory with a space (as in this repo's own path) is quoted so the desktop
    /// session reads it as one word; `$` and `"` are escaped.
    #[test]
    fn test_autostart_exec_quotes_special_paths() {
        let text = autostart_entry_content(Path::new("/home/a b/$x\"y/spawn-at"));
        assert!(
            text.contains("Exec=\"/home/a b/\\$x\\\"y/spawn-at\" update --check --headless\n"),
            "{text}"
        );
    }

    /// With `XDG_CONFIG_HOME` unset (or empty, or relative, as for the config file) the entry stays
    /// under `$HOME/.config`: the pre-fix path, unchanged. Pure function, no real home touched.
    #[test]
    fn test_autostart_path_unset_xdg_uses_home_dot_config() {
        let want = PathBuf::from("/tmp/h/.config/autostart/spawn-at-update-check.desktop");
        assert_eq!(autostart_path_from(None, Some("/tmp/h")).unwrap(), want);
        assert_eq!(autostart_path_from(Some(""), Some("/tmp/h")).unwrap(), want);
        assert_eq!(
            autostart_path_from(Some("rel"), Some("/tmp/h")).unwrap(),
            want
        );
        assert!(autostart_path_from(None, None).is_err());
    }

    /// An absolute `XDG_CONFIG_HOME` moves the entry there, the same directory the config file uses.
    #[test]
    fn test_autostart_path_honours_xdg_config_home() {
        let dir = std::env::temp_dir().join(format!("spawn-at-xdg-{}", std::process::id()));
        let xdg = dir.to_str().unwrap();
        let got = autostart_path_from(Some(xdg), Some("/tmp/h")).unwrap();
        assert_eq!(got, dir.join("autostart/spawn-at-update-check.desktop"));
        let cfg =
            spawn_at_core::settings::config_location(None, Some(xdg), Some("/tmp/h")).unwrap();
        assert_eq!(
            got.parent().unwrap().parent(),
            cfg.parent().unwrap().parent()
        );
    }

    /// A relative path is refused rather than written: the entry would fail at login again.
    #[test]
    fn test_autostart_refuses_relative_path() {
        assert!(install_login_autostart_entry(Path::new("spawn-at")).is_err());
    }

    /// The update checker saves its table into a file that also holds `spawn-at config`
    /// settings and unknown keys: they all survive, and the update table is replaced.
    #[test]
    fn test_save_keeps_settings_and_unknown_keys() {
        use crate::platform::linux::config_store::LinuxConfigStore;
        use spawn_at_core::settings::SettingsStore;
        let dir = std::env::temp_dir().join(format!("spawn-at-save-{}", std::process::id()));
        let store = LinuxConfigStore::at(dir.join("config.toml"));
        store
            .write(
                "[wait]\ncap_ms = 120\nodd = 3\n\n[update]\nchannel = \"stable\"\n\n[zzz]\nk = 1\n",
            )
            .unwrap();
        let mut cfg = Config::default();
        cfg.update.channel = "beta".into();
        cfg.save_to(&store).unwrap();
        let text = store.read().unwrap().unwrap();
        for kept in ["cap_ms = 120", "odd = 3", "[zzz]", "channel = \"beta\""] {
            assert!(text.contains(kept), "{kept} lost in:\n{text}");
        }
        assert!(!text.contains("stable"), "{text}");
        let _ = std::fs::remove_dir_all(&dir);
    }

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
        assert_eq!(
            deserialized.update.cached_stable_version.as_deref(),
            Some("v0.2.0")
        );
        assert_eq!(
            deserialized.update.cached_beta_version.as_deref(),
            Some("v0.2.0-beta.2")
        );
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
        assert_eq!(
            cfg.update.cached_version_for_channel("stable"),
            Some("v0.1.0")
        );
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

/// The autostart file on disk, written into a temporary `XDG_CONFIG_HOME`. The environment is
/// shared by all test threads, so each test holds `ENV_LOCK` while it changes it.
#[cfg(test)]
mod autostart_file_tests {
    use super::*;

    const EXE: &str = "/home/u/.local/bin/spawn-at";
    /// The entry exactly as it has been written since beta.2, byte for byte.
    const EXPECTED: &str = "[Desktop Entry]\nType=Application\nName=spawn-at Update Check\n\
         Exec=/home/u/.local/bin/spawn-at update --check --headless\nHidden=false\nNoDisplay=true\n\
         X-GNOME-Autostart-enabled=true\nComment=Check for spawn-at updates silently on login\n";

    /// Runs `f` with `XDG_CONFIG_HOME` pointing at a fresh temporary directory and the desktop
    /// variables set as given, then restores every variable.
    fn with_session<T>(
        desktop: &str,
        session: &str,
        backend: Option<&str>,
        f: impl FnOnce(&Path) -> T,
    ) -> T {
        let _env = crate::diagnostics::ENV_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!(
            "spawn-at-autostart-{}-{}",
            std::process::id(),
            desktop.replace(|c: char| !c.is_alphanumeric(), "_")
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let names = [
            "XDG_CONFIG_HOME",
            "XDG_CURRENT_DESKTOP",
            "XDG_SESSION_TYPE",
            "SPAWN_AT_BACKEND",
        ];
        let saved: Vec<_> = names.iter().map(std::env::var_os).collect();
        std::env::set_var("XDG_CONFIG_HOME", &dir);
        std::env::set_var("XDG_CURRENT_DESKTOP", desktop);
        std::env::set_var("XDG_SESSION_TYPE", session);
        match backend {
            Some(b) => std::env::set_var("SPAWN_AT_BACKEND", b),
            None => std::env::remove_var("SPAWN_AT_BACKEND"),
        }
        let out = f(&dir);
        for (n, v) in names.iter().zip(saved) {
            match v {
                Some(v) => std::env::set_var(n, v),
                None => std::env::remove_var(n),
            }
        }
        let _ = fs::remove_dir_all(&dir);
        out
    }

    fn entry(dir: &Path) -> PathBuf {
        dir.join("autostart/spawn-at-update-check.desktop")
    }

    /// GNOME Wayland, a supported X11 session and the opt-in unsupported case (`install.sh` sets
    /// `SPAWN_AT_BACKEND=x11` on KDE Wayland) all get the same entry, byte for byte: it carries no
    /// backend variable because `update --check --headless` never starts a backend.
    #[test]
    fn test_autostart_entry_is_identical_on_every_desktop() {
        for (desktop, session, backend) in [
            ("ubuntu:GNOME", "wayland", None),
            ("KDE", "x11", None),
            ("KDE", "wayland", Some("x11")),
        ] {
            let text = with_session(desktop, session, backend, |dir| {
                install_login_autostart_entry(Path::new(EXE)).unwrap();
                fs::read_to_string(entry(dir)).unwrap()
            });
            assert_eq!(text, EXPECTED, "{desktop} {session} {backend:?}");
            assert!(!text.contains("SPAWN_AT_BACKEND") && !text.contains("env "));
        }
    }

    /// Uninstall removes the entry whatever desktop it runs on, and does nothing when there is none.
    #[test]
    fn test_uninstall_removes_the_entry_on_every_desktop() {
        for (desktop, session, backend) in [
            ("ubuntu:GNOME", "wayland", None),
            ("KDE", "x11", None),
            ("KDE", "wayland", Some("x11")),
        ] {
            with_session(desktop, session, backend, |dir| {
                install_login_autostart_entry(Path::new(EXE)).unwrap();
                assert!(entry(dir).exists());
                uninstall_login_autostart_entry();
                assert!(!entry(dir).exists(), "{desktop}");
                uninstall_login_autostart_entry();
            });
        }
    }

    /// Installing again, from another desktop or for a binary in another place, rewrites the
    /// entry; a stale or hand-edited file is not kept.
    #[test]
    fn test_reinstall_rewrites_a_stale_entry() {
        with_session("KDE", "wayland", Some("x11"), |dir| {
            fs::create_dir_all(entry(dir).parent().unwrap()).unwrap();
            fs::write(entry(dir), "stale\n").unwrap();
            install_login_autostart_entry(Path::new(EXE)).unwrap();
            assert_eq!(fs::read_to_string(entry(dir)).unwrap(), EXPECTED);
            install_login_autostart_entry(Path::new("/usr/local/bin/spawn-at")).unwrap();
            let text = fs::read_to_string(entry(dir)).unwrap();
            assert!(text.contains("\nExec=/usr/local/bin/spawn-at update --check --headless\n"));
            assert!(!text.contains(".local/bin"));
        });
    }
}
