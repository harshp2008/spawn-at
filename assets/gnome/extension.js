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
import Gio from 'gi://Gio';
import Meta from 'gi://Meta';
import GLib from 'gi://GLib';
import Clutter from 'gi://Clutter';

import { DBusManager } from './dbus.js';
import { CloakManager } from './cloak.js';
import { CommitLatch } from './commit.js';
import { FocusPulse } from './pulse.js';
import { computeAnchoredPosition } from './anchor.js';
import { SessionLogger } from './logger.js';
import { validateInstructions } from './validator.js';

const ARM_EXPIRY_MS = 15000;
const WILDCARD_EXPIRY_MS = 1200;
const GEOMETRY_FLOOR_W = 100;
const GEOMETRY_FLOOR_H = 60;
const ANCHOR_RETIRE_QUIET_MS = 1000;
const FRAME_WAIT_CEILING_MS = 50;

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
        this._armedSpawns = new Map();
        this._wildcardTarget = null;
        this._wildcardTimeoutId = 0;
        this._anchors = new Map();

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
            { Clutter }
        );

        // Initialize commit and pulse managers
        this._commitLatch = new CommitLatch(this);
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

        // Flush persistent logs
        this._logger?.flushSync();

        // Teardown D-Bus
        this._dbusManager?.unexport();
    }

    // =======================================================================
    // 2. LOGGING DELEGATES
    // =======================================================================

    logTime(tag, extra = '', window = null) {
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

    _enqueueBatch(window, actor, instructions, targetId = '') {
        this._batchQueue.push({ window, actor, instructions, targetId });
        this._processQueue();
    }

    async _processQueue() {
        if (this._disabled || this._batchBusy || this._batchQueue.length === 0)
            return;

        this._batchBusy = true;
        const { window, actor, instructions, targetId } = this._batchQueue.shift();

        try {
            await this._runBatch(window, actor, instructions, targetId);
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
        } finally {
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

    async _runBatch(window, actor, instructions, targetId = '') {
        const ops = instructions
            .map(inst => this._normalizeInstruction(inst))
            .filter(Boolean);

        const ctx = {
            window,
            actor,
            targetId,
            targetW: null,
            targetH: null,
            safeW: null,
            safeH: null,
            clampedSafeSize: null,
            preSetSizeFrame: null,
            preSetSizeBuf: null,
            anchorPayload: null,
            positionedW: -1,
            positionedH: -1,
            revealed: false,
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
                    case 'Snapshot':            this._cloakManager.createSnapshot(actor); break;
                    case 'Cloak':               this._cloak(actor); break;
                    case 'DestroySnapshot':     this._cloakManager.destroySnapshot(actor); break;
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
            if (!ctx.revealed)
                this._uncloak(actor);
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
    }

    async _stepWaitForCommit(ctx, args) {
        ctx.commit = await this._commitLatch.waitForCommit(
            ctx.window, ctx.actor, args.timeout_ms,
            ctx.targetW, ctx.targetH, ctx.safeW, ctx.safeH);
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

        await this._commitLatch.waitForFrames(global.stage, 1, FRAME_WAIT_CEILING_MS);
        if (this._disabled)
            return;

        this._reveal(ctx);
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

    _reveal(ctx) {
        const { window, actor } = ctx;
        this._uncloak(actor);
        ctx.revealed = true;

        const rect = window.get_frame_rect();
        const buf = window.get_buffer_rect ? window.get_buffer_rect() : rect;
        this.logTime('UNCLOAK_4D_REVEAL',
            `requestedSize=(${ctx.targetW}x${ctx.targetH}) ` +
            `postFrame=(${rect.x},${rect.y},${rect.width}x${rect.height}) ` +
            `bufferRect=(${buf.x},${buf.y},${buf.width}x${buf.height})`,
            window);

        if (ctx.anchorPayload)
            this._armAnchor(window, ctx.anchorPayload);

        const winId = typeof window.get_id === 'function' ? window.get_id() : 0;
        const targetId = ctx.targetId || '';
        const sizeRaised = Boolean(ctx.clampedSafeSize && (ctx.clampedSafeSize.w > ctx.targetW || ctx.clampedSafeSize.h > ctx.targetH));
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
                destWin.activate(global.get_current_time());
                return true;
            }
        }

        const workspace = global.display.get_workspace_manager().get_active_workspace();
        const next = this._getMRUWindows(workspace)
            .find(w => w !== window && !w.minimized && !w.skip_taskbar);

        if (next) {
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

    get ProtocolVersion() {
        return 2;
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

    GetWindows() {
        const workspace = global.display.get_workspace_manager().get_active_workspace();
        const windows = this._getMRUWindows(workspace);
        return JSON.stringify(windows.map(win => {
            const frame = win.get_frame_rect();
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
        return window ? this._focusWindowObject(window) : false;
    }

    DefocusWindow(target, mode, destination) {
        const window = this._findWindow(target);
        return window ? this._defocusWindowObject(window, mode, destination) : false;
    }

    SetWindowState(target, state) {
        const window = this._findWindow(target);
        if (!window)
            return false;

        this._disarmAnchor(window, 'EXTERNAL_STATE');

        switch (state) {
            case 'maximize':
                this._maximize(window);
                return true;
            case 'unmaximize':
                this._unmaximize(window);
                return true;
            case 'minimize':
                window.minimize();
                return true;
            case 'unminimize':
                window.unminimize();
                return true;
            case 'restore':
                if (window.minimized)
                    window.unminimize();
                if (this._isMaximized(window))
                    this._unmaximize(window);
                return true;
            default:
                return false;
        }
    }
}
