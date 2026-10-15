//! # GNOME Compositor Backend & Extension Driver
//!
//! Provides the primary compositor backend for GNOME Shell on Linux across both
//! Wayland and X11 sessions.
//!
//! ## Architectural Overview: Zero-Flicker Window Placement on GNOME
//!
//! ### 1. The Wayland Isolation Barrier
//! Wayland compositors (specifically Mutter in GNOME) enforce a strict isolation model:
//! client applications do not know their absolute desktop coordinates and are denied
//! any protocol requests to position their own windows globally on screen.
//! Unlike X11, there is no `XMoveWindow` or EWMH `_NET_MOVERESIZE_WINDOW` equivalent
//! available to arbitrary processes.
//!
//! ### 2. The Arming & Pre-Registration Pattern
//! To place a window at exact coordinates on its very first frame without flash-and-jump,
//! the sequence of events is inverted:
//!
//! - **Conventional (Faulty) Flow:**
//!   1. Spawn process -> 2. Wait for window to map -> 3. Reposition window.
//!      Result: Window flickers at default location for 1-5 frames before jumping.
//!
//! - **Spawn-At (Zero-Flicker) Flow:**
//!   1. **D-Bus Arm:** Call `ArmSpawn(target_id, instructions_json)` on the GNOME Shell extension.
//!      The compositor registers an expected window target in memory *before* the
//!      application process is even spawned.
//!   2. **Process Spawn:** The child process is launched with `XDG_ACTIVATION_TOKEN` / `DESKTOP_STARTUP_ID`.
//!      It connects to the display socket and submits its surface.
//!   3. **Synchronous Interception:** Mutter triggers `window-created`. The extension
//!      matches the window's startup ID / `app_id` with the armed target.
//!   4. **Opacity Cloaking:** As soon as Mutter creates the `ClutterActor` during `map`,
//!      the extension sets `actor.opacity = 0` synchronously. Mutter renders nothing to the
//!      screen while the initial frame geometry settles.
//!   5. **Atomic Reveal:** Once `window.get_frame_rect()` matches the target coordinates,
//!      `actor.opacity = 255` is restored.
//!
//! ## Table of Contents
//! - **1. CLI Configuration & Arguments**
//!   - [`GnomeInstallArgs`]: Flags controlling extension enablement, X11 reload, and Wayland logout during install.
//!   - [`GnomeUninstallArgs`]: Flags controlling file cleanup, X11 reload, and Wayland logout during uninstall.
//! - **2. Session & Desktop Detection**
//!   - [`is_wayland_session`]: Detects whether current desktop is running on Wayland via `loginctl`.
//! - **3. Execution & Reload Helpers**
//!   - [`restart_gnome_shell_x11`]: Simulates `Alt+F2` -> `r` -> `Enter` via `xdotool` on X11.
//! - **4. Driver Construction & Internal State**
//!   - [`GnomeWaylandDriver`]: Core driver managing D-Bus communication with the GNOME extension.
//! - **5. Pre-Map Arming Driver Trait**
//!   - [`Driver`]: Pre-registers window targets and provides launch environment variables.
//! - **6. Runtime Compositor Backend Trait**
//!   - [`CompositorBackend`]: Implements `install`, `uninstall`, window transformations, state queries, and daemon mode.
//! - **7. Unit Tests**
//!   - Test suite verifying argument defaults, `--no-action` overrides, flag toggles, and session queries.
//!
//! ## Files
//! - `args.rs`: the install and uninstall flags (section 1).
//! - `detect.rs`: GNOME Shell version and Wayland session detection (section 2).
//! - `deploy.rs`: the embedded extension files, `install` / `uninstall` of the extension, and the
//!   log-out / reload helpers (sections 2 and 3).
//! - `driver.rs`: `GnomeWaylandDriver`, the claim subscription, and the `Driver` and
//!   `CompositorBackend` implementations (sections 4 to 6).
//! - `tests.rs`: the unit tests (section 7).
//! - `dbus.rs`, `mechanics.rs`, `logout.rs`: unchanged.

pub mod dbus;
pub mod logout;
pub mod mechanics;

mod args;
mod deploy;
mod detect;
mod driver;
#[cfg(test)]
mod tests;

// The public paths the rest of the crate uses (`platform::linux::gnome::...`) are kept here.
pub use args::{GnomeInstallArgs, GnomeUninstallArgs};
pub use deploy::GnomeSessionActions;
pub use detect::is_wayland_session;
pub use driver::GnomeWaylandDriver;
