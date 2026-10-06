/**
 * spawn-at — precise window spawning, sizing, positioning and orchestration
 * for GNOME Shell (Wayland / X11).
 *
 * ARCHITECTURE OVERVIEW
 * ---------------------
 *  1. Discovery   : D-Bus `ArmSpawn` registers instructions; `window-created` / `map`
 *                   claim them for the matching window and hard-cloak it (opacity 0).
 *  2. Queue       : claimed batches run one at a time (FIFO mutex) so Wayland configure
 *                   traffic for concurrent spawns never interleaves.
 *  3. Pipeline    : SetSize -> WaitForCommit -> SetPositionAnchored -> Uncloak.
 *  4. Anchor      : after reveal, a *reactive anchor* keeps the window glued to its
 *                   requested screen anchor if the client later changes its own size.
 *
 * THE GTK3/VTE "76px -> 93px" PROBLEM (why the code looks the way it does)
 * ------------------------------------------------------------------------
 *  A background gnome-terminal window commits only its bare headerbar (~76px). VTE
 *  finishes its row/column layout only after the window has *lost keyboard focus*
 *  (empirically: alt-tab away fixes it; typing, resizing or clicking do not). Mutter
 *  pins the top-left corner, so the later growth pushes the bottom edge off-screen.
 *
 *  Two independent defences are used, neither of which guesses geometry:
 *   - FOCUS CYCLE (wake): while the window is still cloaked, give *this exact window*
 *     focus and immediately hand focus back to whoever had it. The resulting focus-out
 *     makes the toolkit finish layout before the user ever sees the window.
 *   - REACTIVE ANCHOR: if the window still changes size later (focus cycle rejected,
 *     client changes its mind, ...), re-solve the anchored position on `size-changed`.
 *
 * INSTRUCTION SCHEMA (JSON array passed over D-Bus)
 * -------------------------------------------------
 *   "Snapshot" | "Cloak" | "DestroySnapshot"
 *   { SetSize: { w, h } }
 *   { WaitForCommit: { timeout_ms } }
 *   { SetPositionAnchored: { screen_anchor_x, screen_anchor_y, pivot_u, pivot_v,
 *                            offset_x, offset_y, margin_top/bottom/left/right,
 *                            area: 'screen' | 'workarea' } }
 *   { Uncloak: { delay_ms, wake } }      // wake:false disables the focus cycle
 */

import { Extension } from 'resource:///org/gnome/shell/extensions/extension.js';
import Gio from 'gi://Gio';
import Meta from 'gi://Meta';
import GLib from 'gi://GLib';
import Clutter from 'gi://Clutter';

// ===========================================================================
// D-BUS INTERFACE
// ===========================================================================

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

// ===========================================================================
// TIMING CONSTANTS (the only "magic numbers" — none depend on pixel sizes)
// ===========================================================================

/** How long an armed instruction set lives if no window claims it. */
const ARM_EXPIRY_MS = 15000;
/** How long a wildcard ("*") arm lives if no window claims it. */
const WILDCARD_EXPIRY_MS = 1200;
/** How long an unmatched freshly-mapped window stays cloaked waiting for a late match. */
const PREEMPTIVE_CLOAK_MS = 100;

/** Commit latch: geometry must be unchanged this long to count as "settled". */
const COMMIT_QUIET_MS = 60;
/** Commit latch: minimum wait when the window never changes size at all. */
const COMMIT_MIN_TIMEOUT_MS = 120;
/** Commit latch: absolute upper bound, even if the client keeps resizing. */
const COMMIT_HARD_CAP_MS = 1500;

/** Default blind delay at the start of the Uncloak phase (lets buffers paint). */
const DEFAULT_UNCLOAK_DELAY_MS = 300;

/** Focus cycle: how long to wait for a focus change to be honoured. */
const FOCUS_WAIT_MS = 150;
/** After a wake: wait this long for any size change to even begin. */
const WAKE_IDLE_MS = 120;
/** After a wake: size must be quiet this long once it starts changing. */
const WAKE_QUIET_MS = 60;
/** After a wake: absolute upper bound. */
const WAKE_CEILING_MS = 300;

/** Reactive anchor retires this long after its last correction. */
const ANCHOR_RETIRE_QUIET_MS = 1000;

// ===========================================================================
// EXTENSION
// ===========================================================================

export default class SpawnAtExtension extends Extension {

    // =======================================================================
    // 1. LIFECYCLE
    // =======================================================================

    enable() {
        this._disabled = false;

        // Pending spawn instructions: target_id -> Array<instructions[]>
        this._armedSpawns = new Map();
        // Instructions for a "*" arm, claimed by the first window that appears
        this._wildcardTarget = null;
        this._wildcardTimeoutId = 0;

        // Every GLib timer we own, so disable() can cancel them all
        this._timerIds = new Set();
        // Actors we have cloaked, so disable() can never leave one invisible
        this._cloakedActors = new Set();
        // Clutter.Clone snapshots keyed by their source actor
        this._snapshots = new Map();

        // Per-window bookkeeping for discovery (instructions, mapped flag, ...)
        this._windowStates = new WeakMap();
        // Windows currently under reactive-anchor control: Meta.Window -> state
        this._anchors = new Map();

        // FIFO mutex queue state
        this._batchQueue = [];
        this._batchBusy = false;

        // Compositor signals
        this._windowCreatedId = this._tryConnect(global.display, 'window-created',
            (_display, window) => this._handleWindowCreated(window));
        this._mapId = this._tryConnect(global.window_manager, 'map',
            (_wm, actor) => this._handleActorMap(actor));

        // If the user starts dragging/resizing an anchored window, they own it now
        this._grabEndId = this._tryConnect(global.display, 'grab-op-end',
            (_display, window) => {
                if (window && this._anchors.has(window))
                    this._disarmAnchor(window, 'USER_GRAB');
            });

        // Monitor/workarea changes are forwarded to D-Bus listeners
        this._monitorManager = this._try(() => global.backend?.get_monitor_manager?.() ?? null);
        if (this._monitorManager) {
            this._monitorsChangedId = this._tryConnect(this._monitorManager, 'monitors-changed',
                () => this._emitWorkareaChanged());
        }
        this._workareasChangedId = this._tryConnect(global.display, 'workareas-changed',
            () => this._emitWorkareaChanged());

        // D-Bus export
        try {
            this._dbusImpl = Gio.DBusExportedObject.wrapJSObject(DBUS_IFACE, this);
            this._dbusImpl.export(Gio.DBus.session, '/org/gnome/Shell/Extensions/SpawnAt');
        } catch (e) {
            console.error(`[SpawnAt] Failed to export D-Bus interface: ${e}`);
        }
    }

    disable() {
        // Tell any in-flight async batch to stop touching things
        this._disabled = true;
        this._batchQueue = [];
        this._batchBusy = false;

        // Release every reactive anchor (disconnects per-window signals)
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

        // Cancel all timers (including wildcard and expiry timers)
        for (const id of this._timerIds)
            GLib.Source.remove(id);
        this._timerIds.clear();
        this._wildcardTimeoutId = 0;
        this._wildcardTarget = null;
        this._armedSpawns.clear();

        // Never leave a window invisible or a snapshot overlay behind
        for (const actor of [...this._cloakedActors])
            this._uncloak(actor);
        for (const actor of [...this._snapshots.keys()])
            this._destroySnapshot(actor);

        // Tear down D-Bus
        if (this._dbusImpl) {
            this._try(() => this._dbusImpl.unexport());
            this._dbusImpl = null;
        }
    }

    // =======================================================================
    // 2. LOGGING
    // =======================================================================

    /** Debug logging is enabled by SPAWN_AT_DEBUG=1|true or the SetLogging D-Bus call. */
    _isLoggingEnabled() {
        if (this._loggingEnabled !== undefined)
            return this._loggingEnabled;
        const env = GLib.getenv('SPAWN_AT_DEBUG');
        this._loggingEnabled = env === '1' || env === 'true';
        return this._loggingEnabled;
    }

    /** Timestamped debug line, relative to the most recent ArmSpawn call. */
    _logTime(tag, extra = '') {
        if (!this._isLoggingEnabled())
            return;
        const nowUs = GLib.get_monotonic_time();
        if (!this._t0)
            this._t0 = nowUs;
        const elapsedMs = ((nowUs - this._t0) / 1000.0).toFixed(2);
        console.error(`[spawn-at-time] +${elapsedMs}ms | ${tag} ${extra}`);
    }

    /** D-Bus: toggle debug logging at runtime. */
    SetLogging(enabled) {
        this._loggingEnabled = Boolean(enabled);
    }

    // =======================================================================
    // 3. SMALL UTILITIES
    // =======================================================================

    /** Run fn, returning `fallback` if it throws. */
    _try(fn, fallback = null) {
        try { return fn(); } catch (_e) { return fallback; }
    }

    /** connect() wrapper that logs instead of throwing; returns 0 on failure. */
    _tryConnect(target, signal, callback) {
        try {
            return target.connect(signal, callback);
        } catch (e) {
            console.error(`[SpawnAt] Failed to connect '${signal}': ${e}`);
            return 0;
        }
    }

    /** disconnect() wrapper that tolerates dead objects and zero ids. */
    _safeDisconnect(target, id) {
        if (!target || !id)
            return;
        this._try(() => target.disconnect(id));
    }

    /** Coerce to a finite number, otherwise return the default. */
    _num(value, fallback = 0) {
        return Number.isFinite(value) ? value : fallback;
    }

    /**
     * Schedule a one-shot timer that is tracked for cleanup.
     * @returns {number} source id usable with _removeTimer
     */
    _addTimer(ms, callback) {
        const id = GLib.timeout_add(GLib.PRIORITY_DEFAULT, ms, () => {
            this._timerIds.delete(id);
            callback();
            return GLib.SOURCE_REMOVE;
        });
        this._timerIds.add(id);
        return id;
    }

    /** Cancel a timer created with _addTimer (safe on 0 / already-fired ids). */
    _removeTimer(id) {
        if (id && this._timerIds.delete(id))
            GLib.Source.remove(id);
    }

    /** Promise that resolves after `ms` milliseconds. */
    _sleep(ms) {
        return new Promise(resolve => this._addTimer(ms, resolve));
    }

    /** Resolve the Clutter actor for a Meta.Window across Shell versions. */
    _getActor(window) {
        if (typeof window.get_compositor_private === 'function')
            return window.get_compositor_private();
        return global.window_manager.get_window_actor_for_meta_window?.(window) ?? null;
    }

    /** Disable Mutter's map animation (it would fade the actor in over our cloak). */
    _suppressMapAnimation(window) {
        if ('no_map_animation' in window)
            window.no_map_animation = true;
    }

    // --- Version-tolerant window state helpers -----------------------------
    // Newer Mutter versions changed/removed the MaximizeFlags-based API, so every
    // call is feature-detected with a fallback instead of assumed.

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

    /** Toolkit-reported minimum size as [w, h]; [0, 0] when unavailable. */
    _getMinSize(window) {
        const size = this._try(() => window.get_min_size?.(), null);
        if (!size)
            return [0, 0];
        return [this._num(size[0]), this._num(size[1])];
    }

    /** True while the user is interactively moving/resizing any window. */
    _userIsGrabbing() {
        const op = this._try(() => global.display.get_grab_op?.());
        return op !== undefined && op !== null && op !== Meta.GrabOp.NONE;
    }

    /** Move a window's frame (frame coords account for CSD shadows). */
    _moveFrame(window, x, y) {
        if (window.move_frame)
            window.move_frame(true, x, y);
        else
            window.move(true, x, y);
    }

    _emitWorkareaChanged() {
        this._dbusImpl?.emit_signal('WorkareaChanged', null);
    }

    // =======================================================================
    // 4. SPAWN DISCOVERY (window-created / map)
    // =======================================================================

    /** True when any arm (specific or wildcard) is waiting for a window. */
    _hasArmedSpawns() {
        return this._armedSpawns.size > 0 || this._wildcardTarget !== null;
    }

    /** Fetch (or lazily create) the discovery state for a window. */
    _getState(window) {
        let state = this._windowStates.get(window);
        if (!state) {
            state = {
                instructions: null,   // claimed instructions awaiting dispatch
                mapped: false,        // actor map signal has fired
                dispatched: false,    // batch has been queued
                cloakTimerId: 0,      // pre-emptive cloak timeout
                notifyIds: [],        // late-identifier signal connections
            };
            this._windowStates.set(window, state);
        }
        return state;
    }

    /**
     * Collect every identifier that could match an armed target.
     * The startup id is first: it is unique per spawned process (injected via
     * XDG_ACTIVATION_TOKEN), so matching on it never swaps instructions between
     * concurrent instances of the same app.
     */
    _getIdentifiers(window) {
        const ids = [
            this._try(() => window.get_startup_id?.()),
            this._try(() => window.get_wm_class()),
            this._try(() => window.get_gtk_application_id()),
            this._try(() => window.get_sandboxed_app_id?.()),
        ];
        return ids.filter(id => typeof id === 'string' && id.length > 0);
    }

    /**
     * Find the armed key matching these identifiers.
     * Pass 1 is exact (so a precise startup-id match always beats a fuzzy class
     * match); pass 2 is a case-insensitive substring match in either direction.
     * @returns {string|null}
     */
    _findArmedKey(ids) {
        for (const id of ids) {
            const queue = this._armedSpawns.get(id);
            if (queue && queue.length > 0)
                return id;
        }
        for (const id of ids) {
            const lowerId = id.toLowerCase();
            for (const [key, queue] of this._armedSpawns) {
                if (queue.length === 0)
                    continue;
                const lowerKey = key.toLowerCase();
                if (lowerId.includes(lowerKey) || lowerKey.includes(lowerId))
                    return key;
            }
        }
        return null;
    }

    /**
     * Try to claim armed instructions for this window.
     * On success the instructions are stored on the window's state, the actor is
     * cloaked, and — if the window is already mapped — the batch is dispatched.
     * @returns {boolean} true if the window now has (or had) instructions
     */
    _claimInstructions(window) {
        const state = this._getState(window);
        if (state.instructions || state.dispatched)
            return true;

        let instructions = null;
        const key = this._findArmedKey(this._getIdentifiers(window));

        if (key !== null) {
            const queue = this._armedSpawns.get(key);
            instructions = queue.shift();
            if (queue.length === 0)
                this._armedSpawns.delete(key);
        } else if (this._wildcardTarget) {
            instructions = this._wildcardTarget;
            this._clearWildcard();
        }

        if (!instructions)
            return false;

        state.instructions = instructions;
        const actor = this._getActor(window);
        if (actor)
            this._cloak(actor);

        // A late match (identifiers arrived after map) must still be executed
        if (state.mapped)
            this._dispatch(window);
        return true;
    }

    /** Hand a claimed window's instructions to the serialized queue (once only). */
    _dispatch(window) {
        const state = this._getState(window);
        if (state.dispatched || !state.instructions)
            return;
        const actor = this._getActor(window);
        if (!actor)
            return;

        const instructions = state.instructions;
        state.dispatched = true;
        state.instructions = null;
        this._removeTimer(state.cloakTimerId);
        state.cloakTimerId = 0;

        console.error(`[spawn-at] MATCH FOUND! class="${this._try(() => window.get_wm_class(), '?')}"`);
        this._enqueueBatch(window, actor, instructions);
    }

    /** `window-created`: earliest hook; cloak at tick 0 and attempt a match. */
    _handleWindowCreated(window) {
        if (this._disabled)
            return;

        const actor = this._getActor(window);

        // IMMEDIATE HARD CLOAK: if anything is armed, hide before the first frame
        if (this._hasArmedSpawns() && actor)
            this._cloak(actor);

        this._logTime('WINDOW_CREATED',
            `class="${this._try(() => window.get_wm_class(), '')}" actorFound=${Boolean(actor)}`);
        this._suppressMapAnimation(window);

        if (this._claimInstructions(window))
            return;

        // Wayland often reports wm-class / app-id *after* creation: retry on notify.
        const state = this._getState(window);
        const retry = () => {
            if (this._claimInstructions(window))
                this._disconnectDiscoveryHandlers(window, state);
        };
        state.notifyIds.push(
            this._tryConnect(window, 'notify::wm-class', retry),
            this._tryConnect(window, 'notify::gtk-application-id', retry),
            this._tryConnect(window, 'unmanaged', () => this._disconnectDiscoveryHandlers(window, state)),
        );
    }

    /** Disconnect *all* late-identifier handlers for a window. */
    _disconnectDiscoveryHandlers(window, state) {
        for (const id of state.notifyIds)
            this._safeDisconnect(window, id);
        state.notifyIds = [];
    }

    /** `map`: the actor is about to be shown; dispatch or hold the cloak briefly. */
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

        // Keep the cloak up for windows we own, or may still claim
        if (state.instructions || armed)
            this._cloak(actor);

        this._logTime('ACTOR_MAP', `class="${this._try(() => window.get_wm_class(), '?')}" ` +
            `pid=${this._try(() => window.get_pid(), -1)}`);

        // Already claimed: run it
        if (state.instructions) {
            this._dispatch(window);
            return;
        }

        if (!armed)
            return;

        // PRE-EMPTIVE CLOAK: identifiers may still arrive. Hold the cloak for a short
        // grace period; if no one claims the window by then it is not ours.
        state.cloakTimerId = this._addTimer(PREEMPTIVE_CLOAK_MS, () => {
            state.cloakTimerId = 0;
            if (!state.instructions && !state.dispatched)
                this._uncloak(actor);
        });
    }

    // =======================================================================
    // 5. SERIALIZED BATCH QUEUE (FIFO mutex)
    // =======================================================================

    /**
     * Enqueue a batch. Serialization prevents Wayland configure races between
     * concurrently spawned windows: each window finishes sizing, toolkit layout
     * negotiation and anchoring before the next one starts.
     */
    _enqueueBatch(window, actor, instructions) {
        this._batchQueue.push({ window, actor, instructions });
        this._processQueue();
    }

    /** Queue worker: runs one batch at a time, then recurses to the next. */
    async _processQueue() {
        if (this._disabled || this._batchBusy || this._batchQueue.length === 0)
            return;

        this._batchBusy = true;
        const { window, actor, instructions } = this._batchQueue.shift();

        try {
            await this._runBatch(window, actor, instructions);
        } catch (e) {
            console.error(`[SpawnAt] Batch processing failed in mutex queue: ${e}`);
            // Failsafe: an error must never leave the actor permanently invisible
            this._uncloak(actor);
        } finally {
            this._batchBusy = false;
            this._processQueue();
        }
    }

    // =======================================================================
    // 6. BATCH ENGINE
    // =======================================================================

    /** Normalise "Name" or { Name: args } into { name, args }; null if malformed. */
    _normalizeInstruction(inst) {
        if (typeof inst === 'string')
            return { name: inst, args: {} };
        if (inst && typeof inst === 'object') {
            const name = Object.keys(inst)[0];
            if (name)
                return { name, args: inst[name] ?? {} };
        }
        return null;
    }

    /**
     * Execute an instruction list for one window, strictly in order.
     *
     * Order rationale:
     *  SetSize            -> dispatch the xdg_toplevel configure request
     *  WaitForCommit      -> wait for the client's buffer geometry to settle
     *  SetPositionAnchored-> position against the *committed* size, never the request
     *  Uncloak            -> wake toolkit, re-verify, reveal, arm reactive anchor
     *
     * No pre-positioning happens before the commit: moving an actor before the
     * client buffer settles makes Mutter clamp it off-screen (1-frame corner flash).
     */
    async _runBatch(window, actor, instructions) {
        const ops = instructions
            .map(inst => this._normalizeInstruction(inst))
            .filter(Boolean);

        // Mutable context shared by the step handlers
        const ctx = {
            window,
            actor,
            targetW: null,        // requested width  (from SetSize)
            targetH: null,        // requested height (from SetSize)
            anchorPayload: null,  // last SetPositionAnchored payload
            positionedW: -1,      // frame size at the moment we last positioned
            positionedH: -1,
            revealed: false,      // actor restored to opacity 255
        };

        // Pre-seed the target so a WaitForCommit placed before SetSize still knows it
        const sizeOp = ops.find(op => op.name === 'SetSize');
        if (sizeOp && sizeOp.args.w > 0 && sizeOp.args.h > 0) {
            ctx.targetW = sizeOp.args.w;
            ctx.targetH = sizeOp.args.h;
        }

        try {
            for (const op of ops) {
                if (this._disabled)
                    return;
                this._logTime('BATCH_STEP', `inst=${op.name}`);

                switch (op.name) {
                    case 'Snapshot':            this._createSnapshot(actor); break;
                    case 'Cloak':               this._cloak(actor); break;
                    case 'DestroySnapshot':     this._destroySnapshot(actor); break;
                    case 'SetSize':             this._stepSetSize(ctx, op.args); break;
                    case 'WaitForCommit':       await this._stepWaitForCommit(ctx, op.args); break;
                    case 'SetPositionAnchored': this._stepPosition(ctx, op.args); break;
                    case 'Uncloak':             await this._stepUncloak(ctx, op.args); break;
                    default:
                        console.error(`[SpawnAt] Unknown instruction "${op.name}" ignored`);
                }
            }
        } finally {
            // A batch without an Uncloak (and without an explicit Cloak) must not leave
            // the window hidden by the automatic cloak applied at discovery.
            const wantsHidden = ops.some(op => op.name === 'Cloak');
            if (!ctx.revealed && !wantsHidden)
                this._uncloak(actor);
        }
    }

    /** Step: request a new frame size (asynchronous configure to the client). */
    _stepSetSize(ctx, { w, h }) {
        const { window } = ctx;
        if (!(w > 0 && h > 0))
            return;

        ctx.targetW = w;
        ctx.targetH = h;

        // A maximized window ignores size requests
        if (this._isMaximized(window))
            this._unmaximize(window);

        const frame = window.get_frame_rect();
        this._logTime('SET_SIZE_REQUEST',
            `target=(${w}x${h}) preFrame=(${frame.x},${frame.y},${frame.width}x${frame.height})`);

        // Keep the current origin; only the size is being requested here
        if (window.move_resize_frame)
            window.move_resize_frame(true, frame.x, frame.y, w, h);
        else
            window.resize(true, w, h);
    }

    /** Step: wait for the client's geometry to settle. */
    async _stepWaitForCommit(ctx, args) {
        ctx.commit = await this._waitForCommit(
            ctx.window, ctx.actor, args.timeout_ms, ctx.targetW, ctx.targetH);
    }

    /** Step: place the window using the committed (not requested) size. */
    _stepPosition(ctx, payload) {
        const { window, actor } = ctx;
        ctx.anchorPayload = payload;
        this._applyAnchoredPosition(window, payload);

        // Remember the size we positioned for, so step 4C can detect later growth
        const frame = window.get_frame_rect();
        ctx.positionedW = frame.width;
        ctx.positionedH = frame.height;

        const buf = window.get_buffer_rect ? window.get_buffer_rect() : frame;
        this._logTime('POSITION_SET',
            `frame=(${frame.x},${frame.y},${frame.width}x${frame.height}) ` +
            `buf=(${buf.x},${buf.y}) actor=(${actor.x},${actor.y})`);
    }

    /**
     * Step: the uncloak sequence.
     *   4A  safety delay      — let buffers paint
     *   4B  wake              — focus cycle to force deferred toolkit layout
     *   4C  re-anchor         — fix position if the size changed since placement
     *   4D  reveal + anchor   — show the window and arm the reactive anchor
     */
    async _stepUncloak(ctx, args) {
        const delayMs = Number.isFinite(args.delay_ms) ? args.delay_ms : DEFAULT_UNCLOAK_DELAY_MS;
        const wantWake = args.wake !== false;

        // 4A. Safety delay
        this._logTime('UNCLOAK_4A', `delay=${delayMs}ms`);
        if (delayMs > 0)
            await this._sleep(delayMs);
        if (this._disabled)
            return;

        // 4B. Wake the toolkit (only when the toolkit clamped us above the request)
        if (wantWake && this._isToolkitClamped(ctx)) {
            await this._wakeToolkit(ctx);
            if (this._disabled)
                return;
        }

        // 4C. Re-anchor if geometry moved on since we positioned
        this._reanchorIfResized(ctx);

        // 4D. Reveal and keep the anchor alive
        this._reveal(ctx);
    }

    /**
     * Clamp signature: the window is *larger* than requested in some dimension.
     * That is what a toolkit minimum-size clamp looks like, regardless of app or
     * pixel values. (Smaller-than-requested means a max-size constraint — no wake.)
     */
    _isToolkitClamped(ctx) {
        if (ctx.targetW === null || ctx.targetH === null)
            return false;
        const frame = ctx.window.get_frame_rect();
        return frame.width > ctx.targetW || frame.height > ctx.targetH;
    }

    /**
     * Phase 4C: if the frame size differs from the size we positioned for,
     * recompute the anchored position so margins are preserved.
     */
    _reanchorIfResized(ctx) {
        const { window } = ctx;
        if (!ctx.anchorPayload)
            return;

        const cur = window.get_frame_rect();
        const resized = cur.width !== ctx.positionedW || cur.height !== ctx.positionedH;
        this._logTime('UNCLOAK_4C',
            `positioned=(${ctx.positionedW}x${ctx.positionedH}) actual=(${cur.width}x${cur.height}) ` +
            `needsReanchor=${resized}`);
        if (!resized)
            return;

        this._logTime('REANCHOR_TRIGGERED', `newBounds=(${cur.width}x${cur.height})`);
        this._applyAnchoredPosition(window, ctx.anchorPayload);
        const after = window.get_frame_rect();
        ctx.positionedW = after.width;
        ctx.positionedH = after.height;
    }

    /** Phase 4D: show the window, then hand position upkeep to the reactive anchor. */
    _reveal(ctx) {
        const { window, actor } = ctx;
        this._uncloak(actor);
        ctx.revealed = true;

        const rect = window.get_frame_rect();
        const buf = window.get_buffer_rect ? window.get_buffer_rect() : rect;
        this._logTime('UNCLOAK_4D_REVEAL',
            `finalFrame=(${rect.x},${rect.y},${rect.width}x${rect.height}) buf=(${buf.x},${buf.y})`);

        // Only windows that were positioned need position upkeep
        if (ctx.anchorPayload)
            this._armAnchor(window, ctx.anchorPayload);
    }

    // =======================================================================
    // 7. CLOAK / SNAPSHOT HELPERS
    // =======================================================================

    /**
     * Hide the actor by zeroing opacity. We intentionally do NOT call hide():
     * keeping the actor mapped lets Clutter/Mutter keep negotiating geometry
     * without an unmap/remap cycle when we reveal it.
     */
    _cloak(actor) {
        if (!actor)
            return;
        this._try(() => {
            actor.remove_all_transitions?.();
            actor.opacity = 0;
        });
        this._cloakedActors.add(actor);
    }

    /** Restore full opacity and visibility. Safe on destroyed actors. */
    _uncloak(actor) {
        if (!actor)
            return;
        this._cloakedActors.delete(actor);
        this._try(() => {
            actor.remove_all_transitions?.();
            actor.opacity = 255;
            if (!actor.visible)
                actor.show();
        });
    }

    /** Overlay a Clutter.Clone of the actor (hides tearing during transformations). */
    _createSnapshot(actor) {
        if (this._snapshots.has(actor))
            return;
        const parent = actor.get_parent();
        if (!parent)
            return;
        const clone = new Clutter.Clone({ source: actor, x: actor.x, y: actor.y });
        parent.add_child(clone);
        this._snapshots.set(actor, clone);
    }

    /** Remove the snapshot overlay, if any. */
    _destroySnapshot(actor) {
        const clone = this._snapshots.get(actor);
        if (!clone)
            return;
        this._try(() => clone.destroy());
        this._snapshots.delete(actor);
    }

    // =======================================================================
    // 8. COMMIT LATCH
    // =======================================================================

    /**
     * Wait until the client's frame geometry has settled after SetSize.
     *
     * Resolves (never rejects) when the FIRST of these happens:
     *   AT_TARGET          the frame exactly equals the request
     *   SIZE_SETTLED       the size changed, then stayed unchanged for COMMIT_QUIET_MS
     *   TIMEOUT_NO_CHANGE  the size never changed within max(timeout_ms, 120ms)
     *                      (e.g. already clamped at its minimum)
     *   HARD_CAP           safety bound if a client resizes forever
     *
     * The result reports honestly whether the request was met (`exact`). A settled
     * size that differs from the request is a *clamp*, and — importantly — is NOT
     * necessarily final: a toolkit may still be mid-negotiation. That is why
     * anchoring is revisited later (4C) and kept reactive afterwards.
     *
     * @returns {Promise<{width:number,height:number,exact:boolean,reason:string}>}
     */
    _waitForCommit(window, actor, timeoutMs, targetW = null, targetH = null) {
        return new Promise(resolve => {
            const idleTimeout = Math.max(this._num(timeoutMs), COMMIT_MIN_TIMEOUT_MS);
            const hasTarget = targetW !== null && targetH !== null;

            let finished = false;
            let quietTimer = 0;
            let idleTimer = 0;
            let capTimer = 0;
            let actorSigId = 0;
            let windowSigId = 0;

            const start = window.get_frame_rect();
            let lastW = start.width;
            let lastH = start.height;

            /** Resolve exactly once and release every timer and signal. */
            const finish = reason => {
                if (finished)
                    return;
                finished = true;
                this._removeTimer(quietTimer);
                this._removeTimer(idleTimer);
                this._removeTimer(capTimer);
                this._safeDisconnect(actor, actorSigId);
                this._safeDisconnect(window, windowSigId);

                const rect = this._try(() => window.get_frame_rect(), start);
                const exact = hasTarget && rect.width === targetW && rect.height === targetH;
                this._logTime('COMMIT_RESOLVED',
                    `reason=${reason} frame=(${rect.width}x${rect.height}) exact=${exact}`);
                resolve({ width: rect.width, height: rect.height, exact, reason });
            };

            /** Evaluate geometry on every size/allocation pulse. */
            const check = source => {
                if (finished)
                    return;

                // Defeat any Mutter map transition on every pulse
                this._cloak(actor);

                let rect;
                try {
                    rect = window.get_frame_rect();
                } catch (_e) {
                    finish('ERROR');
                    return;
                }
                this._logTime('COMMIT_SIGNAL',
                    `source=${source} frame=(${rect.width}x${rect.height}) last=(${lastW}x${lastH})`);

                // Exact hit: nothing more to negotiate
                if (hasTarget && rect.width === targetW && rect.height === targetH) {
                    finish('AT_TARGET');
                    return;
                }

                // Size moved: the window is actively negotiating. Restart the quiet timer.
                if (rect.width !== lastW || rect.height !== lastH) {
                    lastW = rect.width;
                    lastH = rect.height;
                    // Once something changed, the "never changed" timeout is moot
                    this._removeTimer(idleTimer);
                    idleTimer = 0;
                    this._removeTimer(quietTimer);
                    quietTimer = this._addTimer(COMMIT_QUIET_MS, () => finish('SIZE_SETTLED'));
                }
            };

            // Both signals feed the same check (allocation also re-enforces the cloak)
            if (actor)
                actorSigId = this._tryConnect(actor, 'notify::allocation', () => check('allocation'));
            windowSigId = this._tryConnect(window, 'size-changed', () => check('size-changed'));

            idleTimer = this._addTimer(idleTimeout, () => finish('TIMEOUT_NO_CHANGE'));
            capTimer = this._addTimer(COMMIT_HARD_CAP_MS, () => finish('HARD_CAP'));

            check('initial');
        });
    }

    /**
     * Wait for a window's size to go quiet.
     * Resolves after `idleMs` if nothing changes, otherwise `quietMs` after the
     * last change, and never later than `ceilingMs`.
     */
    _waitForSizeQuiet(window, idleMs, quietMs, ceilingMs) {
        return new Promise(resolve => {
            let finished = false;
            let timer = 0;
            let ceiling = 0;
            let sigId = 0;

            const finish = () => {
                if (finished)
                    return;
                finished = true;
                this._removeTimer(timer);
                this._removeTimer(ceiling);
                this._safeDisconnect(window, sigId);
                resolve();
            };

            // Every size change pushes the deadline out by `quietMs`
            const rearm = ms => {
                this._removeTimer(timer);
                timer = this._addTimer(ms, finish);
            };

            sigId = this._tryConnect(window, 'size-changed', () => rearm(quietMs));
            ceiling = this._addTimer(ceilingMs, finish);
            rearm(idleMs);
        });
    }

    // =======================================================================
    // 9. TOOLKIT WAKE (FOCUS CYCLE)
    // =======================================================================

    /**
     * Force a deferred toolkit layout while the window is still cloaked.
     *
     * Runs the focus cycle, waits for any resulting resize, and logs the outcome.
     * Skipped if the window already holds focus: it then has nothing to lose.
     */
    async _wakeToolkit(ctx) {
        const { window } = ctx;
        const before = window.get_frame_rect();
        this._logTime('WAKE_START', `clamped=(${before.width}x${before.height}) ` +
            `target=(${ctx.targetW}x${ctx.targetH}) currentFocus=${global.display.focus_window?.get_id()}`);

        if (!(await this._focusCycle(window)))
            return;

        // The growth may land before or after focus is restored: wait for quiet
        await this._waitForSizeQuiet(window, WAKE_IDLE_MS, WAKE_QUIET_MS, WAKE_CEILING_MS);

        const after = window.get_frame_rect();
        this._logTime('WAKE_DONE', `after=(${after.width}x${after.height}) ` +
            `deltaH=${after.height - before.height} deltaW=${after.width - before.width}`);
    }

    /**
     * Give THIS window focus, then return focus to whichever window had it.
     *
     * Targets the exact Meta.Window object (identity, not wm_class), so sibling
     * windows of the same app are never touched. The focus-OUT on this window is
     * what makes GTK/VTE finish its layout.
     *
     * Side effects to be aware of: the user's window briefly loses focus (terminal
     * apps may see focus-out/focus-in events), and a rejected activation can flag the
     * window "demands attention" — which is cleared below.
     *
     * @returns {Promise<boolean>} true if the full cycle ran
     */
    async _focusCycle(window) {
        const display = global.display;
        const previous = display.focus_window;

        this._logTime('FOCUS_CYCLE_INIT', `currentFocus=${previous?.get_id()} target=${window.get_id()}`);

        if (previous === window) {
            // Window ALREADY holds focus: it needs a focus-OUT to complete VTE layout.
            // Find another window or drop focus to desktop, then restore to this window.
            const workspace = global.display.get_workspace_manager().get_active_workspace();
            const altWindow = this._getMRUWindows(workspace).find(w => w !== window && !w.minimized && !w.skip_taskbar);
            
            if (altWindow) {
                altWindow.activate(global.get_current_time());
                await this._waitForFocus(altWindow, FOCUS_WAIT_MS);
            } else if (global.stage?.set_key_focus) {
                global.stage.set_key_focus(null);
                await this._sleep(50);
            }

            // Restore focus back to the spawned window
            window.activate(global.get_current_time());
            const restored = await this._waitForFocus(window, FOCUS_WAIT_MS);
            this._logTime('FOCUS_CYCLE_DONE', `cycleType="pulse_out_and_back" restored=${restored}`);
            return true;
        }

        // Window does NOT hold focus: standard pulse in then out
        if (!previous || previous.minimized) {
            this._logTime('FOCUS_CYCLE_SKIP', 'no distinct previous focus window');
            return false;
        }

        window.activate(global.get_current_time());
        if (!(await this._waitForFocus(window, FOCUS_WAIT_MS))) {
            this._try(() => window.unset_demands_attention());
            this._logTime('FOCUS_CYCLE_ABORT', 'activate() was not honoured');
            return false;
        }

        previous.activate(global.get_current_time());
        const restored = await this._waitForFocus(previous, FOCUS_WAIT_MS);
        this._logTime('FOCUS_CYCLE_DONE', `cycleType="pulse_in_and_restore" restored=${restored}`);
        return true;
    }

    /** Resolve true as soon as `target` is the focus window, false on timeout. */
    _waitForFocus(target, timeoutMs) {
        const display = global.display;
        return new Promise(resolve => {
            if (display.focus_window === target) {
                resolve(true);
                return;
            }

            let sigId = 0;
            let timer = 0;
            const done = ok => {
                this._safeDisconnect(display, sigId);
                this._removeTimer(timer);
                resolve(ok);
            };

            sigId = this._tryConnect(display, 'notify::focus-window', () => {
                if (display.focus_window === target)
                    done(true);
            });
            timer = this._addTimer(timeoutMs, () => done(false));
        });
    }

    // =======================================================================
    // 10. ANCHORED POSITIONING
    // =======================================================================

    /**
     * Pure computation of the anchored position for the window's CURRENT size.
     *
     * Effective size = max(current frame, toolkit-reported minimum). The minimum is a
     * *measured, per-window* lower bound (never inferred from other windows). It can
     * be stale for deferred toolkits, which is why the reactive anchor exists.
     *
     * Position = anchor point - pivot * size + offset, clamped inside the bounds
     * (monitor or work area) minus margins. If the window is larger than the
     * available space, right/bottom margins yield so top-left stays reachable.
     */
    _computeAnchoredPosition(window, payload) {
        const frame = window.get_frame_rect();
        const [minW, minH] = this._getMinSize(window);
        const effW = Math.max(frame.width, minW);
        const effH = Math.max(frame.height, minH);

        const monitor = window.get_monitor();
        const bounds = payload.area === 'screen'
            ? global.display.get_monitor_geometry(monitor)
            : window.get_work_area_for_monitor(monitor);

        const mt = this._num(payload.margin_top);
        const mb = this._num(payload.margin_bottom);
        const ml = this._num(payload.margin_left);
        const mr = this._num(payload.margin_right);

        // Legal range for the top-left corner
        const minX = bounds.x + ml;
        const minY = bounds.y + mt;
        let maxX = bounds.x + bounds.width - mr - effW;
        let maxY = bounds.y + bounds.height - mb - effH;

        // Oversized windows: collapse the range so top-left wins
        const oversized = effW > bounds.width - ml - mr || effH > bounds.height - mt - mb;
        if (maxX < minX)
            maxX = minX;
        if (maxY < minY)
            maxY = minY;

        // Unclamped target from anchor + pivot + offset
        const rawX = this._num(payload.screen_anchor_x)
            - Math.round(this._num(payload.pivot_u) * effW) + this._num(payload.offset_x);
        const rawY = this._num(payload.screen_anchor_y)
            - Math.round(this._num(payload.pivot_v) * effH) + this._num(payload.offset_y);

        const x = Math.max(minX, Math.min(rawX, maxX));
        const y = Math.max(minY, Math.min(rawY, maxY));

        return {
            x, y, rawX, rawY, effW, effH, minW, minH, oversized, bounds,
            range: { minX, maxX, minY, maxY },
        };
    }

    /** Compute the anchored position and move the window there. */
    _applyAnchoredPosition(window, payload) {
        // Kill any Mutter transition that could fight the move
        const actor = this._getActor(window);
        actor?.remove_all_transitions?.();

        const pos = this._computeAnchoredPosition(window, payload);

        if (pos.minW === 0 && pos.minH === 0) {
            this._logTime('FALLBACK_USED',
                `reason="min size unreported" usingFrame=(${pos.effW}x${pos.effH})`);
        }
        if (pos.oversized) {
            console.warn(`[spawn-at] Window is oversized (${pos.effW}x${pos.effH}); ` +
                'bottom/right margins ignored to keep top-left reachable.');
        }
        if (pos.x !== pos.rawX || pos.y !== pos.rawY) {
            this._logTime('MARGIN_CLAMP_ENGAGED',
                `raw=(${pos.rawX},${pos.rawY}) clamped=(${pos.x},${pos.y})`);
        }
        this._logTime('APPLY_ANCHOR',
            `eff=(${pos.effW}x${pos.effH}) bounds=(${pos.bounds.x},${pos.bounds.y},` +
            `${pos.bounds.width}x${pos.bounds.height}) final=(${pos.x},${pos.y})`);

        this._moveFrame(window, pos.x, pos.y);
        return pos;
    }

    // =======================================================================
    // 11. REACTIVE ANCHOR
    // =======================================================================

    /**
     * Keep a positioned window anchored even if the client later changes size.
     *
     * Mutter pins the top-left on client-initiated resizes, so a window anchored to
     * the bottom/right would drift off-screen when it grows. Instead of trusting a
     * one-time calculation, re-solve the position on every `size-changed`.
     *
     * The handler is idempotent (it moves only if the solved position differs from
     * the current one), so moving can never cause a feedback loop.
     *
     * The anchor releases itself when:
     *   - the user grabs/moves/resizes the window   (grab-op-end)
     *   - the window is closed                      (unmanaged)
     *   - an external D-Bus call moves/changes it   (MoveWindow / SetWindowState)
     *   - it has made a correction and then stayed quiet for ANCHOR_RETIRE_QUIET_MS
     */
    _armAnchor(window, payload) {
        // Replace any existing anchor for this window
        this._disarmAnchor(window, 'REARM');

        const state = { payload, sizeId: 0, unmanagedId: 0, retireTimerId: 0 };
        state.sizeId = this._tryConnect(window, 'size-changed',
            () => this._onAnchoredSizeChanged(window, state));
        state.unmanagedId = this._tryConnect(window, 'unmanaged',
            () => this._disarmAnchor(window, 'UNMANAGED'));
        this._anchors.set(window, state);
        this._logTime('ANCHOR_ARMED');
    }

    /** `size-changed` handler of an armed anchor. */
    _onAnchoredSizeChanged(window, state) {
        if (this._disabled)
            return;

        // Never fight an interactive resize, or a window the user maximized
        if (this._userIsGrabbing())
            return;
        if (this._isMaximized(window) || this._isFullscreen(window))
            return;

        const cur = window.get_frame_rect();
        const pos = this._computeAnchoredPosition(window, state.payload);

        // Already where it should be (also makes our own moves harmless)
        if (pos.x === cur.x && pos.y === cur.y)
            return;

        this._logTime('ANCHOR_CORRECTION',
            `size=(${cur.width}x${cur.height}) from=(${cur.x},${cur.y}) to=(${pos.x},${pos.y})`);
        this._moveFrame(window, pos.x, pos.y);

        // A correction happened: retire once the client has been quiet for a while
        this._removeTimer(state.retireTimerId);
        state.retireTimerId = this._addTimer(ANCHOR_RETIRE_QUIET_MS,
            () => this._disarmAnchor(window, 'RETIRED'));
    }

    /** Release a window's reactive anchor and all of its connections. */
    _disarmAnchor(window, reason = '') {
        const state = this._anchors.get(window);
        if (!state)
            return;
        this._removeTimer(state.retireTimerId);
        this._safeDisconnect(window, state.sizeId);
        this._safeDisconnect(window, state.unmanagedId);
        this._anchors.delete(window);
        this._logTime('ANCHOR_RELEASED', `reason=${reason}`);
    }

    // =======================================================================
    // 12. WINDOW LOOKUP (shared by the D-Bus API)
    // =======================================================================

    /** Windows on a workspace in most-recently-used order. */
    _getMRUWindows(workspace) {
        // Resolve the tab-list enum across Mutter versions
        let tabListType = 0;
        if (Meta.TabList?.NORMAL !== undefined)
            tabListType = Meta.TabList.NORMAL;
        else if (Meta.TabListType?.NORMAL !== undefined)
            tabListType = Meta.TabListType.NORMAL;

        const list = this._try(() => global.display.get_tab_list(tabListType, workspace));
        if (list && list.length > 0)
            return list;

        // Fallback: stacking order, topmost first
        return this._try(() => {
            const windows = workspace.list_windows();
            if (typeof global.display.sort_windows_by_stacking === 'function')
                return global.display.sort_windows_by_stacking(windows).slice().reverse();
            return windows;
        }, []);
    }

    /**
     * Find a window by numeric id, numeric pid, wm_class or app id.
     * NOTE: class/app-id lookups return the most recently used match, so they are NOT
     * suitable for singling out one of several same-app windows (internal code uses
     * the Meta.Window object directly for that reason).
     */
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
            const wmClass = w.get_wm_class() || '';
            const appId = w.get_gtk_application_id() || '';
            return wmClass === target || appId === target;
        }) || null;
    }

    // --- Focus helpers operating on a concrete window object ---------------

    _focusWindowObject(window) {
        window.activate(global.get_current_time());
        return true;
    }

    _defocusWindowObject(window, toTarget) {
        if (!window.has_focus())
            return true;

        // 'desktop': drop stage key focus rather than activating another window
        if (toTarget === 'desktop') {
            global.stage.set_key_focus(null);
            return true;
        }

        // Otherwise hand focus to the next usable window in MRU order
        const workspace = global.display.get_workspace_manager().get_active_workspace();
        const next = this._getMRUWindows(workspace)
            .find(w => w !== window && !w.minimized && !w.skip_taskbar);
        if (next)
            next.activate(global.get_current_time());
        else
            global.stage.set_key_focus(null);
        return true;
    }

    // =======================================================================
    // 13. WILDCARD HELPER
    // =======================================================================

    _clearWildcard() {
        this._removeTimer(this._wildcardTimeoutId);
        this._wildcardTimeoutId = 0;
        this._wildcardTarget = null;
    }

    // =======================================================================
    // 14. D-BUS API (method names/signatures are fixed by DBUS_IFACE)
    // =======================================================================

    /**
     * Arm instructions for the next window matching `target_id` ("" or "*" = the
     * next window of any kind). Unclaimed arms expire automatically.
     */
    ArmSpawn(target_id, instructions_json) {
        this._t0 = GLib.get_monotonic_time();
        this._logTime('ARM_SPAWN', `target="${target_id}"`);

        let instructions;
        try {
            instructions = JSON.parse(instructions_json);
        } catch (e) {
            console.error(`[SpawnAt] Invalid JSON instructions: ${e}`);
            return;
        }

        // Wildcard arm: single slot, short-lived
        if (!target_id || target_id === '*') {
            this._clearWildcard();
            this._wildcardTarget = instructions;
            this._wildcardTimeoutId = this._addTimer(WILDCARD_EXPIRY_MS, () => {
                this._wildcardTimeoutId = 0;
                this._wildcardTarget = null;
            });
            return;
        }

        // Targeted arm: FIFO queue per key, so several windows can be armed at once
        if (!this._armedSpawns.has(target_id))
            this._armedSpawns.set(target_id, []);
        this._armedSpawns.get(target_id).push(instructions);

        // Expire just this entry if nobody claims it
        this._addTimer(ARM_EXPIRY_MS, () => {
            const queue = this._armedSpawns.get(target_id);
            if (!queue)
                return;
            const idx = queue.indexOf(instructions);
            if (idx !== -1)
                queue.splice(idx, 1);
            if (queue.length === 0)
                this._armedSpawns.delete(target_id);
        });
    }

    /** Run an instruction batch against an existing window right now (queued). */
    ExecuteBatch(target_id, instructions_json) {
        let instructions;
        try {
            instructions = JSON.parse(instructions_json);
        } catch (e) {
            console.error(`[SpawnAt] Invalid JSON instructions: ${e}`);
            return;
        }

        const window = this._findWindow(target_id);
        if (!window) {
            console.error(`[SpawnAt] ExecuteBatch: no window matches "${target_id}"`);
            return;
        }
        const actor = this._getActor(window);
        if (actor)
            this._enqueueBatch(window, actor, instructions);
    }

    /** Pointer position (global coordinates). */
    GetCursor() {
        const [x, y] = global.get_pointer();
        return [x, y];
    }

    /** Alias of GetCursor kept for API compatibility. */
    GetPointer() {
        return this.GetCursor();
    }

    /** JSON list of per-monitor work areas for the active workspace. */
    GetWorkareas() {
        const workspace = global.workspace_manager.get_active_workspace();
        const count = global.display.get_n_monitors();
        const areas = [];
        for (let i = 0; i < count; i++) {
            const rect = workspace.get_work_area_for_monitor(i);
            areas.push({ x: rect.x, y: rect.y, w: rect.width, h: rect.height });
        }
        return JSON.stringify(areas);
    }

    /** JSON list of windows on the active workspace (MRU order). */
    GetWindows() {
        const workspace = global.display.get_workspace_manager().get_active_workspace();
        const windows = this._getMRUWindows(workspace);
        return JSON.stringify(windows.map(win => {
            const frame = win.get_frame_rect();
            return {
                id: win.get_id ? win.get_id() : null,
                pid: win.get_pid(),
                title: win.get_title() || '',
                class: win.get_wm_class() || win.get_gtk_application_id() || '',
                x: frame.x,
                y: frame.y,
                w: frame.width,
                h: frame.height,
                focused: win.has_focus(),
            };
        }));
    }

    /** Move a window; the caller now owns its position, so release any anchor. */
    MoveWindow(app_id, x, y) {
        const window = this._findWindow(app_id);
        if (!window)
            return;
        this._disarmAnchor(window, 'EXTERNAL_MOVE');
        this._moveFrame(window, x, y);
    }

    /** Give a window keyboard focus. */
    FocusWindow(target) {
        const window = this._findWindow(target);
        return window ? this._focusWindowObject(window) : false;
    }

    /** Take focus away from a window ('desktop' or next-in-MRU). */
    DefocusWindow(target, to_target) {
        const window = this._findWindow(target);
        return window ? this._defocusWindowObject(window, to_target) : false;
    }

    /** maximize | unmaximize | minimize | unminimize | restore */
    SetWindowState(target, state) {
        const window = this._findWindow(target);
        if (!window)
            return false;

        // An external state change supersedes our position upkeep
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
