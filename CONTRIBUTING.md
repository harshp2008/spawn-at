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
│ spawn-at-core:   │             │   platform/      │             │ platform/linux/  │
│ geometry.rs      │             │ (OS & Installer) │             │ (WindowManager)  │
└──────────────────┘             └────────┬─────────┘             └────────┬─────────┘
                                          │                                │
                           ┌──────────────┴──────────────┐   ┌─────────────┴─────────────┐
                           ▼                             ▼   ▼                           ▼
                   installer.rs /               linux/xdg.rs gnome/                    x11/
                   escalate.rs                  (App ID)     (D-Bus IPC)            (X11 Fallback)
```

### Core Components & Directory Structure

- **`src/main.rs`**: Entry point. Parses subcommands (`spawn`, `install`, `uninstall`) using `clap`, renders interactive TUI menus via `dialoguer`, and coordinates target geometry calculation and platform backend execution. Contains no OS- or compositor-specific IPC logic.
- **`crates/spawn-at-core/src/geometry.rs`**: Pure mathematical engine. Computes absolute coordinates, applies mouse offsets, calculates monitor bounding boxes, and handles edge clamping and margin offsets. Independent of any display server or OS APIs.
- **`src/platform/`**: OS-level utilities, compositor abstraction, and system integration:
  - `mod.rs`: Defines the `CompositorBackend` trait and platform bootstrap logic (`init_backend()`).
  - `installer.rs`: Manages user-space (`~/.local/bin`) vs system-wide (`/usr/local/bin`) binary deployment, canonical path resolution, and `$PATH` environment verification.
  - `escalate.rs`: Reactive privilege escalation (`sudo` on Linux/macOS, UAC on Windows) triggered automatically when filesystem operations encounter `PermissionDenied`.
  - `linux/mod.rs`: Unified `LinuxBackend` adapter routing to GNOME Wayland or X11 drivers.
  - `linux/gnome/`: Drivers for GNOME Shell Wayland, communicating with Mutter via session D-Bus IPC (`org.gnome.Shell.Extensions.SpawnAt`).
  - `linux/x11/`: Fallback driver for legacy X11 sessions.
  - `linux/xdg.rs`: FreeDesktop desktop entry parsing, Flatpak/Snap application ID resolution, and `$XDG_DATA_DIRS` lookup.
- **`assets/`**: Embedded asset payloads:
  - `gnome/modern/`: GNOME Shell extension for GNOME 45 and later (`spawn-at@harsh.local`): `extension.js` plus its modules and `metadata.json`.
  - `gnome/legacy/`: the GNOME 42-44 extension, with its own `metadata.json`.

---

## 2. Decoupled Architecture Principles

### 1. Pure Geometry is Window-Manager Agnostic
All coordinate math, cursor offsets, monitor boundary calculations, and margin clamping are handled by `spawn_at_core::geometry`. Drivers receive the already-resolved final pixel position and size.

### 2. Reactive Privilege Escalation Model
Standard CLI and installation workflows run under unprivileged user rights. When an operation requires root privileges (such as writing to `/usr/local/bin`), the installer catches `PermissionDenied` and reactively invokes administrative elevation (`sudo` via `platform::escalate`), presenting a clear explanation to the user.

### 3. Drivers Implement the `CompositorBackend` Trait
All compositor-specific IPC mechanisms (D-Bus, UNIX domain sockets, X11 atom manipulation, Win32 APIs) are isolated inside driver implementations of the `CompositorBackend` trait.

---

## 3. GNOME Wayland "Opacity Cloaking" Lifecycle

Under Wayland, client applications are sandboxed and cannot position their own windows. `spawn-at` uses an embedded GNOME Shell extension ([`assets/gnome/modern/extension.js`](assets/gnome/modern/extension.js)) to place windows with few flashes (see the README, "How well it works"):

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
Create a new module under `src/platform/linux/` (a file `<driver_name>.rs`, or a directory like `gnome/` and `x11/` when it has several files). A driver has two layers, both defined in the repository:

- **`Driver`** (`crates/spawn-at-core/src/driver.rs`): the compositor-facing operations. `arm(batch)` is the required one; the rest have defaults. Read the trait for the current method list.
- **`CompositorBackend`** (`src/platform/mod.rs`): extends `Driver` with `name()`, install and uninstall hooks, and capability flags such as `supports_runtime_transform` and `supports_close`.

`src/platform/linux/x11/mod.rs` (`X11Driver`) is an example to copy; `src/platform/linux/gnome/` (`GnomeWaylandDriver` in `driver.rs`) is the D-Bus one. Application-ID resolution for a command lives in `src/platform/linux/xdg.rs`.

### Step 2: Register in `src/platform/linux/mod.rs`
1. Add `pub mod <driver_name>;` in `src/platform/linux/mod.rs`.
2. Add a branch to `LinuxBackend::bootstrap()`. Today it returns the X11 driver when `SPAWN_AT_BACKEND=x11`, the GNOME driver when `XDG_CURRENT_DESKTOP` contains `GNOME` and `GnomeWaylandDriver::new()` succeeds, and the X11 driver otherwise. Each result is wrapped as `LinuxBackend { driver: Box::new(...) }`.

### Step 3: Add Unit Tests
Add unit tests for IPC payload formatting, geometry translation, or socket communication.

---

## 5. Development Workflow & Testing Commands

### Build & Check
```bash
# Check code for compilation errors
cargo check

# Run unit tests (root package `spawn-at` only; the workspace default is just the product)
cargo test

# Run every suite CI runs (spawn-at-verify must be named, with its features)
cargo test -p spawn-at -p spawn-at-core -p xtask
cargo test -p spawn-at-verify --features tui,fixture
node --test tests/*.js

# Lint everything, including the dev harness
cargo fmt --check
cargo clippy --workspace --all-targets --features spawn-at-verify/tui,spawn-at-verify/fixture -- -D warnings

# A private headless Sway (never your desktop session; see docs/DESIGN_SWAY.md section 11)
scripts/sway-headless.sh start --dry-run
# The Sway live cases, run in that private Sway (nothing is started with --dry-run)
scripts/sway-live.sh --dry-run

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
