/**
 * spawn-at — EXPERIMENT, observe-only: sample a few pixels of an XWayland/X11 window's texture.
 *
 * Runs only when the claimed instructions carry `strategy: "x11-pixel-probe"` (the hidden
 * `--wait-strategy` flag), the window is an XWayland or X11 window, and logging is on. It changes
 * no wait, outcome or order: the wait stays the one the window would have had anyway. The only
 * effect is the cost of the readback itself, which is logged per probe.
 *
 * Question (TEST_PLAN 6.27): does Xwayland hand the compositor a fresh blank buffer at the new size
 * right after the resize (which would explain why `damaged` and the texture size fire early), and
 * can a few pixels tell that blank buffer from "the app has painted"? Per probe it logs, for four
 * 2x2 clips at 1/4 and 3/4 of the texture, the first pixel as #RRGGBBAA, whether the 4 pixels are
 * equal, a class (`transparent`, `uniform`, `mixed`) and a hash. Matching against the fixture's
 * colours (`--state-colors`) is done afterwards from the log, not here.
 *
 * Safety (a Shell crash ends the session): every call is in try/catch; nothing throws into a signal
 * emission; the readback never runs inside the signal handler but from a timer queued by it; at
 * most MAX_PROBES per window, at least MIN_GAP_MS apart, one at a time; X11 windows only.
 *
 * No `gi://` import: the shell provides everything through `host` (tests/x11_probe_test.js).
 */

/** The value of `strategy` that selects this experiment (spelling of `wait_override.rs`). */
export const PROBE_STRATEGY = 'x11-pixel-probe';

/** Probes per window, at most. */
export const MAX_PROBES = 16;

/** Least time between two probes, ms. */
export const MIN_GAP_MS = 8;

/** How long after the reveal the window is still probed, ms. */
export const PROBE_TAIL_MS = 500;

/** Side of each sampled clip, pixels. */
export const CLIP_SIDE = 2;

const msStr = ms => Number(ms).toFixed(3);
const oneWord = s => String(s).replace(/\s+/g, '_').slice(0, 80);

/** The four clip origins: 1/4 and 3/4 of the texture on each axis, kept inside it. */
export function clipPoints(width, height, side = CLIP_SIDE) {
    const px = f => Math.max(0, Math.min(width - side, Math.floor(width * f)));
    const py = f => Math.max(0, Math.min(height - side, Math.floor(height * f)));
    return [[px(0.25), py(0.25)], [px(0.75), py(0.25)], [px(0.25), py(0.75)], [px(0.75), py(0.75)]]
        .map(([x, y]) => ({ x, y, width: side, height: side }));
}

/** FNV-1a (32 bit) of the bytes, as 8 hex digits. */
export function hashBytes(bytes) {
    let h = 0x811c9dc5;
    for (let i = 0; i < bytes.length; i++) {
        h ^= bytes[i] & 0xff;
        h = Math.imul(h, 0x01000193) >>> 0;
    }
    return h.toString(16).padStart(8, '0');
}

/**
 * Describes the RGBA bytes of one clip (4 bytes per pixel, straight alpha): the first pixel, whether
 * every pixel is equal, and the class.
 */
export function describePixels(bytes) {
    if (!bytes || bytes.length < 4)
        return { rgba: 'na', uniform: false, cls: 'empty', hash: '00000000' };
    const hex = i => [0, 1, 2, 3].map(k => (bytes[i + k] & 0xff).toString(16).padStart(2, '0')).join('');
    const first = hex(0);
    let uniform = true;
    let allTransparent = true;
    for (let i = 0; i + 3 < bytes.length; i += 4) {
        if (hex(i) !== first)
            uniform = false;
        if ((bytes[i + 3] & 0xff) !== 0)
            allTransparent = false;
    }
    return {
        rgba: `#${first}`,
        uniform,
        cls: allTransparent ? 'transparent' : (uniform ? 'uniform' : 'mixed'),
        hash: hashBytes(bytes),
    };
}

/** `X11_PROBE_SAMPLE` extra. */
export function formatSample(s) {
    const parts = s.samples.map((d, i) =>
        d.error ? `s${i}=err:${oneWord(d.error)}` : `s${i}=${d.rgba}/${d.cls}/h:${d.hash}`);
    return `signal=${s.signal} seq=${s.seq} mono_ms=${msStr(s.monoMs)} ` +
        `texture=${s.texture ? `(${s.texture.width}x${s.texture.height})` : '(na)'} ` +
        `cost_ms=${msStr(s.costMs)} ${parts.join(' ')}`;
}

/**
 * Probes the texture of spawned X11 windows.
 *
 * `host` provides: isLoggingEnabled(), logTime(tag, extra, window), nowMs(), tryConnect(target, signal, cb) -> id,
 * safeDisconnect(target, id), addTimer(ms, cb) -> id, removeTimer(id), signalExists(actor, name),
 * textureSize(actor) -> {width, height}|{error}|null, readPixels(actor, clip) -> Uint8Array of RGBA
 * (throws when it cannot), readbackReady() -> {ok: boolean, why?: string}.
 */
export class X11PixelProbe {
    constructor(host, tailMs = PROBE_TAIL_MS) {
        this._host = host;
        this._tailMs = tailMs;
        this._watches = new Map();
    }

    get activeCount() {
        return this._watches.size;
    }

    /** Number of signal handlers and timers still held (0 after every watch has ended). */
    get handlerCount() {
        let n = 0;
        for (const w of this._watches.values())
            n += w.ids.length + (w.timer ? 1 : 0) + (w.tailTimer ? 1 : 0);
        return n;
    }

    /** Starts probing `window`; returns whether a watch started. No-op unless logging is on. */
    start(window, actor) {
        const h = this._host;
        try {
            if (!actor || !h.isLoggingEnabled() || this._watches.has(window))
                return false;
            const ready = h.readbackReady();
            const watch = { actor, ids: [], seq: 0, probes: 0, skipped: 0, costMs: 0, lastAt: -Infinity, timer: 0, tailTimer: 0, unmanagedId: 0, ended: false };
            this._watches.set(window, watch);
            h.logTime('X11_PROBE_START', `readback=${ready.ok ? 'available' : `unavailable:${oneWord(ready.why ?? 'unknown')}`} max=${MAX_PROBES} gap_ms=${MIN_GAP_MS} clip=${CLIP_SIDE}x${CLIP_SIDE}`, window);
            for (const name of ['damaged', 'first-frame']) {
                if (!h.signalExists(actor, name))
                    continue;
                const id = h.tryConnect(actor, name, () => this._onSignal(window, watch, name));
                if (id)
                    watch.ids.push({ name, id });
            }
            watch.unmanagedId = h.tryConnect(window, 'unmanaged', () => this._end(window, 'unmanaged'));
            return true;
        } catch (_e) {
            this._end(window, 'error');
            return false;
        }
    }

    /** Marks the reveal and ends the watch `tailMs` later. */
    reveal(window) {
        try {
            const w = this._watches.get(window);
            if (!w || w.ended)
                return;
            this._host.removeTimer(w.tailTimer);
            w.tailTimer = this._host.addTimer(this._tailMs, () => {
                w.tailTimer = 0;
                this._end(window, 'reveal+500ms');
            });
        } catch (_e) {
            // Nothing to do.
        }
    }

    stopAll(reason = 'disabled') {
        for (const window of [...this._watches.keys()])
            this._end(window, reason);
    }

    _onSignal(window, watch, signal) {
        // Never reads pixels here: it queues a timer and returns.
        try {
            const h = this._host;
            if (watch.ended || !h.isLoggingEnabled())
                return;
            watch.seq += 1;
            const now = h.nowMs();
            if (watch.probes >= MAX_PROBES || watch.timer || now - watch.lastAt < MIN_GAP_MS) {
                watch.skipped += 1;
                return;
            }
            const seq = watch.seq;
            watch.timer = h.addTimer(0, () => {
                watch.timer = 0;
                this._probe(window, watch, signal, seq);
            });
        } catch (_e) {
            // Never throws into the compositor's signal emission.
        }
    }

    _probe(window, watch, signal, seq) {
        const h = this._host;
        try {
            if (watch.ended || !h.isLoggingEnabled())
                return;
            const started = h.nowMs();
            const tex = h.textureSize(watch.actor);
            const ok = tex && Number.isFinite(tex.width) && Number.isFinite(tex.height) && tex.width >= CLIP_SIDE && tex.height >= CLIP_SIDE;
            const samples = [];
            if (!ok) {
                samples.push({ error: tex?.error ? `texture:${tex.error}` : 'texture:unreadable' });
            } else {
                for (const clip of clipPoints(tex.width, tex.height)) {
                    try {
                        samples.push(describePixels(h.readPixels(watch.actor, clip)));
                    } catch (e) {
                        samples.push({ error: e?.message ?? String(e) });
                    }
                }
            }
            const costMs = h.nowMs() - started;
            watch.probes += 1;
            watch.costMs += costMs;
            watch.lastAt = h.nowMs();
            h.logTime('X11_PROBE_SAMPLE', formatSample({ signal, seq, monoMs: started, texture: ok ? tex : null, costMs, samples }), window);
        } catch (_e) {
            // Observation never throws.
        }
    }

    _end(window, reason) {
        const w = this._watches.get(window);
        if (!w || w.ended)
            return;
        w.ended = true;
        const h = this._host;
        try {
            for (const { id } of w.ids)
                h.safeDisconnect(w.actor, id);
            w.ids = [];
            h.safeDisconnect(window, w.unmanagedId);
            h.removeTimer(w.timer);
            h.removeTimer(w.tailTimer);
            w.timer = 0;
            w.tailTimer = 0;
            this._watches.delete(window);
            if (h.isLoggingEnabled())
                h.logTime('X11_PROBE_END', `reason=${reason} probes=${w.probes} skipped=${w.skipped} total_cost_ms=${msStr(w.costMs)}`, window);
        } catch (_e) {
            // Nothing to do.
        }
    }
}
