//! The session-bus interface of the spawn-at GNOME Shell extension, as a zbus proxy.
//!
//! Used only by `GnomeWaylandDriver` in `driver.rs` (and by the D-Bus stub tests in this
//! folder). The interface name, object path, method names, argument types and signals mirror the
//! XML in `assets/gnome/modern/dbus.js` (`DBUS_IFACE`); the legacy extension's XML is the same
//! except that it has no `ResizeOutcome` signal. The proxy talks to the bus name
//! `org.gnome.Shell` because the extension exports its object from inside the Shell process.
//!
//! This module has no logic and no error handling: every method returns the raw
//! [`zbus::Result`]. It must not decide what a failed call means; the driver does.
//!
//! `SetLogging` exists in the extension's XML but is not in this proxy; the verify crate
//! (`ext_logging.rs`) calls it on its own.

use zbus::proxy;

/// Proxy for `org.gnome.Shell.Extensions.SpawnAt` at `/org/gnome/Shell/Extensions/SpawnAt`.
///
/// Calls fail with a [`zbus::Error`] when the extension is not loaded (nothing owns the object
/// on the bus) or when the Shell rejects the call. A `bool` result is the extension's own
/// success flag, for example "a window matched the target"; it is not a transport status.
#[proxy(
    default_service = "org.gnome.Shell",
    default_path = "/org/gnome/Shell/Extensions/SpawnAt",
    interface = "org.gnome.Shell.Extensions.SpawnAt"
)]
pub trait SpawnAt {
    /// The extension's protocol number: 6 in `modern/extension.js`, 5 in `legacy/extension.js`.
    /// The driver treats a failed read as 0 (see `driver.rs`).
    #[zbus(property)]
    fn protocol_version(&self) -> zbus::Result<u32>;
    /// Registers `instructions_json` (the JSON of a list of mechanics instructions) for the
    /// next window that matches `target_id`. An empty id or `"*"` registers a wildcard that
    /// matches any window. A named registration lives 15 s and a wildcard 1.2 s unless claimed
    /// first (`ARM_EXPIRY_MS` and `WILDCARD_EXPIRY_MS` in both `extension.js` files). Returns
    /// `Ok` even when the extension rejects the JSON: it logs the
    /// error and returns without a D-Bus error (`ArmSpawn` in `extension.js`).
    fn arm_spawn(&self, target_id: &str, instructions_json: &str) -> zbus::Result<()>;
    /// Removes every registration queued under `target_id` (`"*"` removes the wildcard). The result
    /// is `false` when nothing was registered under that id, so a repeated call is harmless.
    fn disarm_spawn(&self, target_id: &str) -> zbus::Result<bool>;
    /// Runs `instructions_json` on an existing window that matches `target_id`. Returns `Ok`
    /// without doing anything when the JSON is invalid or no window matches: the extension
    /// only logs those cases.
    fn execute_batch(&self, target_id: &str, instructions_json: &str) -> zbus::Result<()>;
    /// Pointer position in screen pixels, `(x, y)`.
    fn get_cursor(&self) -> zbus::Result<(i32, i32)>;
    /// Same value as `get_cursor` (the extension implements one as a call to the other).
    fn get_pointer(&self) -> zbus::Result<(i32, i32)>;
    /// JSON array of `{x, y, w, h}`, one work area per monitor of the active workspace.
    fn get_workareas(&self) -> zbus::Result<String>;
    /// JSON `{schema_version, monitors}` with screen and work-area rectangles per monitor.
    /// The driver asks for it only when the protocol is 4 or higher.
    fn get_layout(&self) -> zbus::Result<String>;
    /// JSON array of the windows of the active workspace, most recently used first.
    fn get_windows(&self) -> zbus::Result<String>;
    /// Moves the window matching `app_id` so its frame starts at `(x, y)` in screen pixels.
    /// Returns `Ok` when no window matches (no error, no flag).
    fn move_window(&self, app_id: &str, x: i32, y: i32) -> zbus::Result<()>;
    /// Raises and activates the window matching `target`. `false` when none matches.
    fn focus_window(&self, target: &str) -> zbus::Result<bool>;
    /// Drops keyboard focus (`mode` `"desktop"`) or hands it to another window (`mode`
    /// `"window"` with `destination`). `false` when `target` matches no window.
    fn defocus_window(&self, target: &str, mode: &str, destination: &str) -> zbus::Result<bool>;
    /// Applies `state`: the extension accepts `maximize`, `unmaximize`, `minimize`,
    /// `unminimize` and `restore`; any other string gives `false`, as does an unmatched target.
    fn set_window_state(&self, target: &str, state: &str) -> zbus::Result<bool>;
    /// Asks the window matching `target` to close (a polite delete request, not a kill).
    /// `false` when none matches.
    fn close_window(&self, target: &str) -> zbus::Result<bool>;
    /// Emitted by the extension on the Shell's `monitors-changed` and `workareas-changed`
    /// events. No code under `src/` subscribes to it (`grep -rn receive_workarea_changed src`).
    #[zbus(signal)]
    fn workarea_changed(&self) -> zbus::Result<()>;
    /// Emitted when an armed target met its window. `success` false carries the reason in
    /// `error`, with `x, y, w, h` all 0. `window_id` is the Mutter window id; `x, y, w, h` are
    /// the final frame in screen pixels; `size_raised` is true when the size the extension
    /// settled on is larger than the requested width or height.
    #[zbus(signal)]
    fn spawn_claimed(
        &self,
        target_id: &str,
        success: bool,
        window_id: u64,
        x: i32,
        y: i32,
        w: u32,
        h: u32,
        size_raised: bool,
        error: &str,
    ) -> zbus::Result<()>;
    /// Protocol 6: how the size request of a native Wayland window ended. Emitted before
    /// `SpawnClaimed` for the same target; an older extension never sends it.
    #[zbus(signal)]
    fn resize_outcome(
        &self,
        target_id: &str,
        outcome: &str,
        final_w: u32,
        final_h: u32,
        waited_ms: u32,
        reason: &str,
    ) -> zbus::Result<()>;
}
