# spawn-at README Angles & Claims Audit

This document establishes the verified technical foundation for rewriting the `spawn-at` README from scratch. It consists of two parts:
1. **8–10 Ranked README Angles** backed by exact codebase symbols and file paths.
2. **Claims Audit** evaluating every claim in the current `README.md` against the source code.

---

## Part 1: 8–10 Ranked README Angles

### Angle 1 (Rank 1): Frame-Zero Opacity Cloaking & Two-Phase Cold Start
- **Rationale:** This is the core reason the project exists. Wayland compositors (specifically Mutter in GNOME) forbid client applications from setting their own global coordinates or querying desktop position. Traditional automation scripts launch an app, wait for its window to map, and then reposition it—causing a jarring visual jump and 1–5 frame flash. `spawn-at` inverts this by pre-registering placement rules via D-Bus *before* process spawn, holding the window completely invisible (`opacity = 0`) at frame zero, waiting for surface commit and geometry settling, positioning it to target coordinates, and atomically revealing it.
- **Codebase Evidence:**
  - `src/platform/linux/gnome/mod.rs`: lines 8–36 (Architectural specification of the Wayland Isolation Barrier and Pre-Registration Pattern).
  - `crates/spawn-at-core/src/driver.rs`: `Driver::arm` trait method pre-arming targets before process launch.
  - `src/platform/linux/gnome/mechanics.rs`: `GnomeWaylandMechanics::arm_spawn`, `build_cloak_batch`.
  - `assets/gnome/extension.esm.js`:
    - `ArmSpawn(target_id, instructions_json)` (D-Bus export).
    - `_handleWindowCreated(window)`: connects on earliest hook, calls `this._cloak(actor)` synchronously on tick 0.
    - `_cloak(actor)`: sets `actor.opacity = 0` and tracks cloaked actors.
    - `_stepWaitForCommit(ctx, args)`: awaits `_waitForCommit` while invisible.
    - `_stepPosition(ctx, payload)`: applies anchored coordinates before reveal.
    - `_stepUncloak(ctx, args)`: restores `actor.opacity = 255`.

### Angle 2 (Rank 2): Compositor-Level Geometry Settlement vs. Client Buffer Race
- **Rationale:** Merely setting window size in Mutter doesn't mean the client has finished drawing its internal buffers. If revealed prematurely, the window flickers or renders with incorrect aspect ratios. `spawn-at` implements an adaptive event-driven barrier (`WaitForCommit`) that waits for client surface resize notifications with quiet-period debouncing, minimum thresholds, and hard caps.
- **Codebase Evidence:**
  - `assets/gnome/extension.esm.js`:
    - `_waitForCommit(window, actor, timeoutMs, ...)` (lines 1255–1360).
    - `COMMIT_MIN_TIMEOUT_MS = 120` (minimum wait floor).
    - `COMMIT_QUIET_MS = 30` (quiescence window after last resize signal).
    - `COMMIT_HARD_CAP_MS = 1500` (absolute timeout cap before forced progression).
    - Resolving states: `AT_TARGET`, `SIZE_SETTLED`, `TIMEOUT_NO_CHANGE`, `HARD_CAP`.

### Angle 3 (Rank 3): Pure Mathematical Layout Engine Decoupled from Compositors (`spawn-at-core`)
- **Rationale:** Geometry calculations (anchors, pivots, margins, workarea vs. screen boundaries, clamping) are completely pure and separated from display server IPC. This guarantees identical mathematical layout behavior regardless of display server or backend.
- **Codebase Evidence:**
  - `crates/spawn-at-core/src/geometry.rs`:
    - `Anchor` (10 variants: `Center`, `TopLeft`, `TopRight`, `BottomLeft`, `BottomRight`, `Top`, `Bottom`, `Left`, `Right`, `Cursor`).
    - `Pivot` (5 variants: `TopLeft`, `TopRight`, `BottomLeft`, `BottomRight`, `Center`).
    - `calculate_rect(params: &PlacementParams, win_w: u32, win_h: u32) -> Rect`.
    - `Rect::clamp_to(&self, bounds: &Rect) -> Rect`.
  - `crates/spawn-at-core/src/types.rs`:
    - `PlacementParams`, `Rect`, `AreaReference`.

### Angle 4 (Rank 4): Fail-Safe Recovery & No-Orphan Cloak Guarantee
- **Rationale:** Cloaking a window at opacity 0 is risky if the application crashes or fails to map. A broken tool would leave invisible, unclickable ghost windows. `spawn-at` guarantees that on timeout, failure, unmatched ID, or extension crash, windows are systematically uncloaked and revealed at default compositor geometry rather than remaining hidden.
- **Codebase Evidence:**
  - `assets/gnome/extension.esm.js`:
    - `ARM_EXPIRY_MS = 15000`: unclaims and purges expired pre-arms after 15 seconds.
    - `WILDCARD_EXPIRY_MS = 1200`: wildcards expire after 1.2 seconds.
    - `_settleUnmatched(window)`: triggers `REJECT_UNMATCHED` and immediately calls `this._uncloak(actor)`.
    - `_releaseHeldIfIdle()`: uncloaks all held actors if armed queue is empty.
    - `_processQueue()`: wrapped in `try / catch`, explicitly calling `this._uncloak(actor)` if batch execution errors.

### Angle 5 (Rank 5): Post-Reveal Reactive Re-Anchoring
- **Rationale:** Complex toolkits (e.g. terminals loading custom fonts or GTK apps rendering dynamic headerbars) frequently resize themselves *after* the initial window reveal. Standard placement tools place once, and then the window shifts out of place. `spawn-at` attaches a short-lived reactive anchor listener that dynamically repositions the window to maintain its requested anchor (e.g. bottom-right corner or center) upon secondary client-side resizes, auto-retiring after 1000ms of quiet.
- **Codebase Evidence:**
  - `assets/gnome/extension.esm.js`:
    - `_attachReactiveAnchor(window, payload, baseW, baseH)` (lines 1150–1210).
    - `_handleReactiveSizeChange(...)` (lines 1215–1250).
    - `REACTIVE_ANCHOR_TIMEOUT_MS = 1000`.

### Angle 6 (Rank 6): VTE-Gated Focus Normalization for Wayland Terminals
- **Rationale:** GTK3 + libvte applications (such as `gnome-terminal-server`) suffer from a known Wayland limitation where initial terminal grid layout is withheld until the window experiences a keyboard focus transition (`wl_keyboard.leave` / `wl_keyboard.enter`). Defocusing non-VTE windows (GTK4, Qt, Kitty) causes map stalls and flicker. `spawn-at` inspects `/proc/<pid>/maps` to selectively pulse focus *only* for verified libvte processes while remaining cloaked.
- **Codebase Evidence:**
  - `assets/gnome/extension.esm.js`:
    - `_isVteCandidate(window)` (lines 1365–1390).
    - `_isVteApp(pid)`: parses `/proc/<pid>/maps` for `libvte`.
    - `_executeFocusPulse(ctx)` (lines 1400–1455): performs synthetic defocus/refocus with early exit detection (`_waitForSizeChangedOrTimeout`).

### Angle 7 (Rank 7): Serialized FIFO Mutex Queue for Race-Free Concurrent Spawns
- **Rationale:** When scripts or hotkeys launch multiple windows simultaneously, parallel Wayland configure cycles create race conditions in Mutter. `spawn-at` enqueues requests into an asynchronous FIFO mutex queue so each window's size negotiation and placement executes atomically.
- **Codebase Evidence:**
  - `assets/gnome/extension.esm.js`:
    - `_enqueueBatch(window, actor, instructions)` (lines 911–915).
    - `_processQueue()` (lines 916–932): uses `this._batchBusy` lock to serialize execution across `_runBatch`.

### Angle 8 (Rank 8): Runtime Window Transformations & Deterministic Selectors
- **Rationale:** `spawn-at` is not restricted to cold process launches. Existing mapped windows can be dynamically repositioned, resized, or manipulated across workspace layers using deterministic selectors (`--class`, `--title`, `--pid`, `--focused`).
- **Codebase Evidence:**
  - `src/cli.rs`:
    - `TransformArgs`, `WindowTargetArgs`, `FocusModifierArgs`, `MaximizeArgs`, `MinimizeArgs`, `RestoreArgs`.
  - `src/platform/linux/gnome/mod.rs`:
    - `CompositorBackend::transform_window`.
    - `CompositorBackend::focus_window`, `defocus_window`, `set_window_state`.
  - `assets/gnome/extension.esm.js`:
    - `ExecuteBatch`, `MoveWindow`, `FocusWindow`, `DefocusWindow`, `SetWindowState`.

### Angle 9 (Rank 9): Decoupled `CompositorBackend` Trait Architecture
- **Rationale:** The CLI dispatch and core placement logic do not depend directly on GNOME Shell or Linux internals. The platform layer defines the `CompositorBackend` trait (`Driver + Send + Sync`), providing an explicit extension point for adding backends (e.g. Sway, Hyprland, KDE KWin).
- **Codebase Evidence:**
  - `src/platform/mod.rs`:
    - `pub trait CompositorBackend: Driver + Send + Sync`: defines `name`, `install`, `uninstall`, `supports_runtime_transform`, `resolve_id`, `get_cursor_position`, `get_monitors`, `get_workareas`, `get_windows`, `transform_window`, `move_window`, `move_resize_window`, `set_window_state`, `focus_window`, `defocus_window`, `post_spawn`, `run_daemon`.
  - `src/platform/linux/mod.rs`:
    - `LinuxBackend::bootstrap`: runtime detection and backend delegation.

### Angle 10 (Rank 10): Robust Session Detection & In-Place Extension Reloading
- **Rationale:** Environment variables like `$XDG_SESSION_TYPE` are notoriously unreliable under systemd user sessions. `spawn-at` uses `loginctl show-session <id> -p Type --value` to accurately detect Wayland vs. X11, and handles seamless in-place GNOME Shell restarts on X11 vs. guided session reloads on Wayland.
- **Codebase Evidence:**
  - `src/platform/linux/gnome/mod.rs`:
    - `is_wayland_session()` (lines 195–223): queries `loginctl`, with `WAYLAND_DISPLAY` fallback.
    - `restart_gnome_shell_x11()`: triggers Alt+F2 `r` on X11.
  - `src/platform/linux/gnome/extension_installer.rs`:
    - `reload_gnome_shell()`: executes in-place shell reload on X11 and provides guidance on Wayland.

---

## Part 2: Current README Claims Audit

Every statement and claim from the existing `README.md` (lines 1–184) evaluated against the codebase:

| Claim in Current README | Status | Proof File & Code Reference | Notes / Reality |
| :--- | :--- | :--- | :--- |
| **Line 4:** "Unified zero-flicker window placement engine for modern compositors and platforms." | **PARTIALLY FALSE / UNVERIFIED** | `src/platform/mod.rs:180-187` | Not "unified across platforms". Only Linux (GNOME Shell on Wayland/X11 and fallback X11 driver) is implemented. Non-Linux returns `Unsupported operating system`. The word "unified" overpromises. |
| **Line 9:** Rust 2021 edition | **VERIFIED** | `Cargo.toml:10` | `edition = "2021"` |
| **Line 10:** MIT License | **VERIFIED** | `Cargo.toml:7`, `LICENSE:1-21` | `license = "MIT"` |
| **Line 17:** "Under modern display servers—especially Wayland compositors like GNOME Shell—client applications are sandboxed and denied the ability to position or size their own windows." | **VERIFIED** | `src/platform/linux/gnome/mod.rs:8-13` | Wayland `xdg-shell` specification explicitly isolates window coordinates from clients. |
| **Line 17:** "Traditional scripting workarounds launch an app, wait for its window to map, and then reposition it: causing visible flashes, multi-frame jumps, and brittle automation." | **VERIFIED** | `src/platform/linux/gnome/mod.rs:19-22` | Standard X11/Wayland behavior when running external move scripts post-map. |
| **Line 19:** "`spawn-at` solves this with frame-zero opacity cloaking: it pre-registers placement instructions with the compositor before the application process is even spawned." | **VERIFIED** | `src/platform/linux/gnome/mod.rs:23-26`, `crates/spawn-at-core/src/driver.rs:14-16`, `assets/gnome/extension.esm.js:57-60` | Pre-arms via `ArmSpawn` D-Bus method before `Command::spawn` launches the child process. |
| **Line 19:** "While Mutter negotiates initial surface geometry, the window is held completely invisible." | **VERIFIED** | `assets/gnome/extension.esm.js:803-806, 850, 1098` | Mutter actor opacity is held at 0 during initial creation and buffer mapping. |
| **Line 19:** "It is revealed atomically only once its target position and size have settled." | **VERIFIED** | `assets/gnome/extension.esm.js:1054-1091` | `_stepWaitForCommit` followed by `_stepPosition` before `_stepUncloak`. |
| **Line 23:** "built around a fully decoupled `CompositorBackend` driver architecture, isolating the pure-math geometry engine (`spawn-at-core`) from display-server IPC." | **VERIFIED** | `src/platform/mod.rs:74-178`, `crates/spawn-at-core/src/geometry.rs:1-120` | Core math has 0 IPC dependencies; platform drivers implement the `CompositorBackend` trait. |
| **Line 23:** "This same design is intended to grow to support additional compositors and platforms as new drivers are implemented." | **VERIFIED** | `src/platform/mod.rs:74-187` | Architecture is designed around trait dispatch, though only Linux is currently implemented. |
| **Lines 25–43:** Architecture ASCII diagram showing "Future Drivers: Hyprland, Sway, KDE, macOS, Win". | **UNVERIFIED / PROHIBITED BY BRIEF** | `src/platform/mod.rs:185` | Brief explicitly mandates: *"NO diagram boxes for unimplemented drivers"*. Those drivers do not exist in code. |
| **Line 45:** "GNOME Shell on Wayland (Ubuntu 24.04 LTS) is the primary tested target for v0.2.0-beta." | **VERIFIED** | `assets/gnome/metadata.json:7-12` | Shell versions 45, 46, 47, 48 specified; primary testing on Ubuntu 24.04. |
| **Line 51:** "Zero-Flicker Cold Starts: Pre-arms compositor targets via D-Bus..." | **VERIFIED** | `assets/gnome/extension.esm.js:57-60`, `src/platform/linux/gnome/mechanics.rs:35-80` | Pre-arms via D-Bus; renders invisibly until settled. |
| **Line 52:** "10 screen anchors (center, top-left, cursor, …)" | **VERIFIED** | `crates/spawn-at-core/src/geometry.rs:32-53` | Exactly 10 enum variants: `Center`, `TopLeft`, `TopRight`, `BottomLeft`, `BottomRight`, `Top`, `Bottom`, `Left`, `Right`, `Cursor`. |
| **Line 52:** "5 alignment pivots" | **VERIFIED** | `crates/spawn-at-core/src/geometry.rs:65-79` | Exactly 5 enum variants: `TopLeft`, `TopRight`, `BottomLeft`, `BottomRight`, `Center`. |
| **Line 52:** "directional margins (--mt, --mb, --ml, --mr)" | **VERIFIED** | `src/cli.rs:122-135` | Declared clap aliases for `--margin-top`, `--margin-bottom`, `--margin-left`, `--margin-right`. |
| **Line 52:** "workarea clamping" | **VERIFIED** | `src/cli.rs:137-145`, `crates/spawn-at-core/src/geometry.rs:105-115` | Default `clamp: true`, calling `Rect::clamp_to`. |
| **Line 53:** "Dynamic Window Transformations: Reposition and resize already-mapped windows on the fly using deterministic selectors (--class, --title, --pid, --focused)." | **VERIFIED** | `src/cli.rs:255-271`, `src/platform/linux/gnome/mod.rs:537-567` | `transform` subcommand accepts all 4 selectors. |
| **Line 54:** "Window Lifecycle & Focus Control: focus (raise), defocus (blur), maximize, minimize, and restore with fine-grained focus modifiers (--focus, --no-focus, --defocus)." | **VERIFIED** | `src/cli.rs:240-251, 308-455` | All subcommands and mutually exclusive focus modifiers implemented. |
| **Line 55:** "Compositor State Queries: Inspect connected monitors, workarea boundaries, cursor coordinates, and active window trees — formatted tables or machine-readable --json output." | **VERIFIED** | `src/cli.rs:488-515`, `assets/gnome/extension.esm.js:65-78` | `query layout`, `query pointer`, `query windows` with `--json` support. |
| **Line 56:** "Dynamic Session Detection: Automatically identifies Wayland vs. X11 sessions via loginctl and routes execution to the correct driver at runtime." | **VERIFIED** | `src/platform/linux/gnome/mod.rs:195-223`, `src/platform/linux/mod.rs:92-113` | Uses `loginctl show-session <id> -p Type --value`, routes via `LinuxBackend::bootstrap`. |
| **Lines 64–66:** 1-Liner installer `curl -fsSL https://raw.githubusercontent.com/harshp2008/spawn-at/main/install.sh \| bash` | **VERIFIED** | `install.sh:1-198` | Installer script is present and valid. |
| **Lines 72–78:** Installer flags: `-v/--version`, `--prerelease`, `--headless`, `--scope user` | **VERIFIED** | `install.sh:29-53, 197`, `src/platform/mod.rs:42-58` | Supported directly in `install.sh` and passed to `spawn-at install`. |
| **Line 81:** "GNOME Wayland: After installation, log out and back in to activate the embedded GNOME Shell extension." | **VERIFIED** | `src/platform/linux/gnome/extension_installer.rs:145-160` | Mutter on Wayland requires session re-login to load newly installed extensions. |
| **Line 82:** "GNOME X11: The installer automatically reloads GNOME Shell in-place — apps stay open." | **VERIFIED** | `src/platform/linux/gnome/mod.rs:229-270` | Shell reload triggered via Alt+F2 `r` on X11. |
| **Lines 86–91:** Build from source instructions (`cargo build --release`, `./target/release/spawn-at install`) | **VERIFIED** | `Cargo.toml`, `src/main.rs` | Standard Cargo build and install entrypoint. |
| **Lines 101–110:** CLI `spawn` examples (`--pos`, `--size`, `--anchor cursor --pivot center`, `--anchor top-right -m 24`, `--no-focus`) | **VERIFIED** | `src/cli.rs:101-145` | All arguments match `SpawnArgs` definition; trailing command doesn't require `--`. |
| **Lines 116–121:** CLI `transform` examples (`--class`, `--anchor center --size`, `--focused`) | **VERIFIED** | `src/cli.rs:195-235` | Matches `TransformArgs` and `WindowTargetArgs`. |
| **Lines 126–136:** Window state examples (`focus`, `defocus --focused --to-desktop`, `defocus --to-window`, `maximize`, `minimize`, `restore`) | **VERIFIED** | `src/cli.rs:308-455` | All subcommands and flags exist and validate properly. |
| **Lines 143–151:** `query` examples (`query layout`, `query pointer`, `query windows`) | **VERIFIED** | `src/cli.rs:488-515` | All subcommands and `--json` flag exist. |
| **Lines 158–177:** Quick Reference table | **VERIFIED** | `src/cli.rs:1-917` | Every listed command and flag matches current clap definitions. |
| **Line 183:** License MIT | **VERIFIED** | `LICENSE:1-21` | Standard MIT license. |

---

## Part 3: Violations in Current README against User Brief

1. **Opening pitch violates constraint:** Current README says *"Unified zero-flicker window placement engine for modern compositors and platforms."* — uses forbidden buzzwords ("unified", "engine", "modern", "platforms") and does NOT name the core problem (Wayland clients can't place their own windows, so launch-then-move flashes).
2. **Missing Before/After block:** Brief requires the usual hack vs spawn-at as two short code/behavior blocks. Current README lacks this.
3. **Examples are generic flag tests:** Examples show isolated flags rather than real workflows (launcher at cursor, scratchpad terminal, dotfile/keybinding integration).
4. **Missing Mermaid sequence diagram:** Current README has an ASCII architecture diagram with unimplemented driver boxes instead of an end-to-end cloaking sequence diagram (CLI -> D-Bus -> extension -> Mutter).
5. **Missing "Why not X" section:** Current README lacks a comparison explaining why xdotool, wmctrl, devilspie, and GNOME window rules/extensions fail or fall short on Wayland.
6. **Architecture diagram violates rules:** Diagram contains boxes for unimplemented drivers ("Hyprland, Sway, KDE, macOS, Win"), explicitly forbidden by the brief.
7. **Missing "Adding a backend" developer section:** Current README does not document the `CompositorBackend` trait methods, where to add backends, or the architecture for contributors.
8. **Emoji bullets and feature checklists:** Current README uses emoji bullets (`⚡`, `🎯`, `🪟`, `🔄`, `🔍`, `🚀`), which the brief explicitly forbids.
