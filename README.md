# spawn-at

A cross-platform CLI window placement engine that launches application windows at exact coordinates, mouse cursor positions, relative offsets, and constrained boundaries under modern display servers (Wayland, X11).

Under Wayland compositors (such as GNOME Shell), client applications are sandboxed and cannot position themselves. `spawn-at` coordinates with compositor extensions and IPC channels to ensure zero-flicker cold-starts on frame zero.

---

## Quick Install

### 1-Line Installer (Recommended)

Run the interactive installer (prompts for User vs. System scope):

```bash
curl -fsSL https://raw.githubusercontent.com/harshp2008/spawn-at/main/install.sh | bash
```

For non-interactive or dotfile automated setup:

```bash
# Install to ~/.local/bin (no root needed)
curl -fsSL https://raw.githubusercontent.com/harshp2008/spawn-at/main/install.sh | bash -s -- --headless --scope user

# Install globally to /usr/local/bin (requests sudo reactively)
curl -fsSL https://raw.githubusercontent.com/harshp2008/spawn-at/main/install.sh | bash -s -- --headless --scope system
```

---

### Build from Source

```bash
git clone https://github.com/harshp2008/spawn-at.git
cd spawn-at
cargo build --release

# Run interactive installer from binary
./target/release/spawn-at install
```

---

## Uninstallation

Remove the binary and unregister compositor extensions:

```bash
# Interactive uninstaller
spawn-at uninstall

# Headless / scripted
spawn-at uninstall --headless --scope user
```

---

## Usage & Examples

### 1. Launch at Explicit Coordinates & Dimensions
Launch a text editor at screen coordinates `(300, 200)` sized to `800x500`:

```bash
spawn-at spawn --pos 300 200 --size 800 500 -- gnome-text-editor --new-window
```

### 2. Launch at Mouse Cursor
Launch a terminal centered at the current mouse cursor location:

```bash
spawn-at spawn --size 600 400 -- gnome-terminal
```

### 3. Launch with Offset from Cursor
Launch a calculator offset by `(150, 150)` pixels from the cursor:

```bash
spawn-at spawn --offset 150 150 --size 600 400 -- gnome-calculator
```

### 4. Safety Margins & Screen Clamping
Prevent windows from bleeding off-screen by enforcing a boundary margin:

```bash
spawn-at spawn --pos 5000 5000 --size 600 400 --margin 50 -- gnome-text-editor
```

### 5. Intercept Next Mapped Window (Wildcard Match)
Intercept and reposition the next window mapped by the compositor within 1200ms:

```bash
spawn-at spawn --class "*" --size 1000 700 -- code
```

---

## Options Reference

| Flag / Option | Description |
| :--- | :--- |
| `-p, --pos <X> <Y>` | Absolute screen coordinates `[X, Y]` in pixels. |
| `-o, --offset <X> <Y>` | Relative offset `[X, Y]` from current mouse cursor position. |
| `-s, --size <W> <H>` | Target window dimensions `[WIDTH, HEIGHT]`. |
| `-c, --class <CLASS>` | Explicit Wayland App ID or WM_CLASS override (e.g. `org.gnome.TextEditor` or `*`). |
| `-m, --margin <PX>` | Global boundary margin offset applied to screen edges. |
| `-t, --bound-top <PX>` | Top boundary inset margin. |
| `-b, --bound-bottom <PX>` | Bottom boundary inset margin. |
| `-l, --bound-left <PX>` | Left boundary inset margin. |
| `-r, --bound-right <PX>` | Right boundary inset margin. |
| `--scope <user\|system>` | Target installation scope: `user` (`~/.local/bin`) or `system` (`/usr/local/bin`). |
| `--skip-bin` | Skip copying or removing the binary to/from `$PATH`. |
| `--headless` | Non-interactive execution without TUI prompts. |

---

## Platform Support

| Platform / Compositor | Status | Driver Implementation |
| :--- | :--- | :--- |
| **GNOME Shell (Wayland/X11)** | Supported | Extension via session D-Bus IPC |
| **Generic X11** | Supported | X11 atom manipulation fallback |
| **Hyprland / Sway** | In Development | Socket IPC driver |
| **macOS & Windows** | Planned | Platform stubs prepared |

---

## Contributing & Development

For architecture details, extension mechanics, and how to write new drivers, refer to [CONTRIBUTING.md](./CONTRIBUTING.md).

---

## License

MIT
