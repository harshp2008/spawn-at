# `spawn-at`

> **Cross-Platform CLI Window Placement Engine & Spatial Launcher**

`spawn-at` is a high-performance CLI utility that launches application windows at exact coordinates, mouse cursor positions, relative offsets, and constrained screen boundaries under modern display servers (Wayland, X11) where legacy tools like `xdotool` fail.

Under Wayland compositors (such as GNOME Shell), applications are sandboxed from knowing global screen coordinates and cannot position themselves. `spawn-at` solves this by combining pre-launch D-Bus IPC pre-arming with **Opacity Cloaking** via an embedded GNOME Shell extension, guaranteeing mathematically perfect, zero-flicker cold-starts on frame zero.

---

## 🌟 Key Features & Architecture Highlights

- ⚡ **Zero-Flicker Cold-Starts ("Opacity Cloaking"):** Leverages an embedded GNOME Shell extension (`spawn-at@harsh.local`) to suppress initial window visibility (`actor.opacity = 0`), apply target frame geometry, and atomically reveal the window (`actor.opacity = 255`) once repositioned and scaled.
- 📐 **Decoupled Geometry & OS Management:** Target coordinate calculation, monitor detection, edge clamping, and OS binary installation are decoupled from window manager drivers.
- 🔌 **Modular Driver System:** Clean separation between display servers/compositors and CLI orchestrations via the `WindowManager` trait. Includes full support for GNOME Shell Wayland/X11, with architecture stubs for Hyprland, Sway, macOS, and Windows.
- 🔐 **Reactive Privilege Escalation:** Clean security model. Standard user operations execute without elevated privileges; administrative elevation (`sudo`) is requested reactively only when encountering `PermissionDenied` (e.g., system-wide installation to `/usr/local/bin`).
- 🖥️ **Interactive TUI & Headless Modes:** Terminal UI driven by `dialoguer` for seamless scope selection (`~/.local/bin` vs `/usr/local/bin`), alongside full `--headless` flags for CI/CD, dotfiles, and script automation.
- 🎯 **Automatic FreeDesktop App ID Resolution:** Resolves binary invocations (e.g. `gnome-text-editor`) to canonical Wayland App IDs (e.g. `org.gnome.TextEditor`) across system binaries, Flatpaks, Snaps, and XDG desktop entries.
- 📦 **Single Self-Contained Binary:** Extension code and metadata are embedded directly into the Rust executable using `include_str!`—no runtime asset files required.

---

## 📦 Installation & Uninstallation Guide

### 1. Build from Source
Ensure you have Rust and Cargo installed:
```bash
git clone https://github.com/harsh/spawn-at.git
cd spawn-at
cargo build --release
```
The compiled binary will be located at `./target/release/spawn-at`.

### 2. Interactive TUI Installation
Run `spawn-at install` in an interactive terminal. You will be prompted to select the target installation scope:

- **User Scope (`~/.local/bin`) [Recommended]**: Installs binary in user home directory (no `sudo` required).
- **System Scope (`/usr/local/bin`)**: Installs binary globally for all users (triggers reactive `sudo` elevation).
- **Compositor Only**: Installs and enables the GNOME Shell extension without copying the binary.

```bash
spawn-at install
```

### 3. Headless / Automated Installation
For automated scripts, dotfile deployments, or CI environments:

```bash
# User-scope installation (~/.local/bin)
spawn-at install --headless --scope user

# System-scope installation (/usr/local/bin)
spawn-at install --headless --scope system

# Install GNOME extension only (skip copying binary to $PATH)
spawn-at install --headless --skip-bin
```

> **Note on `$PATH` Verification:** `spawn-at install` automatically checks if the target directory is present in your active `$PATH` environment variable and displays shell configuration instructions if missing.

### 4. Uninstallation
To disable the extension and remove the binary:

```bash
# Interactive uninstallation
spawn-at uninstall

# Headless uninstallation (User scope)
spawn-at uninstall --headless --scope user

# Headless uninstallation (System scope)
spawn-at uninstall --headless --scope system
```

---

## 🚀 CLI Usage & Examples

### Basic Command Syntax
```bash
spawn-at spawn [OPTIONS] -- <COMMAND> [ARGS...]
```

### Examples

#### 1. Spawn at Explicit Coordinates & Dimensions
Launch a text editor at exact screen coordinates `(300, 200)` with a width of `800px` and height of `500px`:
```bash
spawn-at spawn --pos 300 200 --size 800 500 -- gnome-text-editor --new-window
```

#### 2. Spawn at Mouse Cursor
Launch a terminal window `600x400` positioned at the current mouse cursor location:
```bash
spawn-at spawn --size 600 400 -- gnome-terminal
```

#### 3. Spawn relative to Mouse Cursor with Offset
Launch a window with a `(150, 150)` pixel offset relative to the current cursor position:
```bash
spawn-at spawn --offset 150 150 --size 600 400 -- gnome-calculator
```

#### 4. Edge Clamping & Safety Margins
Spawn an application while constraining it within monitor boundaries with a `50px` outer safety margin. Even if target coordinates extend off-screen (e.g. `5000, 5000`), `spawn-at` automatically clamps the window into view:
```bash
spawn-at spawn --pos 5000 5000 --size 600 400 --margin 50 -- gnome-text-editor
```

#### 5. Intercepting Next Mapped Window (Wildcard Match)
Use `--class "*"` to intercept and position the very next window mapped by the compositor within 1200ms (useful for complex or Electron-based applications like VS Code):
```bash
spawn-at spawn --class "*" --size 1000 700 -- code
```

---

## 📋 Options Reference

| Flag / Option | Description |
| :--- | :--- |
| `-p, --pos <X> <Y>` | Absolute target screen coordinates `[X, Y]` in pixels. |
| `-o, --offset <X> <Y>` | Relative offset `[X, Y]` from current mouse cursor position. |
| `-s, --size <W> <H>` | Target window dimensions `[WIDTH, HEIGHT]`. |
| `-c, --class <CLASS>` | Explicit Wayland App ID or WM_CLASS override (e.g. `org.gnome.TextEditor` or `*`). |
| `-m, --margin <MARGIN>` | Global margin safety offset applied to screen boundaries. |
| `-t, --bound-top <PX>` | Top screen boundary inset margin. |
| `-b, --bound-bottom <PX>` | Bottom screen boundary inset margin. |
| `-l, --bound-left <PX>` | Left screen boundary inset margin. |
| `-r, --bound-right <PX>` | Right screen boundary inset margin. |
| `--scope <user\|system>` | Target installation scope: `user` (`~/.local/bin`) or `system` (`/usr/local/bin`). Default: `user`. |
| `--skip-bin` | Skip copying or removing the binary to/from `$PATH`. |
| `--headless` | Non-interactive execution without TUI prompts. |

---

## 🗺️ Supported Environments & Roadmap

| Platform / Compositor | Status | Driver Implementation |
| :--- | :--- | :--- |
| **GNOME Shell (Wayland/X11)** | ✅ Implemented | Extension `spawn-at@harsh.local` via session D-Bus IPC |
| **Generic X11** | ✅ Implemented | X11 atom manipulation fallback driver |
| **Hyprland / Sway (Wayland)** | 🔄 Planned | Socket IPC driver implementation stub (`src/drivers/`) |
| **macOS** | 🔄 Planned | Platform & installer abstraction stubs (`src/platform/`) |
| **Windows** | 🔄 Planned | Platform & installer abstraction stubs (`src/platform/`) |

---

## 🤝 Contributing

We welcome contributions! For details on the architecture, adding new window manager drivers, and testing, please read [`CONTRIBUTING.md`](./CONTRIBUTING.md).

---

## 📄 License

MIT License.
