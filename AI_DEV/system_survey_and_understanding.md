# System Survey & Architectural Understanding: `spawn-at`

**Document Status:** Final Technical Survey & Architectural Delta  
**Target Blueprint:** [`AI_DEV/spawn-at_master_architecture_blueprint.md`](file:///home/harsh/Documents/GITHUB%20PROJECTS/spawn-at/AI_DEV/spawn-at_master_architecture_blueprint.md)  
**Author:** Antigravity (DeepMind AAC)  
**Date:** October 2026  

---

## 1. System Identity & Current Execution Topology

### 1.1 Current Repository Architecture & Module Layout

The current repository is structured as a monolithic Rust binary crate (`spawn-at`) with a companion GNOME Shell JavaScript extension. The codebase is organized into several key modules:

```text
spawn-at/
├── Cargo.toml                              # Single-binary package definition (tokio, zbus, clap, serde)
├── assets/gnome/
│   ├── metadata.json                       # GNOME extension metadata ("spawn-at@harsh.local")
│   ├── extension.js                        # CommonJS/Legacy extension script
│   └── extension.esm.js                    # ESM extension script implementing D-Bus service & Cloak/Settle
└── src/
    ├── main.rs                             # CLI entrypoint and subcommand router
    ├── cli.rs                              # Command-line argument definitions using Clap
    ├── config.rs                           # Daemon configuration file parser (config.toml)
    ├── target.rs                           # Window resolution and matching algorithms (PID/class/title)
    ├── core/
    │   ├── mod.rs                          # Core geometry re-exports
    │   ├── geometry.rs                     # Spatial math, anchor coordinates, pivot offsets, clamping
    │   └── types.rs                        # PlacementPayload & Instruction enum definitions
    ├── commands/
    │   ├── mod.rs                          # Command execution router
    │   ├── focus.rs                        # Handlers for focus, defocus, and focus modifiers
    │   ├── lifecycle.rs                    # Handlers for maximize, minimize, restore
    │   ├── query.rs                        # Handlers for querying workareas, windows, cursor
    │   └── transform.rs                    # Handlers for runtime window repositioning & resizing
    └── platform/
        ├── mod.rs                          # Trait CompositorBackend & runtime detection (init_backend)
        ├── installer.rs                    # Binary installer (~/.local/bin or /usr/local/bin)
        ├── escalate.rs                     # Sudo escalation utility for system installs
        └── linux/
            ├── mod.rs                      # LinuxBackend adapter dispatching to GNOME or X11
            ├── x11.rs                      # X11 fallback backend (xdotool, wmctrl, xwininfo)
            ├── xdg.rs                      # Application ID resolver (desktop entry scanner)
            └── gnome/
                ├── mod.rs                  # GnomeWaylandDriver implementing CompositorBackend
                └── dbus.rs                 # zbus proxy definition for org.gnome.Shell.Extensions.SpawnAt
```

---

### 1.2 Execution Topology & Detailed Lifecycles

#### A. Lifecycle of `spawn-at spawn`

```text
[ User / Shell ]
       │
       ▼
 1. cli::Cli::parse() / SpawnArgs::validate()  ── (src/main.rs#L161-L171)
       │
       ▼
 2. Driver Initialization: platform::init_backend() ── (src/platform/linux/mod.rs#L78-L93)
       │ Connects to D-Bus Session Bus (zbus)
       ▼
 3. Query Cursor & Workareas via D-Bus:
       ├── driver.get_cursor_position() -> GetCursor / GetPointer
       └── driver.get_workareas() -> GetWorkareas
       │
       ▼
 4. Geometry Synthesis: core::geometry::calculate_placement() ── (src/core/geometry.rs#L125)
       │ Computes screen_anchor_x, screen_anchor_y, pivot offsets, margins
       ▼
 5. Micro-Instruction Assembly: ── (src/main.rs#L218-L226)
       ├── [0] Instruction::Cloak
       ├── [1] Instruction::SetSize { w, h } (if specified)
       ├── [2] Instruction::WaitForCommit { timeout_ms: 300 }
       ├── [3] Instruction::SetPositionAnchored(PlacementPayload)
       └── [4] Instruction::Uncloak
       │
       ▼
 6. Identity Synthesis: Generate XDG_ACTIVATION_TOKEN / DESKTOP_STARTUP_ID:
       │ target_id = format!("spawn-at-{pid}-{timestamp_nanos}")
       ▼
 7. Direct D-Bus Arm: GnomeWaylandDriver::spawn_at() ── (src/platform/linux/gnome/mod.rs#L273-L314)
       │ Calls org.gnome.Shell.Extensions.SpawnAt.ArmSpawn(target_id, instructions_json)
       ▼
 8. Process Spawn: std::process::Command::spawn()
       │ Injected with env: XDG_ACTIVATION_TOKEN=target_id, DESKTOP_STARTUP_ID=target_id
       ▼
 9. [GNOME Shell Extension]: assets/gnome/extension.esm.js
       ├── Window created -> Intercepted -> opacity set to 0 (CLOAK)
       ├── Enqueued into Extension-side FIFO Mutex Queue (_batchQueue)
       ├── Synthetic Focus Bounce -> Stage.set_key_focus(null) -> 20ms -> window.activate()
       ├── Wait for buffer commit / size change (WaitForCommit)
       ├── Apply anchored position & frame clamp (SetPositionAnchored)
       └── Uncloak (opacity = 255) & stage relayout
```

#### B. Lifecycle of Window Mutation & Focus Commands (`focus.rs`, `lifecycle.rs`, `transform.rs`)

1. **Focus/Defocus Routing (`src/commands/focus.rs`):**
   - [`run_focus`](file:///home/harsh/Documents/GITHUB%20PROJECTS/spawn-at/src/commands/focus.rs#L29-L43): Queries active windows via `driver.get_windows()`, resolves the target window using [`target::resolve_target`](file:///home/harsh/Documents/GITHUB%20PROJECTS/spawn-at/src/target.rs#L44), extracts the target identity, and invokes [`backend.focus_window(&target_id)`](file:///home/harsh/Documents/GITHUB%20PROJECTS/spawn-at/src/platform/linux/gnome/mod.rs#L400-L414) (`org.gnome.Shell.Extensions.SpawnAt.FocusWindow`).
   - [`run_defocus`](file:///home/harsh/Documents/GITHUB%20PROJECTS/spawn-at/src/commands/focus.rs#L46-L78): Directly calls [`backend.defocus_window(&target_id, &to)`](file:///home/harsh/Documents/GITHUB%20PROJECTS/spawn-at/src/platform/linux/gnome/mod.rs#L416-L430), transferring focus to the previous window in MRU stack or dropping focus to `global.stage.set_key_focus(null)`.
2. **Lifecycle Modifications (`src/commands/lifecycle.rs`):**
   - Commands (`maximize`, `minimize`, `unminimize`, `restore`) directly dispatch D-Bus requests (`SetWindowState`) followed immediately by [`apply_focus_policy`](file:///home/harsh/Documents/GITHUB%20PROJECTS/spawn-at/src/commands/focus.rs#L11-L26).
3. **Runtime Transforms (`src/commands/transform.rs`):**
   - Synthesizes a snapshot/move/resize/uncloak instruction stream and calls [`backend.execute_batch(&target_id, &instructions)`](file:///home/harsh/Documents/GITHUB%20PROJECTS/spawn-at/src/platform/linux/gnome/mod.rs#L316-L326) directly over D-Bus.

---

## 2. Architectural Gap Analysis (Current Code vs. Blueprint)

The target architecture specified in [`AI_DEV/spawn-at_master_architecture_blueprint.md`](file:///home/harsh/Documents/GITHUB%20PROJECTS/spawn-at/AI_DEV/spawn-at_master_architecture_blueprint.md) defines a robust **3-Tier Architecture** (CLI $\leftrightarrow$ Coordinator UDS Daemon $\leftrightarrow$ GNOME Shell Extension D-Bus). 

Comparing the existing implementation against this blueprint reveals major structural and algorithmic deltas:

| Architectural Dimension | Current Implementation | Blueprint Specification ([`spawn-at_master_architecture_blueprint.md`](file:///home/harsh/Documents/GITHUB%20PROJECTS/spawn-at/AI_DEV/spawn-at_master_architecture_blueprint.md)) | Severity / Impact |
| :--- | :--- | :--- | :--- |
| **Execution Topology** | **2-Tier Direct Architecture:** Every CLI invocation connects directly to GNOME Shell over session D-Bus (`zbus`) and calls `ArmSpawn` followed by local `Command::spawn()`. | **3-Tier Coordinated Architecture:** CLI connects to a single-threaded Coordinator daemon via Unix Domain Socket (`/run/user/$UID/spawn-at/coordinator.sock`). Coordinator alone communicates with D-Bus. | **Critical** (Root cause of concurrency races & D-Bus floods) |
| **Pre-Spawn Assembly Gate (Invariant I1)** | **Non-Existent:** The CLI parses arguments, arms the extension, and immediately spawns the binary child process (`Command::spawn()`). No barriers or contract resolution. | **Strict No-Spawn Gate (I1):** CLI parks at a kernel pipe-HUP barrier. Zero child processes or Wayland surfaces exist until the transaction is fully assembled, sealed, and armed. | **Critical** (Causes multi-window desynchronization and tearing) |
| **Shell Concurrency & Burst Debouncing** | **Uncoordinated:** Multiple background invocations (`cmd1 & cmd2 &`) race to the D-Bus socket independently. Relies solely on extension-side per-window timeout maps. | **Rolling Debounce Auto-Grouping:** Coordinator aggregates concurrent invocations with matching burst keys $(\text{uid}, \text{SID}, \text{PPID})$ inside a rolling 40ms debounce window (adaptive fast-path probe via `/proc/<PPID>/task/*/children`). | **High** (Causes race conditions during script/workspace launches) |
| **Priority Queue & Preemption Engine** | **Missing:** All commands are dispatched immediately in ad-hoc arrival order. No priority lanes or ordering semantics. | **Inverted Priority Preemption Queue:** Lower integer = higher urgency (POSIX `nice`). $P_{\text{assigned}} = P_{\max} + K$ with consecutive integer compaction ($0..N-1$) and starvation prevention (max 8 priority dispatches). | **High** (Transient utilities like launchers block behind multi-window layouts) |
| **Phase 2 Lock Split & Settle Critical Section** | **Monolithic Extension Queue:** The extension maintains an internal JavaScript array `_batchQueue` with a boolean mutex `_batchBusy`, locking during buffer commit and timeouts. | **Atomic Lock Split:** `focus_execution_lock` covers 2A (Arm) + 2B (Spawn) and 2D (Settle). Stage 2C (MapAwait, process cold-start) is lock-free, preventing Electron/heavy app startup delays from freezing the queue. | **High** (Slow starting applications freeze other window operations) |
| **Focus Authority & Soft-Yield Barrier** | **Uncoordinated Direct Activation:** Extension and CLI issue `win.activate()` arbitrarily. Synthetic focus bounces yield to stage unconditionally. | **Single-Writer Focus Authority:** Extension arbitrates focus. Express `--bypass-fifo` launches set a 100ms soft-yield barrier (`bypass_active_until`) suppressing group focus-stealing with deferred focus restoration. | **Medium-High** (Focus stealing races between urgent tools and group cold-starts) |
| **Micro-Instruction Leaky Abstraction** | **Compositor-Specific Instructions in Core:** [`src/core/types.rs`](file:///home/harsh/Documents/GITHUB%20PROJECTS/spawn-at/src/core/types.rs) exposes low-level GNOME Shell internals (`Snapshot`, `Cloak`, `WaitForCommit`, `Uncloak`) to high-level CLI code. | **Declarative Protocol Batches:** Wire frames and coordinator use high-level declarative geometries and transaction contracts; compositor-specific mechanics are strictly encapsulated in platform drivers. | **Medium** (Violates multi-compositor portability) |

---

## 3. Proposed Multi-Crate Workspace Layout

To transition from the monolithic design to the target 3-tier architecture, the repository will be structured as a Cargo workspace with distinct, single-responsibility crates:

```text
spawn-at/
├── Cargo.toml                                # Workspace root manifest
├── crates/
│   ├── spawn-at-proto/                       # Shared IPC protocol, binary frames, serialization
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── frames.rs                     # HELLO, ACK, GO, SPAWNED, SETTLED, WARN, ABORT, CANCEL
│   │       ├── codec.rs                      # Length-prefixed 64KiB framing codec
│   │       └── credentials.rs                # SO_PEERCRED / PID validation types
│   │
│   ├── spawn-at-core/                        # Scheduling logic, math, state machine (platform-agnostic)
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── geometry.rs                   # Absolute coordinates, workareas, anchor math, clamping
│   │       ├── transaction.rs                # Transaction state machine (Registering -> Ready -> Settling)
│   │       ├── priority.rs                   # Priority lane, P_max + K insertion, integer compaction
│   │       ├── burst.rs                      # Burst key evaluation (UID, SID, PPID) and rolling debouncer
│   │       └── lock.rs                       # focus_execution_lock state and lease manager
│   │
│   ├── spawn-at-coordinator/                 # Single-threaded async daemon binary
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── main.rs                       # Daemon entrypoint, tokio current_thread LocalSet
│   │       ├── server.rs                     # UDS listener (/run/user/$UID/spawn-at/coordinator.sock)
│   │       ├── dispatcher.rs                 # Ready-Set dispatch loop and starvation guard
│   │       ├── barrier.rs                    # Pipe-HUP descriptor distributor (SCM_RIGHTS)
│   │       ├── proc.rs                       # Kernel /proc stats, pidfd pinning, sibling probe
│   │       ├── driver/                       # Compositor driver traits and zbus D-Bus interface
│   │       │   ├── mod.rs                    # trait Driver
│   │       │   └── gnome.rs                  # org.spawnat.Coordinator1 & org.spawnat.Shell1 client
│   │       └── ctl.rs                        # spawn-at ctl status / disband / release-hold handlers
│   │
│   └── spawn-at-cli/                         # User-facing CLI binary
│       ├── Cargo.toml
│       └── src/
│           ├── main.rs                       # CLI entrypoint (spawn, transform, query, focus, ctl)
│           ├── client.rs                     # UDS client connecting to coordinator.sock
│           ├── barrier.rs                    # Poll-based blocking on barrier pipe fd (POLLHUP)
│           ├── bypass.rs                     # Express path for --bypass-fifo (direct D-Bus BypassArm)
│           └── args/                         # Clap argument definitions and pre-IPC validation
│               ├── mod.rs
│               ├── spawn.rs                  # SpawnArgs, --group, --group-size, --priority, --hold
│               └── authority.rs              # Client-side flag authority and conflict validator
│
├── extension/                                # GNOME Shell GJS extension
│   ├── metadata.json                         # "spawn-at@harsh.local"
│   └── extension.js                          # Batch claim table, 2D Settle gate, Focus Authority
└── AI_DEV/
    ├── spawn-at_master_architecture_blueprint.md
    └── system_survey_and_understanding.md
```

### Responsibility Breakdown:

1. **`crates/spawn-at-proto`:**
   - Contains zero scheduler logic.
   - Defines fixed wire protocol structures ([`HELLO`](file:///home/harsh/Documents/GITHUB%20PROJECTS/spawn-at/AI_DEV/spawn-at_master_architecture_blueprint.md#L201), [`ACK`](file:///home/harsh/Documents/GITHUB%20PROJECTS/spawn-at/AI_DEV/spawn-at_master_architecture_blueprint.md#L202), [`GO`](file:///home/harsh/Documents/GITHUB%20PROJECTS/spawn-at/AI_DEV/spawn-at_master_architecture_blueprint.md#L205), [`SPAWNED`](file:///home/harsh/Documents/GITHUB%20PROJECTS/spawn-at/AI_DEV/spawn-at_master_architecture_blueprint.md#L206), [`SETTLED`](file:///home/harsh/Documents/GITHUB%20PROJECTS/spawn-at/AI_DEV/spawn-at_master_architecture_blueprint.md#L207), [`WARN`](file:///home/harsh/Documents/GITHUB%20PROJECTS/spawn-at/AI_DEV/spawn-at_master_architecture_blueprint.md#L203), [`ABORT`](file:///home/harsh/Documents/GITHUB%20PROJECTS/spawn-at/AI_DEV/spawn-at_master_architecture_blueprint.md#L204), [`CANCEL`](file:///home/harsh/Documents/GITHUB%20PROJECTS/spawn-at/AI_DEV/spawn-at_master_architecture_blueprint.md#L208)).
   - Enforces the 64 KiB frame ceiling and binary endianness.
2. **`crates/spawn-at-core`:**
   - Implements the pure state machines and queue algorithms.
   - Houses `compact(queue, p_max)`, monotonic `p_assigned` math, burst grouping predicates, and workarea geometry clamping.
   - Unit-testable without async runtime or OS sockets.
3. **`crates/spawn-at-coordinator`:**
   - The single-threaded daemon (`tokio::runtime::Builder::new_current_thread().enable_all().build()`).
   - Owns the UDS server socket, accepts connections, verifies `SO_PEERCRED`, manages the Pipe-HUP descriptors, dispatches batches to GNOME Shell over D-Bus (`org.spawnat.Coordinator1`), and arbitrates `focus_execution_lock`.
4. **`crates/spawn-at-cli`:**
   - Ultra-fast client executable.
   - Runs client-side flag validation ([`Flag Authority Hierarchy`](file:///home/harsh/Documents/GITHUB%20PROJECTS/spawn-at/AI_DEV/spawn-at_master_architecture_blueprint.md#L183-L228)).
   - Connects to `coordinator.sock`, transmits environment/CWD in `HELLO`, waits on barrier fd via `poll(POLLHUP)`, calls `Command::spawn()`, sends `SPAWNED`, and awaits `SETTLED` before exiting.
   - Handles `--bypass-fifo` by speaking directly to the extension's `BypassArm` method without touching the coordinator.
5. **`extension/`:**
   - Manages the Claim Table, Express Claim Table, Opacity Cloaking ($0 \to 255$ ramp), Settle Gate (`RequestSettle` / `BatchSettled`), and the Focus Authority with the 100ms Soft-Yield Barrier.

---

## 4. Actionable Refactoring Roadmap

To transition the active codebase safely without breaking existing desktop functionality, a 4-phase vertical-slice migration roadmap is established:

```text
 ┌────────────────────────────────────────────────────────────────────────┐
 │ PHASE 1: Standalone Pipe-HUP Barrier & IPC Wire Frame Prototype        │
 │ • Establish crates/spawn-at-proto with HELLO/ACK/GO/SPAWNED frames     │
 │ • Build zero-dependency pipe-HUP fd broadcast mechanism & CLI parking  │
 └───────────────────────────────────┬────────────────────────────────────┘
                                     │
                                     ▼
 ┌────────────────────────────────────────────────────────────────────────┐
 │ PHASE 2: UDS Coordinator & Rolling Debounce Auto-Grouper              │
 │ • Implement coordinator.sock with SO_PEERCRED & /proc stat inspection  │
 │ • Add (uid, SID, PPID) burst key detection & 40ms rolling debouncer    │
 │ • Add adaptive fast-path sibling probe (/proc/<PPID>/task/*/children)  │
 └───────────────────────────────────┬────────────────────────────────────┘
                                     │
                                     ▼
 ┌────────────────────────────────────────────────────────────────────────┐
 │ PHASE 3: Inverted Priority Queue & Consecutive Compaction Engine       │
 │ • Implement Priority Lane with P_max + K relative insertion            │
 │ • Implement O(N log N) integer compaction algorithm                    │
 │ • Implement Starvation Guard (max 8 consecutive priority dispatches)   │
 │ • Full unit & property test suite in crates/spawn-at-core              │
 └───────────────────────────────────┬────────────────────────────────────┘
                                     │
                                     ▼
 ┌────────────────────────────────────────────────────────────────────────┐
 │ PHASE 4: Full D-Bus Orchestration, Driver Trait & Extension Settle     │
 │ • Migrate D-Bus proxy into coordinator (org.spawnat.Coordinator1)      │
 │ • Refactor extension for ArmSpawn(batch), RequestSettle & BatchSettled │
 │ • Implement extension Focus Authority & 100ms Soft-Yield Barrier       │
 │ • Implement --bypass-fifo out-of-band express path                     │
 └────────────────────────────────────────────────────────────────────────┘
```

---

### Phase 1: Standalone Pipe-HUP Barrier & IPC Wire Frames
- **Goal:** Prove the non-blocking barrier broadcast idiom and establish wire protocol primitives without modifying compositor code.
- **Tasks:**
  1. Initialize Cargo Workspace: create `crates/spawn-at-proto`, `crates/spawn-at-core`, `crates/spawn-at-coordinator`, and `crates/spawn-at-cli`.
  2. Implement binary frame framing and serialization in `spawn-at-proto` ([`HELLO`](file:///home/harsh/Documents/GITHUB%20PROJECTS/spawn-at/AI_DEV/spawn-at_master_architecture_blueprint.md#L201), [`ACK`](file:///home/harsh/Documents/GITHUB%20PROJECTS/spawn-at/AI_DEV/spawn-at_master_architecture_blueprint.md#L202), [`GO`](file:///home/harsh/Documents/GITHUB%20PROJECTS/spawn-at/AI_DEV/spawn-at_master_architecture_blueprint.md#L205), [`SPAWNED`](file:///home/harsh/Documents/GITHUB%20PROJECTS/spawn-at/AI_DEV/spawn-at_master_architecture_blueprint.md#L206), [`SETTLED`](file:///home/harsh/Documents/GITHUB%20PROJECTS/spawn-at/AI_DEV/spawn-at_master_architecture_blueprint.md#L207)).
  3. Implement `sendmsg` / `recvmsg` `SCM_RIGHTS` fd-passing in `spawn-at-proto`.
  4. Write a test rig: Coordinator opens `nix::unistd::pipe()`, CLI receives read-end, coordinator closes write-end, verifying all $N$ CLIs wake synchronously via `poll()`.

---

### Phase 2: UDS Coordinator & Rolling Debounce Auto-Grouper
- **Goal:** Enable the coordinator daemon to serialize incoming connections and merge shell `&` bursts.
- **Tasks:**
  1. Build the single-threaded Tokio event loop in `spawn-at-coordinator` listening on `/run/user/$UID/spawn-at/coordinator.sock`.
  2. Implement `SO_PEERCRED` extraction and `/proc/<pid>/stat` parser to securely determine `(uid, SID, PPID)`.
  3. Exclude `gnome-shell`, `systemd --user`, and PID 1 from burst parent keys.
  4. Implement the adaptive fast-path probe: inspect `/proc/<PPID>/task/*/children` for sibling processes. Seal size-1 immediately if 0 siblings exist; otherwise arm the 40ms rolling debounce timer.
  5. Connect `spawn-at-cli` to `coordinator.sock` for solitary and `&`-burst launches.

---

### Phase 3: Inverted Priority Queue & Integer Compaction
- **Goal:** Guarantee that urgent system tools never wait behind slow assembling group layouts.
- **Tasks:**
  1. Implement `PriorityLane` in `crates/spawn-at-core` with $P_{\text{assigned}} = P_{\max} + K$ insertion semantics.
  2. Implement `compact()` algorithm in `spawn-at-core` that preserves total ordering while re-indexing tasks to $[0..N-1]$.
  3. Implement `max_consecutive_priority` starvation guard (dispatching 1 baseline entry after 8 priority dispatches).
  4. Implement Client-Side Flag Authority validator in `spawn-at-cli` (rejecting invalid combinations like `--hold` without `--group-size`, or `--bypass-fifo` with `--priority`).
  5. Add unit and integration tests verifying Order-Preservation Invariant (**I5**) and Compaction Overflow Bounds.

---

### Phase 4: Full D-Bus Orchestration, Driver Trait & Extension Settle Gate
- **Goal:** Complete the 3-tier system by binding the coordinator to the GNOME Shell extension via D-Bus and activating the Focus Authority.
- **Tasks:**
  1. Define `trait Driver` in `crates/spawn-at-coordinator` and implement `GnomeDriver` using `zbus`.
  2. Update GNOME Shell extension to expose:
     - `ArmSpawn(batch)`: Installs atomic batch claims.
     - `RequestSettle(batch_id)`: Async request to coordinator to enter 2D Settle section.
     - `BypassArm(entry)`: Express path registering express claims and raising soft-yield barrier.
  3. Implement the 100ms Soft-Yield Barrier (`bypass_active_until`) in `extension.esm.js` to suppress in-flight group `activate()` calls with deferred focus restoration.
  4. Implement the Lock Split in the coordinator: hold `focus_execution_lock` during 2A (Arm) + 2B (Spawn) and 2D (Settle), leaving 2C (MapAwait) lock-free.
  5. Route `--bypass-fifo` in `spawn-at-cli` directly to `BypassArm` and `Command::spawn()`, bypassing the coordinator entirely.

---

## 5. Architectural Invariant Validation Matrix

| Invariant ID | Name | Architectural Enforcement Point |
| :--- | :--- | :--- |
| **I1** | **No-Spawn** | CLI process blocks on the Pipe-HUP barrier (`poll()`); no `Command::spawn()` occurs until `ACK` and `GO`. |
| **I2** | **Single Writer** | Only the single-threaded coordinator event loop mutates transaction state, queues, and locks. |
| **I3** | **Settle Exclusion** | `focus_execution_lock` is held exclusively during 2A/2B and 2D Settle sections; mirrored by Extension Batch Mutex. |
| **I4** | **First-Claim** | Leadership and immutable contract authority belong strictly to the first decoded frame carrying `--group-size`. |
| **I5** | **Order Preservation** | Relative insertion $P_{\text{assigned}} = P_{\max} + K$ and stable compaction guarantee rank$(a) <$ rank$(b)$ for arrival $a < b$. |
| **I6** | **Bounded Waiting** | Every transaction is guarded by monotonic timer deadlines (`auto_group_window_ms`, `group_timeout_ms`, `hold_timeout_ms`). |
| **I7** | **Fail-Open** | If coordinator or extension crashes, CLI prints a warning and executes raw `Command::spawn()` to guarantee application launch. |
| **I8** | **Cloak Finiteness** | Extension uncloaks window actor on `cloak_max_ms` timeout unconditionally. |
| **I9** | **Claim Finiteness** | Claims expire automatically after `claim_ttl_ms`, preventing accidental hijacking of future windows. |
| **I10** | **Non-Blocking Core** | All socket I/O is asynchronous with bounded buffers; `/proc` reads are single-syscall sized and capped. |

---

## 6. Summary Conclusion

The current repository provides a working foundation for single-window zero-flicker placement on GNOME Wayland via direct D-Bus communication. However, it lacks the multi-window coordination, process barrier parking, shell burst debouncing, priority queueing, and focus arbitration specified in the target architecture. 

Migrating to the **3-tier modular workspace layout** (`spawn-at-proto`, `spawn-at-core`, `spawn-at-coordinator`, `spawn-at-cli`, and updated `extension`) according to the 4-phase vertical-slice roadmap will satisfy all 10 system invariants and eliminate visual tearing, focus races, and launch queue deadlocks.
