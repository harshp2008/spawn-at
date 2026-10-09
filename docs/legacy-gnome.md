# Legacy GNOME (42–44) Differences & Compatibility Analysis

**Author:** Antigravity (Advanced Agentic Coding)  
**Date:** 2026-10-09  
**Status:** Implemented & Verified with Test Harness (Pending live GNOME 42 VM session)

---

## 1. Executive Summary

Beginning with GNOME Shell 45, the GNOME project migrated its JavaScript runtime from legacy GJS `imports` (SpiderMonkey global object sharing) to standard ECMAScript Modules (ESM). GNOME Shell 42, 43, and 44 (standard on Ubuntu 22.04 LTS and Debian 12) require legacy GJS conventions and specific Mutter/Clutter API calls.

To provide seamless backward compatibility without compromising the clean ESM architecture of modern GNOME (45–48), `spawn-at` maintains two parallel asset trees:
- `assets/gnome/modern/` (GNOME Shell 45, 46, 47, 48) — Native ESM modules.
- `assets/gnome/legacy/` (GNOME Shell 42, 43, 44) — Legacy GJS `imports` architecture.

Both implementations adhere to the **identical D-Bus interface specification (`ProtocolVersion = 2`)**, emit the **identical `SpawnClaimed` D-Bus signal**, and enforce the **identical zero-flicker cloaking and unmaximize pipeline**.

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

## 3. Protocol & Signal Parity Guarantee

Both trees implement the exact same D-Bus contract:
- **Interface:** `org.gnome.Shell.Extensions.SpawnAt`
- **ProtocolVersion:** `2` (uint32 property)
- **Signal:** `SpawnClaimed(target_id: s, success: b, window_id: t, x: i, y: i, w: u, h: u, size_raised: b, error: s)`
- **Methods:** `ArmSpawn`, `DisarmSpawn`, `ExecuteBatch`, `GetCursor`, `GetPointer`, `GetWorkareas`, `GetWindows`, `MoveWindow`, `FocusWindow`, `DefocusWindow`, `SetWindowState`, `SetLogging`.

An automated contract parity test (`test_legacy_and_modern_dbus_xml_parity`) verifies character-for-character equivalence of the XML interface definition.

---

## 4. Installer Version Detection & Warning

When deploying the extension via `spawn-at install`, the installer:
1. Inspects `gnome-shell --version`.
2. Extracts the major version number.
3. If version is 42, 43, or 44:
   - Deploys `assets/gnome/legacy/`.
   - Emits the mandatory experimental notice:
     ```text
     [spawn-at] WARNING: Support for GNOME Shell 42-44 is experimental and has NOT been tested on a real session yet. It may not work at all. Please report problems at https://github.com/harsh/spawn-at/issues.
     ```
4. If version is >= 45:
   - Deploys `assets/gnome/modern/`.

---

## 5. Verification Status

- **Node.js Test Suite:** Fully verified with mock Mutter/Clutter/D-Bus environments.
- **Contract Tests:** Verified interface XML and protocol version parity in `cargo test`.
- **Live Hardware Status:** Verified on live GNOME Shell 46 (Ubuntu 24.04). GNOME 42–44 compatibility has been verified with static analysis and mock harness, but **has NOT yet been verified on a physical GNOME 42 session or VM**.
