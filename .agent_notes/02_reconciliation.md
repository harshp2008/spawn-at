# 02: Architectural & Documentation Reconciliation

**Project:** `spawn-at`  
**Current State:** v0.2.0-beta  
**Date:** October 2026  
**Auditor:** Antigravity (DeepMind Advanced Agentic Coding)  

This report audits every significant architectural and technical claim made in the historical planning documents (`AI_DEV/system_survey_and_understanding.md` and `AI_DEV/spawn-at_master_architecture_blueprint.md`), as well as the active `README.md`, evaluating them strictly against the verified ground-truth source code.

---

## 1. Evaluation Legend

- **TRUE:** Verified in the codebase with concrete file and symbol evidence.
- **FALSE:** Contradicts the active codebase; the code operates differently.
- **STALE:** Previously true or intended, but no longer matches current file layout or APIs.
- **NOT BUILT:** Designed in architecture blueprints or roadmap discussions, but zero implementation exists in the current codebase.

---

## 2. Audit of `AI_DEV/system_survey_and_understanding.md`

| Section / Claim | Status | Codebase Evidence & Detailed Findings |
| :--- | :---: | :--- |
| **§1.1: Core Decoupling in Progress**<br>*"Spatial geometry calculation, bounding box math, workarea resolution... now reside cleanly within `crates/spawn-at-core`"* | **TRUE** | `crates/spawn-at-core/src/geometry.rs` and `crates/spawn-at-core/src/driver.rs` contain all pure mathematical placement primitives (`Anchor`, `Pivot`, `PlacementParams`, `Rect`, `Batch`, `Driver`). |
| **§1.2: 2-Tier Direct D-Bus Topology**<br>*"Every CLI invocation connects directly to GNOME Shell via session D-Bus (`zbus`)..."* | **TRUE** | `src/main.rs:347-366` calls `driver.arm(batch).await` directly against `GnomeWaylandDriver` which connects via `zbus::Connection::session()`. |
| **§1.3: Extension Hardening Mechanisms**<br>*"Incorporates `OffscreenRedirect.NEVER`, `after-update` stage safety net, VTE candidate gating via `/proc/<pid>/maps`, allocation early-exit in `_waitForCommit`"* | **TRUE** | Verified in `assets/gnome/extension.esm.js`: `OffscreenRedirect.NEVER` (`lines 205, 1189`), `after-update` listener (`line 716`), `_isVteCandidate` (`line 649`), allocation early-exit (`line 1338`). |
| **§1.4: The X11 Gap**<br>*"On X11 sessions, `platform::linux::x11::X11Driver` is currently a stub returning `DriverError::UnsupportedCapability`... lacks native X11 window property manipulation"* | **FALSE** | **Direct Contradiction:** `src/platform/linux/x11.rs` is a **1,417-line fully-featured native X11 driver** implementing `x11rb`, EWMH (`_NET_MOVERESIZE_WINDOW`), RandR, ICCCM `WM_NORMAL_HINTS` (USPosition/PPosition) pre-map injection, `_NET_WM_USER_TIME = 0`, root `SubstructureNotify` event loop, and startup notification matching. GNOME X11 and standalone X11 work natively! |
| **§1.5: Missing 3-Tier Multi-Window Coordination**<br>*"The Coordinator daemon (`spawn-at-coordinator`), Unix Domain Socket (`coordinator.sock`), kernel Pipe-HUP barrier, rolling burst debouncer... remain unbuilt"* | **NOT BUILT** | Verified. No coordinator daemon, domain socket, or pipe barrier exists in the repo. The architecture is strictly 2-tier. |
| **§2: Repository Layout: `extension.js` vs `extension.esm.js`**<br>*"Lists `extension.js` as CJS/ESM bundle and `extension.esm.js` as ESM extension"* | **STALE** | `cmp` proves `assets/gnome/extension.js` and `assets/gnome/extension.esm.js` are 100% byte-for-byte identical. Only `extension.esm.js` is embedded by Rust (`src/platform/linux/gnome/mod.rs:145` and `src/update.rs:289`). `extension.js` in the tree is an unmaintained duplicate. |
| **§3.1: Session Detection via `loginctl`**<br>*"Session type comes from `loginctl`, not `$XDG_SESSION_TYPE`"* | **TRUE** | `src/platform/linux/gnome/mod.rs:195-223` executes `loginctl show-session <id> -p Type --value` and only falls back to `WAYLAND_DISPLAY`. |

---

## 3. Audit of `AI_DEV/spawn-at_master_architecture_blueprint.md`

| Section / Blueprint Proposal | Status | Implementation Reality & Slotting Assessment |
| :--- | :---: | :--- |
| **§3.1: 3-Tier Coordinator Daemon Topology**<br>*CLI -> Coordinator Daemon (`spawn-at-coordinator`) -> Compositor Driver* | **NOT BUILT** | Current topology is 2-tier (CLI directly arms driver via D-Bus or X11 protocol). Can slot into future architecture via a dedicated daemon crate without altering core geometry math. |
| **§3.2: IPC Protocol Frame & Socket**<br>*Framed binary protocol over `/run/user/<uid>/spawn-at/coordinator.sock`* | **NOT BUILT** | No Unix domain socket IPC exists. All current IPC is D-Bus (`zbus`) or native X11 (`x11rb`). Future protocol can reside in `crates/spawn-at-proto`. |
| **§3.3: Kernel Pipe-HUP Barrier**<br>*Anonymous pipe inheritance across fork/exec for settlement detection* | **NOT BUILT** | Current settlement detection uses event loops: D-Bus `wait_for_spawn` polling on Wayland (`src/commands/mod.rs:105`) and `SubstructureNotify` on X11 (`src/platform/linux/x11.rs:776`). |
| **§3.5: Rolling Burst Debouncer**<br>*Grouping simultaneous commands by `(uid, SID, PPID)`* | **NOT BUILT** | Commands execute independently; the Shell extension provides a FIFO batch mutex queue (`assets/gnome/extension.esm.js:911-932`), but no cross-process CLI grouping exists. |
| **§5.1: Inverted Priority Preemption Queue**<br>*Priority-FIFO queue with integer compaction and starvation guards* | **NOT BUILT** | Scheduling urgency (`Urgency::Normal`, `Urgency::Express`) is defined in `crates/spawn-at-core/src/driver.rs:30-36`, but priority compaction is not implemented. |
| **§7.1: Group Scheduling CLI Flags**<br>*`--group`, `--group-size`, `--sideline`, `--hold`, `--hold-timeout`* | **NOT BUILT** | None of these flags exist in `src/cli.rs`. Current CLI surface only supports single-command spawning (`spawn-at spawn [OPTS] <CMD...>`). |
| **§9.1: Compositor-Side Cloak & Settle Pipeline**<br>*Frame-0 Opacity Cloak, WaitForCommit buffer wait, anchoring, uncloak* | **TRUE** | **Fully Built:** The GNOME extension (`assets/gnome/extension.esm.js:975-998`) implements this exact 4-step pipeline: `SetSize` -> `WaitForCommit` -> `SetPositionAnchored` -> `Uncloak`. |

### Blueprint Concepts Worth Preserving on Long-Term Roadmap
1. **`spawn-at-proto` Crate:** Extracting instruction serialization into a shared crate so future daemons, standalone drivers, and tests share a single protocol schema.
2. **Multi-Window Atomic Batches:** Grouping window placement intents (`Batch { entries: Vec<Entry>, reveal: Reveal::Together }`) remains defined in `spawn-at-core` and should be preserved.
3. **Pipe-HUP Barrier:** Providing a lightweight kernel-level synchronization primitive for child processes without requiring PID polling.

---

## 4. Audit of `README.md`

| Claim in `README.md` | Status | Codebase Evidence & Verification |
| :--- | :---: | :--- |
| **Line 13:** *"The finished driver today is GNOME Shell, on both Wayland and X11."* | **TRUE** | Supported and working via `GnomeWaylandDriver` (which drives the GNOME extension over D-Bus on Wayland and X11) and `X11Driver` (native EWMH). |
| **Line 21:** *`curl .../install.sh \| bash -s -- --prerelease`* | **TRUE / RISK** | Pre-releases are required because `install.sh:158` defaults to `/releases/latest` which excludes prereleases, causing silent installs of ancient v0.1.0 unless `--prerelease` is passed. |
| **Line 112:** *"Session type comes from `loginctl`, not `$XDG_SESSION_TYPE`."* | **TRUE** | Verified in `src/platform/linux/gnome/mod.rs:195-223`. |
| **Line 140-141:** *"Tested on Ubuntu with GNOME Shell, on both Wayland and X11."* | **TRUE** | Ubuntu 24.04 LTS with GNOME Shell 46 tested and functional. |
| **Line 143:** *"Other X11 desktops: Experimental native driver. No cloaking, so you'll see the flash."* | **TRUE** | `src/platform/linux/x11.rs` works on standalone X11 without cloaking. |
| **Line 146:** *"Tested on: Ubuntu `<version>`, GNOME Shell `<version>`"* | **STALE** | Unfilled template placeholders remain in documentation. |
| **Line 155:** *"Next up: `<pick one: Hyprland / Sway / KDE>`"* | **STALE** | Unfilled template placeholders remain in documentation. |
| **Line 212:** *`--clamp <true\|false>` Keep the window inside the area (default true)* | **FALSE** | **Broken Contract:** Flag parses in `src/cli.rs:145` but is completely discarded in `src/commands/transform.rs` and `src/main.rs`. Windows are always clamped regardless of flag value. |
| **Badge (Line 6):** *`License: MIT` badge linking to `LICENSE`* | **TRUE / HYGIENE** | `LICENSE` file exists and is MIT, but `Cargo.toml` in root and core omits `license = "MIT"` field. |

---

## 5. Summary of Discrepancies

1. **X11 Driver Reality vs Documentation:** Previous survey documentation declared X11 an unsupported stub. In reality, a complete 1,417-line native X11 driver exists and works, but suffers from debug log pollution (`eprintln!`), socket reconnect churn, and duplicate geometry math.
2. **Duplicate Extension Source:** `assets/gnome/extension.js` is a completely unreferenced duplicate of `assets/gnome/extension.esm.js`.
3. **Ghost `--clamp` Flag:** Documented and CLI-parsed `--clamp` flag has zero operational effect on window placement.
4. **Stale Contributing Guide:** `CONTRIBUTING.md` documents an obsolete `TargetGeometry` driver API that was replaced by `PlacementParams`.
5. **Blueprint Scope:** The 3-tier daemon, coordinator socket, and priority preemption queue described in `AI_DEV/spawn-at_master_architecture_blueprint.md` are unbuilt roadmap concepts; the active project is a clean 2-tier CLI-to-driver tool.
