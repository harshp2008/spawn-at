//! The config file on Linux: where it is and how it is read and written.
//!
//! Location (pure rule in `spawn_at_core::settings::config_location`): the file named by
//! `SPAWN_AT_CONFIG` if set, else `$XDG_CONFIG_HOME/spawn-at/config.toml` for an absolute
//! `XDG_CONFIG_HOME`, else `$HOME/.config/spawn-at/config.toml`.

use spawn_at_core::settings::{config_location, SettingsStore, ENV_CONFIG_PATH};
use std::fs;
use std::io::Write;
use std::path::PathBuf;

/// The user's config file.
pub struct LinuxConfigStore {
    path: Option<PathBuf>,
}

impl LinuxConfigStore {
    /// The store for the environment of this process.
    pub fn from_env() -> Self {
        let get = |k: &str| std::env::var(k).ok();
        Self {
            path: config_location(
                get(ENV_CONFIG_PATH).as_deref(),
                get("XDG_CONFIG_HOME").as_deref(),
                get("HOME").as_deref(),
            ),
        }
    }

    /// A store at `path` (tests).
    #[cfg(test)]
    pub fn at(path: PathBuf) -> Self {
        Self { path: Some(path) }
    }

    /// The file path, or an error when neither `XDG_CONFIG_HOME` nor `HOME` is usable.
    pub fn path(&self) -> Result<&PathBuf, String> {
        self.path
            .as_ref()
            .ok_or_else(|| "cannot find the config directory: HOME is not set".to_string())
    }
}

impl SettingsStore for LinuxConfigStore {
    fn location(&self) -> String {
        self.path.as_ref().map_or_else(
            || "(no config location)".into(),
            |p| p.display().to_string(),
        )
    }

    /// `None` for a file that does not exist (also when its directory does not).
    fn read(&self) -> Result<Option<String>, String> {
        let Ok(path) = self.path() else {
            return Ok(None);
        };
        match fs::read_to_string(path) {
            Ok(t) => Ok(Some(t)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(format!("cannot read config file '{}': {e}", path.display())),
        }
    }

    /// Writes a temporary file in the same directory, flushes it to disk and renames it over the
    /// target, so a reader sees the old file or the new one. The directory is created if needed.
    fn write(&self, text: &str) -> Result<(), String> {
        let path = self.path()?;
        let dir = path
            .parent()
            .ok_or_else(|| format!("'{}' has no directory", path.display()))?;
        fs::create_dir_all(dir)
            .map_err(|e| format!("cannot create config directory '{}': {e}", dir.display()))?;
        let tmp = dir.join(format!(
            ".{}.tmp.{}",
            path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("config"),
            std::process::id()
        ));
        let result = (|| -> std::io::Result<()> {
            let mut f = fs::File::create(&tmp)?;
            f.write_all(text.as_bytes())?;
            f.sync_all()?;
            fs::rename(&tmp, path)
        })();
        if let Err(e) = result {
            let _ = fs::remove_file(&tmp);
            return Err(format!(
                "cannot write config file '{}': {e}",
                path.display()
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("spawn-at-cfg-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        d
    }

    /// A missing file and a missing directory read as "no file"; write creates the directory,
    /// replaces the content, and leaves no temporary file behind.
    #[test]
    fn test_read_missing_write_creates_and_replaces() {
        let dir = tmp("rw");
        let store = LinuxConfigStore::at(dir.join("a/b/config.toml"));
        assert_eq!(store.read(), Ok(None));
        store.write("[wait]\ncap_ms = 1\n").unwrap();
        store.write("[wait]\ncap_ms = 2\n").unwrap();
        assert_eq!(
            store.read().unwrap().as_deref(),
            Some("[wait]\ncap_ms = 2\n")
        );
        let names: Vec<_> = fs::read_dir(dir.join("a/b"))
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(names.len(), 1, "{names:?}");
        let _ = fs::remove_dir_all(&dir);
    }

    /// A path that is a directory is an error on read (not "no file"), so a spawn warns instead
    /// of silently using defaults for a file it could not read.
    #[test]
    fn test_unreadable_file_is_an_error() {
        let dir = tmp("dir");
        fs::create_dir_all(dir.join("config.toml")).unwrap();
        let store = LinuxConfigStore::at(dir.join("config.toml"));
        assert!(store
            .read()
            .unwrap_err()
            .contains("cannot read config file"));
        let _ = fs::remove_dir_all(&dir);
    }
}
