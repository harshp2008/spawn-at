/**
 * spawn-at — observe-only logging of the paint signals of X11 and XWayland windows.
 *
 * On X11 and XWayland the window manager sets the frame size at once, so "frame == target" says
 * nothing about whether the app has painted into it. This module only LOOKS for a better signal:
 * from the dispatch of a spawn until 500 ms after its reveal it logs every `damaged` and
 * `first-frame` emission of the window actor, with the actor texture size, the frame rect and the
 * buffer rect, plus marks for the set-size, the end of the commit wait and the reveal so the lines
 * can be joined on one monotonic clock. It changes no timing, order or outcome, and does nothing
 * unless the SetLogging flag is on (docs/DESIGN_READY_WAIT.md, "X11 observation").
 *
 * No `gi://` import and no GJS-only API: everything the shell provides comes in through `host`,
 * so the formatting and the lifecycle are tested in plain Node (tests/x11_observe_test.js).
 */

/** How long after the reveal the signals are still logged. */
export const OBSERVE_TAIL_MS = 500;

/**
 * The signals that are connected, in the order they are looked up and logged. `thawed` is the
 * actor's end of a frozen state (Mutter freezes a window actor while it waits for an X11 client
 * that took part in `_NET_WM_SYNC_REQUEST` to finish redrawing; TEST_PLAN 6.32): it is only logged,
 * with the same fields as the others, to see whether and when it fires for XWayland windows.
 */
export const SIGNALS = ['damaged', 'first-frame', 'thawed'];

/** True for the window kinds this module watches (see `_windowKind` in extension.js). */
export function isX11Kind(kind) {
    return kind === 'XWayland' || kind === 'X11';
}

const rectStr = r => (r && Number.isFinite(r.width))
    ? `(${r.x},${r.y},${r.width}x${r.height})`
    : '(na)';

const sizeStr = s => (s && Number.isFinite(s.width) && Number.isFinite(s.height))
    ? `(${s.width}x${s.height})`
    : '(na)';

const msStr = ms => Number(ms).toFixed(3);

/** One line, no spaces (the log is split on spaces), at most 60 characters. */
const oneWord = s => String(s).replace(/\s+/g, '_').slice(0, 60);

/** Same, for an error list that is already joined with `|`: only the length is limited, to 240. */
const oneWord2 = s => String(s).replace(/\s+/g, '_').slice(0, 240);

const finiteSize =(w, h) => Number.isFinite(w) && Number.isFinite(h) && w >= 0 && h >= 0;

/**
 * The texture accessors, tried in this order, each in its own try/catch. The chain is the one
 * GNOME 46's Meta-14 typelib has (read with gjs, DESIGN_READY_WAIT section 12): `WindowActor.get_texture()`
 * returns a `Meta.ShapedTexture`, which has no `get_width`; its `get_texture()` returns a `Meta.MultiTexture`
 * with `get_width()` / `get_height()` and `get_plane(n)`, a `Cogl.Texture` with the same two methods.
 */
export const TEXTURE_ACCESSORS = [
    ['shaped>multi', actor => {
        const m = actor.get_texture().get_texture();
        return [m.get_width(), m.get_height()];
    }],
    ['shaped>multi>plane0', actor => {
        const p = actor.get_texture().get_texture().get_plane(0);
        return [p.get_width(), p.get_height()];
    }],
];

/**
 * Reads the size of the actor's surface texture. Returns `{ width, height, via }` for the first
 * accessor that gives a finite size, else `{ error }` listing each accessor's own error message
 * (`name:message`, joined by `|`). Never throws and changes nothing: every call is a getter.
 */
export function readTextureSize(actor) {
    const errors = [];
    for (const [name, read] of TEXTURE_ACCESSORS) {
        try {
            const [w, h] = read(actor);
            if (finiteSize(w, h))
                return { width: w, height: h, via: name };
            errors.push(`${name}:bad_size_${w}x${h}`);
        } catch (e) {
            errors.push(`${name}:${oneWord(e?.message ?? e)}`);
        }
    }
    return { error: errors.join('|') };
}

/** `X11_OBS_START` extra: what the window is and which signals exist. */
export function formatStart(info) {
    return `kind=${info.kind} wayland=${Boolean(info.wayland)} client_type=${info.clientType ?? 'na'} ` +
        `id=${info.id} mono_ms=${msStr(info.monoMs)} ` +
        `damaged=${info.signals.damaged ? 'available' : 'unavailable'} ` +
        `first_frame=${info.signals['first-frame'] ? 'available' : 'unavailable'}`;
}

/** `X11_OBS_MARK` extra: a point of the spawn on the same clock (set_size, commit_resolved, reveal). */
export function formatMark(what, id, monoMs) {
    return `what=${what} id=${id} mono_ms=${msStr(monoMs)}`;
}

/** `X11_OBS_SIGNAL` extra: one emission with the sizes at that moment. */
export function formatSignal(s) {
    // `texture=(WxH) texture_via=<accessor>` when read, `texture=(na) texture_err=<name:message|...>`
    // when every accessor failed, `texture=(na)` when the host gave nothing.
    const tex = s.texture;
    const texExtra = tex?.via ? ` texture_via=${tex.via}` : tex?.error ? ` texture_err=${oneWord2(tex.error)}` : '';
    return `signal=${s.signal} id=${s.id} mono_ms=${msStr(s.monoMs)} seq=${s.seq} ` +
        `texture=${sizeStr(tex)}${texExtra} actor=${sizeStr(s.actor)} ` +
        `frame=${rectStr(s.frame)} buffer=${rectStr(s.buffer)}`;
}

/** `X11_OBS_END` extra: why the watch ended and how much it saw. */
export function formatEnd(id, reason, monoMs, counts) {
    // ` thawed=N` is appended only when the actor was thawed, so a line without one is unchanged.
    const thawed = counts.thawed ? ` thawed=${counts.thawed}` : '';
    return `id=${id} reason=${reason} mono_ms=${msStr(monoMs)} ` +
        `damaged=${counts.damaged ?? 0} first_frame=${counts['first-frame'] ?? 0}${thawed}`;
}

/** The note written once when `thawed` does not exist on the actor class. */
export const THAWED_UNAVAILABLE = 'thawed unavailable';

/** The note written once when `damaged` does not exist on the actor class. */
export const DAMAGED_UNAVAILABLE = 'damaged unavailable';

/**
 * Watches the paint signals of spawned X11 windows.
 *
 * `host` provides (all of it from the extension):
 *   isLoggingEnabled(), logTime(tag, extra, window), nowMs(),
 *   tryConnect(target, signal, cb) -> id, safeDisconnect(target, id),
 *   addTimer(ms, cb) -> id, removeTimer(id),
 *   signalExists(actor, name) -> bool            (GObject.signal_lookup in the shell),
 *   windowId(window), clientType(window), isWayland(),
 *   frameRect(window), bufferRect(window), textureSize(actor), actorSize(actor).
 */
export class X11Observer {
    constructor(host, tailMs = OBSERVE_TAIL_MS) {
        this._host = host;
        this._tailMs = tailMs;
        this._watches = new Map();
        this._unavailableNoted = false;
        this._thawedNoted = false;
    }

    /** Number of windows being watched; 0 when nothing is connected. */
    get activeCount() {
        return this._watches.size;
    }

    /** Number of signal handlers still connected (0 after every watch has ended). */
    get handlerCount() {
        let n = 0;
        for (const w of this._watches.values())
            n += w.ids.length;
        return n;
    }

    /**
     * Starts watching `window` (its `actor`) when it is an X11 or XWayland window and logging is
     * on. Returns whether a watch was started. With logging off, or any other kind of window, it
     * returns at once: nothing is connected, no timer is added, nothing is logged.
     */
    start(window, actor, kind) {
        const h = this._host;
        if (!isX11Kind(kind) || !actor || !h.isLoggingEnabled() || this._watches.has(window))
            return false;
        try {
            const id = h.windowId(window);
            const signals = {};
            for (const name of SIGNALS)
                signals[name] = Boolean(h.signalExists(actor, name));
            const watch = { id, actor, ids: [], seq: 0, counts: {}, tailTimer: 0, unmanagedId: 0, ended: false };
            this._watches.set(window, watch);

            h.logTime('X11_OBS_START', formatStart({
                kind, wayland: h.isWayland(), clientType: h.clientType(window), id,
                monoMs: h.nowMs(), signals,
            }), window);

            if (!signals.damaged && !this._unavailableNoted) {
                this._unavailableNoted = true;
                h.logTime('X11_OBS_NOTE', DAMAGED_UNAVAILABLE, window);
            }
            if (!signals.thawed && !this._thawedNoted) {
                this._thawedNoted = true;
                h.logTime('X11_OBS_NOTE', THAWED_UNAVAILABLE, window);
            }
            for (const name of SIGNALS) {
                if (!signals[name])
                    continue;
                const cid = h.tryConnect(actor, name, () => this._emit(window, watch, name));
                if (cid)
                    watch.ids.push({ name, cid });
            }
            watch.unmanagedId = h.tryConnect(window, 'unmanaged', () => this._end(window, 'unmanaged'));
            return true;
        } catch (_e) {
            this._end(window, 'error');
            return false;
        }
    }

    /** Logs a point of the spawn (`set_size`, `commit_resolved`) for a watched window. */
    mark(window, what) {
        const w = this._watches.get(window);
        if (!w || w.ended || !this._host.isLoggingEnabled())
            return;
        this._host.logTime('X11_OBS_MARK', formatMark(what, w.id, this._host.nowMs()), window);
    }

    /** Marks the reveal and ends the watch `tailMs` later. */
    reveal(window) {
        const w = this._watches.get(window);
        if (!w || w.ended)
            return;
        this.mark(window, 'reveal');
        this._host.removeTimer(w.tailTimer);
        w.tailTimer = this._host.addTimer(this._tailMs, () => this._end(window, 'reveal+500ms'));
    }

    /** Ends every watch (the extension is being disabled). */
    stopAll(reason = 'disabled') {
        for (const window of [...this._watches.keys()])
            this._end(window, reason);
    }

    _emit(window, watch, signal) {
        const h = this._host;
        if (watch.ended || !h.isLoggingEnabled())
            return;
        try {
            watch.seq += 1;
            watch.counts[signal] = (watch.counts[signal] ?? 0) + 1;
            h.logTime('X11_OBS_SIGNAL', formatSignal({
                signal, id: watch.id, monoMs: h.nowMs(), seq: watch.seq,
                texture: h.textureSize(watch.actor), actor: h.actorSize(watch.actor),
                frame: h.frameRect(window), buffer: h.bufferRect(window),
            }), window);
        } catch (_e) {
            // Observation never throws into the compositor's signal emission.
        }
    }

    _end(window, reason) {
        const w = this._watches.get(window);
        if (!w || w.ended)
            return;
        w.ended = true;
        const h = this._host;
        for (const { cid } of w.ids)
            h.safeDisconnect(w.actor, cid);
        w.ids = [];
        h.safeDisconnect(window, w.unmanagedId);
        h.removeTimer(w.tailTimer);
        this._watches.delete(window);
        if (h.isLoggingEnabled())
            h.logTime('X11_OBS_END', formatEnd(w.id, reason, h.nowMs(), w.counts), window);
    }
}
