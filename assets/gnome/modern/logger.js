/**
 * spawn-at — persistent session logging and enriched telemetry.
 *
 * Maintains diagnostic logs at ~/.local/state/spawn-at/session.log
 * with mode 0600 in a mode 0700 directory. Rotates logs across login sessions
 * and enforces a 5MB size ceiling.
 *
 * Telemetry events are buffered in memory and flushed asynchronously off
 * the hot path to avoid perturbing compositor animation loops.
 */

import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import Meta from 'gi://Meta';

// Cache of precomputed Meta.WindowType enum names to avoid Object.entries iteration on every trace
let _windowTypeMap = null;

function getWindowTypeName(typeInt) {
    if (!_windowTypeMap) {
        _windowTypeMap = new Map();
        if (Meta && Meta.WindowType) {
            for (const [name, val] of Object.entries(Meta.WindowType)) {
                _windowTypeMap.set(val, name);
            }
        }
    }
    return _windowTypeMap.get(typeInt) ?? 'UNKNOWN';
}

// WeakMap cache for immutable window properties (id, pid)
const _windowImmutableCache = new WeakMap();

function getImmutableWindowMeta(window) {
    let cached = _windowImmutableCache.get(window);
    if (!cached) {
        let id = -1;
        let pid = -1;
        try { id = window.get_id?.() ?? -1; } catch (_e) {}
        try { pid = window.get_pid?.() ?? -1; } catch (_e) {}
        cached = { id, pid };
        _windowImmutableCache.set(window, cached);
    }
    return cached;
}

export class SessionLogger {
    constructor() {
        this._pendingLogLines = [];
        this._logWriting = false;
        this._logFlushScheduled = false;
        this._sessionLogPath = null;
        this._sessionLogFile = null;
        this._loggingEnabled = undefined;
        this._t0 = 0;
        this._disabled = false;

        // In-memory trace event buffer with capacity cap and drop counter
        this._eventBuffer = [];
        this._maxBufferSize = 1000;
        this._droppedEvents = 0;
        this._flushScheduled = false;
        this._flushSourceId = null;
    }

    /**
     * Initializes session logging directory (0700) and file (0600).
     */
    init() {
        this._pendingLogLines = [];
        this._logWriting = false;
        this._logFlushScheduled = false;
        this._disabled = false;
        this._eventBuffer = [];
        this._droppedEvents = 0;
        this._flushScheduled = false;
        this._flushSourceId = null;

        try {
            const stateDir = GLib.build_filenamev([GLib.get_user_state_dir(), 'spawn-at']);
            GLib.mkdir_with_parents(stateDir, 0o700);

            this._sessionLogPath = GLib.build_filenamev([stateDir, 'session.log']);
            const oldLogPath = GLib.build_filenamev([stateDir, 'session.log.old']);
            this._sessionLogFile = Gio.File.new_for_path(this._sessionLogPath);

            const runtimeDir = GLib.get_user_runtime_dir();
            const markerPath = GLib.build_filenamev([runtimeDir, 'spawn-at-session.marker']);
            const markerFile = Gio.File.new_for_path(markerPath);

            const isNewSession = !markerFile.query_exists(null);
            if (isNewSession) {
                if (this._sessionLogFile.query_exists(null)) {
                    try {
                        const oldFile = Gio.File.new_for_path(oldLogPath);
                        this._sessionLogFile.move(oldFile, Gio.FileCopyFlags.OVERWRITE, null, null);
                    } catch (_e) {}
                }
                try {
                    const st = markerFile.create(Gio.FileCreateFlags.NONE, null);
                    st.close(null);
                } catch (_e) {}
                if (this.isLoggingEnabled())
                    this.queueLogLine(`=== SPAWN-AT SESSION STARTED: ${new Date().toISOString()} ===`);
            } else if (this.isLoggingEnabled()) {
                this.queueLogLine(`--- SPAWN-AT EXTENSION RE-ENABLED: ${new Date().toISOString()} ---`);
            }

            // Ensure mode 0600 on the active log file if it exists
            if (this._sessionLogFile.query_exists(null))
                GLib.chmod(this._sessionLogPath, 0o600);
        } catch (e) {
            console.error(`[SpawnAt] Failed to initialize session logging: ${e}`);
        }
    }

    /** Appends a formatted line to the async write queue and schedules flush. */
    queueLogLine(line) {
        if (!this._pendingLogLines)
            this._pendingLogLines = [];
        this._pendingLogLines.push(line);

        if (!this._logWriting && !this._logFlushScheduled) {
            this._logFlushScheduled = true;
            GLib.idle_add(GLib.PRIORITY_LOW ?? 300, () => {
                this._logFlushScheduled = false;
                this._flushLogQueue();
                return GLib.SOURCE_REMOVE;
            });
        }
    }

    /** Asynchronously writes queued lines to session.log. */
    _flushLogQueue() {
        if (this._disabled && this._pendingLogLines.length === 0)
            return;
        if (this._logWriting || !this._sessionLogFile || this._pendingLogLines.length === 0)
            return;

        this._logWriting = true;
        const chunk = this._pendingLogLines.splice(0).join('\n') + '\n';
        const bytes = new GLib.Bytes(new TextEncoder().encode(chunk));

        try {
            // 5 MB rotation ceiling (FINDING-15)
            if (this._sessionLogFile.query_exists(null)) {
                const info = this._sessionLogFile.query_info('standard::size', Gio.FileQueryInfoFlags.NONE, null);
                if (info && info.get_size() >= 5 * 1024 * 1024) {
                    const oldLogPath = GLib.build_filenamev([GLib.get_user_state_dir(), 'spawn-at', 'session.log.old']);
                    const oldFile = Gio.File.new_for_path(oldLogPath);
                    this._sessionLogFile.move(oldFile, Gio.FileCopyFlags.OVERWRITE, null, null);
                }
            }
        } catch (_e) {}

        try {
            this._sessionLogFile.append_to_async(
                Gio.FileCreateFlags.NONE,
                GLib.PRIORITY_LOW ?? 300,
                null,
                (file, res) => {
                    try {
                        const stream = file.append_to_finish(res);
                        // Enforce 0600 permissions
                        if (this._sessionLogPath)
                            GLib.chmod(this._sessionLogPath, 0o600);

                        stream.write_bytes_async(
                            bytes,
                            GLib.PRIORITY_LOW ?? 300,
                            null,
                            (s, wres) => {
                                try {
                                    s.write_bytes_finish(wres);
                                    s.close_async(GLib.PRIORITY_LOW ?? 300, null, (cs, cres) => {
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

    /**
     * Flushes buffered in-memory trace events to journald and session.log.
     */
    flush() {
        if (this._flushSourceId && GLib.source_remove) {
            GLib.source_remove(this._flushSourceId);
            this._flushSourceId = null;
        }
        this._flushScheduled = false;

        if (this._droppedEvents > 0) {
            const dropMsg = `WARNING: ${this._droppedEvents} trace events were dropped due to buffer capacity`;
            console.error(`[spawn-at-time] ${dropMsg}`);
            this.queueLogLine(`[${new Date().toISOString()}] ${dropMsg}`);
            this._droppedEvents = 0;
        }

        if (this._eventBuffer.length === 0)
            return;

        const events = this._eventBuffer.splice(0);
        const iso = new Date().toISOString();

        for (const payload of events) {
            console.error(`[spawn-at-time] ${payload}`);
            this.queueLogLine(`[${iso}] ${payload}`);
        }
    }

    /** Drains pending lines synchronously during disable(). */
    flushSync() {
        this.flush();
        this._disabled = true;
        if (!this._sessionLogFile || !this._pendingLogLines || this._pendingLogLines.length === 0)
            return;
        try {
            const stream = this._sessionLogFile.append_to(Gio.FileCreateFlags.NONE, null);
            if (stream) {
                const chunk = this._pendingLogLines.splice(0).join('\n') + '\n';
                stream.write_bytes(new GLib.Bytes(new TextEncoder().encode(chunk)), null);
                stream.close(null);
            }
            if (this._sessionLogPath)
                GLib.chmod(this._sessionLogPath, 0o600);
        } catch (_e) {}
    }

    isLoggingEnabled() {
        if (this._loggingEnabled !== undefined)
            return this._loggingEnabled;
        const env = GLib.getenv('SPAWN_AT_DEBUG');
        this._loggingEnabled = env === '1' || env === 'true';
        return this._loggingEnabled;
    }

    setLogging(enabled) {
        const prev = this._loggingEnabled;
        this._loggingEnabled = Boolean(enabled);
        if (this._loggingEnabled) {
            this.logTime('SET_LOGGING', `enabled=true`);
        } else if (prev) {
            this.logTime('SET_LOGGING', `enabled=false`);
            this.flush();
        }
    }

    /** Formats window metadata: ID, PID, WM_CLASS, App ID, window type, protocol. */
    formatWindowMeta(window) {
        if (!window)
            return 'win=[none]';

        const { id, pid } = getImmutableWindowMeta(window);

        let clientType = -1;
        try { clientType = window.get_client_type?.(); } catch (_e) {}

        let protocol = 'Unknown';
        const isWayland = typeof Meta.is_wayland_compositor === 'function' ? Meta.is_wayland_compositor() : false;
        if (clientType === Meta.WindowClientType?.WAYLAND || clientType === 0)
            protocol = 'Wayland';
        else if (clientType === Meta.WindowClientType?.X11 || clientType === 1)
            protocol = isWayland ? 'XWayland' : 'X11';

        let typeInt = -1;
        try { typeInt = window.get_window_type?.(); } catch (_e) {}
        const windowType = getWindowTypeName(typeInt);

        let wmClass = '';
        let appId = '';

        try { wmClass = window.get_wm_class?.() || ''; } catch (_e) {}
        try {
            appId = window.get_gtk_application_id?.() ||
                    window.get_sandboxed_app_id?.() || '';
        } catch (_e) {}

        return `win=[id=${id} pid=${pid} class="${wmClass}" app="${appId}" type=${windowType} proto=${protocol}]`;
    }

    /** Timestamped diagnostic line buffered in-memory and flushed off the hot path. */
    logTime(tag, extra = '', window = null) {
        if (!this.isLoggingEnabled())
            return;

        const nowUs = GLib.get_monotonic_time();
        if (!this._t0)
            this._t0 = nowUs;
        const elapsedMs = ((nowUs - this._t0) / 1000.0).toFixed(2);
        const winInfo = window ? (` ${this.formatWindowMeta(window)}`) : '';
        const payload = `+${elapsedMs}ms | ${tag}${winInfo} ${extra}`.trim();

        if (this._eventBuffer.length >= this._maxBufferSize) {
            this._droppedEvents++;
        } else {
            this._eventBuffer.push(payload);
        }

        if (!this._flushScheduled && !this._disabled) {
            this._flushScheduled = true;
            this._flushSourceId = GLib.idle_add(GLib.PRIORITY_LOW ?? 300, () => {
                this._flushScheduled = false;
                this._flushSourceId = null;
                this.flush();
                return GLib.SOURCE_REMOVE;
            });
        }
    }
}
