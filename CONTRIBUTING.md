# Contributor Guide & Architecture Reference (`CONTRIBUTING.md`)

Welcome to the `spawn-at` developer and contributor guide! This document details the system architecture, directory organization, OS-level platform abstractions, compositor driver interfaces, and development workflows.

---

## 1. System Philosophy & Reorganized Architecture

`spawn-at` uses a decoupled, modular architecture that cleanly separates CLI orchestration, coordinate geometry calculations, platform/OS installer mechanics, and window manager IPC drivers.

```text
                               ┌────────────────────────┐
                               │        main.rs         │
                               │  (CLI Router / TUI)    │
                               └───────────┬────────────┘
                                           │
         ┌─────────────────────────────────┼─────────────────────────────────┐
         ▼                                 ▼                                 ▼
┌──────────────────┐             ┌──────────────────┐             ┌──────────────────┐
│   geometry.rs    │             │   platform/      │             │   drivers/       │
│  (Pure Geometry) │             │ (OS & Installer) │             │ (WindowManager)  │
└──────────────────┘             └────────┬─────────┘             └────────┬─────────┘
                                          │                                │
                           ┌──────────────┴──────────────┐   ┌─────────────┴─────────────┐
                           ▼                             ▼   ▼                           ▼
                   installer.rs /               linux/xdg.rs gnome.rs                  x11.rs
                   escalate.rs                  (App ID)     (D-Bus IPC)            (X11 Fallback)
```

### Core Components & Directory Structure

- **`src/main.rs`**: Entry point. Parses subcommands (`spawn`, `install`, `uninstall`) using `clap`, renders interactive TUI menus via `dialoguer`, and coordinates target geometry calculation and driver execution. Contains no OS- or compositor-specific IPC logic.
- **`src/geometry.rs`**: Pure mathematical engine. Computes absolute coordinates, applies mouse offsets, calculates monitor bounding boxes, and handles edge clamping and margin offsets. Independent of any display server or OS APIs.
- **`src/platform/`**: OS-level utilities and system integration abstractions:
  - `installer.rs`: Manages user-space (`~/.local/bin`) vs system-wide (`/usr/local/bin`) binary deployment, canonical path resolution, and `$PATH` environment verification.
  - `escalate.rs`: Reactive privilege escalation (`sudo` on Linux/macOS, UAC on Windows) triggered automatically when filesystem operations encounter `PermissionDenied`.
  - `linux/xdg.rs`: FreeDesktop desktop entry parsing, Flatpak/Snap application ID resolution, and `$XDG_DATA_DIRS` lookup.
- **`src/drivers/`**: Window manager and compositor driver abstractions:
  - `mod.rs`: Defines the `WindowManager` trait and driver detection logic (`get_active_driver()`).
  - `gnome.rs`: Drivers for GNOME Shell Wayland/X11, communicating with Mutter via session D-Bus IPC (`org.gnome.Shell.Extensions.SpawnAt`).
  - `x11.rs`: Fallback driver for legacy X11 sessions.
- **`assets/`**: Embedded asset payloads:
  - `gnome/extension.esm.js`: GNOME Shell extension code (`spawn-at@harsh.local`).
  - `gnome/metadata.json`: GNOME Shell extension manifest.

---

## 2. Decoupled Architecture Principles

### 1. Pure Geometry is Window-Manager Agnostic
All coordinate math, cursor offsets, monitor boundary calculations, and margin clamping are handled by `geometry.rs`. Drivers receive a resolved `TargetGeometry` struct containing final target pixel coordinates `(x, y)` and dimensions `(w, h)`.

### 2. Reactive Privilege Escalation Model
Standard CLI and installation workflows run under unprivileged user rights. When an operation requires root privileges (such as writing to `/usr/local/bin`), the installer catches `PermissionDenied` and reactively invokes administrative elevation (`sudo` via `platform::escalate`), presenting a clear explanation to the user.

### 3. Drivers Implement the `WindowManager` Trait
All compositor-specific IPC mechanisms (D-Bus, UNIX domain sockets, X11 atom manipulation, Win32 APIs) are isolated inside driver implementations of the `WindowManager` trait.

---

## 3. GNOME Wayland "Opacity Cloaking" Lifecycle

Under Wayland, client applications are sandboxed and cannot position their own windows. `spawn-at` uses an embedded GNOME Shell extension ([`assets/gnome/extension.esm.js`](file:///home/harsh/Documents/GITHUB%20PROJECTS/spawn-at/assets/gnome/extension.esm.js)) to achieve zero-flicker cold-starts:

```text
[spawn-at CLI] ───1. D-Bus Arm(app_id, x, y, w, h)───► [GNOME Shell Extension]
      │                                                         │
      │ 2. Process Spawn                                        │ 3. Intercept window-created
      ▼                                                         ▼
[Target Application] ─────────────────────────────────► [Attach Target Geometry]
                                                                │
                                                                │ 4. Map signal fires:
                                                                │    Set actor.opacity = 0
                                                                ▼
                                                       [Cloaked Geometry Loop]
                                                        (Move/Resize & Poll Frame)
                                                                │
                                                                │ 5. Target geometry met:
                                                                ▼
                                                       [Set actor.opacity = 255]
```

1. **Pre-Arming:** `spawn-at` calls `Arm(app_id, x, y, w, h)` on `org.gnome.Shell.Extensions.SpawnAt` via session D-Bus.
2. **Interception:** When Mutter creates a `MetaWindow`, the extension catches `window-created` and attaches the pre-armed target geometry.
3. **Opacity Suppression:** When the window actor is mapped (`map` signal), the extension synchronously sets `actor.opacity = 0`. The surface is negotiated while completely invisible.
4. **Placement & Scale:** The extension calls `move_resize_frame()` to position and size the window.
5. **Atomic Reveal:** Once `window.get_frame_rect()` matches the target geometry, `actor.opacity = 255` is restored.

---

## 4. Guide: Adding a New Window Manager Driver

To add support for a new window manager or compositor (e.g., Hyprland, Sway, or KDE Plasma):

### Step 1: Create the Driver Module
Create a new file under `src/drivers/<driver_name>.rs` and implement the `WindowManager` trait:

```rust
use crate::drivers::{DriverError, TargetGeometry, WindowManager};
use crate::geometry::Rect;

pub struct HyprlandDriver;

impl WindowManager for HyprlandDriver {
    fn name(&self) -> &'static str {
        "Hyprland"
    }

    fn resolve_id(&self, command: &[String], explicit_class: Option<&str>) -> String {
        let bin = command.first().map(|s| s.as_str()).unwrap_or("");
        crate::platform::linux::xdg::resolve_linux_app_id(bin, explicit_class)
    }

    fn spawn_at(
        &self,
        app_id: &str,
        command: &[String],
        geom: &TargetGeometry,
    ) -> Result<(), DriverError> {
        // Implement Hyprland IPC positioning logic via UNIX socket
        Ok(())
    }

    fn get_cursor_position(&self) -> Option<(i32, i32)> {
        // Query pointer position via Hyprland IPC
        None
    }

    fn get_monitors(&self) -> Vec<Rect> {
        // Query monitor bounds via Hyprland IPC
        Vec::new()
    }
}
```

### Step 2: Register in `src/drivers/mod.rs`
1. Add `pub mod <driver_name>;` in `src/drivers/mod.rs`.
2. Update `get_active_driver()` to detect the environment:

```rust
pub fn get_active_driver() -> Box<dyn WindowManager> {
    if std::env::var("HYPRLAND_INSTANCE_SIGNATURE").is_ok() {
        Box::new(hyprland::HyprlandDriver)
    } else if session_type == "wayland" && desktop.contains("GNOME") {
        Box::new(gnome::GnomeWaylandDriver)
    } else {
        Box::new(x11::X11Driver)
    }
}
```

### Step 3: Add Unit Tests
Add unit tests for IPC payload formatting, geometry translation, or socket communication.

---

## 5. Development Workflow & Testing Commands

### Build & Check
```bash
# Check code for compilation errors
cargo check

# Run unit tests
cargo test

# Build debug binary
cargo build

# Build release binary
cargo build --release
```

### Direct Target Execution
```bash
# Run spawn-at directly from target directory
./target/debug/spawn-at spawn --pos 300 200 --size 800 500 -- gnome-text-editor --new-window
```

### Extension & D-Bus Debugging
```bash
# Inspect GNOME Shell logs for spawn-at extension messages
journalctl -f -o cat /usr/bin/gnome-shell | grep spawn-at

# Introspect D-Bus extension interface
gdbus introspect --session --dest org.gnome.Shell --object-path /org/gnome/Shell/Extensions/SpawnAt

# Test manual D-Bus arming call
gdbus call --session \
  --dest org.gnome.Shell \
  --object-path /org/gnome/Shell/Extensions/SpawnAt \
  --method org.gnome.Shell.Extensions.SpawnAt.Arm "*" 300 200 800 500
```
