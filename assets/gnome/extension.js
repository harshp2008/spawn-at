import { Extension } from 'resource:///org/gnome/shell/extensions/extension.js';
import Gio from 'gi://Gio';
import Meta from 'gi://Meta';
import GLib from 'gi://GLib';
import Clutter from 'gi://Clutter';

const DBUS_IFACE = `
<node>
  <interface name="org.gnome.Shell.Extensions.SpawnAt">
    <method name="ArmSpawn">
      <arg type="s" name="target_id" direction="in"/>
      <arg type="s" name="instructions_json" direction="in"/>
    </method>
    <method name="ExecuteBatch">
      <arg type="s" name="target_id" direction="in"/>
      <arg type="s" name="instructions_json" direction="in"/>
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
        this._armedSpawns = new Map();
        this._wildcardTarget = null;
        this._wildcardTimeoutId = null;

        try {
            this._windowCreatedId = global.display.connect('window-created', (display, window) => {
                this._handleWindowCreated(window);
            });
        } catch (e) {
            console.error(`[SpawnAt] Failed to connect window-created signal: ${e}`);
        }

        try {
            this._mapId = global.window_manager.connect('map', (wm, actor) => {
                this._handleActorMap(actor);
            });
        } catch (e) {
            console.error(`[SpawnAt] Failed to connect map signal: ${e}`);
        }

        try {
            this._dbusImpl = Gio.DBusExportedObject.wrapJSObject(DBUS_IFACE, this);
            this._dbusImpl.export(Gio.DBus.session, '/org/gnome/Shell/Extensions/SpawnAt');
        } catch (e) {
            console.error(`[SpawnAt] Failed to export D-Bus interface: ${e}`);
        }

        try {
            const monitorManager = global.backend?.get_monitor_manager ? global.backend.get_monitor_manager() : null;
            if (monitorManager) {
                this._monitorsChangedId = monitorManager.connect('monitors-changed', () => {
                    if (this._dbusImpl) this._dbusImpl.emit_signal('WorkareaChanged', null);
                });
            }
        } catch (e) {}

        try {
            if (global.display) {
                this._workareasChangedId = global.display.connect('workareas-changed', () => {
                    if (this._dbusImpl) this._dbusImpl.emit_signal('WorkareaChanged', null);
                });
            }
        } catch (e) {}
    }

    _handleWindowCreated(window) {
        const getIdentifiers = (win) => {
            const list = [];
            try {
                const cls = win.get_wm_class();
                if (cls) list.push(cls);
            } catch (e) {}
            try {
                const appId = win.get_gtk_application_id();
                if (appId) list.push(appId);
            } catch (e) {}
            try {
                if (win.get_sandboxed_app_id) {
                    const sb = win.get_sandboxed_app_id();
                    if (sb) list.push(sb);
                }
            } catch (e) {}
            return list;
        };

        const checkMatch = () => {
            if (window._spawnAtInstructions) return;

            const ids = getIdentifiers(window);
            let instructions = null;
            let matchedKey = null;

            for (let id of ids) {
                if (this._armedSpawns.has(id)) {
                    matchedKey = id;
                    break;
                }
                for (let armedKey of this._armedSpawns.keys()) {
                    if (id.toLowerCase().includes(armedKey.toLowerCase()) || 
                        armedKey.toLowerCase().includes(id.toLowerCase())) {
                        matchedKey = armedKey;
                        break;
                    }
                }
                if (matchedKey) break;
            }

            if (matchedKey) {
                instructions = this._armedSpawns.get(matchedKey);
                this._armedSpawns.delete(matchedKey);
            } else if (this._wildcardTarget) {
                instructions = this._wildcardTarget;
                this._clearWildcard();
            }

            if (instructions) {
                window._spawnAtInstructions = instructions;
            }
        };

        checkMatch();

        if (!window._spawnAtInstructions) {
            const sigWm = window.connect('notify::wm-class', () => {
                checkMatch();
                if (window._spawnAtInstructions) window.disconnect(sigWm);
            });
            const sigGtk = window.connect('notify::gtk-application-id', () => {
                checkMatch();
                if (window._spawnAtInstructions) window.disconnect(sigGtk);
            });
        }
    }

    _handleActorMap(actor) {
        let window = actor.meta_window;
        if (!window) return;

        if (window._spawnAtInstructions) {
            actor.hide();
            this._runBatch(window, actor, window._spawnAtInstructions);
            delete window._spawnAtInstructions;
            return;
        }

        // PRE-EMPTIVE CLOAK: Wayland windows often map before 'notify::gtk-application-id' fires.
        // If a spawn is pending, hold the new window cloaked for up to 100ms.
        if (this._armedSpawns.size > 0 || this._wildcardTarget) {
            actor.hide();
            let checkCount = 0;
            let timerId = GLib.timeout_add(GLib.PRIORITY_DEFAULT, 10, () => {
                checkCount++;
                if (window._spawnAtInstructions) {
                    this._runBatch(window, actor, window._spawnAtInstructions);
                    delete window._spawnAtInstructions;
                    return GLib.SOURCE_REMOVE;
                }
                if (checkCount > 10) { // 100ms timeout reached, not our window
                    actor.show();
                    return GLib.SOURCE_REMOVE;
                }
                return GLib.SOURCE_CONTINUE;
            });
        }
    }

    async _runBatch(window, actor, instructions) {
        let targetW = null;
        let targetH = null;

        for (let inst of instructions) {
            if (inst === "Snapshot") {
                this._createSnapshot(actor);
            } else if (inst === "Cloak") {
                this._cloak(actor);
            } else if (inst === "Uncloak") {
                await new Promise(resolve => {
                    GLib.idle_add(GLib.PRIORITY_HIGH, () => {
                        if (this._uncloak) {
                            this._uncloak(actor);
                        } else {
                            actor.show();
                        }
                        resolve();
                        return GLib.SOURCE_REMOVE;
                    });
                });
            } else if (inst === "DestroySnapshot") {
                this._destroySnapshot(actor);
            } else if (inst.SetSize) {
                let { w, h } = inst.SetSize;
                if (w > 0 && h > 0) {
                    targetW = w;
                    targetH = h;
                    if (window.get_maximized && window.get_maximized()) {
                        window.unmaximize(Meta.MaximizeFlags.BOTH);
                    }
                    let frame = window.get_frame_rect();
                    if (window.move_resize_frame) {
                        window.move_resize_frame(true, frame.x, frame.y, w, h);
                    } else {
                        window.resize(true, w, h);
                    }
                }
            } else if (inst.WaitForCommit) {
                await this._waitForCommit(window, actor, inst.WaitForCommit.timeout_ms, targetW, targetH);
            } else if (inst.SetPositionAnchored) {
                this._applyAnchoredPosition(window, inst.SetPositionAnchored);
            }
        }
    }

    _cloak(actor) {
        if (!actor) return;
        actor.hide();
    }

    _uncloak(actor) {
        if (!actor) return;
        actor.show();
    }
    
    _createSnapshot(actor) {
        if (actor._spawnAtClone) return;
        let clone = new Clutter.Clone({ source: actor, x: actor.x, y: actor.y });
        let parent = actor.get_parent();
        if (parent) {
            parent.add_child(clone);
            actor._spawnAtClone = clone;
        }
    }

    _destroySnapshot(actor) {
        if (actor._spawnAtClone) {
            actor._spawnAtClone.destroy();
            delete actor._spawnAtClone;
        }
    }

    _waitForCommit(window, actor, timeout_ms, targetW = null, targetH = null) {
        return new Promise(resolve => {
            let timeoutId = null;
            let idleId = null;
            let actorSigId = null;
            let winSigId = null;
            let resolved = false;

            const cleanup = () => {
                if (actorSigId && actor) {
                    try { actor.disconnect(actorSigId); } catch (e) {}
                    actorSigId = null;
                }
                if (winSigId && window) {
                    try { window.disconnect(winSigId); } catch (e) {}
                    winSigId = null;
                }
                if (timeoutId) {
                    GLib.Source.remove(timeoutId);
                    timeoutId = null;
                }
                if (idleId) {
                    GLib.Source.remove(idleId);
                    idleId = null;
                }
            };

            const finish = () => {
                if (resolved) return;
                resolved = true;
                cleanup();
                resolve();
            };

            const scheduleIdleFinish = () => {
                if (resolved || idleId) return;
                idleId = GLib.idle_add(GLib.PRIORITY_HIGH, () => {
                    idleId = null;
                    finish();
                    return GLib.SOURCE_REMOVE;
                });
            };

            const checkAndTrigger = () => {
                if (resolved || idleId) return;
                if (targetW != null && targetH != null) {
                    try {
                        let frame = window.get_frame_rect();
                        if (Math.abs(frame.width - targetW) > 10 || Math.abs(frame.height - targetH) > 10) {
                            return;
                        }
                    } catch (e) {}
                }
                scheduleIdleFinish();
            };

            if (actor) {
                try {
                    actorSigId = actor.connect('notify::allocation', () => {
                        checkAndTrigger();
                    });
                } catch (e) {}
            }

            if (window) {
                try {
                    winSigId = window.connect('size-changed', () => {
                        checkAndTrigger();
                    });
                } catch (e) {}
            }

            let effectiveTimeout = Math.min(timeout_ms, 60);
            timeoutId = GLib.timeout_add(GLib.PRIORITY_DEFAULT, effectiveTimeout, () => {
                timeoutId = null;
                scheduleIdleFinish();
                return GLib.SOURCE_REMOVE;
            });

            checkAndTrigger();
        });
    }

    _applyAnchoredPosition(window, payload) {
        let frame = window.get_frame_rect();
        let w = frame.width;
        let h = frame.height;
        let x = payload.screen_anchor_x - Math.round(payload.pivot_u * w) + payload.offset_x;
        let y = payload.screen_anchor_y - Math.round(payload.pivot_v * h) + payload.offset_y;

        let monitorIndex = window.get_monitor();
        let workArea = window.get_work_area_for_monitor(monitorIndex);

        // Clamp to workarea bounds to prevent spawning under top bar or off-screen
        x = Math.max(workArea.x, Math.min(x, workArea.x + workArea.width - w));
        y = Math.max(workArea.y, Math.min(y, workArea.y + workArea.height - h));

        if (window.move_frame) {
            window.move_frame(true, x, y);
        } else {
            window.move(true, x, y);
        }
    }

    ArmSpawn(target_id, instructions_json) {
        let instructions = [];
        try {
            instructions = JSON.parse(instructions_json);
        } catch (e) {
            console.error(`[SpawnAt] Invalid JSON instructions: ${e}`);
            return;
        }

        if (!target_id || target_id === "*") {
            this._clearWildcard();
            this._wildcardTarget = instructions;
            this._wildcardTimeoutId = GLib.timeout_add(GLib.PRIORITY_DEFAULT, 1200, () => {
                this._wildcardTarget = null;
                this._wildcardTimeoutId = null;
                return GLib.SOURCE_REMOVE;
            });
        } else {
            this._armedSpawns.set(target_id, instructions);
            GLib.timeout_add(GLib.PRIORITY_DEFAULT, 15000, () => {
                if (this._armedSpawns.has(target_id)) this._armedSpawns.delete(target_id);
                return GLib.SOURCE_REMOVE;
            });
        }
    }

    ExecuteBatch(target_id, instructions_json) {
        let instructions = [];
        try {
            instructions = JSON.parse(instructions_json);
        } catch (e) {
            return;
        }

        const win = this._findWindow(target_id);
        if (win) {
            let actor = null;
            if (typeof win.get_compositor_private === 'function') {
                actor = win.get_compositor_private();
            } else if (global.window_manager.get_window_actor_for_meta_window) {
                actor = global.window_manager.get_window_actor_for_meta_window(win);
            }
            if (actor) {
                this._runBatch(win, actor, instructions).catch(e => {
                    console.error(`[SpawnAt] Batch execution failed: ${e}`);
                    actor.show();
                });
            }
        }
    }

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
            if (list && list.length > 0) return list;
        } catch (e) {}

        try {
            if (workspace && typeof workspace.list_windows === 'function') {
                const windows = workspace.list_windows();
                if (typeof global.display.sort_windows_by_stacking === 'function') {
                    return global.display.sort_windows_by_stacking(windows).slice().reverse();
                }
                return windows;
            }
        } catch (e) {}

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

    MoveWindow(app_id, x, y) {
        const win = this._findWindow(app_id);
        if (win) {
            if (win.move_frame) win.move_frame(true, x, y);
        }
    }

    FocusWindow(target) {
        const win = this._findWindow(target);
        if (!win) return false;
        win.activate(global.get_current_time());
        return true;
    }

    DefocusWindow(target, to_target) {
        const win = this._findWindow(target);
        if (!win) return false;
        if (!win.has_focus()) return true;

        const time = global.get_current_time();
        if (to_target === 'desktop') {
            global.stage.set_key_focus(null);
            return true;
        }

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
        if (!win) return false;
        switch (state) {
            case 'maximize': win.maximize(Meta.MaximizeFlags.BOTH); return true;
            case 'unmaximize': win.unmaximize(Meta.MaximizeFlags.BOTH); return true;
            case 'minimize': win.minimize(); return true;
            case 'unminimize': win.unminimize(); return true;
            case 'restore':
                if (win.minimized) win.unminimize();
                if (win.get_maximized && win.get_maximized()) win.unmaximize(Meta.MaximizeFlags.BOTH);
                return true;
            default: return false;
        }
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
            try { global.display.disconnect(this._windowCreatedId); } catch(e) {}
            this._windowCreatedId = null;
        }
        if (this._mapId) {
            try { global.window_manager.disconnect(this._mapId); } catch(e) {}
            this._mapId = null;
        }
        this._clearWildcard();
        if (this._monitorsChangedId) {
            try { global.backend.get_monitor_manager().disconnect(this._monitorsChangedId); } catch(e) {}
            this._monitorsChangedId = null;
        }
        if (this._workareasChangedId) {
            try { global.display.disconnect(this._workareasChangedId); } catch(e) {}
            this._workareasChangedId = null;
        }
        if (this._dbusImpl) {
            try { this._dbusImpl.unexport(); } catch(e) {}
            this._dbusImpl = null;
        }
        this._armedSpawns.clear();
    }
}
