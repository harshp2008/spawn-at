//! Stand-in for `platform::linux` on operating systems spawn-at does not support yet.
//!
//! The neutral code (`config.rs`, `commands/spawn.rs`, `commands/config.rs`, `update.rs`) calls
//! `crate::platform::linux::config_store::LinuxConfigStore` and
//! `crate::platform::linux::gnome::is_wayland_session`. On a non-Linux target this module is
//! mounted under the name `linux` (see `platform/mod.rs`), so those calls compile unchanged.
//! Nothing here makes spawn-at work off Linux: `main` stops with a clear message first
//! (`platform::ensure_supported_os`).

/// The config file store. It is the real Linux file, compiled as it is; its path rule uses
/// `XDG_CONFIG_HOME` and `HOME`, which is not right for Windows or macOS and is not meant to be
/// used there yet.
#[path = "linux/config_store.rs"]
pub mod config_store;

/// The GNOME pieces the neutral code asks about; there is no GNOME session off Linux.
pub mod gnome {
    /// Always false: no Wayland GNOME session exists on this operating system.
    pub fn is_wayland_session() -> bool {
        false
    }
}

/// No install flags off Linux (there is no backend to configure).
#[derive(clap::Args, Debug, Clone, Default)]
pub struct BackendInstallArgs {}

/// No uninstall flags off Linux.
#[derive(clap::Args, Debug, Clone, Default)]
pub struct BackendUninstallArgs {}
