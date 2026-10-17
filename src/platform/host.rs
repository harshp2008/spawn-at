//! What the operating system decides, as opposed to the compositor: where binaries go, where the
//! login autostart entry lives, what counts as an executable, and how a started program is
//! detached from this process.
//!
//! One implementation per OS, chosen at compile time (`NativeHost`); `platform::host()` hands it
//! out. It reads the environment and the file system and starts no process; it does not talk to
//! a compositor.
// Implementors: `platform::linux::host::NativeHost`, and off Linux the
// compile-only stand-in in `non_linux.rs` (spawn-at stops at `ensure_supported_os` there; its
// values are placeholders). A test in the Linux module asserts `bin_dir(User)` equals
// `user_bin_dir()` when that is `Some`; no such rule is checked for the stand-in.

use super::InstallScope;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Per-OS services used by the installer, the update check and the launcher.
pub trait Host {
    /// The directory `spawn-at install` puts the binary in for `scope`
    /// (Linux: `$HOME/.local/bin` or `/usr/local/bin`).
    ///
    /// Errors: a message string when the directory cannot be determined (Linux, user scope:
    /// `HOME` unset or not UTF-8).
    fn bin_dir(&self, scope: InstallScope) -> Result<PathBuf, String>;

    /// The user's binary directory if `HOME` is set at all (non-UTF-8 values included), as the
    /// search for an existing install needs it.
    fn user_bin_dir(&self) -> Option<PathBuf>;

    /// The system-wide binary directory.
    fn system_bin_dir(&self) -> PathBuf;

    /// Where the login update-check entry is written (Linux: an XDG autostart `.desktop` file).
    ///
    /// Errors: a message string when the config directory cannot be determined.
    fn autostart_entry_path(&self) -> Result<PathBuf, String>;

    /// True if `path` is a regular file the OS would run (Linux: any execute bit set).
    fn is_executable_file(&self, path: &Path) -> bool;

    /// Makes `cmd` start in its own session, so it does not die with this process or its
    /// terminal. Only configures `cmd`; nothing is started here.
    ///
    /// Linux: a `pre_exec` hook that calls `setsid` and, if that fails, `setpgid(0, 0)`.
    fn detach_child(&self, cmd: &mut Command);
}
