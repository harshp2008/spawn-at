# 01: Exhaustive Findings Register

**Project:** `spawn-at`  
**Current State:** v0.2.0-beta  
**Severities:**  
- **S1 (Critical):** Breaks user session, causes security vulnerabilities, crashes, or leaves windows permanently cloaked / invisible.  
- **S2 (Major):** Erroneous behavior in edge cases, resource/memory leaks, performance stalls, or broken abstractions.  
- **S3 (Minor / Maintainability):** Dead/duplicate code, oversized files, misleading names, missing tests, or documentation drift.  

**Categories:**  
- **A:** Logic Bugs & Edge-Case Math  
- **B:** Extension Correctness & Shell Compliance  
- **C:** Dead & Stale Code  
- **D:** Architecture, Structure & Leaky Abstractions  
- **E:** Cross-OS Readiness  
- **F:** Comments & Documentation  
- **G:** Security & Installation  
- **H:** Testing Deficits  
- **I:** Repository Hygiene  

---

## Severity 1: Critical Findings

### [FINDING-01] Window Leaks Invisible State via Signal Handler Leak on Extension Disable
- **Severity:** S1
- **Category:** B (Extension Correctness)
- **Evidence:** [`assets/gnome/extension.esm.js:1347-1355`](file:///home/harsh/Documents/GITHUB%20PROJECTS/spawn-at/assets/gnome/extension.esm.js#L1347-L1355), [`lines 277-280`](file:///home/harsh/Documents/GITHUB%20PROJECTS/spawn-at/assets/gnome/extension.esm.js#L277-L280), [`lines 1306`](file:///home/harsh/Documents/GITHUB%20PROJECTS/spawn-at/assets/gnome/extension.esm.js#L1306)
- **Impact:** When `disable()` is called while `_waitForCommit` is actively waiting for client surface resize, `disable()` clears all timers (`this._timerIds.clear()`). Because the timeout callbacks never fire, `finish()` is never invoked, leaving `actorSigId` (`notify::allocation`) and `windowSigId` (`size-changed`) permanently connected. If that window later resizes, the leaked handler triggers `check()`, which immediately executes `this._cloak(actor)` (line 1306) on a disabled extension, permanently setting `actor.opacity = 0` and rendering the window invisible and unclickable.
- **Proposed Fix:** Track active commit promises in a `Set` on `this`; in `disable()`, iterate and immediately invoke `finish('EXTENSION_DISABLED')` to disconnect signals and uncloak before clearing state.
- **Risk of Fix:** Low.
- **Behavior Change:** Bugfix only.

---

### [FINDING-02] `GetWorkareas` Uses Deprecated `global.workspace_manager` on GNOME Shell 45+
- **Severity:** S3 (Downgraded from S1 after live verification)
- **Category:** B (Extension Correctness)
- **Evidence:** [`assets/gnome/extension.esm.js:1820`](file:///home/harsh/Documents/GITHUB%20PROJECTS/spawn-at/assets/gnome/extension.esm.js#L1820) vs [`line 1831`](file:///home/harsh/Documents/GITHUB%20PROJECTS/spawn-at/assets/gnome/extension.esm.js#L1831)
- **Verification Note:** Tested live with `gdbus call --session --dest org.gnome.Shell --object-path /org/gnome/Shell/Extensions/SpawnAt --method org.gnome.Shell.Extensions.SpawnAt.GetWorkareas`. The call succeeded and returned `('[{"x":0,"y":40,"w":1920,"h":1040}]',)` on GNOME 46 because `global.workspace_manager` remains as a deprecated compatibility alias in Mutter 46. However, line 1820 is deprecated and risks breakage in GNOME 48+.
- **Proposed Fix:** Change line 1820 to `global.display.get_workspace_manager().get_active_workspace()`.
- **Risk of Fix:** None (standard GNOME 45–48 API).
- **Behavior Change:** Deprecation cleanup.

---

### [FINDING-03] Predictable `/tmp` Directory Exposes Users to Insecure Symlink Attacks
- **Severity:** S1
- **Category:** G (Security)
- **Evidence:** [`src/update.rs:230`](file:///home/harsh/Documents/GITHUB%20PROJECTS/spawn-at/src/update.rs#L230)
- **Impact:** `perform_upgrade` creates `PathBuf::from(format!("/tmp/spawn-at-update-{}", release.tag_name))`. On multi-user systems, an unprivileged local attacker can pre-create `/tmp/spawn-at-update-{tag}` or symlink it to arbitrary files, hijacking the downloaded binary or causing arbitrary file corruption when `spawn-at update` is executed.
- **Proposed Fix:** Use `tempfile::Builder::new().prefix("spawn-at-update-").tempdir()` or a secure user-private directory under `XDG_RUNTIME_DIR`.
- **Risk of Fix:** Low.
- **Behavior Change:** Security hardening.

---

### [FINDING-04] Default Bootstrap Installer Silently Downgrades Users to Ancient `v0.1.0`
- **Severity:** S1
- **Category:** G (Security & Install)
- **Evidence:** [`install.sh:158`](file:///home/harsh/Documents/GITHUB%20PROJECTS/spawn-at/install.sh#L158)
- **Impact:** Running `curl -fsSL .../install.sh | bash` queries `https://api.github.com/repos/harshp2008/spawn-at/releases/latest`. GitHub's `/releases/latest` endpoint strictly excludes pre-releases. Because `v0.2.0-alpha` and `v0.2.0-beta` are pre-releases, `/releases/latest` returns `v0.1.0` (which lacks the native X11 driver, modern geometry engine, and hardened extension). Users unknowingly install the obsolete v0.1.0 release.
- **Proposed Fix:** When no stable release >= 0.2.0 exists, have `install.sh` query `/releases` and select the newest release (or default `USE_PRERELEASE=true` during the beta phase).
- **Risk of Fix:** Low.
- **Behavior Change:** Bugfix for release distribution.

---

### [FINDING-05] `_runBatch` Finally Block Prevents Uncloaking on Premature Batch Termination
- **Severity:** S1
- **Category:** B (Extension Correctness)
- **Evidence:** [`assets/gnome/extension.esm.js:977, 994-996`](file:///home/harsh/Documents/GITHUB%20PROJECTS/spawn-at/assets/gnome/extension.esm.js#L977)
- **Impact:** Line 994 checks `const wantsHidden = ops.some(op => op.name === 'Cloak'); if (!ctx.revealed && !wantsHidden) this._uncloak(actor);`. Since every normal spawn batch contains a `Cloak` instruction, `wantsHidden` is always `true`. If `this._disabled` triggers early return at line 977, `_runBatch` exits without throwing an exception (bypassing `_processQueue` catch block), and the finally block skips `this._uncloak(actor)`, leaving the window cloaked at opacity 0 forever.
- **Proposed Fix:** Ensure the `finally` block uncloaks if `this._disabled || !ctx.revealed`, removing the faulty `!wantsHidden` condition.
- **Risk of Fix:** Low.
- **Behavior Change:** Bugfix ensuring no-orphan-cloak guarantee.

---

## Severity 2: Major Findings

### [FINDING-06] Unhandled Floating-Point NaN Comparison Causes Panic in Fuzzy Target Matcher
- **Severity:** S2
- **Category:** A (Logic Bugs)
- **Evidence:** [`src/target.rs:144`](file:///home/harsh/Documents/GITHUB%20PROJECTS/spawn-at/src/target.rs#L144)
- **Impact:** `suggestions.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap())` panics if any score comparison evaluates to `None` (e.g. NaN). This crashes the CLI process with a thread panic instead of returning a formatted error message.
- **Proposed Fix:** Replace `.unwrap()` with `.unwrap_or(std::cmp::Ordering::Equal)` or use `b.0.total_cmp(&a.0)`.
- **Risk of Fix:** None.
- **Behavior Change:** Robustness fix.

---

### [FINDING-07] D-Bus Session Connection Leak in `GnomeWaylandDriver::new`
- **Severity:** S2
- **Category:** B (Extension / IPC)
- **Evidence:** [`src/platform/linux/gnome/mod.rs:279`](file:///home/harsh/Documents/GITHUB%20PROJECTS/spawn-at/src/platform/linux/gnome/mod.rs#L279)
- **Impact:** `let static_conn: &'static zbus::Connection = Box::leak(Box::new(connection));` permanently leaks heap memory and socket file descriptors every time a driver instance is created.
- **Proposed Fix:** Store an owned `zbus::Connection` in `GnomeWaylandDriver` or use `zbus::Proxy::new(conn)` with owned handle.
- **Risk of Fix:** Low.
- **Behavior Change:** Memory leak fix.

---

### [FINDING-08] Leaky Backend Abstraction in `main.rs` via Hardcoded `"X11"` Driver Check
- **Severity:** S2
- **Category:** D (Architecture & Leaky Abstractions)
- **Evidence:** [`src/main.rs:378-383`](file:///home/harsh/Documents/GITHUB%20PROJECTS/spawn-at/src/main.rs#L378-L383)
- **Impact:** `if driver.name() == "X11"` hardcodes X11 dispatch in `main.rs`, bypassing polymorphic trait dispatch. Future backends needing post-spawn hooks (e.g. Sway, Hyprland, Windows) will fail to execute `post_spawn`. Additionally, an unconditional debug line `eprintln!("[spawn-at-main] Detected X11 backend...")` is emitted to stderr.
- **Proposed Fix:** Call `driver.post_spawn(child.id(), &batch).await` unconditionally on all backends. Default trait implementation returns `Ok(())`. Remove debug `eprintln!`.
- **Risk of Fix:** Low.
- **Behavior Change:** Clean abstraction; eliminates noisy stderr output.

---

### [FINDING-09] 17 Unconditional Debug `eprintln!` Statements in Native X11 Driver
- **Severity:** S2
- **Category:** A (Bugs) / D (Structure)
- **Evidence:** [`src/platform/linux/x11.rs:259, 315, 351, 592, 672, 721, 761, 776, 796, 814, 833, 847, 864, 874, 972, 976, 982`](file:///home/harsh/Documents/GITHUB%20PROJECTS/spawn-at/src/platform/linux/x11.rs#L259)
- **Impact:** Running `spawn-at query layout --json` or `spawn-at spawn` on X11 outputs dozens of debug logs to stderr (e.g. `[spawn-at-x11] Querying monitors via RANDR protocol...`), corrupting terminal pipelines expecting clean error output.
- **Proposed Fix:** Guard all debug logging behind an environment variable check (e.g. `SPAWN_AT_DEBUG`) or standard `tracing`/`log` macros.
- **Risk of Fix:** Low.
- **Behavior Change:** Clean CLI stderr output.

---

### [FINDING-10] Native X11 Driver Reconnects to X Server on Every Method Call
- **Severity:** S2
- **Category:** A (Performance / Logic)
- **Evidence:** [`src/platform/linux/x11.rs:1052, 1062, 1072, 1089, 1106, 1125, 1137`](file:///home/harsh/Documents/GITHUB%20PROJECTS/spawn-at/src/platform/linux/x11.rs#L1052)
- **Impact:** Every query and transformation method opens a new socket connection to the X11 server via `X11Session::connect()?`, performs atom resolution, executes one request, and drops the socket. This incurs substantial round-trip latency and socket overhead.
- **Proposed Fix:** Allow `X11Driver` to maintain an initialized `RustConnection` and cached atom table across calls, or share an active session.
- **Risk of Fix:** Medium (requires thread safety / synchronization in async context).
- **Execution Decision:** **DEFERRED to post-beta.** Rationale: `RustConnection` is not `Sync` and queries complete in sub-millisecond local Unix sockets. Refactoring connection sharing into multi-threaded async tasks adds mutex/channel complexity without changing user-visible behavior during this cleanup beta.
- **Behavior Change:** Latency reduction.

---

### [FINDING-11] `--clamp` Flag Parses and Tests in CLI but Does Nothing
- **Severity:** S2
- **Category:** A (Bugs) / C (Dead Code)
- **Evidence:** [`src/cli.rs:145`](file:///home/harsh/Documents/GITHUB%20PROJECTS/spawn-at/src/cli.rs#L145), [`src/commands/transform.rs:70-84`](file:///home/harsh/Documents/GITHUB%20PROJECTS/spawn-at/src/commands/transform.rs#L70-L84), [`src/main.rs:276-290`](file:///home/harsh/Documents/GITHUB%20PROJECTS/spawn-at/src/main.rs#L276-L290)
- **Impact:** `--clamp <true|false>` is documented in `README.md:212` and parsed into `GeometryArgs.clamp`, but is completely ignored when instantiating `PlacementParams`. Users passing `--clamp false` still have their windows forcibly clamped.
- **Proposed Fix:** Add `clamp: bool` to `PlacementParams` (default true) and respect it in `calculate_rect`.
- **Risk of Fix:** Low.
- **Behavior Change:** Fulfills documented feature contract.

---

### [FINDING-12] `ArmSpawn` D-Bus Input Lacks JSON Array and Type Validation
- **Severity:** S2
- **Category:** B (Extension Correctness & Security)
- **Evidence:** [`assets/gnome/extension.esm.js:1751-1761`](file:///home/harsh/Documents/GITHUB%20PROJECTS/spawn-at/assets/gnome/extension.esm.js#L1751-L1761)
- **Impact:** Calling `ArmSpawn("target", "{}")` or passing non-array JSON succeeds `JSON.parse`, but when the window maps, `ops.some` and `for (const op of ops)` throw an uncaught TypeError inside Mutter's main thread.
- **Proposed Fix:** Add strict validation: `if (!Array.isArray(instructions)) throw new Error("Instructions must be an array");`.
- **Risk of Fix:** Low.
- **Behavior Change:** Input validation hardening.

---

### [FINDING-13] Unbounded Memory Growth in `_armedSpawns` Map
- **Severity:** S2
- **Category:** B (Extension Correctness)
- **Evidence:** [`assets/gnome/extension.esm.js:1774-1776`](file:///home/harsh/Documents/GITHUB%20PROJECTS/spawn-at/assets/gnome/extension.esm.js#L1774-L1776)
- **Impact:** `_armedSpawns` has no maximum entry limit. Malicious or misconfigured processes can flood the D-Bus interface with arbitrary target IDs, consuming unbounded GNOME Shell heap memory.
- **Proposed Fix:** Enforce a maximum queue cap (e.g., 64 pending arms) and drop oldest with warning.
- **Risk of Fix:** Low.
- **Behavior Change:** DoS protection.

---

### [FINDING-14] Synchronous `/proc` File I/O in Mutter Main Thread Violates GNOME Guidelines
- **Severity:** S2
- **Category:** B (Extension Correctness & Compliance)
- **Evidence:** [`assets/gnome/extension.esm.js:657, 667`](file:///home/harsh/Documents/GITHUB%20PROJECTS/spawn-at/assets/gnome/extension.esm.js#L657)
- **Impact:** `GLib.file_get_contents('/proc/' + pid + '/maps')` is called synchronously on the compositor's main thread on every window map event. If `/proc` stalls or if an app maps large address spaces, the GNOME Shell frame drops or hangs.
- **Proposed Fix:** Retain VTE detection logic, but add per-PID Map caching (`this._vtePidCache = new Map()`) so `/proc/<pid>/maps` is read at most once per process lifetime. Prune cache on window destruction.
- **Risk of Fix:** Low.
- **Execution Decision:** **SCHEDULED for Phase 1.** Keep VTE detection with per-PID caching.
- **Behavior Change:** Performance hardening.

---

### [FINDING-15] Unbounded Session Log File Growth During Long Sessions
- **Severity:** S2
- **Category:** B (Extension Correctness)
- **Evidence:** [`assets/gnome/extension.esm.js:325-410`](file:///home/harsh/Documents/GITHUB%20PROJECTS/spawn-at/assets/gnome/extension.esm.js#L325-L410)
- **Impact:** `session.log` is only rotated when `/run/user/<uid>/spawn-at-session.marker` is absent (on login/reboot). Users with long uptimes accumulate hundreds of megabytes in `~/.local/state/spawn-at/session.log`.
- **Proposed Fix:** Check file size before writing; rotate if size exceeds 5 MB.
- **Risk of Fix:** Low.
- **Behavior Change:** Resource management fix.

---

### [FINDING-16] Unsafe `.to_str().unwrap()` Panics on Non-UTF-8 Paths in Privilege Escalation
- **Severity:** S2
- **Category:** A (Bugs) / G (Security)
- **Evidence:** [`src/platform/escalate.rs:15, 31, 62`](file:///home/harsh/Documents/GITHUB%20PROJECTS/spawn-at/src/platform/escalate.rs#L15)
- **Impact:** Passing file paths with non-UTF-8 bytes causes thread panics.
- **Proposed Fix:** Pass `&Path` or `&OsStr` directly to `Command::arg()`, which natively supports non-UTF-8 paths without conversion.
- **Risk of Fix:** Low.
- **Behavior Change:** Robustness fix.

---

### [FINDING-17] Unsafe Path Conversions in Update Manager
- **Severity:** S2
- **Category:** A (Bugs) / G (Security)
- **Evidence:** [`src/update.rs:238, 248`](file:///home/harsh/Documents/GITHUB%20PROJECTS/spawn-at/src/update.rs#L238)
- **Impact:** `tar_path.to_str().unwrap()` and `tmp_dir.to_str().unwrap()` panic if paths contain non-UTF-8 characters.
- **Proposed Fix:** Pass `&Path` directly to `Command::arg()`.
- **Risk of Fix:** Low.
- **Behavior Change:** Robustness fix.

---

### [FINDING-18] Self-Updater Deploys Old Compiled-in Extension Instead of New Release
- **Severity:** S2
- **Category:** G (Security & Install) / A (Bugs)
- **Evidence:** [`src/update.rs:289-291`](file:///home/harsh/Documents/GITHUB%20PROJECTS/spawn-at/src/update.rs#L289-L291)
- **Impact:** In `perform_upgrade`, after copying `new_binary` into place, the update logic writes `include_str!("../assets/gnome/extension.esm.js")` from the *running (old)* process to disk. The newly downloaded extension is ignored!
- **Proposed Fix:** After installing the new binary, invoke `new_binary install --extension-only --headless` to write the new extension files.
- **Risk of Fix:** Low.
- **Behavior Change:** Bugfix for updater.

---

### [FINDING-19] Installer Downloads Archive Without Checksum Verification
- **Severity:** S2
- **Category:** G (Security)
- **Evidence:** [`install.sh:181`](file:///home/harsh/Documents/GITHUB%20PROJECTS/spawn-at/install.sh#L181), [`src/update.rs:237`](file:///home/harsh/Documents/GITHUB%20PROJECTS/spawn-at/src/update.rs#L237)
- **Impact:** Both `install.sh` and `spawn-at update` download release archives without verifying SHA256 checksums or cryptographic signatures, exposing users to MITM or CDN tampering.
- **Proposed Fix:** Publish `SHA256SUMS` with GitHub releases and verify archive hash before extraction.
- **Risk of Fix:** Low.
- **Behavior Change:** Security hardening.

---

## Severity 3: Minor & Maintainability Findings

### [FINDING-20] 1,899-Line Dead Duplicate `assets/gnome/extension.js`
- **Severity:** S3
- **Category:** C (Dead Code)
- **Evidence:** [`assets/gnome/extension.js`](file:///home/harsh/Documents/GITHUB%20PROJECTS/spawn-at/assets/gnome/extension.js) vs [`assets/gnome/extension.esm.js`](file:///home/harsh/Documents/GITHUB%20PROJECTS/spawn-at/assets/gnome/extension.esm.js)
- **Impact:** `cmp` proves both files are 100% byte-for-byte identical. Rust embeds `extension.esm.js` and writes it to disk as `extension.js`. The tracked `extension.js` in git is completely redundant.
- **Proposed Fix:** Delete `assets/gnome/extension.js`.
- **Risk of Fix:** None.
- **Behavior Change:** Refactor only.

---

### [FINDING-21] Trivial 2-Line Re-Export Shim `src/core/mod.rs`
- **Severity:** S3
- **Category:** C (Dead Code) / D (Structure)
- **Evidence:** [`src/core/mod.rs:1-3`](file:///home/harsh/Documents/GITHUB%20PROJECTS/spawn-at/src/core/mod.rs#L1-L3)
- **Impact:** Contains only `pub use spawn_at_core::driver; pub use spawn_at_core::geometry;`. Creates unnecessary module indirection.
- **Proposed Fix:** Remove `src/core/mod.rs` and import directly from `spawn_at_core`.
- **Risk of Fix:** None.
- **Behavior Change:** Refactor only.

---

### [FINDING-22] Dead v0.1 Daemon Configuration Leftovers in `config.rs`
- **Severity:** S3
- **Category:** C (Dead Code)
- **Evidence:** [`src/config.rs:14-25, 30`](file:///home/harsh/Documents/GITHUB%20PROJECTS/spawn-at/src/config.rs#L14-L25)
- **Impact:** `WindowMode`, `AppRule`, and `config.apps` are remnants of the removed v0.1 daemon and are never used in active runtime.
- **Proposed Fix:** Remove `WindowMode`, `AppRule`, and `apps` field from `Config`.
- **Risk of Fix:** None.
- **Behavior Change:** Refactor only.

---

### [FINDING-23] Dead Background Daemon Code in `dbus.rs` and `CompositorBackend`
- **Severity:** S3
- **Category:** C (Dead Code)
- **Evidence:** [`src/platform/linux/gnome/dbus.rs:29-110`](file:///home/harsh/Documents/GITHUB%20PROJECTS/spawn-at/src/platform/linux/gnome/dbus.rs#L29-L110), [`src/platform/mod.rs:175`](file:///home/harsh/Documents/GITHUB%20PROJECTS/spawn-at/src/platform/mod.rs#L175)
- **Impact:** `run_daemon` and `sync_windows` are completely dead functions never invoked by CLI or commands.
- **Proposed Fix:** Remove `run_daemon`, `sync_windows`, and the `run_daemon` method from `CompositorBackend`.
- **Risk of Fix:** None.
- **Behavior Change:** Refactor only.

---

### [FINDING-24] Dead v0.1 Calculation Functions and Types in `spawn-at-core`
- **Severity:** S3
- **Category:** C (Dead Code)
- **Evidence:** [`crates/spawn-at-core/src/geometry.rs:83-113, 291-349`](file:///home/harsh/Documents/GITHUB%20PROJECTS/spawn-at/crates/spawn-at-core/src/geometry.rs#L83-L113)
- **Impact:** `GeometryParams`, `TargetGeometry`, and `calculate()` are unused outside legacy tests.
- **Proposed Fix:** Deprecate or remove unused v0.1 calculation helpers and unify onto `PlacementParams` / `calculate_rect`.
- **Risk of Fix:** Low.
- **Behavior Change:** Refactor only.

---

### [FINDING-25] ~100 Lines of Duplicate Geometry Math in Native X11 Driver
- **Severity:** S3
- **Category:** D (Structure / Duplication)
- **Evidence:** [`src/platform/linux/x11.rs:86-180`](file:///home/harsh/Documents/GITHUB%20PROJECTS/spawn-at/src/platform/linux/x11.rs#L86-L180)
- **Impact:** `calculate_rect_from_placement` duplicates `spawn-at-core::geometry::calculate_rect` line-for-line.
- **Proposed Fix:** Delegate directly to `spawn_at_core::geometry::calculate_rect`.
- **Risk of Fix:** Low.
- **Behavior Change:** Refactor only.

---

### [FINDING-26] Duplicate Anchor Calculation Logic in `mechanics.rs`
- **Severity:** S3
- **Category:** D (Structure / Duplication)
- **Evidence:** [`src/platform/linux/gnome/mechanics.rs:84-135`](file:///home/harsh/Documents/GITHUB%20PROJECTS/spawn-at/src/platform/linux/gnome/mechanics.rs#L84-L135)
- **Impact:** `screen_anchor_x` and `screen_anchor_y` calculation duplicates logic from `spawn-at-core`.
- **Proposed Fix:** Delegate anchor resolution to `spawn-at-core`.
- **Risk of Fix:** Low.
- **Behavior Change:** Refactor only.

---

### [FINDING-27] Misleading Function Names Claiming External Utility Execution
- **Severity:** S3
- **Category:** D (Naming That Lies)
- **Evidence:** [`src/platform/linux/mod.rs:12, 44`](file:///home/harsh/Documents/GITHUB%20PROJECTS/spawn-at/src/platform/linux/mod.rs#L12)
- **Impact:** Functions named `query_xrandr_monitors` and `query_xdotool_cursor` do not invoke external tools; they talk directly to `x11rb`.
- **Proposed Fix:** Rename to `query_x11_monitors` and `query_x11_cursor`.
- **Risk of Fix:** None.
- **Execution Decision:** **SCHEDULED for Phase 3/4.** Clean internal rename.
- **Behavior Change:** Refactor only.

---

### [FINDING-28] Misleading Driver Name `GnomeWaylandDriver` Handles GNOME X11
- **Severity:** S3
- **Category:** D (Naming That Lies)
- **Evidence:** [`src/platform/linux/gnome/mod.rs:269`](file:///home/harsh/Documents/GITHUB%20PROJECTS/spawn-at/src/platform/linux/gnome/mod.rs#L269)
- **Impact:** The driver is named `GnomeWaylandDriver`, but it handles both GNOME on Wayland and GNOME on X11 (via the Shell extension).
- **Proposed Fix:** Rename to `GnomeDriver` or `GnomeShellDriver`.
- **Risk of Fix:** Low.
- **Behavior Change:** Refactor only.

---

### [FINDING-29] `CompositorBackend` Trait is Too Wide (17 Methods)
- **Severity:** S3
- **Category:** D (Structure) / E (Cross-OS Readiness)
- **Evidence:** [`src/platform/mod.rs:75-178`](file:///home/harsh/Documents/GITHUB%20PROJECTS/spawn-at/src/platform/mod.rs#L75-L178)
- **Impact:** Implementing a new driver requires implementing or stubbing 17 disparate methods across installation, daemon loops, window transformations, and state queries.
- **Proposed Fix:** Split into core placement interface (`Driver` + `WindowPlacementBackend`) and capability traits (`WindowLifecycle`, `WindowTransform`), or provide default no-op methods with a clean capability query.
- **Risk of Fix:** Low.
- **Behavior Change:** Refactor only.

---

### [FINDING-30] Oversized Monolithic Files Exceeding Recommended Limits
- **Severity:** S3
- **Category:** D (Structure)
- **Evidence:**
  - `assets/gnome/extension.esm.js`: 1,899 lines (Limit: ~500 lines)
  - `src/platform/linux/x11.rs`: 1,417 lines (Limit: ~400 lines)
  - `src/cli.rs`: 916 lines (500 lines are inline tests)
  - `src/platform/linux/gnome/mod.rs`: 803 lines
- **Impact:** Poor maintainability, difficult review, tight coupling.
- **Proposed Fix:** Split JS into ESM modules; split `x11.rs` into `x11/connection.rs`, `x11/events.rs`, `x11/query.rs`; extract CLI tests to separate test module.
- **Risk of Fix:** Low.
- **Behavior Change:** Refactor only.

---

### [FINDING-31] Missing License Field in All `Cargo.toml` Files
- **Severity:** S3
- **Category:** I (Repository Hygiene)
- **Evidence:** [`Cargo.toml:1-5`](file:///home/harsh/Documents/GITHUB%20PROJECTS/spawn-at/Cargo.toml#L1-L5), [`crates/spawn-at-core/Cargo.toml:1-6`](file:///home/harsh/Documents/GITHUB%20PROJECTS/spawn-at/crates/spawn-at-core/Cargo.toml#L1-L6)
- **Impact:** Neither crate specifies `license = "MIT"`, despite README and LICENSE declaring MIT.
- **Proposed Fix:** Add `license = "MIT"` to both manifests.
- **Risk of Fix:** None.
- **Behavior Change:** Repo hygiene.

---

### [FINDING-32] Missing CI Workflow for Commits and PRs on `main`
- **Severity:** S3
- **Category:** I (Repository Hygiene)
- **Evidence:** [`.github/workflows/release.yml:3-7`](file:///home/harsh/Documents/GITHUB%20PROJECTS/spawn-at/.github/workflows/release.yml#L3-L7)
- **Impact:** CI only runs when pushing tags matching `v*`. Pull requests and commits to `main` have no automated testing, clippy, or formatting verification.
- **Proposed Fix:** Add a standard `ci.yml` workflow triggering on push and pull_request against `main`.
- **Risk of Fix:** Low.
- **Behavior Change:** CI infrastructure.

---

### [FINDING-33] Version Numbers Mismatched Across Codebase
- **Severity:** S3
- **Category:** I (Repository Hygiene)
- **Evidence:** `Cargo.toml` declares `0.1.0`, git tag is `v0.2.0-alpha`, documentation refers to `0.2.0-beta`.
- **Impact:** Inconsistent release metadata.
- **Proposed Fix:** Align versions to `0.2.0-beta.1` as planned for Stage 2.
- **Risk of Fix:** Low.
- **Execution Decision:** **SCHEDULED for Phase 5.** Bump Cargo manifests and extension metadata.json to `0.2.0-beta.1`.
- **Behavior Change:** Version metadata.

---

### [FINDING-34] Stale Placeholders in `README.md`
- **Severity:** S3
- **Category:** F (Documentation)
- **Evidence:** [`README.md:15, 81, 146, 155`](file:///home/harsh/Documents/GITHUB%20PROJECTS/spawn-at/README.md#L146)
- **Impact:** Contains unpopulated template tags: `Ubuntu <version>`, `GNOME Shell <version>`, `<!-- TODO: demo gif -->`, `<pick one: Hyprland / Sway / KDE>`.
- **Proposed Fix:** Replace with verified host environment data (Ubuntu 24.04 LTS, GNOME Shell 46).
- **Risk of Fix:** None.
- **Behavior Change:** Documentation accuracy.

---

### [FINDING-35] Stale Architectural API in `CONTRIBUTING.md`
- **Severity:** S3
- **Category:** F (Documentation)
- **Evidence:** [`CONTRIBUTING.md:51, 99, 119`](file:///home/harsh/Documents/GITHUB%20PROJECTS/spawn-at/CONTRIBUTING.md#L51)
- **Impact:** Claims drivers receive `geom: &TargetGeometry`, which was deprecated in favor of `PlacementParams`.
- **Proposed Fix:** Update `CONTRIBUTING.md` to reflect `PlacementParams`.
- **Risk of Fix:** None.
- **Behavior Change:** Documentation accuracy.

---

### [FINDING-36] Geometry Unit Testing Gaps
- **Severity:** S3
- **Category:** H (Testing Deficits)
- **Evidence:** [`crates/spawn-at-core/src/geometry.rs:514-550`](file:///home/harsh/Documents/GITHUB%20PROJECTS/spawn-at/crates/spawn-at-core/src/geometry.rs#L514-L550)
- **Impact:** Unit tests only test 8 basic corner anchors. No tests exist for full 10x5 Anchor x Pivot cross-matrix, negative offsets, windows larger than monitor workareas, or multi-monitor negative coordinate layouts.
- **Proposed Fix:** Add exhaustive characterization test matrix.
- **Risk of Fix:** Low.
- **Behavior Change:** Testing improvement.

---

### [FINDING-37] Missing D-Bus Instruction JSON Golden File Tests
- **Severity:** S3
- **Category:** H (Testing Deficits)
- **Evidence:** [`src/platform/linux/gnome/mechanics.rs`](file:///home/harsh/Documents/GITHUB%20PROJECTS/spawn-at/src/platform/linux/gnome/mechanics.rs)
- **Impact:** The JSON serialization contract between Rust and the GNOME Shell extension has no golden schema tests, risking accidental breaking changes during refactors.
- **Proposed Fix:** Add golden JSON tests asserting serialized instruction batches.
- **Risk of Fix:** Low.
- **Behavior Change:** Regression prevention.
