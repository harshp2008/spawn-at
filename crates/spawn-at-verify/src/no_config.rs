//! Keeps the user's `spawn-at` config file out of harness runs.
//!
//! A run measures the defaults (or the flags the harness passes itself), so a `wait.cap_ms` the
//! user set for their own desktop must not leak into it. `spawn-at` reads its config from the file
//! named by `SPAWN_AT_CONFIG` when that is set; the harness sets it to a path that does not
//! exist, which `spawn-at` reads as "no config file".

/// The environment variable `spawn-at` reads (`spawn_at_core::settings::ENV_CONFIG_PATH`; a test
/// below keeps the two equal, because this crate only dev-depends on the core).
pub const ENV_CONFIG_PATH: &str = "SPAWN_AT_CONFIG";

use std::process::Command;

/// A path no config file is ever at.
pub const NO_CONFIG_PATH: &str = "/nonexistent/spawn-at-verify-no-config/config.toml";

/// Makes `cmd` (a `spawn-at` invocation of a run) ignore the user's config file.
pub fn isolate(cmd: &mut Command) {
    cmd.env(ENV_CONFIG_PATH, NO_CONFIG_PATH);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The variable name here is the one the product reads.
    #[test]
    fn test_env_name_matches_the_core() {
        assert_eq!(ENV_CONFIG_PATH, spawn_at_core::settings::ENV_CONFIG_PATH);
    }

    /// The command carries `SPAWN_AT_CONFIG=<nonexistent path>` whatever the parent has set, and a
    /// real child process sees exactly that value even with an `XDG_CONFIG_HOME` holding a config
    /// of its own (the stand-in child prints its environment instead of reading a file).
    #[test]
    fn test_isolated_command_sees_a_nonexistent_config_path() {
        let dir = std::env::temp_dir().join(format!("spawn-at-noconfig-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("spawn-at")).unwrap();
        std::fs::write(dir.join("spawn-at/config.toml"), "[wait]\ncap_ms = 7\n").unwrap();
        let mut cmd = Command::new("sh");
        cmd.args(["-c", "printf '%s' \"$SPAWN_AT_CONFIG\""])
            .env("XDG_CONFIG_HOME", &dir)
            .env(ENV_CONFIG_PATH, dir.join("spawn-at/config.toml"));
        isolate(&mut cmd);
        let out = cmd.output().unwrap();
        assert_eq!(String::from_utf8_lossy(&out.stdout), NO_CONFIG_PATH);
        assert!(!std::path::Path::new(NO_CONFIG_PATH).exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
