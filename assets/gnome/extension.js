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
    <method name="SetLogging">
      <arg type="b" name="enabled" direction="in"/>
    </method>
    <signal name="WorkareaChanged"/>
  </interface>
</node>`;

export default class SpawnAtExtension extends Extension {
    _isLoggingEnabled() {
        if (this._loggingEnabled !== undefined) {
            return this._loggingEnabled;
        }
        let envVal = GLib.getenv('SPAWN_AT_DEBUG');
        this._loggingEnabled = envVal === '1' || envVal === 'true';
        return this._loggingEnabled;
    }

    _logTime(tag, extra = "") {
        if (!this._isLoggingEnabled()) return;

        let nowUs = GLib.get_monotonic_time();
        if (!this._t0) this._t0 = nowUs;
        let elapsedMs = ((nowUs - this._t0) / 1000.0).toFixed(2);
        console.error(`[spawn-at-time] +${elapsedMs}ms | ${tag} ${extra}`);
    }

    SetLogging(enabled) {
        this._loggingEnabled = Boolean(enabled);
    }

    enable() {
        this._armedSpawns = new Map();
        this._wildcardTarget = null;
        this._wildcardTimeoutId = null;

        // Mutex queue state: serializes window orchestration batches to prevent
        // focus and geometry race conditions during concurrent window creations.
        this._batchQueue = [];
        this._batchBusy = false;

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
        let cls = "";
        try { cls = window.get_wm_class() || ""; } catch (_) {}
        let actor = (typeof window.get_compositor_private === 'function')
            ? window.get_compositor_private()
            : (global.window_manager.get_window_actor_for_meta_window
                ? global.window_manager.get_window_actor_for_meta_window(window)
                : null);

        // IMMEDIATE HARD CLOAK: If any spawn is armed, kill opacity at tick 0
        if ((this._armedSpawns.size > 0 || this._wildcardTarget) && actor) {
            if (actor.remove_all_transitions) actor.remove_all_transitions();
            actor.opacity = 0;
        }

        let actorStatus = actor ? `actorFound=true visible=${actor.visible} opacity=${actor.opacity}` : `actorFound=false`;
        this._logTime("WINDOW_CREATED", `class="${cls}" ${actorStatus}`);

        if ('no_map_animation' in window) {
            window.no_map_animation = true;
        }

        const getIdentifiers = (win) => {
            const list = [];
            // DETERMINISTIC TOKEN MATCHING:
            // Query the window's startup ID first. By injecting a unique XDG_ACTIVATION_TOKEN
            // into each spawned process, matching on startup ID takes precedence over generic
            // WM classes and ensures concurrent instances of the same application never swap instructions.
            try {
                if (win.get_startup_id) {
                    const sid = win.get_startup_id();
                    if (sid) list.push(sid);
                }
            } catch (e) {}
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
                if (this._armedSpawns.has(id) && this._armedSpawns.get(id).length > 0) {
                    matchedKey = id;
                    break;
                }
                for (let armedKey of this._armedSpawns.keys()) {
                    let queue = this._armedSpawns.get(armedKey);
                    if (queue && queue.length > 0 &&
                        (id.toLowerCase().includes(armedKey.toLowerCase()) || 
                         armedKey.toLowerCase().includes(id.toLowerCase()))) {
                        matchedKey = armedKey;
                        break;
                    }
                }
                if (matchedKey) break;
            }

            if (matchedKey) {
                let queue = this._armedSpawns.get(matchedKey);
                instructions = queue.shift();
                if (queue.length === 0) this._armedSpawns.delete(matchedKey);
            } else if (this._wildcardTarget) {
                instructions = this._wildcardTarget;
                this._clearWildcard();
            }

            if (instructions) {
                window._spawnAtInstructions = instructions;
                let actor = (typeof window.get_compositor_private === 'function') 
                    ? window.get_compositor_private() 
                    : (global.window_manager.get_window_actor_for_meta_window 
                        ? global.window_manager.get_window_actor_for_meta_window(window) 
                        : null);
                if (actor) {
                    if (actor.remove_all_transitions) actor.remove_all_transitions();
                    actor.opacity = 0;
                }
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
        let metaWin = actor.meta_window || (actor.get_meta_window ? actor.get_meta_window() : null);
        let window = metaWin;
        
        if (window && 'no_map_animation' in window) {
            window.no_map_animation = true;
        }

        // Maintain hard cloak if this window has pending instructions or if an armed wildcard exists
        if ((window && window._spawnAtInstructions) || this._armedSpawns.size > 0 || this._wildcardTarget) {
            if (actor.remove_all_transitions) actor.remove_all_transitions();
            actor.opacity = 0;
        }

        let cls = metaWin ? metaWin.get_wm_class() : "unknown";
        this._logTime("ACTOR_MAP", `class="${cls}" visible=${actor.visible} opacity=${actor.opacity}`);

        let wmClass = window ? window.get_wm_class() : "unknown";
        let gtkAppId = (window && window.get_gtk_application_id) ? window.get_gtk_application_id() : "none";
        let pid = window ? window.get_pid() : -1;
        console.error(`[spawn-at] WINDOW CREATED: class="${wmClass}", gtkAppId="${gtkAppId}", pid=${pid}`);

        if (!window) return;

        if (window._spawnAtInstructions) {
            console.error(`[spawn-at] MATCH FOUND! Processing batch for wmClass="${wmClass}"`);
            // Reroute direct execution through the serialized mutex queue
            this._enqueueBatch(window, actor, window._spawnAtInstructions);
            delete window._spawnAtInstructions;
            return;
        }

        // PRE-EMPTIVE CLOAK: Wayland windows often map before 'notify::gtk-application-id' fires.
        // If a spawn is pending, hold the new window cloaked for up to 100ms.
        if (this._armedSpawns.size > 0 || this._wildcardTarget) {
            actor.opacity = 0;
            let checkCount = 0;
            let timerId = GLib.timeout_add(GLib.PRIORITY_DEFAULT, 10, () => {
                checkCount++;
                if (window._spawnAtInstructions) {
                    // Match found during cloak poll: reroute to serialized batch mutex queue
                    this._enqueueBatch(window, actor, window._spawnAtInstructions);
                    delete window._spawnAtInstructions;
                    return GLib.SOURCE_REMOVE;
                }
                if (checkCount > 10) { // 100ms timeout reached, not our window
                    if (this._uncloak) {
                        this._uncloak(actor);
                    } else if (actor) {
                        if (actor.remove_all_transitions) actor.remove_all_transitions();
                        actor.opacity = 255;
                        actor.show();
                    }
                    return GLib.SOURCE_REMOVE;
                }
                return GLib.SOURCE_CONTINUE;
            });
        }
    }

    /**
     * Enqueues an orchestration batch instruction set into the serialized FIFO mutex queue.
     *
     * Serialization is critical in concurrent window spawn environments (such as multi-window
     * CLI grid launches or rapid terminal spawns). Parallel execution of window transformation
     * batches causes race conditions over Mutter's single-threaded focus and layout state machine.
     * Enqueuing ensures that each window completes its sizing, asynchronous focus transition,
     * and placement lifecycle without interference.
     *
     * @param {Meta.Window} window - Target Mutter window instance
     * @param {Clutter.Actor} actor - Corresponding Clutter window actor
     * @param {Array<Object|string>} instructions - Array of batch operation descriptors
     */
    _enqueueBatch(window, actor, instructions) {
        this._batchQueue.push({ window, actor, instructions });
        this._processQueue();
    }

    /**
     * Mutex Queue Worker.
     *
     * Asynchronously serializes batch executions to prevent focus collisions and geometry race conditions.
     *
     * Why serialization is necessary:
     * 1. Focus Contention Prevention: GTK3/VTE widgets require a clean 'focus-out' -> 'focus-in'
     *    transition to calculate font metrics and minimum geometry. If multiple windows execute
     *    this sequence concurrently, they steal focus from each other mid-handshake, breaking
     *    geometry calculation and causing layout freezes.
     * 2. Wayland Protocol Stream Synchronization: Mutter layout passes and Wayland buffer commits
     *    rely on asynchronous event loop turns. Processing batches sequentially guarantees that
     *    each window's buffer commit and anchored positioning settle before the next batch starts.
     */
    async _processQueue() {
        if (this._batchBusy || this._batchQueue.length === 0) {
            return;
        }

        this._batchBusy = true;
        const { window, actor, instructions } = this._batchQueue.shift();

        try {
            await this._runBatch(window, actor, instructions);
        } catch (e) {
            console.error(`[SpawnAt] Batch processing failed in mutex queue: ${e}`);
            if (actor) {
                // Failsafe uncloak: ensure the actor is not left permanently invisible on error
                this._uncloak(actor);
            }
        } finally {
            this._batchBusy = false;
            // Recursively process any remaining items in the queue
            this._processQueue();
        }
    }

    /**
     * Core Batch Orchestration Engine.
     *
     * Executes the sequence of instructions (e.g. Snapshot, Cloak, SetSize, WaitForCommit,
     * SetPositionAnchored, Uncloak) for a targeted window.
     *
     * @param {Meta.Window} window - The window being configured.
     * @param {Clutter.Actor} actor - The Clutter scene-graph actor for the window.
     * @param {Array<Object|string>} instructions - Ordered instruction pipeline.
     */
    async _runBatch(window, actor, instructions) {
        let targetW = null;
        let targetH = null;
        let lastAnchorPayload = null;
        let lastPositionedW = -1;
        let lastPositionedH = -1;

        let sizeInst = instructions.find(i => i && i.SetSize);
        if (sizeInst) {
            targetW = sizeInst.SetSize.w;
            targetH = sizeInst.SetSize.h;
        }

        // Preliminary tick-0 pre-positioning pass to eliminate top-left flash
        let anchorInst = instructions.find(i => i && i.SetPositionAnchored);
        if (anchorInst) {
            this._applyAnchoredPosition(window, anchorInst.SetPositionAnchored, targetW, targetH);
        }

        for (let inst of instructions) {
            this._logTime("BATCH_STEP", `inst=${typeof inst === 'string' ? inst : Object.keys(inst)[0]}`);
            if (inst === "Snapshot") {
                this._createSnapshot(actor);
            } else if (inst === "Cloak") {
                this._cloak(actor);
            } else if (inst === "Uncloak" || (typeof inst === 'object' && inst !== null && 'Uncloak' in inst)) {
                let delay_ms = 300; // Default to 300ms if no flag is provided
                if (typeof inst === 'object' && inst.Uncloak && typeof inst.Uncloak.delay_ms === 'number') {
                    delay_ms = inst.Uncloak.delay_ms;
                }

                if (delay_ms > 0) {
                    await new Promise(resolve => {
                        GLib.timeout_add(GLib.PRIORITY_DEFAULT, delay_ms, () => {
                            resolve();
                            return GLib.SOURCE_REMOVE;
                        });
                    });
                }

                let current_rect = window ? window.get_frame_rect() : null;
                if (current_rect && lastAnchorPayload && (current_rect.width !== lastPositionedW || current_rect.height !== lastPositionedH)) {
                    console.error(`[spawn-at] INFO: Window resized to ${current_rect.width}x${current_rect.height} (enforced by toolkit minimum dimensions).`);
                    this._applyAnchoredPosition(window, lastAnchorPayload);
                }

                if (actor) {
                    if (actor.remove_all_transitions) actor.remove_all_transitions();
                    actor.opacity = 255;
                    actor.show();
                }
                if (window && window.has_focus) {
                    this._logTime("DEBUG", "Initiating post-uncloak focus bounce to force GTK layout");
                    global.stage.set_key_focus(null);
                    GLib.timeout_add(GLib.PRIORITY_DEFAULT, 15, () => {
                        if (window && window.activate) {
                            window.activate(global.get_current_time());
                            this._logTime("DEBUG", "Post-uncloak focus restored");
                        }
                        return GLib.SOURCE_REMOVE;
                    });
                }
                let rect = window ? window.get_frame_rect() : { x: 0, y: 0, width: 0, height: 0 };
                let buf = (window && window.get_buffer_rect) ? window.get_buffer_rect() : rect;
                this._logTime("UNCLOAK_TRIGGERED", `delay=${delay_ms}ms frame=(${rect.x},${rect.y},${rect.width}x${rect.height}) buf=(${buf.x},${buf.y}) actor=(${actor.x},${actor.y}) visible=${actor ? actor.visible : false} opacity=${actor ? actor.opacity : -1}`);
                if (global.stage && global.stage.queue_relayout) {
                    global.stage.queue_relayout();
                }

                if (lastAnchorPayload) {
                    let lateSigId = window.connect('size-changed', () => {
                        let r = window.get_frame_rect();
                        this._logTime("DEBUG", `Late size-changed fired: ${r.width}x${r.height}`);
                        if (r.width !== lastPositionedW || r.height !== lastPositionedH) {
                            this._applyAnchoredPosition(window, lastAnchorPayload);
                            lastPositionedW = r.width;
                            lastPositionedH = r.height;
                        }
                    });
                    // Detach listener after 1200ms once toolkit layout has fully stabilized
                    GLib.timeout_add(GLib.PRIORITY_DEFAULT, 1200, () => {
                        if (window && lateSigId) {
                            try { window.disconnect(lateSigId); } catch(e) {}
                        }
                        return GLib.SOURCE_REMOVE;
                    });
                }
            } else if (inst === "DestroySnapshot") {
                this._destroySnapshot(actor);
            } else if (inst.SetSize) {
                let { w, h } = inst.SetSize;
                if (w > 0 && h > 0) {
                    targetW = w;
                    targetH = h;
                    if (window) {
                        window._targetW = w;
                        window._targetH = h;
                    }
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
                lastAnchorPayload = inst.SetPositionAnchored;
                // Apply anchor strictly to the final stabilized geometry
                this._applyAnchoredPosition(window, inst.SetPositionAnchored);
                
                let f0 = window.get_frame_rect();
                lastPositionedW = f0.width;
                lastPositionedH = f0.height;
                let b0 = window.get_buffer_rect ? window.get_buffer_rect() : f0;
                this._logTime("POSITION_SET", `frame=(${f0.x},${f0.y},${f0.width}x${f0.height}) buf=(${b0.x},${b0.y}) actor=(${actor.x},${actor.y})`);
            }
        }

        if (window) {
            delete window._targetW;
            delete window._targetH;
            delete window._targetX;
            delete window._targetY;
        }
    }

    /**
     * Instantly cloaks the window actor by neutralizing opacity and removing transitions.
     * 
     * CRITICAL: We intentionally do NOT call `actor.hide()`. Keeping the actor mapped in Clutter's
     * scene graph hierarchy allows Clutter and Mutter to perform layout, allocation, and geometry
     * negotiations without triggering unmap/remap cycle glitches when uncloaked.
     */
    _cloak(actor) {
        if (!actor) return;
        if (actor.remove_all_transitions) actor.remove_all_transitions();
        actor.opacity = 0;
    }

    /**
     * Restores window actor visibility and queues a Clutter stage relayout.
     */
    _uncloak(actor) {
        if (!actor) return;
        if (actor.remove_all_transitions) actor.remove_all_transitions();
        actor.opacity = 255;
        if (!actor.visible) actor.show();
    }
    
    /**
     * Creates a temporary Clutter.Clone snapshot of the window actor.
     * Used during dynamic resize/move transformations to eliminate visual tearing.
     */
    _createSnapshot(actor) {
        if (actor._spawnAtClone) return;
        let clone = new Clutter.Clone({ source: actor, x: actor.x, y: actor.y });
        let parent = actor.get_parent();
        if (parent) {
            parent.add_child(clone);
            actor._spawnAtClone = clone;
        }
    }

    /**
     * Destroys the temporary clone snapshot once geometry and uncloak settle.
     */
    _destroySnapshot(actor) {
        if (actor._spawnAtClone) {
            actor._spawnAtClone.destroy();
            delete actor._spawnAtClone;
        }
    }

    /**
     * Dynamic Geometry & Commit Latch.
     *
     * Monitors Mutter's allocation and size-change signals to detect when a client
     * (GTK/Wayland application) has committed its new buffer geometry matching the requested size.
     *
     * Lifecycle Details:
     * - Attaches listeners to actor 'notify::allocation' and window 'size-changed'.
     * - Evaluates whether current geometry is within tolerance of target geometry (`isNearTarget`).
     * - Employs a 60ms debounce settling timer to accommodate toolkits that resize in multiple passes
     *   (such as terminal emulators snapping to font grids).
     * - Enforces a fallback timeout ceiling (min 120ms) so windows with hard min/max constraints do not stall.
     */
    async _waitForCommit(window, actor, timeout_ms, targetW = null, targetH = null) {
        if (!window) return;

        return new Promise(resolve => {
            let timeoutId = null;
            let actorSigId = null;
            let winSigId = null;
            let settleTimerId = null;
            let resolved = false;

            const cleanup = () => {
                if (settleTimerId) { GLib.Source.remove(settleTimerId); settleTimerId = null; }
                if (actorSigId && actor) { try { actor.disconnect(actorSigId); } catch(e){} actorSigId = null; }
                if (winSigId && window) { try { window.disconnect(winSigId); } catch(e){} winSigId = null; }
                if (timeoutId) { GLib.Source.remove(timeoutId); timeoutId = null; }
            };

            const doResolve = (reason) => {
                if (resolved) return;
                resolved = true;
                this._logTime("COMMIT_RESOLVED", `reason=${reason}`);
                cleanup();
                resolve();
            };

            let initialRect = window.get_frame_rect();
            let initialW = initialRect.width;
            let initialH = initialRect.height;
            let hasChanged = false;
            let lastW = initialW;
            let lastH = initialH;

            const isNearTarget = (w, h) => {
                if (targetW == null || targetH == null) return false;
                return Math.abs(w - targetW) <= 45 && Math.abs(h - targetH) <= 45;
            };

            const check = () => {
                if (resolved) return;

                // Neutralize active Mutter map transitions on every geometry pulse
                if (actor) {
                    if (actor.remove_all_transitions) actor.remove_all_transitions();
                    actor.opacity = 0;
                }

                try {
                    let rect = window.get_frame_rect();
                    this._logTime("DEBUG", `Geometry check: ${rect.width}x${rect.height}`);

                    if (rect.width !== initialW || rect.height !== initialH) {
                        hasChanged = true;
                    }

                    if (rect.width !== lastW || rect.height !== lastH) {
                        lastW = rect.width;
                        lastH = rect.height;
                        if (settleTimerId) GLib.Source.remove(settleTimerId);
                        settleTimerId = GLib.timeout_add(GLib.PRIORITY_DEFAULT, 60, () => {
                            settleTimerId = null;
                            doResolve("SIZE_SETTLED_AFTER_CHANGE");
                            return GLib.SOURCE_REMOVE;
                        });
                        return;
                    }

                    if (isNearTarget(rect.width, rect.height)) {
                        doResolve("AT_TARGET");
                        return;
                    }

                    if (hasChanged && !settleTimerId) {
                        settleTimerId = GLib.timeout_add(GLib.PRIORITY_DEFAULT, 60, () => {
                            settleTimerId = null;
                            doResolve("SIZE_STABLE");
                            return GLib.SOURCE_REMOVE;
                        });
                    }
                } catch (e) {
                    doResolve("ERROR");
                }
            };

            if (actor) {
                try { actorSigId = actor.connect('notify::allocation', () => check()); } catch (e) {}
            }
            if (window) {
                try { winSigId = window.connect('size-changed', () => check()); } catch (e) {}
            }

            // Cap timeout so windows already clamped at minimum size don't stall
            let effectiveTimeout = Math.max(timeout_ms || 120, 120);
            timeoutId = GLib.timeout_add(GLib.PRIORITY_DEFAULT, effectiveTimeout, () => {
                timeoutId = null;
                doResolve("TIMEOUT");
                return GLib.SOURCE_REMOVE;
            });

            check();
        });
    }

    /**
     * Computes and applies anchored window position.
     *
     * Accounts for pivot offsets (pivot_u, pivot_v), CSD frame geometry, monitor bounds/workareas,
     * toolkit minimum size constraints, and margin constraints. Uses `window.move_frame` to correctly
     * calculate client-side decoration (CSD) shadow offsets in Wayland.
     *
     * Dynamic Toolkit Minimums (Margin Bounds Clamping):
     * CSD applications (e.g. GTK/VTE) initially report an intermediate frame height (e.g. 76px bare headerbar)
     * prior to font metric negotiation. Querying `window.get_min_size()` ensures that anchoring math
     * accounts for the true minimum widget dimensions (~93px+), preventing bottom/right margin violations
     * when the window later expands downward or rightward.
     */
    _applyAnchoredPosition(window, payload, overrideW = null, overrideH = null) {
        let frame = window.get_frame_rect();

        let [minW, minH] = [0, 0];
        if (typeof window.get_min_size === 'function') {
            try { [minW, minH] = window.get_min_size(); } catch (e) {}
        }

        // Clamp effective dimensions to the toolkit's true minimum size constraints
        let effW = overrideW !== null ? Math.max(overrideW, minW) : Math.max(frame.width, minW);
        let effH = overrideH !== null ? Math.max(overrideH, minH) : Math.max(frame.height, minH);

        let monitorIndex = window.get_monitor();
        let bounds = payload.area === 'screen' 
            ? global.display.get_monitor_geometry(monitorIndex)
            : window.get_work_area_for_monitor(monitorIndex);

        let mt = payload.margin_top !== undefined ? payload.margin_top : 0;
        let mb = payload.margin_bottom !== undefined ? payload.margin_bottom : 0;
        let ml = payload.margin_left !== undefined ? payload.margin_left : 0;
        let mr = payload.margin_right !== undefined ? payload.margin_right : 0;

        let minX = bounds.x + ml;
        let maxX = bounds.x + bounds.width - mr - effW;
        let minY = bounds.y + mt;
        let maxY = bounds.y + bounds.height - mb - effH;

        let availW = bounds.width - ml - mr;
        let availH = bounds.height - mt - mb;

        if (effW > availW || effH > availH) {
            console.error(`[spawn-at] WARN: Window is oversized (${effW}x${effH}); bottom and right margins will be ignored to preserve top-left accessibility.`);
        }

        if (maxX < minX) maxX = minX;
        if (maxY < minY) maxY = minY;

        let raw_x = payload.screen_anchor_x - Math.round(payload.pivot_u * effW) + payload.offset_x;
        let raw_y = payload.screen_anchor_y - Math.round(payload.pivot_v * effH) + payload.offset_y;

        let clamped_x = Math.max(minX, Math.min(raw_x, maxX));
        let clamped_y = Math.max(minY, Math.min(raw_y, maxY));

        if (!(effW > availW || effH > availH) && (clamped_x !== raw_x || clamped_y !== raw_y)) {
            console.error(`[spawn-at] INFO: Window repositioned from (${raw_x}, ${raw_y}) to (${clamped_x}, ${clamped_y}) due to boundary margin clamp.`);
        }

        let x = clamped_x;
        let y = clamped_y;

        if (window) {
            window._targetX = x;
            window._targetY = y;
        }

        if (window.move_frame) {
            window.move_frame(true, x, y);
        } else {
            window.move(true, x, y);
        }
    }

    ArmSpawn(target_id, instructions_json) {
        this._t0 = GLib.get_monotonic_time();
        this._logTime("ARM_SPAWN", `target="${target_id}"`);
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
            if (!this._armedSpawns.has(target_id)) {
                this._armedSpawns.set(target_id, []);
            }
            this._armedSpawns.get(target_id).push(instructions);

            // Expire this specific instruction entry after 15 seconds if unused
            GLib.timeout_add(GLib.PRIORITY_DEFAULT, 15000, () => {
                let queue = this._armedSpawns.get(target_id);
                if (queue) {
                    let idx = queue.indexOf(instructions);
                    if (idx !== -1) queue.splice(idx, 1);
                    if (queue.length === 0) this._armedSpawns.delete(target_id);
                }
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
                // Reroute direct batch execution through the serialized mutex queue
                this._enqueueBatch(win, actor, instructions);
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
        // Clean up pending queue items and reset mutex busy state
        this._batchQueue = [];
        this._batchBusy = false;

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
