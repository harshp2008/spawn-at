# System Survey & Architectural Understanding: `spawn-at`

**Document Status:** Comprehensive Technical Survey & Codebase Audit (Current State)  
**Target Architecture Blueprint:** [`AI_DEV/spawn-at_master_architecture_blueprint.md`](file:///home/harsh/Documents/GITHUB%20PROJECTS/spawn-at/AI_DEV/spawn-at_master_architecture_blueprint.md)  
**Author:** Antigravity (DeepMind AAC)  
**Date:** October 2026  

---

## 1. Executive Summary & Codebase Audit Scope

This document provides an exhaustive, ground-truth technical survey and architectural audit of the `spawn-at` codebase. It reflects the exact state of the repository following recent refactoring of the core geometry engine into a dedicated workspace crate ([`crates/spawn-at-core`](file:///home/harsh/Documents/GITHUB%20PROJECTS/spawn-at/crates/spawn-at-core/src/lib.rs)), the advanced hardening of the GNOME Shell extension ([`assets/gnome/extension.esm.js`](file:///home/harsh/Documents/GITHUB%20PROJECTS/spawn-at/assets/gnome/extension.esm.js)), and the analysis of the current X11 driver gap.

### Key Findings:
1. **Core Decoupling in Progress:** Spatial geometry calculation, bounding box math, workarea resolution, and declarative driver interfaces (`Batch`, `Entry`, `Reveal`, `FocusIntent`, `Urgency`, `Driver`) now reside cleanly within [`crates/spawn-at-core`](file:///home/harsh/Documents/GITHUB%20PROJECTS/spawn-at/crates/spawn-at-core/src/driver.rs).
2. **Current 2-Tier Direct D-Bus Topology:** Every CLI invocation currently connects directly to GNOME Shell via session D-Bus (`zbus`) using `GnomeWaylandDriver::arm()` / `ArmSpawn` followed by immediate local process execution (`std::process::Command::spawn()`).
3. **Advanced Shell Extension Hardening:** The GNOME extension incorporates state-of-the-art Wayland zero-flicker mechanics:
   - `Clutter.OffscreenRedirect.NEVER` during the Cloak phase to prevent Mutter frame caching and slide artifacts.
   - An aggressive `after-update` safety net on `global.stage` that forcibly re-zeroes opacity against Mutter map animations.
   - VTE-candidate gating (`_isVteCandidate`) via `/proc/<pid>/maps` inspecting `libvte` to restrict synthetic focus pulses solely to GTK3+VTE terminals, protecting GTK4, Qt, and Electron apps from surface drops.
   - Elimination of the blind 300ms `UNCLOAK_4A` safety delay for non-VTE apps.
   - Allocation early-exit in `_waitForCommit` resolving immediately if allocation fires with stable dimensions.
4. **The X11 Gap:** On X11 sessions, `platform::linux::x11::X11Driver` is currently a stub returning `DriverError::UnsupportedCapability` for positioning and transformation. It lacks native X11 window property manipulation (`x11rb`/`xcb` setting `WM_NORMAL_HINTS` and `_NET_WM_USER_TIME`), causing X11 windows to fall back to uncoordinated top-left spawns.
5. **Missing 3-Tier Multi-Window Coordination:** The Coordinator daemon (`spawn-at-coordinator`), Unix Domain Socket (`coordinator.sock`), kernel Pipe-HUP barrier, rolling burst debouncer, priority preemption queue, and atomic settle locks specified in the master blueprint remain unbuilt.

---

## 2. Current Repository Architecture & Module Layout

The workspace is organized as a Cargo workspace containing the user-facing CLI binary crate and the pure core engine library:

```text
spawn-at/
├── Cargo.toml                                    # Workspace manifest (members: ".", "crates/spawn-at-core")
├── assets/gnome/
│   ├── metadata.json                             # GNOME extension metadata ("spawn-at@harsh.local")
│   ├── extension.js                              # GNOME Shell extension script (CJS/ESM bundle)
│   └── extension.esm.js                          # GNOME Shell ESM extension implementation
├── crates/
│   └── spawn-at-core/                            # Platform-agnostic core crate
│       ├── Cargo.toml
│       └── src/
│           ├── lib.rs                            # Re-exports for geometry and driver primitives
│           ├── geometry.rs                       # Spatial math, anchors, pivots, margins, clamp, diagnostics
│           └── driver.rs                         # Declarative Batch, Entry, Reveal, FocusIntent, Driver trait
└── src/
    ├── main.rs                                   # CLI entrypoint, command routing, execution flow
    ├── cli.rs                                    # Clap argument parser and command definitions
    ├── config.rs                                 # Desktop daemon configuration (~/.config/spawn-at/config.toml)
    ├── target.rs                                 # Window target resolution (PID, class, title, focus)
    ├── core/
    │   └── mod.rs                                # Re-exports from spawn_at_core
    ├── commands/
    │   ├── mod.rs                                # Command dispatch interface
    │   ├── focus.rs                              # Handlers for focus, defocus, and focus modifiers
    │   ├── lifecycle.rs                          # Handlers for maximize, minimize, unminimize, restore
    │   ├── query.rs                              # Handlers for workarea layout, cursor, and window queries
    │   └── transform.rs                          # Handlers for runtime window repositioning and resizing
    └── platform/
        ├── mod.rs                                # CompositorBackend trait, init_backend, InstallScope
        ├── installer.rs                          # Binary installer (~/.local/bin or /usr/local/bin)
        ├── escalate.rs                           # Sudo escalation utility for system installation
        └── linux/
            ├── mod.rs                            # LinuxBackend adapter dispatching to GNOME or X11
            ├── xdg.rs                            # Application ID resolver (desktop entry scanner)
            ├── x11.rs                            # X11 fallback driver (currently stubbed / unsupported)
            └── gnome/
                ├── mod.rs                        # GnomeWaylandDriver implementing CompositorBackend
                ├── dbus.rs                       # zbus proxy definition for org.gnome.Shell.Extensions.SpawnAt
                └── mechanics.rs                  # Micro-instruction generator & PlacementPayload synthesis
```

### 2.1 Module Responsibilities

| Module / Component | Path | Core Responsibility |
| :--- | :--- | :--- |
| **`spawn-at-core::geometry`** | [`crates/spawn-at-core/src/geometry.rs`](file:///home/harsh/Documents/GITHUB%20PROJECTS/spawn-at/crates/spawn-at-core/src/geometry.rs) | Pure mathematical placement engine. Calculates absolute pixel rectangles from anchors (`Center`, `TopLeft`, `Cursor`, etc.), pivots, margins, workarea bounds, and runs diagnostic validation. |
| **`spawn-at-core::driver`** | [`crates/spawn-at-core/src/driver.rs`](file:///home/harsh/Documents/GITHUB%20PROJECTS/spawn-at/crates/spawn-at-core/src/driver.rs) | Declarative driver contract: defines `Batch`, `Entry`, `Reveal`, `FocusIntent`, `Urgency`, `Armed`, and the async `Driver` trait. |
| **`CLI & Router`** | [`src/main.rs`](file:///home/harsh/Documents/GITHUB%20PROJECTS/spawn-at/src/main.rs), [`src/cli.rs`](file:///home/harsh/Documents/GITHUB%20PROJECTS/spawn-at/src/cli.rs) | Parses command-line invocations via `clap`, checks geometry diagnostics, constructs declarative batches, arms the driver, and launches child processes. |
| **`Command Handlers`** | [`src/commands/`](file:///home/harsh/Documents/GITHUB%20PROJECTS/spawn-at/src/commands/) | Decoupled sub-command handlers (`focus`, `defocus`, `maximize`, `minimize`, `restore`, `transform`, `query`) interacting with the active `CompositorBackend`. |
| **`Target Resolver`** | [`src/target.rs`](file:///home/harsh/Documents/GITHUB%20PROJECTS/spawn-at/src/target.rs) | Resolves specific window instances from desktop window lists by matching PID, window class (Wayland `app_id` / X11 `WM_CLASS`), title substrings, or active focus status. |
| **`Compositor Abstraction`** | [`src/platform/mod.rs`](file:///home/harsh/Documents/GITHUB%20PROJECTS/spawn-at/src/platform/mod.rs), [`src/platform/linux/mod.rs`](file:///home/harsh/Documents/GITHUB%20PROJECTS/spawn-at/src/platform/linux/mod.rs) | Defines `CompositorBackend` trait (extending `Driver`). Dispatches at runtime between `GnomeWaylandDriver` and `X11Driver` based on `$XDG_SESSION_TYPE` and `$XDG_CURRENT_DESKTOP`. |
| **`GNOME Wayland Driver`** | [`src/platform/linux/gnome/`](file:///home/harsh/Documents/GITHUB%20PROJECTS/spawn-at/src/platform/linux/gnome/) | Translates high-level declarative `Batch` intents into low-level `Instruction` sequences (`Cloak`, `SetSize`, `WaitForCommit`, `SetPositionAnchored`, `Uncloak`), serializing to JSON and calling the extension via `zbus`. |
| **`X11 Fallback Driver`** | [`src/platform/linux/x11.rs`](file:///home/harsh/Documents/GITHUB%20PROJECTS/spawn-at/src/platform/linux/x11.rs) | Minimal fallback driver. Currently lacks window placement mechanics and returns `UnsupportedCapability` for transformations. |
| **`GNOME Shell Extension`** | [`assets/gnome/extension.esm.js`](file:///home/harsh/Documents/GITHUB%20PROJECTS/spawn-at/assets/gnome/extension.esm.js) | GJS extension inside `gnome-shell`. Intercepts `window-created` / `map`, claims armed targets, enforces opacity cloaking, runs the FIFO execution queue, gates VTE focus pulses, positions frames, and arms reactive anchors. |

---

## 3. Current Execution Lifecycles

### 3.1 GNOME Wayland Cold-Start Spawn Lifecycle (`spawn-at spawn`)

```text
[ CLI Invocations ] (e.g. spawn-at spawn --anchor center -- alacritty)
       │
       ▼
 1. Argument Parsing & Validation (src/main.rs#L165-L175, src/cli.rs)
       ├── SpawnArgs::validate() -> enforces coordinate & size sanity
       └── platform::init_backend() -> LinuxBackend::bootstrap() -> GnomeWaylandDriver::new()
       │
       ▼
 2. Compositor Environment Query over D-Bus (src/main.rs#L178-L185)
       ├── GnomeWaylandDriver::get_cursor_position() -> D-Bus: GetCursor / GetPointer
       └── GnomeWaylandDriver::get_workareas() -> D-Bus: GetWorkareas -> monitors & workareas
       │
       ▼
 3. Geometry Synthesis & Diagnostics (crates/spawn-at-core/src/geometry.rs)
       ├── resolve_workarea() -> matches active monitor
       ├── check_geometry_diagnostics() -> emits warnings for sub-minimum or oversized bounds
       └── entry_key generation: "spawn-at-{pid}-{timestamp_nanos}"
       │
       ▼
 4. Declarative Batch Construction (src/main.rs#L244-L255)
       └── Batch { id: 1, entries: [Entry { key, app_hint, placement }], reveal: Together, focus: Exclusive, ... }
       │
       ▼
 5. Driver Arming & Instruction Lowering (src/platform/linux/gnome/mod.rs#L107-L146)
       ├── mechanics::build_instructions_for_entry(entry) ->
       │     [0] Instruction::Cloak
       │     [1] Instruction::SetSize { w, h } (if explicit)
       │     [2] Instruction::WaitForCommit { timeout_ms: 300 }
       │     [3] Instruction::SetPositionAnchored(PlacementPayload)
       │     [4] Instruction::Uncloak
       ├── Proxy calls org.gnome.Shell.Extensions.SpawnAt.ArmSpawn(target_id, instructions_json)
       └── Returns Armed { launch_env: [("XDG_ACTIVATION_TOKEN", key), ("DESKTOP_STARTUP_ID", key)] }
       │
       ▼
 6. Process Launch (src/main.rs#L271-L284)
       └── std::process::Command::spawn() with injected activation tokens
       │
══════════════════════════════════════════════════════════════════════════════
 [ GNOME Shell Compositor Process ] (assets/gnome/extension.esm.js)
══════════════════════════════════════════════════════════════════════════════
       │
 7. Window Interception & Hard Cloak (extension.esm.js#L690-L770, L1179-L1206)
       ├── global.display "window-created" / global.window_manager "map" fired
       ├── Matched against armed spawn table via startup_id, app_id, or pid
       ├── _cloak(actor):
       │     ├── actor.remove_all_transitions()
       │     ├── actor.set_offscreen_redirect(Clutter.OffscreenRedirect.NEVER) ── (Stops Mutter frame cache)
       │     ├── actor.opacity = 0
       │     └── Connects "notify::opacity" to enforce opacity = 0
       └── global.stage "after-update" safety net aggressively re-zeros opacity if Mutter resets it
       │
 8. FIFO Execution Queue (extension.esm.js#L219, L920-L970)
       ├── Enqueued into _batchQueue; runs when _batchBusy == false
       │
 9. Sizing & Commit Latch Pipeline (extension.esm.js#L980-L1070, L1255-L1356)
       ├── Step 1 (SetSize): calls window.move_resize_frame(true, x, y, targetW, targetH)
       └── Step 2 (WaitForCommit): waits for buffer commit
             ├── Early-Exit: Resolves immediately on 'AT_TARGET' or 'SIZE_SETTLED'
             └── Early-Exit: Resolves if 'allocation' fires but size doesn't change
       │
10. Anchored Positioning (extension.esm.js#L1050-L1083)
       ├── Step 3 (SetPositionAnchored): computes absolute screen coordinates with margins & pivot offsets
       └── window.move_frame(true, posX, posY)
       │
11. Uncloak Sequence & VTE-Gated Focus (extension.esm.js#L1085-L1170)
       ├── 4A (Safety delay): 0ms (skipped completely for ultra-fast spawn)
       ├── 4B (Focus Pulse): _isVteCandidate(window)
       │     ├── Checks /proc/<pid>/maps for 'libvte'
       │     ├── IF VTE terminal (e.g. gnome-terminal): brief defocus pulse with 100ms early-exit
       │     └── IF Non-VTE (GTK4, Qt, Electron, Alacritty, Kitty): SKIP focus pulse (prevents surface drops)
       ├── 4C (Re-anchor): Re-anchors if toolkit resized during pulse
       ├── Frame flush: Flushes 1 compositor frame
       └── 4D (Reveal):
             ├── _uncloak(actor) -> actor.set_offscreen_redirect(AUTOMATIC_FOR_OPACITY), actor.opacity = 255
             └── _armAnchor(window) -> attaches reactive anchor (repositions window if client later resizes)
```

---

### 3.2 The Broken X11 Flow & Analysis

When executing under an X11 desktop environment (e.g. `XDG_SESSION_TYPE=x11`), the execution topology breaks down:

```text
[ CLI Invocations on X11 ] (e.g. spawn-at spawn --anchor center -- xterm)
       │
       ▼
 1. LinuxBackend::bootstrap() detects X11 (src/platform/linux/mod.rs#L88-L91)
       └── Instantiates Box::new(X11Driver)
       │
       ▼
 2. Environment Query
       ├── X11Driver::get_workareas() calls query_xrandr_monitors() -> parses `xrandr` stdout
       └── X11Driver::get_cursor_position() calls query_xdotool_cursor() -> parses `xdotool` stdout
       │
       ▼
 3. Geometry Synthesis
       └── Correctly calculates PlacementParams using pure math in spawn-at-core
       │
       ▼
 4. Driver Arming: X11Driver::arm(batch) (src/platform/linux/x11.rs#L27-L36)
       └── Generates XDG_ACTIVATION_TOKEN and DESKTOP_STARTUP_ID, but DOES NOTHING with window placement!
       │
       ▼
 5. Process Launch (src/main.rs#L277)
       └── std::process::Command::spawn() launches process
       │
       ▼
 6. Result: The window spawns at the default top-left (0,0) or at an arbitrary position chosen by the X11 Window Manager.
    - No X11 window hints (WM_NORMAL_HINTS, USPosition, PPosition) are set.
    - No EWMH actions (_NET_MOVERESIZE_WINDOW) are dispatched.
    - All runtime transformation subcommands (transform, move, focus, set_window_state) fail immediately with:
      "Driver execution error: The active compositor does not support this feature".
```

---

## 4. Current Delta: Recent Architectural Evolution

The following table details the technical features and optimizations implemented in the current codebase that supersede previous survey documents:

| Architectural Feature | Implementation Point | Technical Rationale & Exact Behavior |
| :--- | :--- | :--- |
| **`OffscreenRedirect.NEVER` Cloaking** | [`assets/gnome/extension.esm.js#L1192-L1194`](file:///home/harsh/Documents/GITHUB%20PROJECTS/spawn-at/assets/gnome/extension.esm.js#L1192-L1194) | When Mutter repositions a newly mapped window actor, Clutter's default offscreen redirect mechanism caches the initial unpositioned frame and re-renders it as a sliding texture. Calling `actor.set_offscreen_redirect(Clutter.OffscreenRedirect.NEVER)` completely prevents Mutter from caching and sliding the actor during coordinate changes. Restored to `AUTOMATIC_FOR_OPACITY` on uncloak. |
| **`after-update` Global Safety Net** | [`assets/gnome/extension.esm.js#L205-L216`](file:///home/harsh/Documents/GITHUB%20PROJECTS/spawn-at/assets/gnome/extension.esm.js#L205-L216) | Mutter frequently overrides `actor.opacity = 0` during map animations (e.g. window open fade/zoom effects). An event listener attached to `global.stage.connect('after-update')` iterates all tracked cloaked actors on every stage paint and aggressively re-zeroes opacity if non-zero, guaranteeing zero visual leak. |
| **VTE-Candidate Gating (`_isVteCandidate`)** | [`assets/gnome/extension.esm.js#L649-L687`](file:///home/harsh/Documents/GITHUB%20PROJECTS/spawn-at/assets/gnome/extension.esm.js#L649-L687) | GTK3+VTE terminals (such as `gnome-terminal-server`) suffer from a layout negotiation bug where grid dimensions are only finalized after a focus cycle (`wl_keyboard.leave`/`enter`). However, GTK4/Libadwaita, Qt, Kitty, and Electron applications do *not* suffer from this bug; defocusing GTK4 windows while cloaked causes Wayland surface drops and 350ms map stalls. The extension inspects `/proc/<pid>/maps` for `libvte` and window class names, gating the focus pulse *strictly* to VTE apps and skipping it for all modern toolkits. |
| **Removal of 300ms `UNCLOAK_4A` Delay** | [`assets/gnome/extension.esm.js#L1095-L1098`](file:///home/harsh/Documents/GITHUB%20PROJECTS/spawn-at/assets/gnome/extension.esm.js#L1095-L1098) | Previously, a static 300ms delay was injected at the start of uncloaking to let buffers paint. Because `_waitForCommit` and the `after-update` cloak safety net now provide absolute synchronization guarantees, this blind delay has been completely eliminated (`delay=0ms`), reducing cold-start latency to near-instantaneous speeds. |
| **`_waitForCommit` Allocation Early-Exit** | [`assets/gnome/extension.esm.js#L1338-L1345`](file:///home/harsh/Documents/GITHUB%20PROJECTS/spawn-at/assets/gnome/extension.esm.js#L1338-L1345) | If Clutter fires `notify::allocation` but the frame size has already matched target dimensions or remained stable, the commit latch immediately clears the idle timer and triggers the 30ms quiet timer (`SIZE_SETTLED`), avoiding the fallback 120ms timeout. |
| **Reactive Anchor Auto-Retirement** | [`assets/gnome/extension.esm.js#L140, L1740-L1770`](file:///home/harsh/Documents/GITHUB%20PROJECTS/spawn-at/assets/gnome/extension.esm.js#L140) | After window reveal, a reactive anchor maintains the window glued to its screen anchor during post-spawn client-side size adjustments. The anchor automatically retires 1000ms after the last correction or immediately upon the user initiating an interactive drag/resize (`global.display.connect('grab-op-end')`). |
| **Sub-Chrome Geometry Guard Floor** | [`assets/gnome/extension.esm.js#L29-L35, L147-L158`](file:///home/harsh/Documents/GITHUB%20PROJECTS/spawn-at/assets/gnome/extension.esm.js#L29-L35) | Enforces a minimum geometry floor of 100x60px before passing dimensions to Mutter. This prevents CSD titlebar padding subtraction from causing negative widget allocations in GTK3/Pixman/XWayland and OpenGL rejection in GPU terminals. |
| **Asynchronous Session Logging** | [`assets/gnome/extension.esm.js#L38-L42, L280-L370`](file:///home/harsh/Documents/GITHUB%20PROJECTS/spawn-at/assets/gnome/extension.esm.js#L38-L42) | High-throughput telemetry and timestamps are logged asynchronously via `Gio.File` async streams to `~/.local/state/spawn-at/session.log`, automatically rotated across login sessions without stalling the GNOME Shell main loop. |

---

## 5. Architectural Gap Analysis: Current 2-Tier vs. Target 3-Tier Blueprint

The active codebase operates on a **2-Tier Direct Architecture**, whereas the production blueprint ([`AI_DEV/spawn-at_master_architecture_blueprint.md`](file:///home/harsh/Documents/GITHUB%20PROJECTS/spawn-at/AI_DEV/spawn-at_master_architecture_blueprint.md)) mandates a **3-Tier Coordinated Architecture**.

```text
CURRENT 2-TIER DIRECT ARCHITECTURE:
┌─────────────────┐       Direct D-Bus (zbus)       ┌──────────────────────────┐
│ spawn-at (CLI)  │ ──────────────────────────────> │ GNOME Shell Extension    │
│ (Every instance)│ <────────────────────────────── │ (spawn-at@harsh.local)   │
└────────┬────────┘                                 └──────────────────────────┘
         │ std::process::Command::spawn()
         ▼
  [ Child Process ]

--------------------------------------------------------------------------------

TARGET 3-TIER COORDINATED ARCHITECTURE:
┌─────────────────┐       Unix Domain Socket (UDS)    ┌──────────────────────────┐
│ spawn-at (CLI)  │ ════════════════════════════════> │ spawn-at-coordinator     │
│ (Thin Client)   │ <════════════════════════════════ │ (Single-Threaded Daemon) │
└────────┬────────┘      Pipe-HUP Barrier (SCM_RIGHTS)└────────────┬─────────────┘
         │ (Blocks on poll(POLLHUP))                               │ D-Bus Batch
         │                                                         ▼
         │                                            ┌──────────────────────────┐
         │ std::process::Command::spawn() after GO    │ GNOME Shell Extension    │
         ▼                                            │ (Atomic Settle Gate &    │
  [ Child Process ]                                   │  Claim Table Mutex)      │
                                                      └──────────────────────────┘
```

### 5.1 Dimensional Gap Comparison

| Architectural Dimension | Current Implementation (Actual Code) | Blueprint Specification ([`spawn-at_master_architecture_blueprint.md`](file:///home/harsh/Documents/GITHUB%20PROJECTS/spawn-at/AI_DEV/spawn-at_master_architecture_blueprint.md)) | Criticality & Gap Analysis |
| :--- | :--- | :--- | :--- |
| **Execution Topology** | **2-Tier Direct:** Each CLI instance connects directly to D-Bus (`org.gnome.Shell.Extensions.SpawnAt`) and immediately spawns its child process locally. | **3-Tier Coordinated:** CLI is a thin client communicating over UDS (`/run/user/$UID/spawn-at/coordinator.sock`). The Coordinator daemon alone talks to D-Bus. | **Critical:** CLI instances cannot synchronize with each other during concurrent script launches (`app1 & app2 &`). |
| **Coordinator Daemon** | **Missing:** No daemon exists. Configuration and backend selection occur per CLI invocation. | **`spawn-at-coordinator`:** Single-threaded Tokio event loop managing transaction state, peer credentials, queues, and locks. | **Critical:** Missing daemon binary, socket server, and transaction state engine. |
| **Pre-Spawn Assembly Gate (Invariant I1)** | **Non-Existent:** Child process is spawned immediately after D-Bus `ArmSpawn` returns. Zero barrier coordination between multiple CLI processes. | **Kernel Pipe-HUP Barrier:** CLI receives the read-end of a kernel pipe via `SCM_RIGHTS` and blocks on `poll(POLLHUP)`. Process does NOT spawn until coordinator seals batch. | **Critical:** Essential for zero visual tearing during multi-window layout initialization. |
| **Shell Burst Debouncing** | **Missing:** Concurrent background spawns (`cmd1 & cmd2 &`) race to D-Bus independently. | **Rolling Debounce Auto-Grouping:** Coordinator groups incoming CLI connections matching `(uid, SID, PPID)` within a rolling 40ms window into a single atomic multi-window transaction. | **High:** Required for scripted desktop layout deployment. |
| **Priority Queue & Preemption Engine** | **Missing:** Commands execute in ad-hoc FIFO arrival order. | **Inverted Priority Preemption Queue:** Lower integer = higher priority ($P_{\text{assigned}} = P_{\max} + K$). Starvation guard ensures max 8 priority dispatches before yielding. | **High:** Urgent utilities (launchers, popups) are blocked behind slow multi-window batches. |
| **Lock Split & Atomic Settle** | **Extension FIFO Array:** Internal JS array `_batchQueue` with boolean `_batchBusy` lock. | **Atomic Lock Split:** `focus_execution_lock` held during 2A (Arm) + 2B (Spawn) and 2D (Settle). Stage 2C (MapAwait) is lock-free, preventing slow-starting apps from freezing the queue. | **High:** Slow cold-starts currently stall the extension queue. |
| **Out-of-Band Express Path (`--bypass-fifo`)** | **Unimplemented:** All commands pass through the same single-lane pipeline. | **BypassArm & Soft-Yield Barrier:** Urgent requests register an express claim and raise a 100ms barrier (`bypass_active_until`) suppressing group focus stealing. | **Medium-High:** Prevents focus race conditions. |
| **Multi-Crate Workspace** | **Partial (2 Crates):** Root `spawn-at` + `crates/spawn-at-core`. | **Complete (5 Crates):** `spawn-at-proto`, `spawn-at-core`, `spawn-at-coordinator`, `spawn-at-cli`, and `assets/gnome`. | **Architectural:** Protocol frames and daemon need explicit crate boundaries. |

---

## 6. The X11 Driver Gap & Remediation Strategy

### 6.1 Why the Current X11 Driver Fails
The current `X11Driver` in [`src/platform/linux/x11.rs`](file:///home/harsh/Documents/GITHUB%20PROJECTS/spawn-at/src/platform/linux/x11.rs) is an empty stub that does not manipulate window properties. It relies on `xrandr` and `xdotool` only for querying screens and cursor coordinates.

Crucially, the GNOME Shell extension (`extension.esm.js`) is an internal Mutter/Clutter component. In X11 sessions, calling the Shell extension via D-Bus either fails or cannot control standard X11 window placement before the window is mapped. Consequently, X11 windows ignore `spawn-at` placement arguments and map to default top-left coordinates.

### 6.2 The Native X11 Architecture Solution
To achieve robust window placement on X11, `spawn-at` must handle X11 window mechanics **natively within the Rust driver**, completely bypassing the GNOME Shell extension:

```text
[ Native X11 Placement Flow in Rust ]
       │
       ▼
 1. Driver Detection: Session is X11
       │
       ▼
 2. Environment Preparation:
       ├── Pre-calculate window placement geometry via spawn-at-core::geometry
       └── Export startup notification environment: DESKTOP_STARTUP_ID="spawn-at-{pid}-{time}"
       │
       ▼
 3. Native X11 Connection (using x11rb or xcb crate):
       ├── Connect to X11 display ($DISPLAY)
       ├── Intercept/Listen to SubstructureNotify / SubstructureRedirect on Root Window
       │
       ▼
 4. Pre-Map Property Injection (Intercepting Client Creation):
       ├── Intercept MapRequest or client window creation matching startup ID / PID
       ├── Set WM_NORMAL_HINTS:
       │     ├── Set flags: USPosition | PPosition | USSize | PSize
       │     └── Set x, y, width, height to calculated geometry
       ├── Set _NET_WM_USER_TIME = 0 (prevents unwanted focus stealing before settle)
       ├── Set _NET_WM_DESKTOP / Workarea placement
       │
       ▼
 5. Post-Map EWMH Adjustment:
       ├── Send EWMH ClientMessage: _NET_MOVERESIZE_WINDOW
       └── Dispatch XMapWindow to present window cleanly at target coordinates
```

By implementing this native X11 engine directly in Rust, `spawn-at` becomes fully functional across all X11 desktop environments (GNOME X11, KDE X11, XFCE, i3, bspwm) without requiring any GNOME Shell extensions.

---

## 7. Actionable Refactoring Roadmap

To ensure continuous stability and clear milestone validation, the refactoring roadmap prioritizes fixing the **Native X11 Driver** before undertaking the full **3-Tier Coordinator Refactor**.

```text
  ┌────────────────────────────────────────────────────────────────────────┐
  │ PHASE 1: Native X11 Driver & Core Crate Hardening                      │
  │ • Add x11rb dependency for zero-C native X11 protocol integration      │
  │ • Implement native X11 window intercept, WM_NORMAL_HINTS & EWMH moves   │
  │ • Enable transform, query, focus, and state commands on X11           │
  └───────────────────────────────────┬────────────────────────────────────┘
                                      │
                                      ▼
  ┌────────────────────────────────────────────────────────────────────────┐
  │ PHASE 2: Standalone Pipe-HUP Barrier & Protocol Crate                  │
  │ • Create crates/spawn-at-proto with binary frames (HELLO, ACK, GO, etc)│
  │ • Implement SCM_RIGHTS file descriptor passing over UDS                │
  │ • Validate zero-dependency kernel pipe-HUP barrier & CLI parking       │
  └───────────────────────────────────┬────────────────────────────────────┘
                                      │
                                      ▼
  ┌────────────────────────────────────────────────────────────────────────┐
  │ PHASE 3: Coordinator Daemon & Rolling Debounce Auto-Grouper            │
  │ • Build crates/spawn-at-coordinator single-threaded Tokio daemon       │
  │ • Implement UDS server (/run/user/$UID/spawn-at/coordinator.sock)      │
  │ • Implement (uid, SID, PPID) burst detection & 40ms rolling debouncer  │
  │ • Implement Inverted Priority Preemption Queue with compaction         │
  └───────────────────────────────────┬────────────────────────────────────┘
                                      │
                                      ▼
  ┌────────────────────────────────────────────────────────────────────────┐
  │ PHASE 4: Full Multi-Compositor D-Bus & Extension Settle Gate           │
  │ • Migrate D-Bus communication into coordinator driver layer            │
  │ • Upgrade GNOME extension with Claim Table, RequestSettle & BatchSettled│
  │ • Implement 100ms Soft-Yield Barrier & --bypass-fifo express path      │
  │ • Deploy thin CLI binary (crates/spawn-at-cli)                         │
  └────────────────────────────────────────────────────────────────────────┘
```

---

### Phase 1: Native X11 Driver & Core Crate Hardening
- **Objective:** Fix the broken X11 fallback by implementing native, pure-Rust X11 window placement and management.
- **Key Tasks:**
  1. Add `x11rb` (with `x11rb/allow-unsafe-code` disabled or minimal) to `Cargo.toml`.
  2. Implement native window property configuration in [`src/platform/linux/x11.rs`](file:///home/harsh/Documents/GITHUB%20PROJECTS/spawn-at/src/platform/linux/x11.rs):
     - Calculate target geometries using [`crates/spawn-at-core/src/geometry.rs`](file:///home/harsh/Documents/GITHUB%20PROJECTS/spawn-at/crates/spawn-at-core/src/geometry.rs).
     - Set `WM_NORMAL_HINTS` (`USPosition`, `USSize`, `PPosition`, `PSize`) on managed windows.
     - Send `_NET_MOVERESIZE_WINDOW` EWMH client messages for runtime `transform`, `move`, and `restore`.
     - Implement `get_windows()`, `focus_window()`, and `set_window_state()` using EWMH `_NET_CLIENT_LIST`, `_NET_ACTIVE_WINDOW`, and `_NET_WM_STATE`.
  3. Ensure all unit and CLI integration tests pass seamlessly across both Wayland and X11 sessions.

---

### Phase 2: Standalone Pipe-HUP Barrier & Protocol Crate
- **Objective:** Establish the low-overhead IPC wire protocol and validate synchronous process parking without touching compositor drivers.
- **Key Tasks:**
  1. Create `crates/spawn-at-proto` defining fixed wire frames:
     - `HELLO` (0x01), `ACK` (0x02), `WARN` (0x03), `ABORT` (0x04), `GO` (0x05), `SPAWNED` (0x06), `SETTLED` (0x07), `CANCEL` (0x08).
  2. Implement length-prefixed binary framing (64 KiB ceiling) and endianness enforcement.
  3. Implement `sendmsg` / `recvmsg` `SCM_RIGHTS` fd passing for transferring the barrier pipe read-end.
  4. Build a test rig: Coordinator creates `nix::unistd::pipe()`, CLI receives read-end, coordinator closes write-end, verifying all $N$ CLIs wake synchronously via `poll(POLLHUP)`.

---

### Phase 3: Coordinator Daemon & Rolling Debounce Auto-Grouper
- **Objective:** Construct the single-threaded coordinator daemon to serialize incoming spawns and auto-group shell bursts.
- **Key Tasks:**
  1. Create `crates/spawn-at-coordinator` with a single-threaded Tokio runtime (`tokio::runtime::Builder::new_current_thread()`).
  2. Implement UDS server listening on `/run/user/$UID/spawn-at/coordinator.sock` with `SO_PEERCRED` validation.
  3. Implement burst key detection `(uid, SID, PPID)` via `/proc/<pid>/stat`, filtering out system processes (`gnome-shell`, `systemd`).
  4. Implement the adaptive fast-path sibling probe via `/proc/<PPID>/task/*/children`:
     - If sibling count is 0, seal size-1 batch immediately.
     - If siblings exist, arm the 40ms rolling debounce timer.
  5. Implement `PriorityLane` in `crates/spawn-at-core`:
     - Monotonic relative insertion: $P_{\text{assigned}} = P_{\max} + K$.
     - Consecutive integer compaction preserving arrival order (**Invariant I5**).
     - Starvation guard enforcing a baseline dispatch after 8 priority dispatches.

---

### Phase 4: Full Multi-Compositor D-Bus & Extension Settle Gate
- **Objective:** Finalize the 3-Tier architecture by connecting the coordinator to the GNOME Shell extension and activating the Focus Authority.
- **Key Tasks:**
  1. Move `GnomeWaylandDriver` from CLI into `spawn-at-coordinator`.
  2. Update GNOME Shell extension (`extension.esm.js`) to expose:
     - `ArmSpawn(batch)`: Registers atomic multi-window claim tables.
     - `RequestSettle(batch_id)`: Enters 2D Settle critical section under coordinator lock.
     - `BypassArm(entry)`: Express path registering express claims and setting `bypass_active_until`.
  3. Implement the Atomic Lock Split in the coordinator:
     - Hold `focus_execution_lock` during 2A (Arm) + 2B (Spawn) and 2D (Settle).
     - Keep 2C (MapAwait) completely lock-free so slow-launching apps cannot block incoming requests.
  4. Implement thin CLI executable (`crates/spawn-at-cli`) with client-side flag validation, barrier parking, and `--bypass-fifo` direct routing.

---

## 8. Architectural Invariant Validation Matrix

The target architecture enforces 10 strict system invariants to prevent visual tearing, deadlocks, and race conditions:

| Invariant ID | Invariant Name | Architectural Enforcement Point |
| :--- | :--- | :--- |
| **I1** | **Strict No-Spawn Gate** | CLI process blocks on the kernel Pipe-HUP barrier (`poll(POLLHUP)`); zero child processes or Wayland surfaces exist until batch is fully assembled and armed. |
| **I2** | **Single Writer Authority** | Only the single-threaded coordinator event loop mutates transaction states, queues, and leases. |
| **I3** | **Settle Exclusion** | `focus_execution_lock` is held exclusively during 2A/2B and 2D Settle phases; mirrored by extension batch mutex. |
| **I4** | **First-Claim Authority** | Leadership and immutable contract authority belong strictly to the first decoded frame carrying `--group-size`. |
| **I5** | **Order Preservation** | Relative insertion $P_{\text{assigned}} = P_{\max} + K$ and stable compaction guarantee rank$(a) <$ rank$(b)$ for arrival timestamps $a < b$. |
| **I6** | **Bounded Waiting** | Every transaction is guarded by monotonic timer deadlines (`auto_group_window_ms`, `group_timeout_ms`, `hold_timeout_ms`). |
| **I7** | **Fail-Open Launch Safety** | If coordinator or extension crashes, CLI prints a diagnostic warning and executes raw `Command::spawn()` to guarantee application launch. |
| **I8** | **Cloak Finiteness** | Extension uncloaks window actor on `cloak_max_ms` timeout unconditionally, preventing invisible zombie windows. |
| **I9** | **Claim Finiteness** | Extension claims expire automatically after `claim_ttl_ms`, preventing accidental hijacking of future window instances. |
| **I10** | **Non-Blocking Core** | All socket I/O is asynchronous with bounded buffers; `/proc` reads are single-syscall sized and capped. |

---

## 9. Conclusion & Next Steps

The current `spawn-at` repository possesses a world-class, battle-tested single-window zero-flicker placement engine for GNOME Wayland, featuring advanced Clutter cloaking, VTE-candidate gating, sub-chrome geometry guard floors, and allocation-settling optimizations.

The immediate priorities to complete the system are:
1. **Remediate the X11 Driver Gap:** Build native X11 window placement via `x11rb` directly in the Rust driver so X11 sessions do not require the GNOME Shell extension.
2. **Execute the 3-Tier Multi-Window Coordinator Migration:** Implement `spawn-at-proto`, `spawn-at-coordinator`, kernel Pipe-HUP barrier parking, rolling debounce auto-grouping, and inverted priority queues to achieve flawless multi-window workspace orchestration.
