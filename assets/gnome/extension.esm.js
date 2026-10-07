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
 * TOOLKIT GROWTH (e.g. gnome-terminal 76px -> 93px) & GATED FOCUS PULSE
 * ---------------------------------------------------------------------
 *  Some toolkits (specifically GTK3 + VTE, e.g. gnome-terminal-server) commit a
 *  clamped minimum size first and only finish full grid layout negotiation after
 *  receiving a wl_keyboard.leave / wl_keyboard.enter cycle.
 *  Non-VTE toolkits (GTK4/Libadwaita, Qt, Kitty, GTK3 non-VTE, XWayland) do NOT
 *  suffer from this defect; defocusing GTK4 windows while cloaked causes Wayland
 *  surface drops, map stalls, and visual flicker.
 *  The extension gates the focus pulse via:
 *   - VTE CANDIDATE GATING: Inspects /proc/<pid>/maps and window classes to only
 *     pulse processes running libvte. Non-VTE applications completely skip the pulse.
 *   - EARLY-EXIT DETECTION: Defocuses briefly; if size-changed does not fire within
 *     PULSE_LEAVE_IDLE_MS, immediately restores focus and aborts the pulse.
 *
 * GEOMETRY GUARD FLOOR
 * --------------------
 *  Enforces a hard minimum floor (100x60) so micro-geometry requests (e.g. 30x20)
 *  never pass sub-chrome dimensions to Mutter. This prevents CSD titlebar subtraction
 *  from causing negative inner widget allocations in GTK3/Pixman/XWayland, and avoids
 *  framebuffer rejection in OpenGL engines like Kitty.
 *
 * PERSISTENT SESSION LOGGING & ENRICHED TELEMETRY
 * -----------------------------------------------
 *  Maintains persistent diagnostic logs at ~/.local/state/spawn-at/session.log
 *  using non-blocking asynchronous I/O (Gio.File async streams). Previous session
 *  logs are rotated to session.log.old on new login sessions (/run/user/<uid> marker),
 *  ensuring debug data survives test runs and extension reloads without premature loss.
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
      <arg type="s" name="mode" direction="in"/>
      <arg type="s" name="destination" direction="in"/>
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
// TIMING CONSTANTS
// ===========================================================================

/** How long an armed instruction set lives if no window claims it. */
const ARM_EXPIRY_MS = 15000;
/** How long a wildcard ("*") arm lives if no window claims it. */
const WILDCARD_EXPIRY_MS = 1200;

/** Commit latch: geometry must be unchanged this long to count as "settled". */
const COMMIT_QUIET_MS = 60;
/** Commit latch: minimum wait when the window never changes size at all. */
const COMMIT_MIN_TIMEOUT_MS = 120;
/** Commit latch: absolute upper bound, even if the client keeps resizing. */
const COMMIT_HARD_CAP_MS = 1500;

/** Default blind delay at the start of the Uncloak phase (lets buffers paint). */
const DEFAULT_UNCLOAK_DELAY_MS = 300;

/** After a wake: wait this long for any size change to even begin. */
const WAKE_IDLE_MS = 120;
/** After a wake: size must be quiet this long once it starts changing. */
const WAKE_QUIET_MS = 60;
/** After a wake: absolute upper bound. */
const WAKE_CEILING_MS = 300;

/** Pulse: after defocus, how long to wait for client size-changed signal before early-exit. */
const PULSE_LEAVE_IDLE_MS = 150;
/** Pulse: ceiling for Mutter to report the window focused again. */
const PULSE_REFOCUS_CEILING_MS = 250;
/** Max wait for the compositor frames flushed just before reveal. */
const FRAME_WAIT_CEILING_MS = 100;

/** Reactive anchor retires this long after its last correction. */
const ANCHOR_RETIRE_QUIET_MS = 1000;

// ===========================================================================
// GEOMETRY CONSTANTS
// ===========================================================================

/**
 * Absolute minimum geometry floor sent to Mutter, in pixels.
 *
 * Rationale: toolkits (GTK3 CSD, Pixman) subtract internal chrome
 * (titlebars, padding) from the frame size before allocating the container.
 * If we pass a frame height smaller than that chrome the inner allocation goes
 * negative, producing Pixman assertion failures and XWayland crashes.
 * Similarly, Kitty calculates padding and rejects requests smaller than ~30x25.
 *
 * - Width  100 px: narrower than any decoratable GTK titlebar button row.
 * - Height  60 px: comfortably above a typical 25–37 px CSD titlebar.
 */
const GEOMETRY_FLOOR_W = 100;
const GEOMETRY_FLOOR_H = 60;

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

        // Windows held cloaked while an arm is pending and they are not yet resolved
        this._heldWindows = new Set();

        // Per-window bookkeeping for discovery (instructions, mapped flag, ...)
        this._windowStates = new WeakMap();
        // Windows currently under reactive-anchor control: Meta.Window -> state
        this._anchors = new Map();

        // Inert focus parking actor
        this._dummy = new Clutter.Actor({
            name: 'spawn-at-defocus-dummy',
            reactive: false,
            opacity: 0,
            width: 1,
            height: 1,
        });
        global.stage.add_child(this._dummy);

        this._cloakSafetyId = this._tryConnect(global.stage, 'after-update', () => {
            if (this._cloakedActors.size === 0)
                return;
            for (const actor of [...this._cloakedActors]) {
                if (actor.is_destroyed?.())
                    continue;
                if (actor.opacity !== 0) {
                    actor.remove_all_transitions?.();
                    actor.opacity = 0;
                }
            }
        });

        // FIFO mutex queue state
        this._batchQueue = [];
        this._batchBusy = false;

        // Initialize persistent session logging
        this._initSessionLogging();

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
        this._safeDisconnect(global.stage, this._cloakSafetyId);
        this._safeDisconnect(global.display, this._windowCreatedId);
        this._safeDisconnect(global.window_manager, this._mapId);
        this._safeDisconnect(global.display, this._grabEndId);
        this._safeDisconnect(this._monitorManager, this._monitorsChangedId);
        this._safeDisconnect(global.display, this._workareasChangedId);
        this._cloakSafetyId = this._windowCreatedId = this._mapId = this._grabEndId = 0;
        this._monitorsChangedId = this._workareasChangedId = 0;
        this._monitorManager = null;

        // Cancel all timers (including wildcard and expiry timers)
        for (const id of this._timerIds)
            GLib.Source.remove(id);
        this._timerIds.clear();
        this._wildcardTimeoutId = 0;
        this._wildcardTarget = null;
        this._armedSpawns.clear();

        this._heldWindows.clear();

        // Never leave a window invisible or a snapshot overlay behind
        for (const actor of [...this._cloakedActors])
            this._uncloak(actor);
        for (const actor of [...this._snapshots.keys()])
            this._destroySnapshot(actor);

        // Clean up inert dummy actor
        if (this._dummy) {
            if (global.stage.get_key_focus() === this._dummy)
                global.stage.set_key_focus(null);
            this._try(() => this._dummy.destroy());
            this._dummy = null;
        }

        // Flush any remaining persistent log entries before unexporting
        this._flushSessionLogSync();

        // Tear down D-Bus
        if (this._dbusImpl) {
            this._try(() => this._dbusImpl.unexport());
            this._dbusImpl = null;
        }
    }

    // =======================================================================
    // 2. PERSISTENT LOGGING & ENRICHED TELEMETRY
    // =======================================================================

    /**
     * Initialize persistent session logging to ~/.local/state/spawn-at/session.log.
     * Uses /run/user/<uid>/spawn-at-session.marker to detect login session lifecycle:
     * - New login session (marker missing): rotates existing session.log -> session.log.old.
     * - Same session (marker exists): appends to session.log without data loss.
     */
    _initSessionLogging() {
        this._pendingLogLines = [];
        this._logWriting = false;
        this._logFlushScheduled = false;

        try {
            const stateDir = GLib.build_filenamev([GLib.get_user_state_dir(), 'spawn-at']);
            GLib.mkdir_with_parents(stateDir, 0o755);

            this._sessionLogPath = GLib.build_filenamev([stateDir, 'session.log']);
            const oldLogPath = GLib.build_filenamev([stateDir, 'session.log.old']);
            this._sessionLogFile = Gio.File.new_for_path(this._sessionLogPath);

            const runtimeDir = GLib.get_user_runtime_dir();
            const markerPath = GLib.build_filenamev([runtimeDir, 'spawn-at-session.marker']);
            const markerFile = Gio.File.new_for_path(markerPath);

            const isNewSession = !markerFile.query_exists(null);
            if (isNewSession) {
                // Rotate previous log if present so history is preserved
                if (this._sessionLogFile.query_exists(null)) {
                    try {
                        const oldFile = Gio.File.new_for_path(oldLogPath);
                        this._sessionLogFile.move(oldFile, Gio.FileCopyFlags.OVERWRITE, null, null);
                    } catch (_e) {}
                }
                // Touch the session marker in /run/user/<uid> (wiped on logout/reboot by systemd)
                try {
                    const st = markerFile.create(Gio.FileCreateFlags.NONE, null);
                    st.close(null);
                } catch (_e) {}
                this._queueLogLine(`=== SPAWN-AT SESSION STARTED: ${new Date().toISOString()} ===`);
            } else {
                this._queueLogLine(`--- SPAWN-AT EXTENSION RE-ENABLED: ${new Date().toISOString()} ---`);
            }
        } catch (e) {
            console.error(`[SpawnAt] Failed to initialize session logging: ${e}`);
        }
    }

    /** Append a formatted line to the async write queue and trigger flush. */
    _queueLogLine(line) {
        if (!this._pendingLogLines)
            this._pendingLogLines = [];
        this._pendingLogLines.push(line);

        if (!this._logWriting && !this._logFlushScheduled) {
            this._logFlushScheduled = true;
            GLib.idle_add(GLib.PRIORITY_LOW, () => {
                this._logFlushScheduled = false;
                this._flushLogQueue();
                return GLib.SOURCE_REMOVE;
            });
        }
    }

    /** Asynchronously write queued lines to ~/.local/state/spawn-at/session.log. */
    _flushLogQueue() {
        if (this._disabled && this._pendingLogLines.length === 0)
            return;
        if (this._logWriting || !this._sessionLogFile || this._pendingLogLines.length === 0)
            return;

        this._logWriting = true;
        const chunk = this._pendingLogLines.splice(0).join('\n') + '\n';
        const bytes = new GLib.Bytes(new TextEncoder().encode(chunk));

        try {
            this._sessionLogFile.append_to_async(
                Gio.FileCreateFlags.NONE,
                GLib.PRIORITY_LOW,
                null,
                (file, res) => {
                    try {
                        const stream = file.append_to_finish(res);
                        stream.write_bytes_async(
                            bytes,
                            GLib.PRIORITY_LOW,
                            null,
                            (s, wres) => {
                                try {
                                    s.write_bytes_finish(wres);
                                    s.close_async(GLib.PRIORITY_LOW, null, (cs, cres) => {
                                        try { cs.close_finish(cres); } catch (_e) {}
                                        this._logWriting = false;
                                        if (this._pendingLogLines.length > 0)
                                            this._flushLogQueue();
                                    });
                                } catch (_err) {
                                    try { s.close(null); } catch (_e) {}
                                    this._logWriting = false;
                                }
                            }
                        );
                    } catch (_err) {
                        this._logWriting = false;
                    }
                }
            );
        } catch (_err) {
            this._logWriting = false;
        }
    }

    /** Synchronously drain pending log buffer during disable(). */
    _flushSessionLogSync() {
        if (!this._sessionLogFile || !this._pendingLogLines || this._pendingLogLines.length === 0)
            return;
        try {
            const stream = this._sessionLogFile.append_to(Gio.FileCreateFlags.NONE, null);
            if (stream) {
                const chunk = this._pendingLogLines.splice(0).join('\n') + '\n';
                stream.write_bytes(new GLib.Bytes(new TextEncoder().encode(chunk)), null);
                stream.close(null);
            }
        } catch (_e) {}
    }

    /** Debug logging is enabled by default unless SPAWN_AT_DEBUG=0|false or SetLogging(false). */
    _isLoggingEnabled() {
        if (this._loggingEnabled !== undefined)
            return this._loggingEnabled;
        const env = GLib.getenv('SPAWN_AT_DEBUG');
        this._loggingEnabled = env !== '0' && env !== 'false';
        return this._loggingEnabled;
    }

    /** Formats rich window metadata for diagnostic logging. */
    _formatWindowMeta(window) {
        if (!window)
            return 'win=[none]';

        const clientType = this._try(() => window.get_client_type?.());
        let protocol = 'Unknown';
        if (clientType === Meta.WindowClientType?.WAYLAND || clientType === 0)
            protocol = 'Wayland';
        else if (clientType === Meta.WindowClientType?.X11 || clientType === 1)
            protocol = 'XWayland';

        const typeInt = this._try(() => window.get_window_type?.(), -1);
        let windowType = 'UNKNOWN';
        if (Meta.WindowType) {
            for (const [name, val] of Object.entries(Meta.WindowType)) {
                if (val === typeInt) {
                    windowType = name;
                    break;
                }
            }
        }

        const id = this._try(() => window.get_id?.(), -1);
        const pid = this._try(() => window.get_pid?.(), -1);
        const wmClass = this._try(() => window.get_wm_class?.(), '') || '';
        const appId = this._try(() => window.get_gtk_application_id?.(), '') ||
                      this._try(() => window.get_sandboxed_app_id?.(), '') || '';

        return `win=[id=${id} pid=${pid} class="${wmClass}" app="${appId}" type=${windowType} proto=${protocol}]`;
    }

    /** Timestamped diagnostic line written to session.log and console.error. */
    _logTime(tag, extra = '', window = null) {
        const nowUs = GLib.get_monotonic_time();
        if (!this._t0)
            this._t0 = nowUs;
        const elapsedMs = ((nowUs - this._t0) / 1000.0).toFixed(2);
        const winInfo = window ? (` ${this._formatWindowMeta(window)}`) : '';
        const payload = `+${elapsedMs}ms | ${tag}${winInfo} ${extra}`.trim();

        // Always record to the persistent user session log file
        const iso = new Date().toISOString();
        this._queueLogLine(`[${iso}] ${payload}`);

        // Emit to console when logging is active
        if (this._isLoggingEnabled())
            console.error(`[spawn-at-time] ${payload}`);
    }

    /** D-Bus: toggle debug console logging at runtime. */
    SetLogging(enabled) {
        this._loggingEnabled = Boolean(enabled);
        this._logTime('SET_LOGGING', `enabled=${this._loggingEnabled}`);
    }

    // =======================================================================
    // 3. SMALL UTILITIES & DETECTION HELPERS
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

    /** Schedule a tracked one-shot timer. */
    _addTimer(ms, callback) {
        const id = GLib.timeout_add(GLib.PRIORITY_DEFAULT, ms, () => {
            this._timerIds.delete(id);
            callback();
            return GLib.SOURCE_REMOVE;
        });
        this._timerIds.add(id);
        return id;
    }

    /** Cancel a timer created with _addTimer. */
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
        // MetaWindow property (older Mutter)
        if ('no_map_animation' in window)
            window.no_map_animation = true;

        // MetaWindowActor property — this is the one current Mutter actually
        // consults before starting the fade-in transition on map.
        const actor = this._getActor(window);
        if (actor) {
            if ('no_map_animation' in actor)
                actor.no_map_animation = true;
            // Kill any default easing that Mutter might re-apply on map.
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

    /**
     * Detect whether a window belongs to a GTK3/VTE terminal application.
     *
     * Rationale:
     * Only GTK3 + VTE applications (most notably gnome-terminal-server) suffer from
     * the deferred grid metric negotiation bug where initial layout clamps to ~76px
     * and requires a wl_keyboard.leave / wl_keyboard.enter focus cycle to commit full
     * terminal geometry.
     *
     * Non-VTE applications (GTK4/Libadwaita, Qt, Kitty, GTK3 non-VTE, XWayland):
     * 1. Do NOT have this bug and do NOT grow from a focus pulse (deltaH = 0).
     * 2. GTK4/Libadwaita apps drop their Wayland surface when defocused while
     *    cloaked, causing a 350ms stall and visual flicker.
     *
     * Detection strategy:
     *  1. Inspect process memory maps: check if /proc/<pid>/maps maps 'libvte'.
     *     (Virtually zero cost: procfs RAM read taking ~1.5ms).
     *  2. Check known VTE terminal identifiers (wm_class, app_id, comm).
     */
    _isVteCandidate(window) {
        if (!window)
            return false;

        const pid = this._try(() => window.get_pid?.(), -1);
        if (pid > 0) {
            // Check /proc/<pid>/maps
            try {
                const [ok, contents] = GLib.file_get_contents(`/proc/${pid}/maps`);
                if (ok) {
                    const text = new TextDecoder().decode(contents);
                    if (text.includes('libvte'))
                        return true;
                }
            } catch (_e) {}

            // Check /proc/<pid>/comm
            try {
                const [ok, commBytes] = GLib.file_get_contents(`/proc/${pid}/comm`);
                if (ok) {
                    const comm = new TextDecoder().decode(commBytes).trim().toLowerCase();
                    if (comm.includes('gnome-terminal') || comm === 'terminator' ||
                        comm === 'tilix' || comm === 'guake') {
                        return true;
                    }
                }
            } catch (_e) {}
        }

        // Check window identifiers
        const ids = this._getIdentifiers(window).map(id => id.toLowerCase());
        const vteKeywords = ['gnome-terminal', 'terminator', 'tilix', 'guake', 'xfce4-terminal', 'vte'];
        for (const id of ids) {
            if (vteKeywords.some(kw => id.includes(kw)))
                return true;
        }

        return false;
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
                instructions: null,
                mapped: false,
                dispatched: false,
                notifyIds: [],
            };
            this._windowStates.set(window, state);
        }
        return state;
    }

    /** Collect every identifier that could match an armed target. */
    _getIdentifiers(window) {
        const ids = [
            this._try(() => window.get_startup_id?.()),
            this._try(() => window.get_wm_class?.()),
            this._try(() => window.get_gtk_application_id?.()),
            this._try(() => window.get_sandboxed_app_id?.()),
        ];
        return ids.filter(id => typeof id === 'string' && id.length > 0);
    }

    /** Find the armed key matching these identifiers. */
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

    /** Try to claim armed instructions for this window. */
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
        this._heldWindows.delete(window);
        const actor = this._getActor(window);
        if (actor)
            this._cloak(actor);

        this._releaseHeldIfIdle();

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
        this._heldWindows.delete(window);

        this._logTime('DISPATCH', `instructionsCount=${instructions.length}`, window);
        this._enqueueBatch(window, actor, instructions);
    }

    /** `window-created`: earliest hook; cloak at tick 0 and attempt a match. */
    _handleWindowCreated(window) {
        if (this._disabled)
            return;

        this._suppressMapAnimation(window);
        const actor = this._getActor(window);
        if (this._hasArmedSpawns() && actor)
            this._cloak(actor);
        this._logTime('WINDOW_CREATED', `actorFound=${Boolean(actor)}`, window);

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

    /** `map`: actor about to be shown. Hold cloak if armed, or reject. */
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

        this._logTime('ACTOR_MAP', '', window);

        if (state.instructions) {
            this._dispatch(window);
            return;
        }
        if (state.dispatched)
            return;
        if (!armed) {
            if (this._cloakedActors.has(actor))
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
        this._logTime('REJECT_UNMATCHED', '', window);
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
    // 5. SERIALIZED BATCH QUEUE (FIFO mutex)
    // =======================================================================

    _enqueueBatch(window, actor, instructions) {
        this._batchQueue.push({ window, actor, instructions });
        this._processQueue();
    }

    async _processQueue() {
        if (this._disabled || this._batchBusy || this._batchQueue.length === 0)
            return;

        this._batchBusy = true;
        const { window, actor, instructions } = this._batchQueue.shift();

        try {
            await this._runBatch(window, actor, instructions);
        } catch (e) {
            console.error(`[SpawnAt] Batch processing failed in mutex queue: ${e}`);
            this._uncloak(actor);
        } finally {
            this._batchBusy = false;
            this._processQueue();
        }
    }

    // =======================================================================
    // 6. BATCH ENGINE
    // =======================================================================

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

    async _runBatch(window, actor, instructions) {
        const ops = instructions
            .map(inst => this._normalizeInstruction(inst))
            .filter(Boolean);

        const ctx = {
            window,
            actor,
            targetW: null,
            targetH: null,
            safeW: null,
            safeH: null,
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
                this._logTime('BATCH_STEP', `inst=${op.name}`, window);

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
            const wantsHidden = ops.some(op => op.name === 'Cloak');
            if (!ctx.revealed && !wantsHidden)
                this._uncloak(actor);
        }
    }

    /**
     * Step: request a new frame size with two-tier geometry floor protection.
     *
     * Prevents sub-minimum raw geometry from being pushed to Mutter:
     *  - Tier 1: window-reported minimum (via get_min_size()).
     *  - Tier 2: GEOMETRY_FLOOR_W (100px) and GEOMETRY_FLOOR_H (60px).
     *
     * Safe size sent to Mutter is max(requested, minReported, floor).
     * The requested size is preserved in ctx.targetW/H for clamp comparisons.
     */
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

        const preFrame = window.get_frame_rect();
        const preBuf = window.get_buffer_rect ? window.get_buffer_rect() : preFrame;
        ctx.preSetSizeFrame = preFrame;
        ctx.preSetSizeBuf = preBuf;

        if (safeW !== w || safeH !== h) {
            this._logTime('SET_SIZE_CLAMPED',
                `requestedSize=(${w}x${h}) reportedMinSize=(${minW}x${minH}) ` +
                `floor=(${floorW}x${floorH}) clampedSafeSize=(${safeW}x${safeH})`,
                window);
        }

        if (this._isMaximized(window))
            this._unmaximize(window);

        this._logTime('SET_SIZE_REQUEST',
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

    /** Step: wait for client geometry to settle with enriched telemetry. */
    async _stepWaitForCommit(ctx, args) {
        ctx.commit = await this._waitForCommit(
            ctx.window, ctx.actor, args.timeout_ms,
            ctx.targetW, ctx.targetH, ctx.safeW, ctx.safeH);
    }

    /** Step: place the window using committed size and record telemetry. */
    _stepPosition(ctx, payload) {
        const { window, actor } = ctx;
        ctx.anchorPayload = payload;

        const preFrame = window.get_frame_rect();
        const preBuf = window.get_buffer_rect ? window.get_buffer_rect() : preFrame;

        this._applyAnchoredPosition(window, payload);

        const postFrame = window.get_frame_rect();
        const postBuf = window.get_buffer_rect ? window.get_buffer_rect() : postFrame;
        ctx.positionedW = postFrame.width;
        ctx.positionedH = postFrame.height;

        this._logTime('POSITION_SET',
            `preFrame=(${preFrame.x},${preFrame.y},${preFrame.width}x${preFrame.height}) ` +
            `postFrame=(${postFrame.x},${postFrame.y},${postFrame.width}x${postFrame.height}) ` +
            `bufferRect=(${postBuf.x},${postBuf.y},${postBuf.width}x${postBuf.height}) ` +
            `actor=(${actor.x},${actor.y}) ` +
            `delta=(${postFrame.x - preFrame.x},${postFrame.y - preFrame.y})`,
            window);
    }

    /**
     * Step: uncloak sequence with selective pulse gating.
     *   4A  Safety delay        - let initial buffers paint in the dark
     *   4B  Gated focus pulse   - executed ONLY for VTE candidates with early-exit
     *   4C  Re-anchor           - solve position for final size while still cloaked
     *   4D  Reveal + anchor     - reveal actor, arm reactive anchor
     */
    async _stepUncloak(ctx, args) {
        const delayMs = Number.isFinite(args.delay_ms) ? args.delay_ms : DEFAULT_UNCLOAK_DELAY_MS;
        const wantWake = args.wake !== false;

        // 4A. Safety delay (wait for initial Wayland buffers in the dark)
        this._logTime('UNCLOAK_4A', `delay=${delayMs}ms`, ctx.window);
        if (delayMs > 0)
            await this._sleep(delayMs);
        if (this._disabled)
            return;

        this._cloak(ctx.actor);

        // 4B. Gated focus pulse
        const clamped = this._isToolkitClamped(ctx);
        const isVte = this._isVteCandidate(ctx.window);
        this._logTime('UNCLOAK_4B', `wantWake=${wantWake} clamped=${clamped} isVte=${isVte}`, ctx.window);

        if (wantWake && clamped) {
            if (!isVte) {
                // Non-VTE applications (GTK4, Qt, Kitty, GParted, Mousepad) skip pulse entirely
                this._logTime('FOCUS_PULSE_SKIP_NOT_VTE',
                    `reason="non-VTE toolkit; bypassing synthetic defocus"`,
                    ctx.window);
            } else {
                // Only GTK3+VTE apps run the pulse (with early-exit detection)
                await this._executeFocusPulse(ctx);
                if (this._disabled)
                    return;
                this._cloak(ctx.actor);
            }
        }

        // 4C. Re-anchor for final size, still invisible
        this._reanchorIfResized(ctx);

        // Flush frame updates before revealing
        await this._waitForFrames(2, FRAME_WAIT_CEILING_MS);
        if (this._disabled)
            return;

        // 4D. Reveal LAST
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

        const cur = window.get_frame_rect();
        const resized = cur.width !== ctx.positionedW || cur.height !== ctx.positionedH;
        this._logTime('UNCLOAK_4C',
            `positioned=(${ctx.positionedW}x${ctx.positionedH}) actual=(${cur.width}x${cur.height}) ` +
            `needsReanchor=${resized}`,
            window);
        if (!resized)
            return;

        this._logTime('REANCHOR_TRIGGERED', `newBounds=(${cur.width}x${cur.height})`, window);
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
        this._logTime('UNCLOAK_4D_REVEAL',
            `requestedSize=(${ctx.targetW}x${ctx.targetH}) ` +
            `postFrame=(${rect.x},${rect.y},${rect.width}x${rect.height}) ` +
            `bufferRect=(${buf.x},${buf.y},${buf.width}x${buf.height})`,
            window);

        if (ctx.anchorPayload)
            this._armAnchor(window, ctx.anchorPayload);
    }

    // =======================================================================
    // 7. CLOAK / SNAPSHOT HELPERS
    // =======================================================================

    _cloak(actor) {
        if (!actor)
            return;

        // Register FIRST so any notify::opacity emission mid-setup is caught.
        this._cloakedActors.add(actor);

        this._try(() => {
            actor.remove_all_transitions?.();

            // Prevent Mutter from caching a visible frame and replaying it
            // during position changes — this is what makes the window visibly
            // "slide" from top-left to its anchor.
            if (typeof actor.set_offscreen_redirect === 'function')
                actor.set_offscreen_redirect(Clutter.OffscreenRedirect.NEVER);

            actor.opacity = 0;

            if (!actor._spawnAtOpacityId) {
                actor._spawnAtOpacityId = this._tryConnect(actor, 'notify::opacity', () => {
                    if (this._cloakedActors.has(actor) && actor.opacity !== 0) {
                        actor.remove_all_transitions?.();
                        actor.opacity = 0;
                    }
                });
            }
        });
    }

    _uncloak(actor) {
        if (!actor)
            return;
        this._cloakedActors.delete(actor);
        this._try(() => {
            if (actor._spawnAtOpacityId) {
                this._safeDisconnect(actor, actor._spawnAtOpacityId);
                delete actor._spawnAtOpacityId;
            }
            actor.remove_all_transitions?.();
            if (typeof actor.set_offscreen_redirect === 'function')
                actor.set_offscreen_redirect(Clutter.OffscreenRedirect.AUTOMATIC_FOR_OPACITY);
            if (typeof actor.set_easing_duration === 'function')
                actor.set_easing_duration(0);
            actor.opacity = 255;
            if (!actor.visible)
                actor.show();
        });
    }

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
     * Logs exact resolving signal: AT_TARGET, SIZE_SETTLED, TIMEOUT_NO_CHANGE, HARD_CAP.
     */
    _waitForCommit(window, actor, timeoutMs, targetW = null, targetH = null, safeW = null, safeH = null) {
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
                const buf = window.get_buffer_rect ? window.get_buffer_rect() : rect;
                const exact = hasTarget && (
                    (rect.width === targetW && rect.height === targetH) ||
                    (safeW !== null && safeH !== null && rect.width === safeW && rect.height === safeH)
                );
                const deltaW = rect.width - start.width;
                const deltaH = rect.height - start.height;
                const deltaReqW = targetW !== null ? (rect.width - targetW) : 0;
                const deltaReqH = targetH !== null ? (rect.height - targetH) : 0;

                this._logTime('COMMIT_RESOLVED',
                    `signal="${reason}" ` +
                    `postFrame=(${rect.x},${rect.y},${rect.width}x${rect.height}) ` +
                    `bufferRect=(${buf.x},${buf.y},${buf.width}x${buf.height}) ` +
                    `delta=(${deltaW}x${deltaH}) deltaReq=(${deltaReqW}x${deltaReqH}) ` +
                    `exact=${exact}`,
                    window);
                resolve({ width: rect.width, height: rect.height, exact, reason });
            };

            const check = source => {
                if (finished)
                    return;

                this._cloak(actor);

                let rect;
                try {
                    rect = window.get_frame_rect();
                } catch (_e) {
                    finish('ERROR');
                    return;
                }

                this._logTime('COMMIT_SIGNAL',
                    `source=${source} frame=(${rect.width}x${rect.height}) last=(${lastW}x${lastH})`,
                    window);

                // Exact match: window met requested target or clamped safe target
                if (hasTarget && (
                    (rect.width === targetW && rect.height === targetH) ||
                    (safeW !== null && safeH !== null && rect.width === safeW && rect.height === safeH)
                )) {
                    finish('AT_TARGET');
                    return;
                }

                if (rect.width !== lastW || rect.height !== lastH) {
                    lastW = rect.width;
                    lastH = rect.height;
                    this._removeTimer(idleTimer);
                    idleTimer = 0;
                    this._removeTimer(quietTimer);
                    quietTimer = this._addTimer(COMMIT_QUIET_MS, () => finish('SIZE_SETTLED'));
                }
            };

            if (actor)
                actorSigId = this._tryConnect(actor, 'notify::allocation', () => check('allocation'));
            windowSigId = this._tryConnect(window, 'size-changed', () => check('size-changed'));

            idleTimer = this._addTimer(idleTimeout, () => finish('TIMEOUT_NO_CHANGE'));
            capTimer = this._addTimer(COMMIT_HARD_CAP_MS, () => finish('HARD_CAP'));

            check('initial');
        });
    }

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
    // 9. FOCUS PULSE (GATED WITH EARLY-EXIT)
    // =======================================================================

    /**
     * Focus pulse, executed strictly under the cloak for verified VTE candidates.
     * Incorporates early-exit detection to avoid wasting time if the window is already
     * at its minimum cell grid floor (e.g. Terminator).
     */
    async _executeFocusPulse(ctx) {
        const { window, actor } = ctx;
        const winId = window.get_id ? window.get_id() : -1;
        const startFrame = window.get_frame_rect();
        const startBuf = window.get_buffer_rect ? window.get_buffer_rect() : startFrame;

        this._logTime('FOCUS_PULSE_INIT',
            `windowId=${winId} preFrame=(${startFrame.x},${startFrame.y},${startFrame.width}x${startFrame.height}) ` +
            `bufferRect=(${startBuf.x},${startBuf.y},${startBuf.width}x${startBuf.height}) ` +
            `requestedSize=(${ctx.targetW}x${ctx.targetH}) opacity=${actor?.opacity}`,
            window);

        // 1. Drop seat focus to desktop -> client receives wl_keyboard.leave
        this._defocusWindowObject(window, 'desktop');
        this._logTime('FOCUS_PULSE_DEFOCUSED', `windowId=${winId}`, window);

        // 2. Early-exit detection: listen for size-changed within PULSE_LEAVE_IDLE_MS
        const resized = await this._waitForSizeChangedOrTimeout(window, PULSE_LEAVE_IDLE_MS);
        if (this._disabled)
            return;

        if (!resized) {
            // Early-exit: client did NOT resize upon defocus; restore focus immediately
            this._logTime('FOCUS_PULSE_EARLY_EXIT',
                `windowId=${winId} reason="no size-changed within ${PULSE_LEAVE_IDLE_MS}ms"`,
                window);
            this._focusWindowObject(window);
            await this._waitForWindowCondition(
                window, ['notify::appears-focused', 'focus'],
                () => window.has_focus(), PULSE_REFOCUS_CEILING_MS);
            this._logTime('FOCUS_PULSE_EARLY_EXIT_DONE', `windowId=${winId}`, window);
            return;
        }

        // Full execution: size-changed fired; wait for VTE geometry expansion to settle
        this._logTime('FOCUS_PULSE_FULL_EXECUTION_START', `windowId=${winId}`, window);
        await this._waitForSizeQuiet(window, 0, WAKE_QUIET_MS, WAKE_CEILING_MS);
        if (this._disabled)
            return;
        this._cloak(actor);

        // 3. Restore focus -> client receives wl_keyboard.enter
        this._focusWindowObject(window);
        this._logTime('FOCUS_PULSE_REFOCUSED', `windowId=${winId}`, window);

        // 4. Confirm focus is acknowledged
        const focused = await this._waitForWindowCondition(
            window, ['notify::appears-focused', 'focus'],
            () => window.has_focus(), PULSE_REFOCUS_CEILING_MS);
        this._logTime('FOCUS_PULSE_FOCUS_CONFIRMED', `focused=${focused}`, window);

        await this._waitForSizeQuiet(window, WAKE_IDLE_MS, WAKE_QUIET_MS, WAKE_CEILING_MS);

        const endFrame = window.get_frame_rect();
        const endBuf = window.get_buffer_rect ? window.get_buffer_rect() : endFrame;
        const deltaW = endFrame.width - startFrame.width;
        const deltaH = endFrame.height - startFrame.height;

        this._logTime('FOCUS_PULSE_FULL_EXECUTION_DONE',
            `windowId=${winId} postFrame=(${endFrame.x},${endFrame.y},${endFrame.width}x${endFrame.height}) ` +
            `bufferRect=(${endBuf.x},${endBuf.y},${endBuf.width}x${endBuf.height}) ` +
            `delta=(${deltaW}x${deltaH}) deltaH=${deltaH}`,
            window);
    }

    _waitForSizeChangedOrTimeout(window, timeoutMs) {
        return new Promise(resolve => {
            let finished = false;
            let timerId  = 0;
            let sigId    = 0;

            const finish = ok => {
                if (finished)
                    return;
                finished = true;
                this._removeTimer(timerId);
                this._safeDisconnect(window, sigId);
                resolve(ok);
            };

            sigId   = this._tryConnect(window, 'size-changed', () => finish(true));
            timerId = this._addTimer(timeoutMs, () => finish(false));
        });
    }

    _waitForWindowCondition(window, signals, predicate, ceilingMs) {
        return new Promise(resolve => {
            if (this._try(predicate, false)) {
                resolve(true);
                return;
            }
            let finished = false;
            let ceiling = 0;
            const ids = [];
            const finish = ok => {
                if (finished)
                    return;
                finished = true;
                this._removeTimer(ceiling);
                ids.forEach(id => this._safeDisconnect(window, id));
                resolve(ok);
            };
            const check = () => {
                if (this._try(predicate, false))
                    finish(true);
            };
            for (const sig of signals)
                ids.push(this._tryConnect(window, sig, check));
            ceiling = this._addTimer(ceilingMs, () => finish(false));
        });
    }

    _waitForFrames(count, ceilingMs) {
        return new Promise(resolve => {
            let finished = false;
            let remaining = count;
            let ceiling = 0;
            let sigId = 0;
            const finish = () => {
                if (finished)
                    return;
                finished = true;
                this._removeTimer(ceiling);
                this._safeDisconnect(global.stage, sigId);
                resolve();
            };
            sigId = this._tryConnect(global.stage, 'after-update', () => {
                if (--remaining <= 0)
                    finish();
            });
            ceiling = this._addTimer(ceilingMs, finish);
            this._try(() => global.stage.queue_redraw());
            if (!sigId)
                finish();
        });
    }

    // =======================================================================
    // 10. ANCHORED POSITIONING
    // =======================================================================

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

        const minX = bounds.x + ml;
        const minY = bounds.y + mt;
        let maxX = bounds.x + bounds.width - mr - effW;
        let maxY = bounds.y + bounds.height - mb - effH;

        const oversized = effW > bounds.width - ml - mr || effH > bounds.height - mt - mb;
        if (maxX < minX)
            maxX = minX;
        if (maxY < minY)
            maxY = minY;

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

    _applyAnchoredPosition(window, payload) {
        const actor = this._getActor(window);
        actor?.remove_all_transitions?.();

        const pos = this._computeAnchoredPosition(window, payload);

        if (pos.minW === 0 && pos.minH === 0) {
            this._logTime('FALLBACK_USED',
                `reason="min size unreported" usingFrame=(${pos.effW}x${pos.effH})`,
                window);
        }
        if (pos.oversized) {
            console.warn(`[spawn-at] Window is oversized (${pos.effW}x${pos.effH}); ` +
                'bottom/right margins ignored to keep top-left reachable.');
        }
        if (pos.x !== pos.rawX || pos.y !== pos.rawY) {
            this._logTime('MARGIN_CLAMP_ENGAGED',
                `raw=(${pos.rawX},${pos.rawY}) clamped=(${pos.x},${pos.y})`,
                window);
        }
        this._logTime('APPLY_ANCHOR',
            `eff=(${pos.effW}x${pos.effH}) bounds=(${pos.bounds.x},${pos.bounds.y},` +
            `${pos.bounds.width}x${pos.bounds.height}) final=(${pos.x},${pos.y})`,
            window);

        this._moveFrame(window, pos.x, pos.y);
        return pos;
    }

    // =======================================================================
    // 11. REACTIVE ANCHOR
    // =======================================================================

    _armAnchor(window, payload) {
        this._disarmAnchor(window, 'REARM');

        const state = { payload, sizeId: 0, unmanagedId: 0, retireTimerId: 0 };
        state.sizeId = this._tryConnect(window, 'size-changed',
            () => this._onAnchoredSizeChanged(window, state));
        state.unmanagedId = this._tryConnect(window, 'unmanaged',
            () => this._disarmAnchor(window, 'UNMANAGED'));
        this._anchors.set(window, state);
        this._logTime('ANCHOR_ARMED', '', window);
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

        this._logTime('ANCHOR_CORRECTION',
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
        this._logTime('ANCHOR_RELEASED', `reason=${reason}`, window);
    }

    // =======================================================================
    // 12. WINDOW LOOKUP (shared by the D-Bus API)
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

    // =======================================================================
    // 13. WILDCARD HELPER
    // =======================================================================

    _clearWildcard() {
        this._removeTimer(this._wildcardTimeoutId);
        this._wildcardTimeoutId = 0;
        this._wildcardTarget = null;
    }

    // =======================================================================
    // 14. D-BUS API
    // =======================================================================

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
        this._armedSpawns.get(target_id).push(instructions);

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

    GetCursor() {
        const [x, y] = global.get_pointer();
        return [x, y];
    }

    GetPointer() {
        return this.GetCursor();
    }

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
                x: frame.x,
                y: frame.y,
                w: frame.width,
                h: frame.height,
                focused: win.has_focus(),
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
