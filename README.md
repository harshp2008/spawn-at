# spawn-at

<p align="center">
  <strong>Unified zero-flicker window placement engine for modern compositors and platforms.</strong>
</p>

<p align="center">
  <a href="https://github.com/harshp2008/spawn-at/releases"><img src="https://img.shields.io/github/v/release/harshp2008/spawn-at?include_prereleases&style=flat-square&color=blue" alt="Latest Release"></a>
  <a href="https://github.com/harshp2008/spawn-at/blob/main/Cargo.toml"><img src="https://img.shields.io/badge/rust-2021%20edition-orange?style=flat-square" alt="Rust Edition"></a>
  <a href="https://github.com/harshp2008/spawn-at/blob/main/LICENSE"><img src="https://img.shields.io/badge/license-MIT-green?style=flat-square" alt="License"></a>
</p>

---

## What is `spawn-at`?

Under modern display servers—especially Wayland compositors like GNOME Shell—client applications are sandboxed and denied the ability to position or size their own windows. Traditional scripting workarounds launch an app, wait for its window to map, and then reposition it: causing visible flashes, multi-frame jumps, and brittle automation.

`spawn-at` solves this with **frame-zero opacity cloaking**: it pre-registers placement instructions with the compositor *before* the application process is even spawned. While Mutter negotiates initial surface geometry, the window is held completely invisible. It is revealed atomically only once its target position and size have settled.

### Modular & Cross-Platform Architecture

`spawn-at` is built around a fully decoupled `CompositorBackend` driver architecture, isolating the pure-math geometry engine (`spawn-at-core`) from display-server IPC. This same design is intended to grow to support additional compositors and platforms as new drivers are implemented.

```text
                     ┌───────────────────────────┐
                     │   spawn-at CLI / Router   │
                     └─────────────┬─────────────┘
                                   │
                                   ▼
                     ┌───────────────────────────┐
                     │     CompositorBackend     │
                     │   (Driver Abstraction)    │
                     └─────────────┬─────────────┘
                                   │
        ┌──────────────────────────┼──────────────────────────┐
        ▼                          ▼                          ▼
┌───────────────────┐    ┌───────────────────┐    ┌───────────────────┐
│ GNOME Shell Driver│    │ X11 Native Driver │    │  Future Drivers   │
│  (Wayland & X11)  │    │   (x11rb / EWMH)  │    │  Hyprland, Sway,  │
└───────────────────┘    └───────────────────┘    │  KDE, macOS, Win  │
                                                  └───────────────────┘
```

> **Status:** GNOME Shell on Wayland (Ubuntu 24.04 LTS) is the primary tested target for `v0.2.0-beta`. The driver abstraction is designed to expand to additional Linux compositors (**Hyprland**, **Sway**, **KDE Plasma**) and platforms (**Windows**, **macOS**) as dedicated backends are implemented.

---

## Key Capabilities

- ⚡ **Zero-Flicker Cold Starts:** Pre-arms compositor targets via D-Bus. Renders windows invisibly until geometry is verified and settled, then reveals atomically at the correct position.
- 🎯 **Advanced Spatial Geometry:** Pure mathematical layout engine supporting absolute positions, cursor offsets, 10 screen anchors (`center`, `top-left`, `cursor`, …), 5 alignment pivots, directional margins (`--mt`, `--mb`, `--ml`, `--mr`), and workarea clamping.
- 🪟 **Dynamic Window Transformations:** Reposition and resize already-mapped windows on the fly using deterministic selectors (`--class`, `--title`, `--pid`, `--focused`).
- 🔄 **Window Lifecycle & Focus Control:** `focus` (`raise`), `defocus` (`blur`), `maximize`, `minimize`, and `restore` with fine-grained focus modifiers (`--focus`, `--no-focus`, `--defocus`).
- 🔍 **Compositor State Queries:** Inspect connected monitors, workarea boundaries, cursor coordinates, and active window trees — formatted tables or machine-readable `--json` output.
- 🚀 **Dynamic Session Detection:** Automatically identifies Wayland vs. X11 sessions via `loginctl` and routes execution to the correct driver at runtime.

---

## Installation

### Standard 1-Liner

```bash
curl -fsSL https://raw.githubusercontent.com/harshp2008/spawn-at/main/install.sh | bash
```

### Advanced Bootstrap Flags

```bash
# Pin a specific release
curl -fsSL https://raw.githubusercontent.com/harshp2008/spawn-at/main/install.sh | bash -s -- --version v0.2.0-beta

# Fetch latest pre-release (includes betas and alphas)
curl -fsSL https://raw.githubusercontent.com/harshp2008/spawn-at/main/install.sh | bash -s -- --prerelease

# Non-interactive / headless setup
curl -fsSL https://raw.githubusercontent.com/harshp2008/spawn-at/main/install.sh | bash -s -- --headless --scope user
```

> **GNOME Wayland:** After installation, log out and back in to activate the embedded GNOME Shell extension.  
> **GNOME X11:** The installer automatically reloads GNOME Shell in-place — apps stay open.

### Build from Source

```bash
git clone https://github.com/harshp2008/spawn-at.git
cd spawn-at
cargo build --release
./target/release/spawn-at install
```

---

## CLI Usage & Examples

### 1. Spawning Applications (`spawn`)

```bash
# Spawn at explicit coordinates (X=300, Y=200) sized to 800×500
spawn-at spawn --pos 300 200 --size 800 500 gedit

# Center at the mouse cursor, sized to 600×400
spawn-at spawn --anchor cursor --pivot center --size 600 400 gnome-calculator

# Anchor to top-right of primary workarea with a 24px margin
spawn-at spawn --anchor top-right --size 700 450 -m 24 alacritty

# Spawn without stealing keyboard focus
spawn-at spawn --pos 100 100 --size 500 300 --no-focus gnome-text-editor
```

### 2. Transforming Existing Windows (`transform`)

```bash
# Reposition and resize by application class
spawn-at transform --class org.gnome.Calculator --anchor center --size 400 500

# Re-anchor the currently focused window to the bottom-right corner
spawn-at transform --focused --anchor bottom-right -m 16
```

### 3. Window State & Focus Management

```bash
# Focus / raise a window
spawn-at focus --class code

# Yield focus to the desktop or redirect it to a specific window
spawn-at defocus --focused --to-desktop
spawn-at defocus --class gedit --to-window kitty

# Maximize / Minimize / Restore
spawn-at maximize --class org.gnome.Terminal
spawn-at minimize --title "Spotify"
spawn-at restore --class org.gnome.Terminal
```

### 4. Querying Compositor State (`query`)

```bash
# View active display layout
spawn-at query layout
spawn-at query layout --json

# Print current cursor position
spawn-at query pointer

# List open windows with geometry and PIDs
spawn-at query windows
spawn-at query windows --json
```

---

## Quick Reference

| Command / Flag | Description |
| :--- | :--- |
| `spawn` | Spawn application at target geometry |
| `transform` | Reposition or resize existing mapped window |
| `focus` / `raise` / `activate` | Transfer keyboard focus to window |
| `defocus` / `unfocus` / `blur` | Yield or redirect keyboard focus |
| `maximize` / `minimize` / `restore` | Change window state |
| `query layout\|pointer\|windows` | Fetch compositor state |
| `install` / `uninstall` | Manage the embedded GNOME Shell extension |
| `update` | Check for or install updates (`--check`, `--channel`, `--force`) |
| `-p, --pos <X> <Y>` | Absolute screen coordinates |
| `-s, --size <W> <H>` | Target window dimensions |
| `-a, --anchor <ANCHOR>` | `center`, `top-left`, `top-right`, `bottom-left`, `bottom-right`, `top`, `bottom`, `left`, `right`, `cursor` |
| `--pivot <PIVOT>` | `top-left`, `top-right`, `bottom-left`, `bottom-right`, `center` |
| `--monitor <MONITOR>` | Monitor index (`0`, `1`), `cursor`, or `primary` |
| `-m, --margin <PX>` | Universal margin in pixels (default: `16`) |
| `--mt` / `--mb` / `--ml` / `--mr` | Top, bottom, left, right directional margin insets |
| `--focus` / `--no-focus` / `--defocus` | Fine-grained focus control |
| `--no-wait` | Exit without waiting for compositor sync |
| `--json` | Machine-readable output for `query` commands |

---

## License

MIT License. See [LICENSE](LICENSE) for details.
