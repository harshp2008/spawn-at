# 04: Execution Work Plan & Decisions

**Project:** `spawn-at`  
**Planned Version:** v0.2.0-beta.1  
**Auditor:** Antigravity (DeepMind Advanced Agentic Coding)  

This work plan outlines the atomic, phased execution path for Stage 2. Execution only begins after explicit user approval ("go"). Every phase defines a strict verification gate that must pass before progressing to subsequent phases.

---

## 1. Decisions Requiring User Approval

Before initiating Stage 2 execution, the following architectural and behavioral decisions require your explicit sign-off:

| # | Topic | Current Behavior | Proposed Target | Impact / Alternatives |
| :-: | :--- | :--- | :--- | :--- |
| **D1** | **`--clamp` Flag Semantics** | Flag parses in CLI (`src/cli.rs:145`), but is completely ignored; windows are always clamped. | Wire `--clamp` to `PlacementParams.clamp`. When `--clamp false`, allow windows to be positioned partially off-screen or overflow workarea boundaries without clamping. | Alternatively: deprecate and remove `--clamp` if all windows must always stay bounded. (Recommendation: wire it to fulfill documented contract). |
| **D2** | **Internal Driver Renaming** | Named `GnomeWaylandDriver` in `src/platform/linux/gnome/mod.rs`, though it drives both GNOME Wayland and GNOME X11. | Rename internal struct to `GnomeDriver` or `GnomeShellDriver`. | Refactor only; zero CLI or D-Bus impact. |
| **D3** | **`install.sh` Default Release Resolution** | Defaults to `/releases/latest`, which excludes pre-releases and silently installs obsolete `v0.1.0`. | During v0.2.0 beta/prerelease cycle, default to querying `/releases` and selecting the newest release (or default `USE_PRERELEASE=true`). | Ensures new users test modern v0.2.0-beta instead of ancient v0.1.0. |
| **D4** | **Extension Modularization Strategy** | Single monolithic 1,899-line file (`extension.esm.js`). | Ship 7 clean ECMAScript modules (`extension.js`, `dbus.js`, `cloak.js`, `commit.js`, `pulse.js`, `anchor.js`, `logger.js`) natively supported in GNOME Shell 45–48. | Installer embeds all 7 files and deploys them to the extension folder. Requires a GNOME Shell session re-login to verify. |
| **D5** | **Removal of Dead Config Settings** | `config.toml` parses `WindowMode`, `AppRule`, and `config.apps` from v0.1 daemon. | Remove these dead structs; preserve only `update` preferences (`auto_check`, `channel`, `notify`). | Refactor only; cleaner config schema. |

---

## 2. Stage 2 Execution Phases & Atomic Commits

### Phase 0: Safety Net & Characterization Test Harness
**Objective:** Establish bulletproof regression baselines before modifying any source code.
- **Git Actions:**
  - Create branch: `git checkout -b refactor/v0.2.0-beta.1`
  - Create tag: `git tag pre-cleanup-beta`
- **Commits:**
  - `test(core): add comprehensive geometry characterization test suite`  
    Exhaustive matrix of 10 Anchors x 5 Pivots, negative offsets, monitor bounds, oversized windows, multi-monitor negative coordinates.
  - `test(gnome): add golden instruction JSON serialization tests`  
    Assert exact JSON payloads produced by `mechanics.rs` for `ArmSpawn` and `ExecuteBatch`.
  - `test(cli): add CLI argument parsing characterization tests`  
    Snapshot CLI flag parsing across `spawn`, `transform`, `focus`, `lifecycle`, and `query`.
  - `test(query): add query JSON schema characterization tests`  
    Assert schema compliance of `query layout --json` and `query windows --json`.
- **Verification Gate 0:** `cargo test --workspace` passes 100% green with 0 failures.

---

### Phase 1: Critical Bug & Security Fixes (S1 & S2)
**Objective:** Resolve all functional bugs, leaks, and security risks with dedicated regression tests.
- **Commits:**
  - `fix(extension): resolve GetWorkareas TypeError on GNOME Shell 45+`  
    Fix `global.workspace_manager` -> `global.display.get_workspace_manager().get_active_workspace()`. [FINDING-02]
  - `fix(extension): prevent window invisibility leaks on extension disable`  
    Track active commit promises; disconnect Clutter/MetaWindow signal handlers and uncloak in `disable()`. Fix premature batch return uncloaking in `_runBatch`. [FINDING-01, FINDING-05]
  - `fix(update): use secure temporary directory for update downloads`  
    Replace `/tmp/spawn-at-update-{tag}` with secure private directory. [FINDING-03]
  - `fix(target): use total_cmp to prevent NaN panic in fuzzy matcher`  
    Replace `.partial_cmp().unwrap()` with `.total_cmp()`. [FINDING-06]
  - `fix(gnome): eliminate zbus connection leak in GnomeWaylandDriver`  
    Store owned connection handle instead of `Box::leak`. [FINDING-07]
  - `fix(platform): use OsStr to prevent path unwrapping panics in elevate and update`  
    Pass `&Path` directly to `Command::arg()`. [FINDING-16, FINDING-17]
  - `fix(cli): eliminate hardcoded X11 driver check in main orchestration`  
    Call `driver.post_spawn()` polymorphically across all drivers. [FINDING-08]
  - `fix(x11): silence unconditional debug logging in X11 driver`  
    Guard 17 `eprintln!` statements behind `SPAWN_AT_DEBUG`. [FINDING-09]
  - `fix(cli): wire --clamp flag to placement parameters and geometry solver`  
    Honor `--clamp <true|false>` in `commands/transform.rs` and `main.rs`. [FINDING-11]
  - `fix(extension): add input array validation and bounds to D-Bus ArmSpawn`  
    Validate `Array.isArray(instructions)`; cap pending arm queue to 64 items; cap `session.log` to 5MB. [FINDING-12, FINDING-13, FINDING-15]
  - `fix(update): deploy newly downloaded extension during self-update`  
    Invoke new binary to deploy extension files during upgrade. [FINDING-18]
- **Verification Gate 1:** `cargo test --workspace` passes; regression tests verify all bug fixes.

---

### Phase 2: Dead & Stale Code Elimination
**Objective:** Strip away obsolete code, redundant files, and duplicate math with compiler/grep proofs.
- **Commits:**
  - `chore(extension): remove redundant dead copy assets/gnome/extension.js`  
    Delete `assets/gnome/extension.js` (compiler only uses `extension.esm.js`). [FINDING-20]
  - `refactor(core): remove trivial re-export shim src/core/mod.rs`  
    Directly use `spawn_at_core`. [FINDING-21]
  - `refactor(config): remove unused v0.1 daemon configuration structs`  
    Remove `WindowMode`, `AppRule`, and `config.apps`. [FINDING-22]
  - `refactor(platform): remove dead daemon methods from dbus and backend trait`  
    Remove `run_daemon`, `sync_windows`, and `CompositorBackend::run_daemon`. [FINDING-23]
  - `refactor(core): remove dead v0.1 calculation helpers in spawn-at-core`  
    Remove `GeometryParams`, `TargetGeometry`, and `calculate()`. [FINDING-24]
  - `refactor(x11): delegate X11 geometry math to spawn-at-core`  
    Remove 100 duplicate lines of `calculate_rect_from_placement`. [FINDING-25]
  - `refactor(gnome): delegate anchor resolution to spawn-at-core in mechanics`  
    Remove duplicate screen anchor math in `mechanics.rs`. [FINDING-26]
- **Verification Gate 2:** `cargo build --workspace`, `cargo test --workspace`, and `cargo machete` all pass cleanly.

---

### Phase 3: Codebase Modularization & File Splits
**Objective:** Decompose oversized files into clean, single-responsibility modules.
- **Step 1 (Moves first):** `git mv` operations to preserve git history.
- **Commits:**
  - `refactor(cli): split cli.rs into mod.rs, args.rs, and tests.rs`  
    Extract schema structs and argument tests into dedicated files. [FINDING-30]
  - `refactor(x11): modularize 1,400-line X11 driver into focused modules`  
    Split into `x11/mod.rs`, `x11/session.rs`, `x11/events.rs`, `x11/query.rs`. [FINDING-30]
  - `refactor(commands): extract dedicated spawn command handler from main.rs`  
    Extract `commands/spawn.rs` to streamline `main.rs` down to ~250 lines.
  - `refactor(extension): decompose extension into 7 focused ESM modules`  
    Split into `extension.js`, `dbus.js`, `cloak.js`, `commit.js`, `pulse.js`, `anchor.js`, `logger.js`. [FINDING-30]
  - `feat(installer): update binary installer to embed and install multi-module extension`  
    Embed all 7 ESM modules and metadata.json in `src/platform/installer.rs`.
- **Verification Gate 3:**
  - `node --check assets/gnome/*.js` passes on all 7 files.
  - `cargo check --workspace` and `cargo test --workspace` pass.
  - **STOP & MANUAL CHECK:** Stop and provide exact live re-login verification steps for GNOME Shell.

---

### Phase 3.5: Truthful Diagnostics, Signal-Driven Claim Notification & Legacy GNOME Port
**Objective:** Truthful diagnostics, fast signal-driven placement confirmation, unmaximization, and platform-independent driver traits.
- **Tasks & Architectural Changes:**
  - **Task 1: SpawnClaimed D-Bus Signal (Option A):**
    - ProtocolVersion bumped to 2. Extension emits `SpawnClaimed(target_id: s, success: b, window_id: t, x: i, y: i, w: u, h: u, size_raised: b, error: s)`.
    - CLI subscribes before calling `arm`, avoiding race conditions on fast-mapping windows.
    - Fast failure reporting: failures emit immediately without waiting out the 2s timeout.
    - Polling fallback on stale extension (`ProtocolVersion < 2`) with informative `INFO` log.
    - `app_id` added to `GetWindows` and used by polling matcher.
    - `gnome-terminal-server` paired with `org.gnome.Terminal` in extension matching.
  - **Task 2: Unmaximize on Explicit Placement:**
    - Unmaximize window before placement when explicit size, pos, or anchor is requested.
    - Settle delay (~200ms) re-check; bounded re-assertion if client re-maximizes after map.
    - Journal evidence analyzed: (a) new window created and restored maximized state; (b) app reused existing window.
    - Reused existing windows: detect when app activates an existing window without creating a new one, print warning, and do not move or resize.
  - **Task 3 & 4: Centralized Diagnostics & Settle Delay:**
    - Unified `src/diagnostics.rs` module with typed `Diagnostic` enum.
    - Both `spawn` and `transform` query settled rect after 200ms delay.
    - Truthful reporting: distinguish between size raised to minimum vs workarea clamping.
  - **Task 5: Platform Independence:**
    - Added `supports_claim_wait`, `prepare_claim_wait`, and `get_window_rect` to `CompositorBackend` trait.
  - **Task 6 & 7: Verification & Smoke Test:**
    - Node tests (signal emission, error status, protocol version, unmaximize + re-assertion).
    - Rust unit tests (subscribe-before-arm ordering, fallback on stale extension, launcher daemon handoff, diagnostics parity).
    - Live smoke test (`scripts/smoke-live.sh`) covering center, top-right, bottom-right, sub-minimum size on terminal, calculator, and text-editor.
  - **Task 8: Legacy GNOME (42-44) Port:**
    - Port extension to legacy GJS imports (`imports.gi.*`).
    - Parity XML and ProtocolVersion 2 test harness.
- **Verification Gate 3.5:** All unit tests, Node tests, and live smoke tests pass within 1px tolerance.

---

### Phase 4: Trait Redesign & Cross-OS Extensibility
**Objective:** Restructure platform abstractions so non-Linux platforms compile and adding drivers is turnkey.
- **Commits:**
  - `refactor(cargo): gate Linux-only dependencies (zbus, x11rb) under target cfg`  
    Configure `[target.'cfg(target_os = "linux")'.dependencies]`.
  - `refactor(platform): introduce BackendCapabilities and streamline CompositorBackend`  
    Redefine `CompositorBackend` with capability reporting and clean defaults. [FINDING-29]
  - `feat(platform): implement UnsupportedBackend for clean cross-compilation`  
    Add `src/platform/unsupported.rs` for non-Linux OS targets.
  - `test(platform): implement mock backend driver conformance test harness`  
    Add automated test suite that validates any `CompositorBackend` implementation.
  - `docs: add comprehensive ADDING_A_BACKEND.md guide`  
    Document step-by-step instructions for adding Hyprland, Sway, KDE, macOS, Windows drivers.
- **Verification Gate 4:** Clean compilation; mock backend conformance test suite passes.

---

### Phase 5: Repository Hygiene, Docs & Final Polish
**Objective:** Clean up repository metadata, CI workflows, and formatting.
- **Commits:**
  - `chore(cargo): add license = "MIT" and package metadata to all manifests`  
    Add license and repository fields to root and core manifests. [FINDING-31]
  - `ci: add GitHub Actions CI workflow for main branch and pull requests`  
    Create `.github/workflows/ci.yml` running test, clippy, and fmt checks. [FINDING-32]
  - `fix(install): update install.sh to resolve beta releases and check checksums`  
    Update release resolution and document hash checking. [FINDING-04, FINDING-19]
  - `docs: update CONTRIBUTING.md and remove stale placeholders in README`  
    Fill in active Ubuntu/GNOME versions and update driver API documentation. [FINDING-34, FINDING-35]
  - `style: apply cargo fmt and resolve all clippy warnings workspace-wide`  
    Eliminate 18 clippy warnings; format entire codebase cleanly.
- **Verification Gate 5:**
  - `cargo fmt --check` exits 0.
  - `cargo clippy --workspace --all-targets -- -D warnings` exits 0.
  - All unit and characterization tests pass.
  - Write `.agent-notes/99_final_report.md`.
