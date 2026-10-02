/**
 * Spawn-At GNOME Shell Extension
 *
 * ARCHITECTURAL OVERVIEW & THE "WHY" BEHIND OPACITY CLOAKING ON WAYLAND:
 *
 * 1. The Wayland Security & Isolation Model:
 *    Under Wayland, unlike X11, client applications are explicitly barred from
 *    querying global display coordinates or setting their own absolute window
 *    positions. The compositor (Mutter in GNOME) maintains exclusive ownership
 *    of window placement. As a result, external CLI tools cannot move or position
 *    Wayland windows via standard protocols.
 *
 * 2. Why Traditional Post-Launch Repositioning Fails (The "Flash-and-Jump" Bug):
 *    In naive window positioning approaches, a CLI launches the application and
 *    polls window lists until the window appears, then sends a move/resize request.
 *    By the time the move request reaches Mutter, the window has already been:
 *      (a) Mapped into the Clutter scene graph,
 *      (b) Rendered at Mutter's default placement (often centered or cascading),
 *      (c) Flushed to the display hardware (KMS / DRM plane).
 *    When the resize/move takes effect several frames later, the user perceives an
 *    annoying, jarring flicker or visual jump.
 *
 * 3. The Zero-Flicker "Opacity Cloaking" Solution:
 *    This extension hooks directly into GNOME Shell's compositor internals via two
 *    critical events:
 *      - `window-created`: Triggered immediately when Mutter allocates a `MetaWindow`
 *        for a new Wayland toplevel surface, before any scene graph actor is mapped.
 *        We inspect the window's `wm_class` / `app_id` and match it with pending
 *        spawn targets registered via our D-Bus `Arm` method.
 *      - `map` (on `global.window_manager`): Triggered when Mutter constructs the
 *        `MetaWindowActor` (a ClutterActor) to begin displaying the window.
 *        Synchronously, within the very first execution turn of `map`, we set:
 *          `actor.opacity = 0`
 *        This ensures Mutter renders the window as completely transparent during the
 *        initial Wayland client `xdg_surface.configure` handshake.
 *
 * 4. Compositor Thread Timing & Frame Rect Verification:
 *    When `window.move_resize_frame()` is invoked, Wayland clients process the
 *    configure event asynchronously. The compositor might take 1 to 3 frame cycles
 *    (16ms-50ms) to negotiate buffer dimensions with the client.
 *    We periodically poll `window.get_frame_rect()`. Only when the actual rendered
 *    frame rectangle satisfies the target coordinates (or when dimension clamping is
 *    detected or a safe timeout threshold expires), we restore `actor.opacity = 255`.
 *    Result: The window appears instantly and atomically at the exact target location!
 */

import { Extension } from 'resource:///org/gnome/shell/extensions/extension.js';
import Gio from 'gi://Gio';
import Meta from 'gi://Meta';
import GLib from 'gi://GLib';

const DBUS_IFACE = `
<node>
  <interface name="org.gnome.Shell.Extensions.SpawnAt">
    <method name="Arm">
      <arg type="s" name="identifier" direction="in"/>
      <arg type="i" name="x" direction="in"/>
      <arg type="i" name="y" direction="in"/>
      <arg type="i" name="w" direction="in"/>
      <arg type="i" name="h" direction="in"/>
      <arg type="i" name="min_x" direction="in"/>
      <arg type="i" name="max_x" direction="in"/>
      <arg type="i" name="min_y" direction="in"/>
      <arg type="i" name="max_y" direction="in"/>
    </method>
    <method name="GetCursor">
      <arg type="i" name="x" direction="out"/>
      <arg type="i" name="y" direction="out"/>
    </method>
    <method name="GetPointer">
      <arg type="i" name="x" direction="out"/>
      <arg type="i" name="y" direction="out"/>
    </method>
    <method name="GetWorkareas">
      <arg type="s" name="json_layout" direction="out"/>
    </method>
    <method name="GetWindows">
      <arg type="s" name="json_windows" direction="out"/>
    </method>
    <method name="MoveResizeWindow">
      <arg type="s" name="target" direction="in"/>
      <arg type="i" name="x" direction="in"/>
      <arg type="i" name="y" direction="in"/>
      <arg type="i" name="w" direction="in"/>
      <arg type="i" name="h" direction="in"/>
      <arg type="b" name="success" direction="out"/>
    </method>
    <method name="MoveWindow">
      <arg type="s" name="app_id" direction="in"/>
      <arg type="i" name="x" direction="in"/>
      <arg type="i" name="y" direction="in"/>
    </method>
    <method name="FocusWindow">
      <arg type="s" name="target" direction="in"/>
      <arg type="b" name="success" direction="out"/>
    </method>
    <method name="DefocusWindow">
      <arg type="s" name="target" direction="in"/>
      <arg type="s" name="to_target" direction="in"/>
      <arg type="b" name="success" direction="out"/>
    </method>
    <method name="SetWindowState">
      <arg type="s" name="target" direction="in"/>
      <arg type="s" name="state" direction="in"/>
      <arg type="b" name="success" direction="out"/>
    </method>
    <signal name="WorkareaChanged"/>
  </interface>
</node>`;

export default class SpawnAtExtension extends Extension {
    enable() {
        // Map of armed App IDs / WM_CLASS identifiers -> target geometries
        this._armedSpawns = new Map();

        // Wildcard target: used when armed with "*" to catch the next unmapped window
        this._wildcardTarget = null;
        this._wildcardTimeoutId = null;

        // 1. Hook into window creation
        // This is called synchronously by Mutter when a new MetaWindow is instantiated.
        try {
            this._windowCreatedId = global.display.connect('window-created', (display, window) => {
                this._handleWindowCreated(window);
            });
        } catch (e) {
            console.error(`[SpawnAt] Failed to connect window-created signal: ${e}`);
        }

        // 2. Hook into compositor actor mapping
        // This is called when Mutter wraps the MetaWindow in a ClutterActor for display.
        try {
            this._mapId = global.window_manager.connect('map', (wm, actor) => {
                this._handleActorMap(actor);
            });
        } catch (e) {
            console.error(`[SpawnAt] Failed to connect map signal: ${e}`);
        }

        // 3. Export D-Bus interface for the Rust CLI client
        try {
            this._dbusImpl = Gio.DBusExportedObject.wrapJSObject(DBUS_IFACE, this);
            this._dbusImpl.export(Gio.DBus.session, '/org/gnome/Shell/Extensions/SpawnAt');
        } catch (e) {
            console.error(`[SpawnAt] Failed to export D-Bus interface: ${e}`);
        }

        // 4. Hook into workspace/monitor changes
        try {
            const monitorManager = global.backend?.get_monitor_manager ? global.backend.get_monitor_manager() : null;
            if (monitorManager) {
                this._monitorsChangedId = monitorManager.connect('monitors-changed', () => {
                    if (this._dbusImpl) {
                        this._dbusImpl.emit_signal('WorkareaChanged', null);
                    }
                });
            }
        } catch (e) {
            console.error(`[SpawnAt] Failed to connect monitors-changed signal: ${e}`);
        }

        try {
            if (global.display) {
                this._workareasChangedId = global.display.connect('workareas-changed', () => {
                    if (this._dbusImpl) {
                        this._dbusImpl.emit_signal('WorkareaChanged', null);
                    }
                });
            }
        } catch (e) {
            console.error(`[SpawnAt] Failed to connect workareas-changed signal: ${e}`);
        }
    }

    /**
     * Inspects newly created windows to attach armed placement targets.
     */
    _handleWindowCreated(window) {
        const checkMatch = (id) => {
            let target = null;

            // Check exact App ID / WM_CLASS match
            if (id && this._armedSpawns.has(id)) {
                target = this._armedSpawns.get(id);
                this._armedSpawns.delete(id);
            } 
            // Fallback: Wildcard match catches the very next window within timeout
            else if (this._wildcardTarget) {
                target = this._wildcardTarget;
                this._clearWildcard();
            }

            if (target) {
                // Attach target geometry directly onto the MetaWindow instance
                window._spawnAtTarget = target;
            }
        };

        // Under Wayland, wm_class or gtk_application_id can be populated asynchronously
        // as the client completes initial surface negotiation.
        let cls = window.get_wm_class();
        if (!cls) {
            // Listen for late wm-class property notification
            let sigId = window.connect('notify::wm-class', () => {
                let lateCls = window.get_wm_class();
                if (lateCls) {
                    checkMatch(lateCls);
                    window.disconnect(sigId);
                }
            });

            // Also check wildcard immediately if armed
            if (this._wildcardTarget) {
                checkMatch(null);
            }
        } else {
            checkMatch(cls);
        }
    }

    /**
     * Intercepts window actor mapping to execute zero-flicker Opacity Cloaking.
     */
    _handleActorMap(actor) {
        let window = actor.meta_window;
        if (!window || !window._spawnAtTarget) {
            return;
        }

        const target = window._spawnAtTarget;
        delete window._spawnAtTarget;

        // STEP 1: OPACITY CLOAKING
        // Suppress actor visibility immediately in the map callback before the compositor
        // submits the frame to the GPU render pass.
        actor.opacity = 0;

        // STEP 2: Clear any maximized state that would restrict repositioning
        window.unmaximize(Meta.MaximizeFlags.BOTH);

        // STEP 3: Apply initial geometry frame mutation
        if (target.w > 0 && target.h > 0) {
            window.move_resize_frame(true, target.x, target.y, target.w, target.h);
        } else {
            window.move_frame(true, target.x, target.y);
        }

        // STEP 4: Asynchronous settling loop
        // Wayland clients configure surface buffers asynchronously. We poll the frame
        // rectangle every 10ms to verify that the geometry matches the target before
        // making the window visible.
        let checkCount = 0;
        const maxChecks = 50; // Maximum duration: ~500ms safety clamp

        GLib.timeout_add(GLib.PRIORITY_DEFAULT, 10, () => {
            checkCount++;

            // Ensure opacity stays cloaked while Mutter adjusts the geometry
            actor.opacity = 0;

            let frame = window.get_frame_rect();

            let actualW = frame.width;
            let actualH = frame.height;
            let finalX = target.x;
            let finalY = target.y;

            // Right boundary clamp (Auto-Anchor if original intent touched wall)
            if (target.max_x !== undefined && target.max_x >= 0) {
                if ((finalX + actualW > target.max_x) || (target.x + target.w >= target.max_x)) {
                    finalX = target.max_x - actualW;
                }
            }
            // Left boundary clamp
            if (target.min_x !== undefined && target.min_x >= 0) {
                if ((finalX < target.min_x) || (target.x <= target.min_x)) {
                    finalX = target.min_x;
                }
            }
            // Bottom boundary clamp (Auto-Anchor if original intent touched wall)
            if (target.max_y !== undefined && target.max_y >= 0) {
                if ((finalY + actualH > target.max_y) || (target.y + target.h >= target.max_y)) {
                    finalY = target.max_y - actualH;
                }
            }
            // Top boundary clamp
            if (target.min_y !== undefined && target.min_y >= 0) {
                if ((finalY < target.min_y) || (target.y <= target.min_y)) {
                    finalY = target.min_y;
                }
            }

            // Apply boundary-corrected position
            // To prevent Mutter from centering the window against a rejected sub-minimum target size,
            // we update the requested bounds to perfectly match the actual settled buffer.
            if (frame.x !== finalX || frame.y !== finalY) {
                if (window._spawnAtLastX !== finalX || window._spawnAtLastY !== finalY || 
                    window._spawnAtLastW !== actualW || window._spawnAtLastH !== actualH) {
                    
                    window.move_resize_frame(true, finalX, finalY, actualW, actualH);
                    
                    window._spawnAtLastX = finalX;
                    window._spawnAtLastY = finalY;
                    window._spawnAtLastW = actualW;
                    window._spawnAtLastH = actualH;
                }
            }

            let posMatch = Math.abs(frame.x - finalX) <= 1 && Math.abs(frame.y - finalY) <= 1;

            // Require both position match and at least 6 ticks (~60ms) of settling 
            let settled = posMatch && (checkCount >= 6);
            let timedOut = (checkCount >= maxChecks);

            if (settled || timedOut) {
                if (!posMatch) {
                    window.move_resize_frame(true, finalX, finalY, actualW, actualH);
                }
                
                // Clean up state
                delete window._spawnAtLastX;
                delete window._spawnAtLastY;
                delete window._spawnAtLastW;
                delete window._spawnAtLastH;
                
                actor.opacity = 255;
                return GLib.SOURCE_REMOVE;
            }

            return GLib.SOURCE_CONTINUE;
        });
    }

    /**
     * D-Bus Method: Arm(identifier, x, y, w, h, min_x, max_x, min_y, max_y)
     * Primes the compositor to intercept the specified application window.
     */
    Arm(identifier, x, y, w, h, min_x, max_x, min_y, max_y) {
        let rect = { x, y, w, h, min_x, max_x, min_y, max_y };

        if (!identifier || identifier === "*") {
            // Wildcard: intercept the very next unmapped window within 1200ms
            this._clearWildcard();
            this._wildcardTarget = rect;

            this._wildcardTimeoutId = GLib.timeout_add(GLib.PRIORITY_DEFAULT, 1200, () => {
                this._wildcardTarget = null;
                this._wildcardTimeoutId = null;
                return GLib.SOURCE_REMOVE;
            });
        } else {
            this._armedSpawns.set(identifier, rect);

            // Auto-clean stale armed spawns after 15 seconds if application never launched
            GLib.timeout_add(GLib.PRIORITY_DEFAULT, 15000, () => {
                if (this._armedSpawns.has(identifier)) {
                    this._armedSpawns.delete(identifier);
                }
                return GLib.SOURCE_REMOVE;
            });
        }
    }

    /**
     * D-Bus Method: GetCursor() -> (x, y)
     * Queries the pointer's current global screen coordinates.
     */
    GetCursor() {
        let [x, y] = global.get_pointer();
        return [x, y];
    }

    GetPointer() {
        let [x, y] = global.get_pointer();
        return [x, y];
    }

    GetWorkareas() {
        let areas = [];
        let numMonitors = global.display.get_n_monitors();
        let workspace = global.workspace_manager.get_active_workspace();
        
        for (let i = 0; i < numMonitors; i++) {
            let rect = workspace.get_work_area_for_monitor(i);
            areas.push({
                x: rect.x,
                y: rect.y,
                w: rect.width,
                h: rect.height
            });
        }
        return JSON.stringify(areas);
    }

    GetWindows() {
        const display = global.display;
        const workspace = display.get_workspace_manager().get_active_workspace();
        const windows = display.get_tab_list(0, workspace);

        const winList = windows.map(win => {
            const frame = win.get_frame_rect();
            return {
                id: win.get_id ? win.get_id() : null,
                pid: win.get_pid(),
                title: win.get_title() || "",
                class: win.get_wm_class() || win.get_gtk_application_id() || "",
                x: frame.x,
                y: frame.y,
                w: frame.width,
                h: frame.height,
                focused: win.has_focus()
            };
        });

        return JSON.stringify(winList);
    }

    _getMRUWindows(workspace) {
        let tabListType = 0;
        if (typeof Meta !== 'undefined' && Meta.TabList && Meta.TabList.NORMAL !== undefined) {
            tabListType = Meta.TabList.NORMAL;
        } else if (typeof Meta.TabListType !== 'undefined' && Meta.TabListType.NORMAL !== undefined) {
            tabListType = Meta.TabListType.NORMAL;
        }

        try {
            const list = global.display.get_tab_list(tabListType, workspace);
            if (list && list.length > 0) {
                return list;
            }
        } catch (e) {
            console.warn(`[spawn-at] get_tab_list failed: ${e}`);
        }

        try {
            if (workspace && typeof workspace.list_windows === 'function') {
                const windows = workspace.list_windows();
                if (typeof global.display.sort_windows_by_stacking === 'function') {
                    return global.display.sort_windows_by_stacking(windows).slice().reverse();
                }
                return windows;
            }
        } catch (e) {
            console.warn(`[spawn-at] window list fallback failed: ${e}`);
        }

        return [];
    }

    _findWindow(target) {
        const display = global.display;
        const workspace = display.get_workspace_manager().get_active_workspace();
        const windows = this._getMRUWindows(workspace);

        const targetNum = parseInt(target, 10);
        if (!isNaN(targetNum) && targetNum > 0) {
            const idMatch = windows.find(w => w.get_id && w.get_id() === targetNum);
            if (idMatch) return idMatch;
            const pidMatch = windows.find(w => w.get_pid && w.get_pid() === targetNum);
            if (pidMatch) return pidMatch;
        }

        return windows.find(w => {
            const wmClass = w.get_wm_class() || "";
            const appId = w.get_gtk_application_id() || "";
            return wmClass === target || appId === target;
        }) || null;
    }

    MoveResizeWindow(target, x, y, w, h) {
        const win = this._findWindow(target);
        if (!win) {
            return false;
        }

        if (win.get_maximized && win.get_maximized()) {
            win.unmaximize(Meta.MaximizeFlags.BOTH);
        }

        const frame = win.get_frame_rect();
        const finalX = (x !== -1) ? x : frame.x;
        const finalY = (y !== -1) ? y : frame.y;
        const finalW = (w > 0) ? w : frame.width;
        const finalH = (h > 0) ? h : frame.height;

        // Use Mutter's frame move & resize API
        if (win.move_resize_frame) {
            win.move_resize_frame(true, finalX, finalY, finalW, finalH);
        } else {
            win.move_frame(true, finalX, finalY);
        }

        return true;
    }

    FocusWindow(target) {
        const win = this._findWindow(target);
        if (!win) {
            return false;
        }
        const time = global.get_current_time();
        win.activate(time);
        return true;
    }

    DefocusWindow(target, to_target) {
        const win = this._findWindow(target);
        if (!win) {
            return false;
        }

        if (!win.has_focus()) {
            return true;
        }

        const time = global.get_current_time();
        if (to_target === 'desktop') {
            global.stage.set_key_focus(null);
            return true;
        }

        // Standard yield (previous / MRU window)
        const workspace = global.display.get_workspace_manager().get_active_workspace();
        const windows = this._getMRUWindows(workspace);
        const nextWin = windows.find(w => w !== win && !w.minimized && !w.skip_taskbar);
        if (nextWin) {
            nextWin.activate(time);
        } else {
            global.stage.set_key_focus(null);
        }

        return true;
    }

    SetWindowState(target, state) {
        const win = this._findWindow(target);
        if (!win) {
            return false;
        }
        switch (state) {
            case 'maximize':
                win.maximize(Meta.MaximizeFlags.BOTH);
                return true;
            case 'unmaximize':
                win.unmaximize(Meta.MaximizeFlags.BOTH);
                return true;
            case 'minimize':
                win.minimize();
                return true;
            case 'unminimize':
                win.unminimize();
                return true;
            case 'restore':
                if (win.minimized) win.unminimize();
                if (win.get_maximized && win.get_maximized()) win.unmaximize(Meta.MaximizeFlags.BOTH);
                return true;
            default:
                return false;
        }
    }

    MoveWindow(app_id, x, y) {
        this.MoveResizeWindow(app_id, x, y, -1, -1);
    }

    _clearWildcard() {
        if (this._wildcardTimeoutId) {
            GLib.Source.remove(this._wildcardTimeoutId);
            this._wildcardTimeoutId = null;
        }
        this._wildcardTarget = null;
    }

    disable() {
        if (this._windowCreatedId) {
            try {
                global.display.disconnect(this._windowCreatedId);
            } catch (e) {
                console.error(`[SpawnAt] Failed to disconnect windowCreatedId: ${e}`);
            }
            this._windowCreatedId = null;
        }

        if (this._mapId) {
            try {
                global.window_manager.disconnect(this._mapId);
            } catch (e) {
                console.error(`[SpawnAt] Failed to disconnect mapId: ${e}`);
            }
            this._mapId = null;
        }

        this._clearWildcard();

        if (this._monitorsChangedId) {
            try {
                const monitorManager = global.backend?.get_monitor_manager ? global.backend.get_monitor_manager() : null;
                if (monitorManager) {
                    monitorManager.disconnect(this._monitorsChangedId);
                }
            } catch (e) {
                console.error(`[SpawnAt] Failed to disconnect monitorsChangedId: ${e}`);
            }
            this._monitorsChangedId = null;
        }

        if (this._workareasChangedId) {
            try {
                global.display.disconnect(this._workareasChangedId);
            } catch (e) {
                console.error(`[SpawnAt] Failed to disconnect workareasChangedId: ${e}`);
            }
            this._workareasChangedId = null;
        }

        if (this._dbusImpl) {
            try {
                this._dbusImpl.unexport();
            } catch (e) {
                console.error(`[SpawnAt] Failed to unexport D-Bus interface: ${e}`);
            }
            this._dbusImpl = null;
        }

        this._armedSpawns.clear();
    }
}
