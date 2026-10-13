/**
 * spawn-at — precise window spawning, sizing, positioning and orchestration
 * for GNOME Shell (Wayland / X11).
 *
 * Modular ESM architecture:
 * - extension.js: Shell extension lifecycle, discovery, and batch dispatch
 * - dbus.js: D-Bus interface definition and service management
 * - cloak.js: Clutter actor opacity cloaking, snapshot clones, and watchdogs
 * - commit.js: Mutter geometry allocation and frame settlement latches
 * - pulse.js: VTE terminal heartbeat gating and focus pulses
 * - anchor.js: Pure mathematical coordinate computation and bounding clamping
 * - logger.js: Persistent asynchronous telemetry with rotation and 0700/0600 security
 * - validator.js: Strict instruction pipeline syntax and boundary validator
 */

import { Extension } from 'resource:///org/gnome/shell/extensions/extension.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import Gio from 'gi://Gio';
import Meta from 'gi://Meta';
import GLib from 'gi://GLib';
import Clutter from 'gi://Clutter';
import GObject from 'gi://GObject';

import { DBusManager } from './dbus.js';
import { CloakManager } from './cloak.js';
import { CommitLatch, readyReason } from './commit.js';
import { FocusPulse } from './pulse.js';
import { computeAnchoredPosition } from './anchor.js';
import { SessionLogger } from './logger.js';
import { validateInstructions } from './validator.js';
import { X11Observer, readTextureSize } from './x11_observe.js';

const ARM_EXPIRY_MS = 15000;
const WILDCARD_EXPIRY_MS = 1200;
const GEOMETRY_FLOOR_W = 100;
const GEOMETRY_FLOOR_H = 60;
const ANCHOR_RETIRE_QUIET_MS = 1000;
const FRAME_WAIT_CEILING_MS = 50;
// Most re-anchor checks made right before the reveal; at most one extra frame wait (2 - 1).
const LATE_RESIZE_MAX_PASSES = 2;

export default class SpawnAtExtension extends Extension {

    // =======================================================================
    // 1. LIFECYCLE
    // =======================================================================

    enable() {
        this._disabled = false;
        this._timerIds = new Set();
        this._windowStates = new Map();
        this._heldWindows = new Set();
        this._batchQueue = [];
        this._batchBusy = false;
        this._currentBatchItem = null;
        this._armedSpawns = new Map();
        this._wildcardTarget = null;
        this._wildcardTimeoutId = 0;
        this._anchors = new Map();
        this._skippedEffectsActors = new Set();

        // Initialize logging module
        this._logger = new SessionLogger();
        this._logger.init();

        // Initialize cloaking manager
        this._cloakManager = new CloakManager(
            global.stage,
            {
                setTimeout: (cb, ms) => this._addTimer(ms, cb),
                clearTimeout: id => this._removeTimer(id),
            },
            { Clutter, helper: this }
        );

        // Initialize commit and pulse managers
        this._commitLatch = new CommitLatch(this);
        this._x11Observer = new X11Observer(this._x11ObserveHost());
        this._focusPulse = new FocusPulse(this, this._commitLatch);

        // Dummy focus actor
        try {
            this._dummy = new Clutter.Actor({ reactive: false, can_focus: true, opacity: 0 });
            global.stage.add_child(this._dummy);
        } catch (_e) {
            this._dummy = null;
        }

        // Connect compositor signals
        this._windowCreatedId = this._tryConnect(global.display, 'window-created',
            (_disp, win) => this._handleWindowCreated(win));
        this._mapId = this._tryConnect(global.window_manager, 'map',
            (_wm, actor) => this._handleActorMap(actor));
        this._grabEndId = this._tryConnect(global.display, 'grab-op-end', () => {
            for (const win of [...this._anchors.keys()])
                this._disarmAnchor(win, 'USER_GRAB');
        });

        this._monitorManager = global.backend?.get_monitor_manager?.();
        if (this._monitorManager) {
            this._monitorsChangedId = this._tryConnect(this._monitorManager, 'monitors-changed',
                () => this._emitWorkareaChanged());
        }
        this._workareasChangedId = this._tryConnect(global.display, 'workareas-changed',
            () => this._emitWorkareaChanged());

        // D-Bus export
        this._dbusManager = new DBusManager(this);
        this._dbusManager.export();
    }

    disable() {
        this._disabled = true;
        this._batchQueue = [];
        this._batchBusy = false;
        this._currentBatchItem = null;

        // End the observe-only X11 watches (disconnects their signal handlers)
        this._x11Observer?.stopAll();

        // Disarm reactive anchors
        for (const window of [...this._anchors.keys()])
            this._disarmAnchor(window, 'DISABLE');

        // Disconnect compositor signals
        this._safeDisconnect(global.display, this._windowCreatedId);
        this._safeDisconnect(global.window_manager, this._mapId);
        this._safeDisconnect(global.display, this._grabEndId);
        this._safeDisconnect(this._monitorManager, this._monitorsChangedId);
        this._safeDisconnect(global.display, this._workareasChangedId);
        this._windowCreatedId = this._mapId = this._grabEndId = 0;
        this._monitorsChangedId = this._workareasChangedId = 0;
        this._monitorManager = null;

        // Cancel timers
        for (const id of this._timerIds)
            GLib.Source.remove(id);
        this._timerIds.clear();
        this._wildcardTimeoutId = 0;
        this._wildcardTarget = null;
        this._armedSpawns.clear();
        this._heldWindows.clear();

        // Cloak manager teardown (uncloaks all actors unconditionally)
        this._cloakManager?.disable();

        // Cleanup dummy actor
        if (this._dummy) {
            if (global.stage.get_key_focus() === this._dummy)
                global.stage.set_key_focus(null);
            this._try(() => this._dummy.destroy());
            this._dummy = null;
        }

        // Cleanup lingering skipped effects
        if (this._skippedEffectsActors) {
            for (const actor of this._skippedEffectsActors)
                this._clearSkippedEffects(actor);
            this._skippedEffectsActors.clear();
        }
        if (this._currentBatchItem?.actor)
            this._clearSkippedEffects(this._currentBatchItem.actor);

        // Flush persistent logs
        this._logger?.flushSync();

        // Teardown D-Bus
        this._dbusManager?.unexport();
    }

    // =======================================================================
    // 2. LOGGING DELEGATES
    // =======================================================================

    isLoggingEnabled() {
        return this._logger?.isLoggingEnabled() ?? false;
    }

    logTime(tag, extra = '', window = null) {
        if (!this.isLoggingEnabled())
            return;
        this._logger?.logTime(tag, extra, window);
    }

    SetLogging(enabled) {
        this._logger?.setLogging(enabled);
    }

    // =======================================================================
    // 3. UTILITIES & TIMERS
    // =======================================================================

    isDisabled() {
        return this._disabled;
    }

    _try(fn, fallback = null) {
        try { return fn(); } catch (_e) { return fallback; }
    }

    tryConnect(target, signal, callback) {
        return this._tryConnect(target, signal, callback);
    }

    _tryConnect(target, signal, callback) {
        try {
            return target.connect(signal, callback);
        } catch (e) {
            console.error(`[SpawnAt] Failed to connect '${signal}': ${e}`);
            return 0;
        }
    }

    safeDisconnect(target, id) {
        this._safeDisconnect(target, id);
    }

    _safeDisconnect(target, id) {
        if (!target || !id)
            return;
        this._try(() => target.disconnect(id));
    }

    // Milliseconds on the monotonic clock the shell's logger counts from.
    nowMs() {
        return GLib.get_monotonic_time() / 1000;
    }

    addTimer(ms, callback) {
        return this._addTimer(ms, callback);
    }

    _addTimer(ms, callback) {
        const id = GLib.timeout_add(GLib.PRIORITY_DEFAULT, ms, () => {
            this._timerIds.delete(id);
            callback();
            return GLib.SOURCE_REMOVE;
        });
        this._timerIds.add(id);
        return id;
    }

    removeTimer(id) {
        this._removeTimer(id);
    }

    _removeTimer(id) {
        if (id && this._timerIds.delete(id))
            GLib.Source.remove(id);
    }

    _num(value, fallback = 0) {
        return Number.isFinite(value) ? value : fallback;
    }

    _getActor(window) {
        if (typeof window.get_compositor_private === 'function')
            return window.get_compositor_private();
        return global.window_manager.get_window_actor_for_meta_window?.(window) ?? null;
    }

    _logActorTrace(tag, actor, window, extra = '') {
        if (!this.isLoggingEnabled())
            return;
        const opacity = (actor && typeof actor.opacity === 'number') ? actor.opacity : -1;
        let frameStr = 'frame=(0,0,0x0)';
        if (window && typeof window.get_frame_rect === 'function') {
            const f = window.get_frame_rect();
            frameStr = `frame=(${f.x},${f.y},${f.width}x${f.height})`;
        } else if (actor && typeof actor.x === 'number') {
            frameStr = `frame=(${actor.x},${actor.y},${actor.width || 0}x${actor.height || 0})`;
        }
        const msg = `opacity=${opacity} ${frameStr}${extra ? ' ' + extra : ''}`.trim();
        this.logTime(tag, msg, window);
    }

    _suppressMapAnimation(window) {
        if ('no_map_animation' in window)
            window.no_map_animation = true;
        const actor = this._getActor(window);
        if (actor) {
            if ('no_map_animation' in actor)
                actor.no_map_animation = true;
            if (typeof actor.set_easing_duration === 'function')
                actor.set_easing_duration(0);
        }
    }

    _isMaximized(window) {
        if (typeof window.is_maximized === 'function')
            return window.is_maximized();
        if (typeof window.get_maximized === 'function')
            return window.get_maximized() !== 0;
        return Boolean(window.maximized_horizontally || window.maximized_vertically);
    }

    _isFullscreen(window) {
        if (typeof window.is_fullscreen === 'function')
            return window.is_fullscreen();
        return Boolean(window.fullscreen);
    }

    _maximize(window) {
        try { window.maximize(Meta.MaximizeFlags.BOTH); } catch (_e) { window.maximize(); }
    }

    _unmaximize(window) {
        try { window.unmaximize(Meta.MaximizeFlags.BOTH); } catch (_e) { window.unmaximize(); }
    }

    _getMinSize(window) {
        const size = this._try(() => window.get_min_size?.(), null);
        if (!size)
            return [0, 0];
        return [this._num(size[0]), this._num(size[1])];
    }

    _userIsGrabbing() {
        const op = this._try(() => global.display.get_grab_op?.());
        return op !== undefined && op !== null && op !== Meta.GrabOp.NONE;
    }

    _moveFrame(window, x, y) {
        if (window.move_frame)
            window.move_frame(true, x, y);
        else
            window.move(true, x, y);
    }

    _emitWorkareaChanged() {
        this._dbusManager?.emitSignal('WorkareaChanged', null);
    }

    cloak(actor) {
        this._cloakManager?.cloak(actor);
    }

    _cloak(actor) {
        this._cloakManager?.cloak(actor);
    }

    uncloak(actor) {
        this._cloakManager?.uncloak(actor);
    }

    _uncloak(actor) {
        this._cloakManager?.uncloak(actor);
    }

    focusWindow(window) {
        return this._focusWindowObject(window);
    }

    defocusWindow(window, mode = 'desktop', destination = '') {
        return this._defocusWindowObject(window, mode, destination);
    }

    // =======================================================================
    // 4. SPAWN DISCOVERY (window-created / map)
    // =======================================================================

    _hasArmedSpawns() {
        return this._armedSpawns.size > 0 || this._wildcardTarget !== null;
    }

    _getState(window) {
        let state = this._windowStates.get(window);
        if (!state) {
            state = {
                instructions: null,
                mapped: false,
                dispatched: false,
                notifyIds: [],
            };
            this._windowStates.set(window, state);
        }
        return state;
    }

    _getIdentifiers(window) {
        const ids = [
            this._try(() => window.get_startup_id?.()),
            this._try(() => window.get_wm_class?.()),
            this._try(() => window.get_gtk_application_id?.()),
            this._try(() => window.get_sandboxed_app_id?.()),
        ];
        return ids.filter(id => typeof id === 'string' && id.length > 0);
    }

    _matchesArmKey(id, key) {
        if (!id || !key) return false;
        if (id === key) return true;
        const lowerId = id.toLowerCase();
        const lowerKey = key.toLowerCase();
        if (lowerId === lowerKey) return true;
        if (lowerId.includes(lowerKey) || lowerKey.includes(lowerId)) return true;
        // Known daemon/launcher pairings (e.g. gnome-terminal launcher vs gnome-terminal-server daemon)
        if ((lowerKey === 'org.gnome.terminal' || lowerKey === 'gnome-terminal') &&
            (lowerId === 'gnome-terminal-server' || lowerId === 'gnome-terminal')) return true;
        if (lowerKey === 'gnome-terminal-server' &&
            (lowerId === 'org.gnome.terminal' || lowerId === 'gnome-terminal')) return true;
        return false;
    }

    _findArmedKey(ids) {
        for (const id of ids) {
            const queue = this._armedSpawns.get(id);
            if (queue && queue.length > 0)
                return id;
        }
        for (const id of ids) {
            for (const [key, queue] of this._armedSpawns) {
                if (queue.length === 0)
                    continue;
                if (this._matchesArmKey(id, key))
                    return key;
            }
        }
        return null;
    }

    _claimInstructions(window) {
        const state = this._getState(window);
        if (state.instructions || state.dispatched)
            return true;

        let instructions = null;
        let claimedKey = null;
        const key = this._findArmedKey(this._getIdentifiers(window));

        if (key !== null) {
            const queue = this._armedSpawns.get(key);
            instructions = queue.shift();
            claimedKey = key;
            if (queue.length === 0)
                this._armedSpawns.delete(key);
        } else if (this._wildcardTarget) {
            instructions = this._wildcardTarget;
            claimedKey = '*';
            this._clearWildcard();
        }

        if (!instructions)
            return false;

        state.instructions = instructions;
        state.targetId = claimedKey;
        this._heldWindows.delete(window);
        const actor = this._getActor(window);
        if (actor)
            this._cloak(actor);

        this._releaseHeldIfIdle();

        if (state.mapped)
            this._dispatch(window);
        return true;
    }

    _dispatch(window) {
        const state = this._getState(window);
        if (state.dispatched || !state.instructions)
            return;
        const actor = this._getActor(window);
        if (!actor)
            return;

        const instructions = state.instructions;
        const targetId = state.targetId || '';
        state.dispatched = true;
        state.instructions = null;
        this._heldWindows.delete(window);

        this.logTime('DISPATCH', `instructionsCount=${instructions.length}`, window);
        this._enqueueBatch(window, actor, instructions, targetId);
    }

    _handleWindowCreated(window) {
        if (this._disabled)
            return;

        this._suppressMapAnimation(window);
        const actor = this._getActor(window);
        if (this._hasArmedSpawns() && actor)
            this._cloak(actor);
        this.logTime('WINDOW_CREATED', `actorFound=${Boolean(actor)}`, window);

        if (this._claimInstructions(window))
            return;

        const state = this._getState(window);
        const retry = () => {
            if (this._claimInstructions(window))
                this._disconnectDiscoveryHandlers(window, state);
            else
                this._settleUnmatched(window);
        };
        state.notifyIds.push(
            this._tryConnect(window, 'notify::wm-class', retry),
            this._tryConnect(window, 'notify::gtk-application-id', retry),
            this._tryConnect(window, 'unmanaged', () => {
                this._heldWindows.delete(window);
                this._disconnectDiscoveryHandlers(window, state);
            }),
        );
    }

    _disconnectDiscoveryHandlers(window, state) {
        for (const id of state.notifyIds)
            this._safeDisconnect(window, id);
        state.notifyIds = [];
    }

    _handleActorMap(actor) {
        if (this._disabled)
            return;

        const window = actor.meta_window ?? actor.get_meta_window?.() ?? null;
        if (!window)
            return;

        this._suppressMapAnimation(window);

        const state = this._getState(window);
        state.mapped = true;
        const armed = this._hasArmedSpawns();

        if (state.instructions || state.dispatched || armed)
            this._cloak(actor);

        this.logTime('ACTOR_MAP', '', window);

        if (state.instructions) {
            this._dispatch(window);
            return;
        }
        if (state.dispatched)
            return;
        if (!armed) {
            if (this._cloakManager?.cloakedActors?.has(actor))
                this._uncloak(actor);
            return;
        }

        if (this._claimInstructions(window))
            return;

        this._heldWindows.add(window);
        this._settleUnmatched(window);
    }

    _hasIdentity(window) {
        return this._try(() => window.get_wm_class?.(), '') !== '' ||
            this._try(() => window.get_gtk_application_id?.(), '') !== '' ||
            this._try(() => window.get_sandboxed_app_id?.(), '') !== '';
    }

    _settleUnmatched(window) {
        const state = this._getState(window);
        if (!state.mapped || state.instructions || state.dispatched)
            return;
        if (!this._hasIdentity(window) && this._hasArmedSpawns())
            return;

        this._heldWindows.delete(window);
        const actor = this._getActor(window);
        this.logTime('REJECT_UNMATCHED', '', window);
        if (actor)
            this._uncloak(actor);
    }

    _releaseHeldIfIdle() {
        if (this._hasArmedSpawns())
            return;
        for (const window of [...this._heldWindows]) {
            const state = this._getState(window);
            this._heldWindows.delete(window);
            if (state.instructions || state.dispatched)
                continue;
            const actor = this._getActor(window);
            if (actor)
                this._uncloak(actor);
        }
    }

    // =======================================================================
    // 5. SERIALIZED BATCH QUEUE
    // =======================================================================

    _findBatchForWindow(window) {
        if (!window)
            return null;
        if (this._currentBatchItem && this._currentBatchItem.window === window)
            return this._currentBatchItem;
        if (Array.isArray(this._batchQueue))
            return this._batchQueue.find(item => item.window === window) || null;
        return null;
    }

    _enqueueBatch(window, actor, instructions, targetId = '') {
        const item = {
            window,
            actor,
            instructions,
            targetId,
            deferredFocus: false,
            focusApplied: false,
        };
        this._batchQueue.push(item);
        this._processQueue();
    }

    async _processQueue() {
        if (this._disabled || this._batchBusy || this._batchQueue.length === 0)
            return;

        this._batchBusy = true;
        const item = this._batchQueue.shift();
        this._currentBatchItem = item;
        const { window, actor, instructions, targetId } = item;

        try {
            await this._runBatch(window, actor, instructions, targetId, item);
        } catch (e) {
            console.error(`[SpawnAt] Batch processing failed in mutex queue: ${e}`);
            const winId = typeof window.get_id === 'function' ? window.get_id() : 0;
            try {
                const params = new GLib.Variant('(sbtiiuubs)', [
                    targetId || '',
                    false,
                    winId,
                    0, 0, 0, 0,
                    false,
                    e.message || String(e),
                ]);
                this._dbusManager?.emitSignal('SpawnClaimed', params);
            } catch (_err) {}
            this._uncloak(actor);
            this._clearSkippedEffects(actor);
            if (item.deferredFocus && !item.focusApplied) {
                this.logTime('DEFERRED_FOCUS_FALLBACK', 'batch failed; applying deferred focus', window);
                this._focusWindowObject(window);
                item.focusApplied = true;
            }
        } finally {
            this._logger?.flush();
            this._currentBatchItem = null;
            this._batchBusy = false;
            this._processQueue();
        }
    }

    // =======================================================================
    // 6. BATCH ENGINE & EXECUTION
    // =======================================================================

    _normalizeInstruction(inst) {
        if (!inst)
            return null;
        if (typeof inst === 'string')
            return { name: inst, args: {} };
        if (typeof inst === 'object') {
            if (typeof inst.name === 'string')
                return { name: inst.name, args: (inst.args && typeof inst.args === 'object') ? inst.args : {} };
            const name = Object.keys(inst)[0];
            if (name)
                return { name, args: (inst[name] && typeof inst[name] === 'object') ? inst[name] : {} };
        }
        return null;
    }

    async _runBatch(window, actor, instructions, targetId = '', item = null) {
        // Observe-only (x11_observe.js): from the start of the batch; a no-op unless logging is on.
        this._x11Observer?.start(window, actor, this._windowKind(window));

        const ops = instructions
            .map(inst => this._normalizeInstruction(inst))
            .filter(Boolean);

        const wasMinimized = Boolean(window.minimized || (typeof window.is_hidden === 'function' && window.is_hidden()));
        const wasCovered = this._isWindowFullyCovered(window);

        const ctx = {
            window,
            actor,
            targetId,
            item: item || this._findBatchForWindow(window),
            targetW: null,
            targetH: null,
            safeW: null,
            safeH: null,
            clampedSafeSize: null,
            preSetSizeFrame: null,
            preSetSizeBuf: null,
            setSizeAtMs: null,
            resize: null,
            anchorPayload: null,
            positionedW: -1,
            positionedH: -1,
            revealed: false,
            wasMinimized,
            wasCovered,
        };

        const sizeOp = ops.find(op => op.name === 'SetSize');
        if (sizeOp && sizeOp.args.w > 0 && sizeOp.args.h > 0) {
            ctx.targetW = sizeOp.args.w;
            ctx.targetH = sizeOp.args.h;
        }

        try {
            for (const op of ops) {
                if (this._disabled)
                    return;
                this.logTime('BATCH_STEP', `inst=${op.name}`, window);

                switch (op.name) {
                    case 'Snapshot':
                        if (this._isSnapshotEligible(window, actor)) {
                            this._cloakManager.createSnapshot(actor, window);
                        } else {
                            const reason = (window.minimized || (typeof window.is_hidden === 'function' && window.is_hidden()))
                                ? 'minimized'
                                : (this._isWindowFullyCovered(window)
                                    ? 'covered'
                                    : 'hidden_or_inactive_workspace');
                            this.logTime('SNAPSHOT_SKIPPED', `reason="${reason}"`, window);
                        }
                        break;
                    case 'Cloak':               this._cloak(actor); this._logActorTrace('CLOAK', actor, window); break;
                    case 'DestroySnapshot':     this._cloakManager.destroySnapshot(actor, window); break;
                    case 'SetSize':             this._stepSetSize(ctx, op.args); break;
                    case 'WaitForCommit':       await this._stepWaitForCommit(ctx, op.args); break;
                    case 'SetPositionAnchored': this._stepPosition(ctx, op.args); break;
                    case 'Uncloak':             await this._stepUncloak(ctx, op.args); break;
                    default:
                        console.error(`[SpawnAt] Unknown instruction "${op.name}" ignored`);
                        throw new Error(`[SpawnAt] Unknown instruction "${op.name}"`);
                }
            }
        } finally {
            if (!ctx.revealed) {
                this._uncloak(actor);
                this._logActorTrace('UNCLOAK', actor, window);
            }
            if (this._cloakManager?.snapshots?.has(actor)) {
                this._cloakManager.destroySnapshot(actor, window);
            }
            this._clearSkippedEffects(actor);
            this._logger?.flush();
        }
    }

    _stepSetSize(ctx, { w, h }) {
        const { window } = ctx;
        if (!(w > 0 && h > 0))
            return;

        const [minW, minH] = this._getMinSize(window);
        const floorW = Math.max(GEOMETRY_FLOOR_W, minW);
        const floorH = Math.max(GEOMETRY_FLOOR_H, minH);
        const safeW  = Math.max(w, floorW);
        const safeH  = Math.max(h, floorH);

        ctx.targetW = w;
        ctx.targetH = h;
        ctx.safeW = safeW;
        ctx.safeH = safeH;
        ctx.clampedSafeSize = { w: safeW, h: safeH };

        const preFrame = window.get_frame_rect();
        const preBuf = window.get_buffer_rect ? window.get_buffer_rect() : preFrame;
        ctx.preSetSizeFrame = preFrame;
        ctx.preSetSizeBuf = preBuf;

        if (safeW !== w || safeH !== h) {
            this.logTime('SET_SIZE_CLAMPED',
                `requestedSize=(${w}x${h}) reportedMinSize=(${minW}x${minH}) ` +
                `floor=(${floorW}x${floorH}) clampedSafeSize=(${safeW}x${safeH})`,
                window);
        }

        if (this._isMaximized(window))
            this._unmaximize(window);

        this.logTime('SET_SIZE_REQUEST',
            `requestedSize=(${w}x${h}) clampedSafeSize=(${safeW}x${safeH}) ` +
            `reportedMinSize=(${minW}x${minH}) ` +
            `preFrame=(${preFrame.x},${preFrame.y},${preFrame.width}x${preFrame.height}) ` +
            `bufferRect=(${preBuf.x},${preBuf.y},${preBuf.width}x${preBuf.height})`,
            window);

        if (window.move_resize_frame)
            window.move_resize_frame(true, preFrame.x, preFrame.y, safeW, safeH);
        else
            window.resize(true, safeW, safeH);
        ctx.setSizeAtMs = this.nowMs();
        this._x11Observer?.mark(window, 'set_size');

        this._logActorTrace('AFTER_SET_SIZE', ctx.actor, window);
    }

    // Native Wayland windows with a size request use the ready-wait machine; everything else keeps
    // the original rule (CommitLatch.waitForCommit) and says so with strategy=legacy. The kind, not
    // the session, decides: an XWayland window inside a Wayland session reports its frame size
    // immediately, which the machine would read as an instant answer.
    //
    // A developer override rides in `args` of this one instruction (`strategy`, `cap_ms`, validator.js)
    // and so applies to this spawn only; nothing is stored on the extension. `legacy` sends even a
    // native Wayland window down the original rule (`why=override-legacy`); `ready-wait` changes
    // nothing a native window would not do anyway; `cap_ms` replaces the 250 ms cap of the machine.
    async _stepWaitForCommit(ctx, args) {
        const { window } = ctx;
        const kind = this._windowKind(window);
        const hasRequest = ctx.safeW !== null && ctx.safeH !== null;
        const forceLegacy = args.strategy === 'legacy';
        const useMachine = kind === 'NativeWayland' && hasRequest && !forceLegacy;

        this._logWaitOverride(window, args, useMachine, hasRequest);

        if (useMachine) {
            const before = ctx.preSetSizeFrame;
            const request = {
                kind,
                target: [ctx.safeW, ctx.safeH],
                before: [before.width, before.height],
                resizable: typeof window.allows_resize === 'function' ? Boolean(window.allows_resize()) : true,
                fullscreen: this._isFullscreen(window),
                tiled: this._isTiled(window),
                startedAtMs: ctx.setSizeAtMs ?? this.nowMs(),
            };
            if (args.cap_ms !== undefined)
                request.policy = { cap_ms: args.cap_ms };
            const result = await this._commitLatch.waitForReady(window, request);
            ctx.resize = result;
            this._x11Observer?.mark(window, 'commit_resolved');
            return;
        }

        const startedAt = this.nowMs();
        const why = forceLegacy && kind === 'NativeWayland' && hasRequest
            ? 'override-legacy'
            : (hasRequest ? 'not-native-wayland' : 'no-size-request');
        this.logTime('READY_WAIT_START',
            `strategy=legacy kind=${kind} why=${why} ` +
            `timeoutMs=${args.timeout_ms}`,
            window);
        const reason = await this._commitLatch.waitForCommit(
            window, ctx.actor, args.timeout_ms,
            ctx.targetW, ctx.targetH, ctx.safeW, ctx.safeH);
        ctx.commit = reason;
        this._x11Observer?.mark(window, 'commit_resolved');
        this.logTime('READY_WAIT_RESOLVED',
            `strategy=legacy outcome=none elapsed_ms=${(this.nowMs() - startedAt).toFixed(1)} reason=${reason}`,
            window);
    }

    // Says what a developer override did, in one line before READY_WAIT_START (which then shows the
    // effective strategy and cap). Silent when the instruction carries no override field, so a
    // default spawn logs exactly what it did before. `unused` names valid fields that had no
    // effect on this window; `ignored` (last, free text) names fields the validator dropped.
    _logWaitOverride(window, args, useMachine, hasRequest) {
        const ignored = args.ignored ?? [];
        if (args.strategy === undefined && args.cap_ms === undefined && ignored.length === 0)
            return;

        const unused = [];
        if (args.strategy === 'ready-wait' && !useMachine)
            unused.push(`strategy(ready-wait:${hasRequest ? 'not-native-wayland' : 'no-size-request'})`);
        if (args.cap_ms !== undefined && !useMachine)
            unused.push('cap_ms(legacy-has-no-cap)');

        this.logTime('READY_WAIT_OVERRIDE',
            `strategy=${args.strategy ?? 'none'} cap_ms=${args.cap_ms ?? 'none'} ` +
            `unused=${unused.length > 0 ? unused.join(';') : 'none'} ` +
            `ignored=${ignored.length > 0 ? ignored.join(';') : 'none'}`,
            window);
    }

    // What the observe-only X11 logging (x11_observe.js) needs from the shell. Every call is
    // guarded: a missing API gives 'na' in the log, never an exception into the batch.
    //
    // `damaged` and `first-frame` exist on Meta.WindowActor in GNOME 46 (seen in the log, 80 of 80
    // runs). The texture size is read by readTextureSize (x11_observe.js): ShapedTexture has no
    // get_width, so it goes through its MultiTexture; HYPOTHESIS (not run in a Shell) that this
    // follows the committed buffer. The runtime check below decides, not a version.
    _x11ObserveHost() {
        const size = o => (o && Number.isFinite(o.w) && Number.isFinite(o.h))
            ? { width: o.w, height: o.h } : null;
        return {
            isLoggingEnabled: () => this.isLoggingEnabled(),
            logTime: (tag, extra, window) => this.logTime(tag, extra, window),
            nowMs: () => this.nowMs(),
            tryConnect: (t, s, cb) => this._tryConnect(t, s, cb),
            safeDisconnect: (t, id) => this._safeDisconnect(t, id),
            addTimer: (ms, cb) => this._addTimer(ms, cb),
            removeTimer: id => this._removeTimer(id),
            signalExists: (actor, name) => this._try(() => {
                const gtype = actor.constructor?.$gtype ?? Meta.WindowActor?.$gtype;
                return GObject.signal_lookup(name, gtype) !== 0;
            }, false),
            windowId: w => this._try(() => w.get_id(), -1),
            clientType: w => this._try(() => w.get_client_type?.(), null),
            isWayland: () => this._try(() => Meta.is_wayland_compositor(), false),
            frameRect: w => this._try(() => w.get_frame_rect(), null),
            bufferRect: w => this._try(() => w.get_buffer_rect?.(), null),
            textureSize: a => readTextureSize(a),
            actorSize: a => this._try(() => size({ w: a.width, h: a.height }), null),
        };
    }

    // 'NativeWayland', 'XWayland', 'X11' or 'unknown' (no get_client_type). Same rule as the
    // protocol label in logger.js: an X11 client under a Wayland compositor is XWayland.
    _windowKind(window) {
        const type = this._try(() => window.get_client_type?.(), null);
        if (type === null || type === undefined)
            return 'unknown';
        if (type === Meta.WindowClientType?.WAYLAND || type === 0)
            return 'NativeWayland';
        if (type === Meta.WindowClientType?.X11 || type === 1) {
            const wayland = typeof Meta.is_wayland_compositor === 'function' && Meta.is_wayland_compositor();
            return wayland ? 'XWayland' : 'X11';
        }
        return 'unknown';
    }

    // HYPOTHESIS: `get_tile_match` is in the Meta-14 typelib (CONFIRMED by `strings`), but whether it
    // is non-null for a window tiled alone, and whether `tile_mode` exists, was not run in a shell.
    _isTiled(window) {
        if (this._try(() => window.get_tile_match?.(), null))
            return true;
        const mode = window.tile_mode;
        return mode !== undefined && mode !== null && mode !== 0;
    }

    _stepPosition(ctx, payload) {
        const { window, actor } = ctx;
        ctx.anchorPayload = payload;

        if (this._isMaximized(window))
            this._unmaximize(window);

        const preFrame = window.get_frame_rect();
        const preBuf = window.get_buffer_rect ? window.get_buffer_rect() : preFrame;

        this._applyAnchoredPosition(window, payload);

        const postFrame = window.get_frame_rect();
        const postBuf = window.get_buffer_rect ? window.get_buffer_rect() : postFrame;
        ctx.positionedW = postFrame.width;
        ctx.positionedH = postFrame.height;

        this.logTime('POSITION_SET',
            `preFrame=(${preFrame.x},${preFrame.y},${preFrame.width}x${preFrame.height}) ` +
            `postFrame=(${postFrame.x},${postFrame.y},${postFrame.width}x${postFrame.height}) ` +
            `bufferRect=(${postBuf.x},${postBuf.y},${postBuf.width}x${postBuf.height}) ` +
            `actor=(${actor.x},${actor.y}) ` +
            `delta=(${postFrame.x - preFrame.x},${postFrame.y - preFrame.y})`,
            window);

        this._logActorTrace('AFTER_SET_POSITION_ANCHORED', actor, window);
    }

    async _stepUncloak(ctx, args) {
        const wantWake = args.wake !== false;
        this.logTime('UNCLOAK_4A', `delay=0ms (skipped)`, ctx.window);
        this._cloak(ctx.actor);

        const clamped = this._isToolkitClamped(ctx);
        const isVte = this._focusPulse.isVteCandidate(ctx.window);
        this.logTime('UNCLOAK_4B', `wantWake=${wantWake} clamped=${clamped} isVte=${isVte}`, ctx.window);

        if (wantWake && clamped) {
            if (!isVte) {
                this.logTime('FOCUS_PULSE_SKIP_NOT_VTE',
                    `reason="non-VTE toolkit; bypassing synthetic defocus"`,
                    ctx.window);
            } else {
                await this._focusPulse.execute(ctx);
                if (this._disabled)
                    return;
                this._cloak(ctx.actor);
            }
        }

        this._reanchorIfResized(ctx);

        const isCurrentlyMinimized = Boolean(ctx.window.minimized || (typeof ctx.window.is_hidden === 'function' && ctx.window.is_hidden()));
        const hasDeferredFocus = Boolean(ctx.item?.deferredFocus);
        if (ctx.wasMinimized || isCurrentlyMinimized) {
            this.logTime('PRE_REVEAL_UNMINIMIZE', 'unminimizing prior to uncloak', ctx.window);
            this._skipWindowEffects(ctx.actor);
            if (typeof ctx.window.unminimize === 'function')
                ctx.window.unminimize();
            this._clearSkippedEffects(ctx.actor);
            this._focusWindowObject(ctx.window);
            if (ctx.item)
                ctx.item.focusApplied = true;
        } else if (ctx.wasCovered || hasDeferredFocus) {
            this.logTime('PRE_REVEAL_RAISE',
                hasDeferredFocus ? 'raising prior to uncloak (deferred focus)' : 'window was covered; raising prior to uncloak',
                ctx.window);
            this._focusWindowObject(ctx.window);
            if (ctx.item)
                ctx.item.focusApplied = true;
        }

        await this._commitLatch.waitForFrames(global.stage, 1, FRAME_WAIT_CEILING_MS);
        if (this._disabled)
            return;

        // The 4C check above ran before this frame wait, and the reactive anchor is only armed by
        // _reveal(). A client resize that landed in between would be anchored for its old size.
        // So check again in the same tick as the reveal: anything before the check is corrected
        // here, anything after the arm reaches the reactive anchor, and nothing can land in
        // between because this tick runs without yielding to the main loop. After a correction
        // wait one more frame (bounded by LATE_RESIZE_MAX_PASSES) so the moved window is painted
        // before it is revealed; the last check is still in the reveal's tick.
        let passes = 0;
        while (this._reanchorLateResize(ctx) && ++passes < LATE_RESIZE_MAX_PASSES) {
            await this._commitLatch.waitForFrames(global.stage, 1, FRAME_WAIT_CEILING_MS);
            if (this._disabled)
                return;
        }

        this._reveal(ctx);
    }

    // Suppresses window animations (e.g. unminimize/map) for the actor.
    // Note: Inspected on GNOME Shell 46 (Ubuntu 24.04). Behavior on GNOME 42-44, 45, and 47-48
    // is unverified live and protected by feature-checks and try/catch fail-open semantics.
    _skipWindowEffects(actor) {
        if (!actor)
            return;
        try {
            if (typeof Main !== 'undefined' && Main.wm) {
                if (typeof Main.wm.skipNextEffect === 'function')
                    Main.wm.skipNextEffect(actor);
                else if (Main.wm._skippedActors && typeof Main.wm._skippedActors.add === 'function')
                    Main.wm._skippedActors.add(actor);
            }
            if (global.window_manager) {
                if (typeof global.window_manager.skipNextEffect === 'function')
                    global.window_manager.skipNextEffect(actor);
                else if (global.window_manager._skippedActors && typeof global.window_manager._skippedActors.add === 'function')
                    global.window_manager._skippedActors.add(actor);
            }
            actor.remove_all_transitions?.();
            actor.set_scale?.(1.0, 1.0);
            actor.set_pivot_point?.(0, 0);
            this._skippedEffectsActors?.add(actor);
        } catch (_e) {}
    }

    _clearSkippedEffects(actor) {
        if (!actor)
            return;
        try {
            this._skippedEffectsActors?.delete(actor);
            if (typeof Main !== 'undefined' && Main.wm && Main.wm._skippedActors)
                Main.wm._skippedActors.delete(actor);
            if (global.window_manager && global.window_manager._skippedActors)
                global.window_manager._skippedActors.delete(actor);
        } catch (_e) {}
    }

    _isToolkitClamped(ctx) {
        if (ctx.targetW === null || ctx.targetH === null)
            return false;
        const frame = ctx.window.get_frame_rect();
        return frame.width > ctx.targetW || frame.height > ctx.targetH;
    }

    _reanchorIfResized(ctx) {
        const { window } = ctx;
        if (!ctx.anchorPayload)
            return;

        if (this._isMaximized(window)) {
            this._unmaximize(window);
            this._applyAnchoredPosition(window, ctx.anchorPayload);
            const after = window.get_frame_rect();
            ctx.positionedW = after.width;
            ctx.positionedH = after.height;
            return;
        }

        const cur = window.get_frame_rect();
        const resized = cur.width !== ctx.positionedW || cur.height !== ctx.positionedH;
        this.logTime('UNCLOAK_4C',
            `positioned=(${ctx.positionedW}x${ctx.positionedH}) actual=(${cur.width}x${cur.height}) ` +
            `needsReanchor=${resized}`,
            window);
        if (!resized)
            return;

        this.logTime('REANCHOR_TRIGGERED', `newBounds=(${cur.width}x${cur.height})`, window);
        this._applyAnchoredPosition(window, ctx.anchorPayload);
        const after = window.get_frame_rect();
        ctx.positionedW = after.width;
        ctx.positionedH = after.height;
    }

    // Re-anchors a window whose frame size changed since it was last positioned, right before the
    // reveal. Returns true when it moved the window. Maximized windows are left to the 4C check.
    _reanchorLateResize(ctx) {
        const { window } = ctx;
        if (!ctx.anchorPayload || this._isMaximized(window))
            return false;

        const cur = window.get_frame_rect();
        if (cur.width === ctx.positionedW && cur.height === ctx.positionedH)
            return false;

        this.logTime('LATE_RESIZE_CORRECTED',
            `positioned=(${ctx.positionedW}x${ctx.positionedH}) actual=(${cur.width}x${cur.height})`,
            window);
        this._applyAnchoredPosition(window, ctx.anchorPayload);
        const after = window.get_frame_rect();
        ctx.positionedW = after.width;
        ctx.positionedH = after.height;
        return true;
    }

    _reveal(ctx) {
        const { window, actor } = ctx;
        this._uncloak(actor);
        ctx.revealed = true;
        this._logActorTrace('UNCLOAK', actor, window);

        const rect = window.get_frame_rect();
        const buf = window.get_buffer_rect ? window.get_buffer_rect() : rect;
        this.logTime('UNCLOAK_4D_REVEAL',
            `requestedSize=(${ctx.targetW}x${ctx.targetH}) ` +
            `postFrame=(${rect.x},${rect.y},${rect.width}x${rect.height}) ` +
            `bufferRect=(${buf.x},${buf.y},${buf.width}x${buf.height})`,
            window);

        this._x11Observer?.reveal(window);

        if (ctx.anchorPayload)
            this._armAnchor(window, ctx.anchorPayload);

        const winId =typeof window.get_id === 'function' ? window.get_id() : 0;
        const targetId = ctx.targetId || '';
        const sizeRaised = Boolean(ctx.clampedSafeSize && (ctx.clampedSafeSize.w > ctx.targetW || ctx.clampedSafeSize.h > ctx.targetH));

        // Only the ready-wait strategy has an outcome; legacy waits send none. Emitted first: the
        // CLI reads it after it has seen SpawnClaimed, and signals from one sender arrive in order.
        if (ctx.resize) {
            try {
                const { outcome, final } = ctx.resize;
                const uint = n => Math.max(0, Math.round(Number(n) || 0));
                this._dbusManager?.emitSignal('ResizeOutcome', new GLib.Variant('(ssuuus)', [
                    targetId,
                    outcome.outcome,
                    uint(final[0]),
                    uint(final[1]),
                    uint(outcome.waited_ms),
                    readyReason(outcome),
                ]));
            } catch (e) {
                console.error(`[SpawnAt] Failed to emit ResizeOutcome signal: ${e}`);
            }
        }

        try {
            const params = new GLib.Variant('(sbtiiuubs)', [
                targetId,
                true,
                winId,
                rect.x,
                rect.y,
                rect.width,
                rect.height,
                sizeRaised,
                '',
            ]);
            this._dbusManager?.emitSignal('SpawnClaimed', params);
        } catch (e) {
            console.error(`[SpawnAt] Failed to emit SpawnClaimed signal: ${e}`);
        }
    }

    // =======================================================================
    // 7. ANCHORED POSITIONING & REACTIVE ANCHORS
    // =======================================================================

    _computeAnchoredPosition(window, payload) {
        const frame = window.get_frame_rect();
        const minSize = this._getMinSize(window);
        const monitor = window.get_monitor();
        const bounds = payload.area === 'screen'
            ? global.display.get_monitor_geometry(monitor)
            : window.get_work_area_for_monitor(monitor);

        return computeAnchoredPosition(bounds, frame, minSize, payload);
    }

    _applyAnchoredPosition(window, payload) {
        const actor = this._getActor(window);
        actor?.remove_all_transitions?.();

        const pos = this._computeAnchoredPosition(window, payload);

        if (pos.minW === 0 && pos.minH === 0) {
            this.logTime('FALLBACK_USED',
                `reason="min size unreported" usingFrame=(${pos.effW}x${pos.effH})`,
                window);
        }
        if (pos.oversized) {
            console.warn(`[spawn-at] Window is oversized (${pos.effW}x${pos.effH}); ` +
                'bottom/right margins ignored to keep top-left reachable.');
        }
        if (pos.x !== pos.rawX || pos.y !== pos.rawY) {
            this.logTime('MARGIN_CLAMP_ENGAGED',
                `raw=(${pos.rawX},${pos.rawY}) clamped=(${pos.x},${pos.y})`,
                window);
        }
        this.logTime('APPLY_ANCHOR',
            `eff=(${pos.effW}x${pos.effH}) bounds=(${pos.bounds.x},${pos.bounds.y},` +
            `${pos.bounds.width}x${pos.bounds.height}) final=(${pos.x},${pos.y})`,
            window);

        this._moveFrame(window, pos.x, pos.y);
        return pos;
    }

    _armAnchor(window, payload) {
        this._disarmAnchor(window, 'REARM');

        const state = { payload, sizeId: 0, unmanagedId: 0, retireTimerId: 0 };
        state.sizeId = this._tryConnect(window, 'size-changed',
            () => this._onAnchoredSizeChanged(window, state));
        state.unmanagedId = this._tryConnect(window, 'unmanaged',
            () => this._disarmAnchor(window, 'UNMANAGED'));
        this._anchors.set(window, state);
        this.logTime('ANCHOR_ARMED', '', window);
    }

    _onAnchoredSizeChanged(window, state) {
        if (this._disabled)
            return;
        if (this._userIsGrabbing())
            return;
        if (this._isMaximized(window) || this._isFullscreen(window))
            return;

        const cur = window.get_frame_rect();
        const pos = this._computeAnchoredPosition(window, state.payload);

        if (pos.x === cur.x && pos.y === cur.y)
            return;

        this.logTime('ANCHOR_CORRECTION',
            `size=(${cur.width}x${cur.height}) from=(${cur.x},${cur.y}) to=(${pos.x},${pos.y})`,
            window);
        this._moveFrame(window, pos.x, pos.y);

        this._removeTimer(state.retireTimerId);
        state.retireTimerId = this._addTimer(ANCHOR_RETIRE_QUIET_MS,
            () => this._disarmAnchor(window, 'RETIRED'));
    }

    _disarmAnchor(window, reason = '') {
        if (!this._anchors)
            return;
        const state = this._anchors.get(window);
        if (!state)
            return;
        this._removeTimer(state.retireTimerId);
        this._safeDisconnect(window, state.sizeId);
        this._safeDisconnect(window, state.unmanagedId);
        this._anchors.delete(window);
        this.logTime('ANCHOR_RELEASED', `reason=${reason}`, window);
    }

    // =======================================================================
    // 8. WINDOW MANAGEMENT HELPERS
    // =======================================================================

    _getMRUWindows(workspace) {
        let tabListType = 0;
        if (Meta.TabList?.NORMAL !== undefined)
            tabListType = Meta.TabList.NORMAL;
        else if (Meta.TabListType?.NORMAL !== undefined)
            tabListType = Meta.TabListType.NORMAL;

        const list = this._try(() => global.display.get_tab_list(tabListType, workspace));
        if (list && list.length > 0)
            return list;

        return this._try(() => {
            const windows = workspace.list_windows();
            if (typeof global.display.sort_windows_by_stacking === 'function')
                return global.display.sort_windows_by_stacking(windows).slice().reverse();
            return windows;
        }, []);
    }

    _isWindowFullyCovered(window, workspace = null) {
        return this._try(() => {
            if (!window || typeof window.get_frame_rect !== 'function')
                return false;

            const targetRect = window.get_frame_rect();
            if (!targetRect || targetRect.width <= 0 || targetRect.height <= 0)
                return false;

            const wsMgr = global.workspace_manager || (typeof global.display?.get_workspace_manager === 'function'
                ? global.display.get_workspace_manager()
                : null);
            const ws = workspace || (wsMgr && typeof wsMgr.get_active_workspace === 'function'
                ? wsMgr.get_active_workspace()
                : null);

            if (!ws || typeof ws.list_windows !== 'function')
                return false;

            const windows = ws.list_windows();
            if (!Array.isArray(windows) || windows.length <= 1)
                return false;

            if (typeof global.display?.sort_windows_by_stacking !== 'function')
                return false;

            const stacked = global.display.sort_windows_by_stacking(windows);
            if (!Array.isArray(stacked))
                return false;

            const idx = stacked.indexOf(window);
            if (idx === -1 || idx === stacked.length - 1)
                return false;

            const aboveWindows = stacked.slice(idx + 1);
            const coveringRects = [];

            for (const aboveWin of aboveWindows) {
                if (!aboveWin || aboveWin === window)
                    continue;

                if (aboveWin.minimized || (typeof aboveWin.is_hidden === 'function' && aboveWin.is_hidden()))
                    continue;

                if (aboveWin.is_override_redirect?.())
                    continue;

                if (typeof aboveWin.located_on_workspace === 'function' && !aboveWin.located_on_workspace(ws)) {
                    if (!(typeof aboveWin.is_on_all_workspaces === 'function' && aboveWin.is_on_all_workspaces()))
                        continue;
                }

                const aboveActor = this._getActor(aboveWin);
                if (aboveActor) {
                    const isVis = typeof aboveActor.is_visible === 'function' ? aboveActor.is_visible() : aboveActor.visible;
                    if (isVis === false)
                        continue;
                    if (typeof aboveActor.opacity === 'number' && aboveActor.opacity < 255)
                        continue;
                }

                if (typeof aboveWin.get_frame_rect === 'function') {
                    const r = aboveWin.get_frame_rect();
                    if (r && r.width > 0 && r.height > 0) {
                        // Fast path: if a single window above completely encloses targetRect
                        if (r.x <= targetRect.x && r.y <= targetRect.y &&
                            (r.x + r.width) >= (targetRect.x + targetRect.width) &&
                            (r.y + r.height) >= (targetRect.y + targetRect.height)) {
                            return true;
                        }
                        coveringRects.push(r);
                    }
                }
            }

            if (coveringRects.length === 0)
                return false;

            // Multi-window coverage: subtract all covering rectangles from targetRect
            let regions = [{ ...targetRect }];

            for (const cov of coveringRects) {
                const nextRegions = [];
                for (const reg of regions) {
                    const left = Math.max(reg.x, cov.x);
                    const top = Math.max(reg.y, cov.y);
                    const right = Math.min(reg.x + reg.width, cov.x + cov.width);
                    const bottom = Math.min(reg.y + reg.height, cov.y + cov.height);

                    if (right <= left || bottom <= top) {
                        nextRegions.push(reg);
                        continue;
                    }

                    if (top > reg.y) {
                        nextRegions.push({ x: reg.x, y: reg.y, width: reg.width, height: top - reg.y });
                    }
                    if (reg.y + reg.height > bottom) {
                        nextRegions.push({ x: reg.x, y: bottom, width: reg.width, height: (reg.y + reg.height) - bottom });
                    }
                    if (left > reg.x) {
                        nextRegions.push({ x: reg.x, y: top, width: left - reg.x, height: bottom - top });
                    }
                    if (reg.x + reg.width > right) {
                        nextRegions.push({ x: right, y: top, width: (reg.x + reg.width) - right, height: bottom - top });
                    }
                }
                regions = nextRegions;
                if (regions.length === 0)
                    return true;
            }

            return regions.length === 0;
        }, false);
    }

    _isSnapshotEligible(window, actor) {
        if (!window || !actor)
            return false;
        if (actor.is_destroyed?.())
            return false;

        if (window.minimized || (typeof window.is_hidden === 'function' && window.is_hidden()))
            return false;

        const isVis = typeof actor.is_visible === 'function' ? actor.is_visible() : actor.visible;
        if (isVis === false)
            return false;

        const wsMgr = global.workspace_manager || (typeof global.display?.get_workspace_manager === 'function'
            ? global.display.get_workspace_manager()
            : null);
        const activeWs = wsMgr?.get_active_workspace?.();
        if (activeWs && typeof window.located_on_workspace === 'function') {
            const onAll = typeof window.is_on_all_workspaces === 'function' && window.is_on_all_workspaces();
            if (!onAll && !window.located_on_workspace(activeWs))
                return false;
        }

        if (this._isWindowFullyCovered(window, activeWs))
            return false;

        return true;
    }

    _findWindow(target) {
        const workspace = global.display.get_workspace_manager().get_active_workspace();
        const windows = this._getMRUWindows(workspace);

        const targetNum = parseInt(target, 10);
        if (!isNaN(targetNum) && targetNum > 0) {
            const byId = windows.find(w => w.get_id?.() === targetNum);
            if (byId)
                return byId;
            const byPid = windows.find(w => w.get_pid?.() === targetNum);
            if (byPid)
                return byPid;
        }

        return windows.find(w => {
            const wmClass = w.get_wm_class?.() || '';
            const appId = w.get_gtk_application_id?.() || '';
            return wmClass === target || appId === target;
        }) || null;
    }

    _focusWindowObject(window) {
        const actor = this._getActor(window);
        this._logActorTrace('RAISE', actor, window);
        if (typeof window.raise === 'function')
            window.raise();
        this._logActorTrace('ACTIVATE', actor, window);
        if (typeof window.activate === 'function')
            window.activate(global.get_current_time());
        return true;
    }

    _defocusWindowObject(window, mode = 'desktop', destination = '') {
        if (mode === 'desktop') {
            global.display.unset_input_focus(global.get_current_time());
            if (this._dummy)
                global.stage.set_key_focus(this._dummy);
            return true;
        }

        if (mode === 'window' && destination) {
            const destWin = this._findWindow(destination);
            if (destWin) {
                const actor = this._getActor(destWin);
                this._logActorTrace('RAISE', actor, destWin);
                if (typeof destWin.raise === 'function')
                    destWin.raise();
                this._logActorTrace('ACTIVATE', actor, destWin);
                destWin.activate(global.get_current_time());
                return true;
            }
        }

        const workspace = global.display.get_workspace_manager().get_active_workspace();
        const next = this._getMRUWindows(workspace)
            .find(w => w !== window && !w.minimized && !w.skip_taskbar);

        if (next) {
            const actor = this._getActor(next);
            this._logActorTrace('RAISE', actor, next);
            if (typeof next.raise === 'function')
                next.raise();
            this._logActorTrace('ACTIVATE', actor, next);
            next.activate(global.get_current_time());
        } else {
            global.display.unset_input_focus(global.get_current_time());
            if (this._dummy)
                global.stage.set_key_focus(this._dummy);
        }
        return true;
    }

    _clearWildcard() {
        this._removeTimer(this._wildcardTimeoutId);
        this._wildcardTimeoutId = 0;
        this._wildcardTarget = null;
    }

    // =======================================================================
    // 9. D-BUS API IMPLEMENTATION
    // =======================================================================

    // 6 adds the ResizeOutcome signal (emitted before SpawnClaimed for native Wayland windows).
    get ProtocolVersion() {
        return 6;
    }

    DisarmSpawn(target_id) {
        if (!target_id)
            return false;
        if (target_id === '*' && this._wildcardTarget) {
            this._clearWildcard();
            return true;
        }
        if (this._armedSpawns && this._armedSpawns.has(target_id)) {
            this._armedSpawns.delete(target_id);
            this._releaseHeldIfIdle();
            return true;
        }
        return false;
    }

    ArmSpawn(target_id, instructions_json) {
        this.logTime('ARM_SPAWN', `target="${target_id}"`);

        let instructions;
        try {
            instructions = validateInstructions(instructions_json);
        } catch (e) {
            console.error(`[SpawnAt] Invalid instructions: ${e.message}`);
            return;
        }

        if (!target_id || target_id === '*') {
            this._clearWildcard();
            this._wildcardTarget = instructions;
            this._wildcardTimeoutId = this._addTimer(WILDCARD_EXPIRY_MS, () => {
                this._wildcardTimeoutId = 0;
                this._wildcardTarget = null;
                this._releaseHeldIfIdle();
            });
            return;
        }

        if (!this._armedSpawns.has(target_id))
            this._armedSpawns.set(target_id, []);
        const queue = this._armedSpawns.get(target_id);
        if (queue.length >= 16) {
            console.warn(`[SpawnAt] Queue limit (16) reached for target "${target_id}", dropping oldest`);
            queue.shift();
        }
        queue.push(instructions);

        this._addTimer(ARM_EXPIRY_MS, () => {
            const queue = this._armedSpawns.get(target_id);
            if (!queue)
                return;
            const idx = queue.indexOf(instructions);
            if (idx !== -1)
                queue.splice(idx, 1);
            if (queue.length === 0)
                this._armedSpawns.delete(target_id);
            this._releaseHeldIfIdle();
        });
    }

    ExecuteBatch(target_id, instructions_json) {
        let instructions;
        try {
            instructions = validateInstructions(instructions_json);
        } catch (e) {
            console.error(`[SpawnAt] Invalid instructions: ${e.message}`);
            return;
        }

        const window = this._findWindow(target_id);
        if (!window) {
            console.error(`[SpawnAt] ExecuteBatch: no window matches "${target_id}"`);
            return;
        }
        const actor = this._getActor(window);
        if (actor)
            this._enqueueBatch(window, actor, instructions, target_id);
    }

    GetCursor() {
        const [x, y] = global.get_pointer();
        return [x, y];
    }

    GetPointer() {
        return this.GetCursor();
    }

    GetWorkareas() {
        const workspace = global.display.get_workspace_manager().get_active_workspace();
        const count = global.display.get_n_monitors();
        const areas = [];
        for (let i = 0; i < count; i++) {
            const rect = workspace.get_work_area_for_monitor(i);
            areas.push({ x: rect.x, y: rect.y, w: rect.width, h: rect.height });
        }
        return JSON.stringify(areas);
    }

    GetLayout() {
        const workspace = global.display.get_workspace_manager().get_active_workspace();
        const count = global.display.get_n_monitors();
        const primaryIndex = this._try(() => global.display.get_primary_monitor(), 0);
        const monitors = [];

        for (let i = 0; i < count; i++) {
            const screenGeom = this._try(() => global.display.get_monitor_geometry(i), null);
            const waGeom = this._try(() => workspace.get_work_area_for_monitor(i), null);
            const scale = this._try(() => global.display.get_monitor_scale?.(i), null);
            const name = this._try(() => {
                if (typeof Main !== 'undefined' && Main.layoutManager?.monitors?.[i]?.connector)
                    return Main.layoutManager.monitors[i].connector;
                const mm = global.backend?.get_monitor_manager?.();
                return mm?.get_monitors?.()?.[i]?.get_connector?.() || null;
            }, null);

            monitors.push({
                index: i,
                name: name || null,
                primary: i === primaryIndex,
                scale: typeof scale === 'number' ? scale : null,
                screen: screenGeom ? { x: screenGeom.x, y: screenGeom.y, w: screenGeom.width, h: screenGeom.height } : null,
                workarea: waGeom ? { x: waGeom.x, y: waGeom.y, w: waGeom.width, h: waGeom.height } : null,
            });
        }
        return JSON.stringify({
            schema_version: 1,
            monitors,
        });
    }

    GetWindows() {
        const workspace = global.display.get_workspace_manager().get_active_workspace();
        const windows = this._getMRUWindows(workspace);
        return JSON.stringify(windows.map(win => {
            const frame = win.get_frame_rect();
            const ws = win.get_workspace ? win.get_workspace() : null;
            const wsIndex = (ws && typeof ws.index === 'function') ? ws.index() : null;
            let state = 'normal';
            if (win.minimized) {
                state = 'minimized';
            } else if (this._isFullscreen(win)) {
                state = 'fullscreen';
            } else if (this._isMaximized(win)) {
                state = 'maximized';
            }

            return {
                id: win.get_id ? win.get_id() : null,
                pid: win.get_pid ? win.get_pid() : -1,
                title: win.get_title ? (win.get_title() || '') : '',
                class: win.get_wm_class ? (win.get_wm_class() || win.get_gtk_application_id?.() || '') : '',
                app_id: win.get_gtk_application_id ? (win.get_gtk_application_id() || '') : '',
                x: frame.x,
                y: frame.y,
                w: frame.width,
                h: frame.height,
                focused: win.has_focus(),
                maximized: this._isMaximized(win),
                minimized: Boolean(win.minimized),
                monitor: typeof win.get_monitor === 'function' ? win.get_monitor() : 0,
                state,
                workspace: wsIndex,
            };
        }));
    }

    MoveWindow(app_id, x, y) {
        const window = this._findWindow(app_id);
        if (!window)
            return;
        this._disarmAnchor(window, 'EXTERNAL_MOVE');
        this._moveFrame(window, x, y);
    }

    FocusWindow(target) {
        const window = this._findWindow(target);
        if (!window)
            return false;

        const pendingItem = this._findBatchForWindow(window);
        if (pendingItem) {
            if (pendingItem.focusApplied) {
                this.logTime('FOCUS_BATCH_ALREADY_APPLIED', `windowId=${window.get_id?.()}`, window);
                this._logger?.flush();
                return true;
            }
            pendingItem.deferredFocus = true;
            this.logTime('FOCUS_DEFERRED', `windowId=${window.get_id?.()}`, window);
            this._logger?.flush();
            return true;
        }

        const res = this._focusWindowObject(window);
        this._logger?.flush();
        return res;
    }

    DefocusWindow(target, mode, destination) {
        const window = this._findWindow(target);
        const res = window ? this._defocusWindowObject(window, mode, destination) : false;
        this._logger?.flush();
        return res;
    }

    SetWindowState(target, state) {
        const window = this._findWindow(target);
        if (!window)
            return false;

        this._disarmAnchor(window, 'EXTERNAL_STATE');

        let res = false;
        switch (state) {
            case 'maximize':
                this._maximize(window);
                res = true;
                break;
            case 'unmaximize':
                this._unmaximize(window);
                res = true;
                break;
            case 'minimize': {
                const actor = this._getActor(window);
                this._logActorTrace('MINIMIZE', actor, window);
                window.minimize();
                res = true;
                break;
            }
            case 'unminimize': {
                const actor = this._getActor(window);
                this._logActorTrace('UNMINIMIZE', actor, window);
                window.unminimize();
                res = true;
                break;
            }
            case 'restore': {
                if (window.minimized) {
                    const actor = this._getActor(window);
                    this._logActorTrace('UNMINIMIZE', actor, window);
                    window.unminimize();
                }
                if (this._isMaximized(window))
                    this._unmaximize(window);
                res = true;
                break;
            }
            default:
                res = false;
                break;
        }
        this._logger?.flush();
        return res;
    }

    CloseWindow(target) {
        const window = this._findWindow(target);
        if (!window)
            return false;

        this._disarmAnchor(window, 'EXTERNAL_CLOSE');
        if (typeof window.delete === 'function') {
            window.delete(global.get_current_time());
            return true;
        }
        return false;
    }
}
