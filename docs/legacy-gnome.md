# Legacy GNOME (42–44) Differences & Compatibility Analysis

**Status:** implemented. The legacy extension is `assets/gnome/legacy/` (8 JavaScript files and `metadata.json`); it has not been run on a real GNOME 42-44 session.

**Date:** 2026-10-09  
**Status:** Implemented and checked with the test harness only. **UNTESTED on a real GNOME 42-44 session**: it may not work. The installer says so before it installs (see the README, "Support").

---

## 1. Executive Summary

Beginning with GNOME Shell 45, the GNOME project migrated its JavaScript runtime from legacy GJS `imports` (SpiderMonkey global object sharing) to standard ECMAScript Modules (ESM). GNOME Shell 42, 43, and 44 (standard on Ubuntu 22.04 LTS and Debian 12; not verified) require legacy GJS conventions and specific Mutter/Clutter API calls.

To provide seamless backward compatibility without compromising the clean ESM architecture of modern GNOME (45–48), `spawn-at` maintains two parallel asset trees:
- `assets/gnome/modern/` (GNOME Shell 45, 46, 47, 48) — Native ESM modules.
- `assets/gnome/legacy/` (GNOME Shell 42, 43, 44) — Legacy GJS `imports` architecture.

Both implementations adhere to the same D-Bus interface specification, except that the modern tree reports `ProtocolVersion = 6` and the legacy tree `ProtocolVersion = 5`: protocol 6 adds the `ResizeOutcome` signal (emitted before `SpawnClaimed` for native Wayland windows), which the legacy tree does not have. Both emit the **identical `SpawnClaimed` D-Bus signal** and run the same step sequence for a spawn (set size, wait for commit, set position, uncloak; the `switch` in each `extension.js`). The modern tree also has the ready-wait (`ready_wait.js`), a re-anchor right before the reveal (`_reanchorLateResize`), a per-spawn wait override and X11 observation logging (`x11_observe.js`); the legacy tree has none of these, and its wait is the 30 ms quiet rule only (`commit.js`). A read-only `diff` of the two trees on 2026-10-20 shows this. `cloak.js`: the logic is the same line for line; the files differ only in the module syntax (modern `export class CloakManager`; legacy `var CloakManager = class CloakManager { ... };` and a `module.exports` guard for the Node tests), in comments (the legacy file has fewer, and its header adds "Legacy GNOME 42-44"), in how `Clutter` is found (the legacy file falls back to `imports.gi.Clutter` when `globalThis.Clutter` is not set), and in how the watchdog warning is written (modern `console.warn`, legacy `log`, same message). Unmaximize: the helper `_unmaximize(window)` is the same two lines in both `extension.js` files (`window.unmaximize(Meta.MaximizeFlags.BOTH)`, and `window.unmaximize()` if that throws), called from the same five places in both (`_stepSetSize`, `_stepPosition`, `_reanchorIfResized`, and twice in `SetWindowState`). That compares the source only: how either tree behaves on a real GNOME 42 to 44 is not verified.

---

## 2. Technical API Differences: Modern (45+) vs Legacy (42–44)

| Domain | Modern GNOME (45–48) | Legacy GNOME (42–44) | Adaptation in `spawn-at` |
| :--- | :--- | :--- | :--- |
| **Module System** | Standard ESM (`import x from './x.js'`, `export default`) | GJS `imports` (`imports.gi.*`, `Me.imports.x`) | Legacy files use `const Me = imports.misc.extensionUtils.getCurrentExtension()` and access submodules via `Me.imports.*`. |
| **Extension Lifecycle** | `class Extension extends ExtensionBase` with `export default class` | Top-level functions: `init()`, `enable()`, `disable()` | Legacy `extension.js` exposes `function init()` returning an instance with `enable()` / `disable()`. |
| **Extension Metadata** | `"shell-version": ["45", "46", "47", "48"]` | `"shell-version": ["42", "43", "44"]` | Maintained in separate `metadata.json` files per asset directory. |
| **Workspace Manager** | `global.display.get_workspace_manager()` | `global.workspace_manager` (deprecated in 45) | Legacy checks `global.workspace_manager` fallback to `global.display.get_workspace_manager()`. |
| **Window Actor Retrieval** | `win.get_compositor_private()` / `global.get_window_actors()` | `win.get_compositor_private()` / `global.get_window_actors()` | Identical API surface; safe across 42–48. |
| **D-Bus Export** | `Gio.DBusExportedObject.wrapJSObject(XML, target)` | `Gio.DBusExportedObject.wrapJSObject(XML, target)` | GJS 1.30+ feature; fully supported across 42–48. Identical XML and signature. |
| **Window Unmaximization** | `win.unmaximize(Meta.MaximizeFlags.BOTH)` | `win.unmaximize(Meta.MaximizeFlags.BOTH)` | Identical API; supported across all Mutter versions. |
| **Logging** | `console.log` / `console.error` | `log()` / `logError()` / `console.log` | Legacy uses `log("[spawn-at] ...")` with console fallback. |

---

## 3. Protocol and signals: what matches and what differs

Both trees implement the same D-Bus contract, apart from the protocol-6 `ResizeOutcome` signal that only the modern tree has:
- **Interface:** `org.gnome.Shell.Extensions.SpawnAt`
- **ProtocolVersion:** `6` in the modern tree, `5` in the legacy tree (uint32 property; bumped from 2 to 3 for CloseWindow, 3 to 4 for GetLayout, and 4 to 5 for monitor/state/workspace in GetWindows; 5 to 6 adds the `ResizeOutcome` signal, modern tree only)
- **Signal:** `SpawnClaimed(target_id: s, success: b, window_id: t, x: i, y: i, w: u, h: u, size_raised: b, error: s)`
- **Methods:** `ArmSpawn`, `DisarmSpawn`, `ExecuteBatch`, `GetCursor`, `GetPointer`, `GetWorkareas`, `GetLayout`, `GetWindows`, `MoveWindow`, `FocusWindow`, `DefocusWindow`, `SetWindowState`, `SetLogging`, `CloseWindow`.

### 3.1 Unverified Meta APIs on GNOME 42–44
In `GetLayout()`, several monitor introspection APIs differ or are optional across Mutter versions:
- `global.display.get_monitor_scale(index)`: Marked **UNVERIFIED on GNOME 42–44**. Mutter's monitor scaling introspection API evolved during the GNOME 42 to 45 transition. It is guarded with optional chaining / existence checks and falls back safely to `null`.
- `global.backend.get_monitor_manager().get_monitors()[i].get_connector()`: Marked **UNVERIFIED on GNOME 42–44**. Connector naming via `MetaMonitorManager` is guarded with `_try` and falls back to `null`.
- `Main.layoutManager.monitors[i].connector`: Checked first, but may be undefined on certain legacy configurations. Falls back to `null`.
No guessed values are returned; any unverified or missing introspection data is reported as `null`.

### 3.2 Unverified APIs in Transformation, Snapshot Gating & Animation Suppression

The zero-flicker transformation pipeline introduced gating for snapshot generation and unminimize animation suppression. **Only GNOME Shell 46 was directly inspected on this system; GNOME Shell 42–44, 45, and 47–48 are UNVERIFIED live and rely on defensive checks and graceful degradation.**

| API | Verification Status (GNOME 42–44) | Verification Status (GNOME 45, 47–48) | Inspected Reference (GNOME 46) | Guard & Fallback Behavior |
| :--- | :--- | :--- | :--- | :--- |
| `window.is_hidden()` | **UNVERIFIED on 42–44** | **UNVERIFIED on 45, 47–48** | Present in Mutter 46 (`Meta.Window.is_hidden`). | Guarded via `typeof window.is_hidden === 'function'`. Fallback evaluates `Boolean(window.minimized)`. If absent, minimized state is still detected via the `minimized` property. |
| `window.located_on_workspace(ws)` | **UNVERIFIED on 42–44** | **UNVERIFIED on 45, 47–48** | Present in Mutter 46. | Guarded via `typeof window.located_on_workspace === 'function'`. If absent or returning false, checks `window.is_on_all_workspaces()`. If the method is missing entirely, workspace filtering safely defaults to passing the window. |
| `Main.wm.skipNextEffect(actor)` / `_skippedActors` | **UNVERIFIED on 42–44** | **UNVERIFIED on 45, 47–48** (internal Shell implementation details) | Present in GNOME Shell 46 `windowManager.js` (`skipNextEffect(actor)` backed by `_skippedActors` Set). | Guarded with: (1) `typeof Main.wm.skipNextEffect === 'function'`, (2) fallback to `Main.wm._skippedActors?.add(actor)`, (3) duplicate checks on `global.window_manager`, (4) explicit defensive removal via `_clearSkippedEffects`, and (5) an outer `try { ... } catch (_e) {}`. If missing, **only the animation suppression fails**, and the window is still placed, sized, and uncloaked correctly. |

An automated contract parity test (`test_legacy_and_modern_dbus_xml_parity`) verifies character-for-character equivalence of the XML interface definition.

---

## 4. Installer Version Detection & Warning

When deploying the extension via `spawn-at install`, the installer:
1. Inspects `gnome-shell --version`.
2. Extracts the major version number.
3. If version is 42, 43, or 44:
   - Before anything is written, prints this notice in yellow (text: `src/platform/install_notice.rs:27-29`, printed at `src/main.rs:109`), with 43 or 44 in place of 42 as detected, and asks "Install anyway?" (default no) unless `--yes`, `--headless` or no terminal is used:
     ```text
     NOTICE: GNOME Shell 42 uses the legacy extension, which is UNTESTED: it has never been run on a real GNOME 42-44.
     It may not work. Bug reports are welcome: https://github.com/harshp2008/spawn-at/issues
     ```
   - Deploys `assets/gnome/legacy/` and prints `Deploying legacy GNOME (42-44) extension to: <dir>` (`src/platform/linux/gnome/deploy.rs:333-338`).
4. If version is >= 45:
   - Deploys `assets/gnome/modern/`.

---

## 5. Verification Status

- **Node.js Test Suite:** Fully verified with mock Mutter/Clutter/D-Bus environments.
- **Contract Tests:** Verified in `cargo test` that the interface XML of the two trees is the same, apart from the `ResizeOutcome` signal of protocol 6.
- **Live Hardware Status:** Verified on live GNOME Shell 46 (Ubuntu 24.04). GNOME 42–44 compatibility has been verified with static analysis and mock harness, but **has NOT yet been verified on a physical GNOME 42 session or VM**.
