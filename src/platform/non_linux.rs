//! Stand-in for `platform::linux` on operating systems spawn-at does not support yet.
//!
//! On a non-Linux target this module is mounted under the name `linux`, so code that names
//! `platform::linux::...` compiles unchanged. It defines only the names that code uses: the config
//! store, the install and uninstall flag structs, a host, and the session-action and update-hook
//! providers.
//!
//! Nothing here makes spawn-at work off Linux: `main` stops with a clear message first
//! (`platform::ensure_supported_os`), so these items are compile-only. The module is built only
//! off Linux, so a Linux build and its tests never compile it; the `check-other-os` CI job only
//! runs `cargo check` for Windows and macOS targets.

// Who uses these names: `src/config.rs`, `src/commands/spawn.rs` and `src/commands/config.rs`
// call `platform::linux::config_store::LinuxConfigStore`, and `src/platform/mod.rs` re-exports
// the flag structs and calls `host::NativeHost`, `session_actions()` and `update_hooks()`
// (grep `linux::` in src/platform/mod.rs). The mount is the `#[path = "non_linux.rs"]` module in
// src/platform/mod.rs.

/// The config file store. It is the real Linux file, compiled as it is; its path rule uses
/// `XDG_CONFIG_HOME` and `HOME`, which is not right for Windows or macOS and is not meant to be
/// used there yet.
#[path = "linux/config_store.rs"]
pub mod config_store;

/// No install flags off Linux (there is no backend to configure).
#[derive(clap::Args, Debug, Clone, Default)]
pub struct BackendInstallArgs {}

/// No uninstall flags off Linux.
#[derive(clap::Args, Debug, Clone, Default)]
pub struct BackendUninstallArgs {}

/// Compile-only [`Host`](crate::platform::host::Host) for other operating systems: the values are
/// placeholders (spawn-at stops before anything uses them).
pub mod host {
    use crate::platform::host::Host;
    use crate::platform::InstallScope;
    use std::path::{Path, PathBuf};
    use std::process::Command;

    /// The host of an unsupported operating system: `bin_dir` and `autostart_entry_path` return
    /// "spawn-at does not support this operating system yet", `user_bin_dir` is `None`, and
    /// `detach_child` does nothing.
    pub struct NativeHost;

    // `is_executable_file` is the one method with real logic: it is true for any regular file and
    // does not look at permission bits, unlike what the name suggests.
    impl Host for NativeHost {
        fn bin_dir(&self, _scope: InstallScope) -> Result<PathBuf, String> {
            Err("spawn-at does not support this operating system yet".to_string())
        }
        fn user_bin_dir(&self) -> Option<PathBuf> {
            None
        }
        fn system_bin_dir(&self) -> PathBuf {
            PathBuf::new()
        }
        fn autostart_entry_path(&self) -> Result<PathBuf, String> {
            Err("spawn-at does not support this operating system yet".to_string())
        }
        fn is_executable_file(&self, p: &Path) -> bool {
            p.metadata().is_ok_and(|m| m.is_file())
        }
        fn detach_child(&self, _cmd: &mut Command) {}
    }
}

struct NoSessionActions;

impl crate::platform::session_actions::SessionActions for NoSessionActions {
    fn log_out(&self) {}
    fn reload_shell(&self) {}
}

/// Compile-only: off Linux nothing logs out or reloads (spawn-at stops before it could).
pub fn session_actions() -> &'static dyn crate::platform::session_actions::SessionActions {
    &NoSessionActions
}

struct NoUpdateHooks;

impl crate::platform::update_hooks::UpdateHooks for NoUpdateHooks {
    fn after_unpack(
        &self,
        _new_binary: &std::path::Path,
    ) -> Option<crate::platform::update_hooks::ChangedExtension> {
        None
    }
    fn announce_logout_needed(&self, _changed: &crate::platform::update_hooks::ChangedExtension) {}
    fn offers_session_action(&self) -> bool {
        false
    }
    fn session_is_wayland(&self) -> bool {
        false
    }
}

/// Compile-only: off Linux an upgrade deploys nothing and offers no session action.
pub fn update_hooks() -> &'static dyn crate::platform::update_hooks::UpdateHooks {
    &NoUpdateHooks
}
