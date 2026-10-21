//! # GNOME Compositor Backend & Extension Driver
//!
//! The backend for a GNOME Shell session that has the spawn-at extension, chosen by
//! `registry.rs` when `XDG_CURRENT_DESKTOP` contains GNOME and the session bus connects. It does
//! not check the session type itself. The driver implements `Driver`, `CompositorBackend` and
//! `Provisioner`; the public paths other modules use are the re-exports at the bottom of this
//! file.
//!
//! It must not place windows by itself: all window knowledge is in the extension
//! (`assets/gnome/`), which this module reaches over the session bus and installs into the
//! user's extensions directory.
//!
//! ## How placement works here
//!
//! The order is inverted compared with "start, then move": the driver registers the placement
//! first and starts the program second.
//!
//! 1. **Arm:** `ArmSpawn(target_id, instructions_json)` on the extension registers the
//!    instruction list (see `mechanics.rs`) for a window that does not exist yet. The extension
//!    holds it for 15 s (named target) or 1.2 s (wildcard `"*"`).
//! 2. **Start:** the spawn command starts the program with `XDG_ACTIVATION_TOKEN` and
//!    `DESKTOP_STARTUP_ID` set to the entry's key.
//! 3. **Match:** the extension listens to `window-created` and `map`. It compares the window's
//!    startup id, WM class, GTK application id and sandboxed app id with the armed target
//!    (case-insensitive, equal or one contains the other, plus a gnome-terminal pairing).
//! 4. **Cloak, place, reveal:** it hides the window (`Cloak` sets the actor's opacity to 0),
//!    runs the size, wait and position steps, and `Uncloak` reveals it. It then emits
//!    `SpawnClaimed` with the final frame.
//!
//! That steps 3 and 4 happen before the first frame is shown is the design goal; this repo has
//! no measurement of how many frames a late move would show (not verified). Why Wayland clients
//! cannot move their own windows is a property of the Wayland protocol, not checked here.
//!
//! ## Table of Contents
//! - **1. CLI Configuration & Arguments**
//!   - [`GnomeInstallArgs`]: Flags controlling extension enablement, X11 reload, and Wayland logout during install.
//!   - [`GnomeUninstallArgs`]: Flags controlling file cleanup, X11 reload, and Wayland logout during uninstall.
//! - **2. Session & Desktop Detection**
//!   - `detect::is_wayland_session`: Wayland or not, from `loginctl show-session`, falling back to `$WAYLAND_DISPLAY`; `detect::detect_gnome_shell_major_version` reads `gnome-shell --version`.
//! - **3. Execution & Reload Helpers**
//!   - `deploy::restart_gnome_shell_x11`: Simulates `Alt+F2` -> `r` -> `Enter` via `xdotool` on X11.
//! - **4. Driver Construction & Internal State**
//!   - [`GnomeWaylandDriver`]: Core driver managing D-Bus communication with the GNOME extension.
//! - **5. Pre-Map Arming Driver Trait**
//!   - [`Driver`](spawn_at_core::Driver): Pre-registers window targets and provides launch environment variables.
//! - **6. Runtime Compositor Backend Trait**
//!   - [`CompositorBackend`](crate::platform::CompositorBackend): window queries and actions, the claim wait, and the capabilities. `install` and `uninstall` are not part of it: they are the `Provisioner` implementation in `deploy.rs`.
//! - **7. Unit Tests**
//!   - Argument defaults and `--no-action` precedence, the version parser, the embedded file lists against `assets/gnome`, the legacy and modern D-Bus XML, and the driver against D-Bus stubs (close, layout, windows, claims, window control).
//!
//! ## Files
//! - `args.rs`: the install and uninstall flags (section 1).
//! - `detect.rs`: GNOME Shell version and Wayland session detection (section 2).
//! - `deploy.rs`: the embedded extension files, `install` / `uninstall` of the extension, and the
//!   log-out / reload helpers (sections 2 and 3).
//! - `driver.rs`: `GnomeWaylandDriver`, the claim subscription, and the `Driver` and
//!   `CompositorBackend` implementations (sections 4 to 6).
//! - `tests.rs`, `tests_control.rs`: the unit tests (section 7), compiled only for tests.
//! - `dbus.rs`: the zbus proxy of the extension's interface.
//! - `mechanics.rs`: the instruction list and placement payload sent to the extension.
//! - `logout.rs`: whether an install still needs a log-out.
//! - `diagnose.rs`: why the extension did not answer on the bus (not installed, disabled, OUT OF DATE, old protocol).

pub mod dbus;
pub mod logout;
pub mod mechanics;

mod args;
mod deploy;
mod detect;
mod diagnose;
mod driver;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_control;

// The public paths the rest of the crate uses (`platform::linux::gnome::...`) are kept here.
pub use args::{GnomeInstallArgs, GnomeUninstallArgs};
pub use deploy::{GnomeSessionActions, GnomeUpdateHooks};
pub use driver::GnomeWaylandDriver;
