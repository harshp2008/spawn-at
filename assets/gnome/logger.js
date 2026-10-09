/**
 * spawn-at — persistent session logging and enriched telemetry.
 *
 * Maintains diagnostic logs at ~/.local/state/spawn-at/session.log
 * with mode 0600 in a mode 0700 directory. Rotates logs across login sessions
 * and enforces a 5MB size ceiling.
 */

import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import Meta from 'gi://Meta';

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
    }

    /**
     * Initializes session logging directory (0700) and file (0600).
     */
    init() {
        this._pendingLogLines = [];
        this._logWriting = false;
        this._logFlushScheduled = false;
        this._disabled = false;

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
                this.queueLogLine(`=== SPAWN-AT SESSION STARTED: ${new Date().toISOString()} ===`);
            } else {
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
            GLib.idle_add(GLib.PRIORITY_LOW, () => {
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
                GLib.PRIORITY_LOW,
                null,
                (file, res) => {
                    try {
                        const stream = file.append_to_finish(res);
                        // Enforce 0600 permissions
                        if (this._sessionLogPath)
                            GLib.chmod(this._sessionLogPath, 0o600);

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

    /** Drains pending lines synchronously during disable(). */
    flushSync() {
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
        this._loggingEnabled = env !== '0' && env !== 'false';
        return this._loggingEnabled;
    }

    setLogging(enabled) {
        this._loggingEnabled = Boolean(enabled);
        this.logTime('SET_LOGGING', `enabled=${this._loggingEnabled}`);
    }

    /** Formats window metadata: ID, PID, WM_CLASS, App ID, window type, protocol. */
    formatWindowMeta(window) {
        if (!window)
            return 'win=[none]';

        let clientType = -1;
        try { clientType = window.get_client_type?.(); } catch (_e) {}

        let protocol = 'Unknown';
        if (clientType === Meta.WindowClientType?.WAYLAND || clientType === 0)
            protocol = 'Wayland';
        else if (clientType === Meta.WindowClientType?.X11 || clientType === 1)
            protocol = 'XWayland';

        let typeInt = -1;
        try { typeInt = window.get_window_type?.(); } catch (_e) {}

        let windowType = 'UNKNOWN';
        if (Meta.WindowType) {
            for (const [name, val] of Object.entries(Meta.WindowType)) {
                if (val === typeInt) {
                    windowType = name;
                    break;
                }
            }
        }

        let id = -1;
        let pid = -1;
        let wmClass = '';
        let appId = '';

        try { id = window.get_id?.() ?? -1; } catch (_e) {}
        try { pid = window.get_pid?.() ?? -1; } catch (_e) {}
        try { wmClass = window.get_wm_class?.() || ''; } catch (_e) {}
        try {
            appId = window.get_gtk_application_id?.() ||
                    window.get_sandboxed_app_id?.() || '';
        } catch (_e) {}

        return `win=[id=${id} pid=${pid} class="${wmClass}" app="${appId}" type=${windowType} proto=${protocol}]`;
    }

    /** Timestamped diagnostic line written to session.log and console.error. */
    logTime(tag, extra = '', window = null) {
        const nowUs = GLib.get_monotonic_time();
        if (!this._t0)
            this._t0 = nowUs;
        const elapsedMs = ((nowUs - this._t0) / 1000.0).toFixed(2);
        const winInfo = window ? (` ${this.formatWindowMeta(window)}`) : '';
        const payload = `+${elapsedMs}ms | ${tag}${winInfo} ${extra}`.trim();

        const iso = new Date().toISOString();
        this.queueLogLine(`[${iso}] ${payload}`);

        if (this.isLoggingEnabled())
            console.error(`[spawn-at-time] ${payload}`);
    }
}
