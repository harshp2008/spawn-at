//! What the operating system decides, as opposed to the compositor: where binaries go, where the
//! login autostart entry lives, what counts as an executable, and how a started program is
//! detached from this process.
//!
//! One implementation per OS, chosen at compile time (`NativeHost`). The Linux one returns the
//! values the code used before this trait existed; off Linux a compile-only stand-in exists
//! (spawn-at stops at `ensure_supported_os` there).

use super::InstallScope;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Per-OS services used by the installer, the update check and the launcher.
pub trait Host {
    /// The directory `spawn-at install` puts the binary in for `scope`
    /// (Linux: `$HOME/.local/bin` or `/usr/local/bin`).
    fn bin_dir(&self, scope: InstallScope) -> Result<PathBuf, String>;

    /// The user's binary directory if `HOME` is set at all (non-UTF-8 values included), as the
    /// search for an existing install needs it.
    fn user_bin_dir(&self) -> Option<PathBuf>;

    /// The system-wide binary directory.
    fn system_bin_dir(&self) -> PathBuf;

    /// Where the login update-check entry is written (Linux: an XDG autostart `.desktop` file).
    fn autostart_entry_path(&self) -> Result<PathBuf, String>;

    /// True if `path` is a regular file the OS would run.
    fn is_executable_file(&self, path: &Path) -> bool;

    /// Makes `cmd` start in its own session, so it does not die with this process or its
    /// terminal.
    fn detach_child(&self, cmd: &mut Command);
}
