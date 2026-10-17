//! The Linux [`Host`]: the values the installer, the update check and the launcher always used.
//!
//! Answers per-OS questions only: where binaries and the autostart entry go, whether a path is
//! an executable file, and how a child is detached. It reads `$HOME` and the XDG variables and
//! the file metadata; it writes nothing and starts no process. Each method's contract is on the
//! [`Host`] trait.

use crate::platform::host::Host;
use crate::platform::InstallScope;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The Linux implementation of [`Host`]: `$HOME/.local/bin` (user) or `/usr/local/bin` (system)
/// for the binary, the XDG autostart path for the update check, any execute bit for
/// "executable", and `setsid` for detaching. Holds no data.
pub struct NativeHost;

impl Host for NativeHost {
    fn bin_dir(&self, scope: InstallScope) -> Result<PathBuf, String> {
        match scope {
            InstallScope::User => {
                let home = std::env::var("HOME").map_err(|_| {
                    "Failed to determine $HOME directory for binary installation.".to_string()
                })?;
                Ok(PathBuf::from(home).join(".local/bin"))
            }
            InstallScope::System => Ok(PathBuf::from("/usr/local/bin")),
        }
    }

    fn user_bin_dir(&self) -> Option<PathBuf> {
        std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/bin"))
    }

    fn system_bin_dir(&self) -> PathBuf {
        PathBuf::from("/usr/local/bin")
    }

    // Reads `XDG_CONFIG_HOME` and `HOME`; the rule that picks between them is
    // `config::autostart_path_from`.
    fn autostart_entry_path(&self) -> Result<PathBuf, String> {
        let get = |k: &str| std::env::var(k).ok();
        crate::config::autostart_path_from(
            get("XDG_CONFIG_HOME").as_deref(),
            get("HOME").as_deref(),
        )
    }

    // `metadata()` follows symlinks. Any of the three execute bits (owner, group, other) counts;
    // whether the current user may actually run it is not checked.
    fn is_executable_file(&self, p: &Path) -> bool {
        use std::os::unix::fs::PermissionsExt;
        p.metadata()
            .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
    }

    fn detach_child(&self, cmd: &mut Command) {
        use std::os::unix::process::CommandExt;
        // SAFETY: the hook runs between fork and exec and calls only `setsid` and `setpgid`,
        // which are async-signal-safe. When `setsid` fails (the child is already a group leader),
        // `setpgid(0, 0)` puts it in its own process group instead; that result is not checked.
        unsafe {
            cmd.pre_exec(|| {
                if libc::setsid() < 0 {
                    libc::setpgid(0, 0);
                }
                Ok(())
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    /// The directories are the ones the installer always used.
    #[test]
    fn test_bin_dirs_are_the_old_values() {
        let h = NativeHost;
        assert_eq!(
            h.bin_dir(InstallScope::System),
            Ok(PathBuf::from("/usr/local/bin"))
        );
        assert_eq!(h.system_bin_dir(), PathBuf::from("/usr/local/bin"));
        if let Some(user) = h.user_bin_dir() {
            assert!(user.ends_with(".local/bin"), "{user:?}");
            assert_eq!(h.bin_dir(InstallScope::User), Ok(user));
        }
    }

    /// The autostart entry is `autostart/spawn-at-update-check.desktop` under the config dir
    /// (the exact rule is tested in `config.rs`).
    #[test]
    fn test_autostart_entry_path_names_the_desktop_file() {
        if let Ok(p) = NativeHost.autostart_entry_path() {
            assert!(
                p.ends_with("autostart/spawn-at-update-check.desktop"),
                "{p:?}"
            );
        }
    }

    /// A regular file with an execute bit is executable; a plain file, a directory and a missing
    /// path are not. Temporary directory only.
    #[test]
    fn test_is_executable_file() {
        let dir = std::env::temp_dir().join(format!("spawn-at-host-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let exe = dir.join("exe");
        let plain = dir.join("plain");
        fs::write(&exe, "#!/bin/sh\n").unwrap();
        fs::write(&plain, "x").unwrap();
        fs::set_permissions(&exe, fs::Permissions::from_mode(0o755)).unwrap();
        fs::set_permissions(&plain, fs::Permissions::from_mode(0o644)).unwrap();
        let h = NativeHost;
        assert!(h.is_executable_file(&exe));
        assert!(!h.is_executable_file(&plain));
        assert!(!h.is_executable_file(&dir));
        assert!(!h.is_executable_file(&dir.join("missing")));
        let _ = fs::remove_dir_all(&dir);
    }

    /// A detached child leads its own session (its session id is its pid); an ordinary child does
    /// not. Reads /proc/self/stat in a child shell.
    #[test]
    fn test_detach_child_starts_a_new_session() {
        fn session_is_own_pid(detach: bool) -> bool {
            let mut cmd = Command::new("sh");
            cmd.args(["-c", "cat /proc/$$/stat"]);
            if detach {
                NativeHost.detach_child(&mut cmd);
            }
            let out = cmd.output().unwrap();
            let stat = String::from_utf8_lossy(&out.stdout).into_owned();
            let pid: u32 = stat.split(' ').next().unwrap().parse().unwrap();
            let session: u32 = stat[stat.rfind(')').unwrap() + 1..]
                .split_whitespace()
                .nth(3)
                .unwrap()
                .parse()
                .unwrap();
            session == pid
        }
        assert!(session_is_own_pid(true));
        assert!(!session_is_own_pid(false));
    }
}
