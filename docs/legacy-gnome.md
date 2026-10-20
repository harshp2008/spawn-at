# Legacy GNOME (42–44) Differences & Compatibility Analysis

Status: implemented and checked with the test harness only. This page compares source only: the legacy extension has never been run on a real GNOME 42-44 session, and **it may not work at all**.

The installer says so before it installs (see the README, "Support"). The legacy extension is `assets/gnome/legacy/`: 8 JavaScript files and `metadata.json`. The two trees were compared with a read-only `diff` on 2026-10-21.

---

## 1. Why there are two trees

From GNOME Shell 45 on, extensions are ECMAScript modules (ESM). GNOME Shell 42, 43 and 44 need the older GJS `imports` style and a few different Mutter and Clutter calls. These are the versions on Ubuntu 22.04 LTS and Debian 12 (not verified).

So spawn-at keeps two asset trees, and the installer picks one from the Shell's major version:

| Tree | Path | Shell versions | ProtocolVersion | JavaScript files |
| :-- | :-- | :-- | :-- | :-- |
| Modern | `assets/gnome/modern/` | 45, 46, 47, 48 | 6 | 10 |
| Legacy | `assets/gnome/legacy/` | 42, 43, 44 | 5 | 8 |

The legacy tree has no `ready_wait.js` and no `x11_observe.js`.

---

## 2. What differs and what is the same

"Same" means the source text is the same. How either tree behaves on a real GNOME 42 to 44 is not verified.

| What | Modern | Legacy | Same or different |
| :-- | :-- | :-- | :-- |
| Shell versions (`metadata.json`) | 45–48 | 42–44 | different |
| Module syntax | `import` / `export` | `imports.gi`, `Me.imports`; `Me` comes from `ExtensionUtils.getCurrentExtension()` | different |
| Entry point | `export default class ... extends Extension` | `function init()` returns the object | different |
| Class definitions | `export class X` | `var X = class X { ... };` plus a `module.exports` guard for the Node tests | different |
| `ProtocolVersion` | 6 | 5 | different |
| `ResizeOutcome` signal | declared | not declared | different |
| Other D-Bus methods and signals | see section 3 | see section 3 | same |
| Steps of a spawn (the `switch` in `extension.js`) | set size, wait for commit, set position, uncloak | same | same |
| Wait for the window | ready-wait (`ready_wait.js`) | 30 ms quiet rule (`commit.js`) | different |
| Re-anchor just before the reveal | `_reanchorLateResize` | none | different |
| Per-spawn wait override | yes | no | different |
| X11 observation logging | `x11_observe.js` | none | different |
| `cloak.js` logic | line for line the same | line for line the same | same |
| Unmaximize helper `_unmaximize(window)` | `window.unmaximize(Meta.MaximizeFlags.BOTH)`, and `window.unmaximize()` if that throws | same | same |
| Window actor lookup | `window.get_compositor_private()` | same | same |
| D-Bus export | `Gio.DBusExportedObject.wrapJSObject` | same | same |
| Workspace manager | `get_workspace_manager()` on 9 lines; `global.workspace_manager` tried first on 2 | 9 lines; tried first on 7 | different |
| Error and warning output in `dbus.js` and `cloak.js` | `console.error`, `console.warn` | `log()` | different |
| How `Clutter` is found in `cloak.js` | `globalThis.Clutter` | `globalThis.Clutter`, then `imports.gi.Clutter` | different |

The unmaximize helper is called from the same five places in both trees: `_stepSetSize`, `_stepPosition`, `_reanchorIfResized`, and twice in `SetWindowState`. The two `cloak.js` files also differ in comments: the legacy file has fewer, and its header adds "Legacy GNOME 42-44".

---

## 3. Protocol and signals

Both trees implement the same D-Bus interface, `org.gnome.Shell.Extensions.SpawnAt`. The only difference is the protocol-6 `ResizeOutcome` signal, which only the modern tree has.

`ProtocolVersion` is a `uint32` property. What each step added:

| Protocol | What it added | Where it is stated |
| :-- | :-- | :-- |
| ProtocolVersion 4 | `GetLayout` and `--area screen` | `src/platform/capability.rs` |
| ProtocolVersion 5 | monitor and state columns of `GetWindows` | `src/platform/capability.rs` |
| ProtocolVersion 6 | the `ResizeOutcome` signal, modern tree only | the D-Bus XML of the two trees |

**Not verified.** Protocol 3 added `CloseWindow`.

The methods are `ArmSpawn`, `DisarmSpawn`, `ExecuteBatch`, `GetCursor`, `GetPointer`, `GetWorkareas`, `GetLayout`, `GetWindows`, `MoveWindow`, `FocusWindow`, `DefocusWindow`, `SetWindowState`, `CloseWindow` and `SetLogging`.

The signals are `WorkareaChanged`, `SpawnClaimed` and, in the modern tree only, `ResizeOutcome`. `SpawnClaimed` carries `target_id` (string), `success` (bool), `window_id` (uint64), `x` and `y` (int), `w` and `h` (uint), `size_raised` (bool) and `error` (string).

The test `test_legacy_and_modern_dbus_xml_parity` compares the XML of the two trees character for character, after it removes the `ResizeOutcome` signal from the modern one.

### 3.1 Monitor introspection in `GetLayout()`

Three monitor calls differ or are optional across Mutter versions. None of them is verified on GNOME 42–44. Each is guarded, and a missing value is reported as `null`: the extension returns no guessed value.

| Call | Guard |
| :-- | :-- |
| `global.display.get_monitor_scale(index)` | checked for existence; falls back to `null` |
| `global.backend.get_monitor_manager().get_monitors()[i].get_connector()` | wrapped in `_try`; falls back to `null` |
| `Main.layoutManager.monitors[i].connector` | tried first; may be undefined on some legacy setups; falls back to `null` |

### 3.2 Transformation, snapshot gating and animation suppression

The zero-flicker transformation path gates snapshots and suppresses the unminimize animation. Only GNOME Shell 46 was directly inspected for these calls (the comment above `_skipWindowEffects` in the legacy `extension.js` says so). They are not verified live on GNOME 42–44, 45 or 47–48, and the legacy tree relies on guards.

| Call | Guard in the legacy tree | If the call is missing |
| :-- | :-- | :-- |
| `window.is_hidden()` | `typeof window.is_hidden === 'function'`, else `Boolean(window.minimized)` | the `minimized` property still detects a minimized window |
| `window.located_on_workspace(ws)` | `typeof` check, then `window.is_on_all_workspaces()` | workspace filtering passes the window |
| `Main.wm.skipNextEffect(actor)` and `_skippedActors` | `typeof` check, then `Main.wm._skippedActors?.add(actor)`, then the same on `global.window_manager`, then `_clearSkippedEffects` to remove, all inside `try { ... } catch (_e) {}` | only the animation suppression fails; the window is still placed, sized and uncloaked |

**Not verified.** All three calls exist in Mutter 46 and Shell 46 (`windowManager.js` for `skipNextEffect`).

---

## 4. What the installer does

`spawn-at install` reads `gnome-shell --version` and takes the major version.

1. For 42, 43 or 44, before it writes anything it prints a yellow notice. The notice names the detected version (42, 43 or 44):

   ```text
   NOTICE: GNOME Shell 42 uses the legacy extension, which is UNTESTED: it has never been run on a real GNOME 42-44.
   It may not work. Bug reports are welcome: https://github.com/harshp2008/spawn-at/issues
   ```

2. It asks "Install anyway?" (default no). `--yes`, `--headless` or having no terminal skips the question.
3. It deploys `assets/gnome/legacy/` and prints `Deploying legacy GNOME (42-44) extension to: <dir>`.
4. For 45 and up it deploys `assets/gnome/modern/`.

---

## 5. Verification status

| Check | What it covers |
| :-- | :-- |
| Node.js tests | the extension code against mock Mutter, Clutter and D-Bus environments, not a real Shell |
| `cargo test` | the two XML interfaces match, apart from `ResizeOutcome` |
| Live GNOME Shell 46 (Ubuntu 24.04) | the modern tree, verified |
| GNOME 42–44, static analysis and the mock harness | the legacy tree |
| GNOME 42–44 on a physical session or a VM | **not verified** |
