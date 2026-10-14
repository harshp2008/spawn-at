/**
 * spawn-at — EXPERIMENT: resize an XWayland/X11 window before it is mapped, then reveal on its
 * first-frame.
 *
 * Off by default and never chosen by the extension itself: it runs only when the claimed
 * instructions carry `strategy: "x11-premap"` (the hidden `--wait-strategy` flag) and the window is
 * an XWayland or X11 window with a size request that was claimed at `window-created`, before
 * `map` (docs/DESIGN_READY_WAIT.md, section 14). Anything else never reaches this module.
 *
 * Why: TEST_PLAN 6.27 phase 1 found that the batch resizes at `map`, 21 to 41 ms after
 * `window-created`, after the fixture's first paint (at the old size) in every idle run. If the
 * resize is issued at `window-created`, the app's FIRST paint might already be at the target size,
 * and `first-frame` (always after map) might then be a valid ready signal. HYPOTHESIS until run.
 *
 * No `gi://` import and no GJS-only API: everything the shell provides comes in through `host`, so
 * the logic is tested in plain Node (tests/x11_premap_test.js). Every host call is wrapped here, so
 * nothing in this module throws into a signal emission.
 */

/** The value of `strategy` that selects this experiment (spelling of `wait_override.rs`). */
export const PREMAP_STRATEGY = 'x11-premap';

/** Cap used when the instruction has no `cap_ms`; the same 250 ms as the ready-wait. */
export const PREMAP_DEFAULT_CAP_MS = 250;

/** The wait never outlasts this, as for the ready-wait. */
export const PREMAP_HARD_MS = 1500;

/** The distinct labels of the ways the wait ends, with the `COMMIT_RESOLVED` reason of each. */
export const PREMAP_LABELS = {
    firstFrame: { label: 'x11-premap-first-frame', commit: 'X11_PREMAP_FIRST_FRAME' },
    cap: { label: 'x11-premap-cap', commit: 'X11_PREMAP_CAP' },
    hard: { label: 'x11-premap-hard', commit: 'X11_PREMAP_HARD' },
    unmanaged: { label: 'x11-premap-unmanaged', commit: 'SKIPPED' },
    noSignal: { label: 'x11-premap-no-first-frame-signal', commit: 'SKIPPED' },
};

const fmt = s => (s && Number.isFinite(s[0]) && Number.isFinite(s[1])) ? `(${s[0]}x${s[1]})` : '(na)';

/**
 * Reads the plan from the normalized instructions (`{name, args}`): a positive `SetSize` and a
 * `WaitForCommit` whose `strategy` is `x11-premap`. Returns `{w, h, capMs}` or `null`.
 */
export function findPremapPlan(ops) {
    try {
        const size = ops.find(op => op.name === 'SetSize');
        const wait = ops.find(op => op.name === 'WaitForCommit');
        if (!size || !wait || wait.args?.strategy !== PREMAP_STRATEGY)
            return null;
        const { w, h } = size.args ?? {};
        if (!(w > 0 && h > 0))
            return null;
        const capMs = Number.isInteger(wait.args.cap_ms) && wait.args.cap_ms >= 1 ? wait.args.cap_ms : undefined;
        return { w, h, capMs };
    } catch (_e) {
        return null;
    }
}

/**
 * Watches the `first-frame` of one window from before its resize. Connected first, resized
 * second, so a `first-frame` that fires before the resize call can be told from one after it.
 *
 * `host` provides: nowMs(), logTime(tag, extra, window), tryConnect(target, signal, cb) -> id,
 * safeDisconnect(target, id), signalExists(actor, name), textureSize(actor) -> {width, height}|{error}|null.
 */
export class PremapWatch {
    constructor(host, window, actor, plan) {
        this._host = host;
        this._window = window;
        this._actor = actor;
        this.plan = plan;
        this.resizeAtMs = null;
        this.firstFrameAtMs = null;
        this.firstFrameTexture = null;
        this.stopped = false;
        this._frameId = 0;
        this._unmanagedId = 0;
        this._listeners = new Set();
        this.hasSignal = false;
        this.frames = 0;
    }

    /** Connects `first-frame` and `unmanaged`. Returns whether the signal exists. */
    start() {
        const h = this._host;
        try {
            this.hasSignal = Boolean(h.signalExists(this._actor, 'first-frame'));
            if (this.hasSignal)
                this._frameId = h.tryConnect(this._actor, 'first-frame', () => this._onFirstFrame());
            this._unmanagedId = h.tryConnect(this._window, 'unmanaged', () => {
                this.stop();
                this._notify('unmanaged');
            });
        } catch (_e) {
            this.hasSignal = false;
        }
        return this.hasSignal;
    }

    /** Records the moment of the resize call. Call it right after `move_resize_frame`. */
    resizeIssued() {
        this.resizeAtMs = this._host.nowMs();
    }

    /** Registers `cb(kind)` for `'first-frame'` (after the resize) or `'unmanaged'`; returns an unsubscribe. */
    listen(cb) {
        this._listeners.add(cb);
        return () => this._listeners.delete(cb);
    }

    _notify(kind) {
        for (const cb of [...this._listeners]) {
            try {
                cb(kind);
            } catch (_e) {
                // A listener never throws into a signal emission.
            }
        }
    }

    _onFirstFrame() {
        try {
            if (this.stopped)
                return;
            const h = this._host;
            const now = h.nowMs();
            this.frames += 1;
            const after = this.resizeAtMs !== null;
            let tex = null;
            try {
                tex = h.textureSize(this._actor);
            } catch (_e) {
                tex = null;
            }
            const texStr = tex?.width !== undefined ? `(${tex.width}x${tex.height})` : '(na)';
            h.logTime('X11_PREMAP_FIRST_FRAME',
                `n=${this.frames} after_resize=${after} since_resize_ms=${after ? (now - this.resizeAtMs).toFixed(1) : 'na'} ` +
                `texture=${texStr} mono_ms=${now.toFixed(3)}`,
                this._window);
            if (after && this.firstFrameAtMs === null) {
                this.firstFrameAtMs = now;
                this.firstFrameTexture = tex;
                this._notify('first-frame');
            }
        } catch (_e) {
            // Observation and wait never throw into the compositor's signal emission.
        }
    }

    /** Disconnects everything. Safe to call twice. */
    stop() {
        if (this.stopped)
            return;
        this.stopped = true;
        try {
            this._host.safeDisconnect(this._actor, this._frameId);
            this._host.safeDisconnect(this._window, this._unmanagedId);
        } catch (_e) {
            // Nothing to do: the window may already be gone.
        }
        this._frameId = 0;
        this._unmanagedId = 0;
    }
}

/**
 * Waits for the first-frame that follows the pre-map resize, bounded by the cap measured from
 * the resize call (the batch starts about 21 to 41 ms later) and by the 1500 ms hard limit.
 *
 * `host` additionally provides: addTimer(ms, cb) -> id, removeTimer(id), frameSize(window) -> [w, h].
 *
 * @returns {Promise<{outcome: object, final: number[], elapsedMs: number}>} the outcome has the
 *   shape of ready_wait.js plus a `label`
 */
export function waitForPremapFirstFrame(host, window, watch, req) {
    return new Promise(resolve => {
        const startedAt = watch.resizeAtMs ?? host.nowMs();
        const hard = PREMAP_HARD_MS;
        const cap = Math.min(Number.isInteger(req.capMs) && req.capMs >= 1 ? req.capMs : PREMAP_DEFAULT_CAP_MS, hard);
        const capIsHard = cap >= hard;
        let timer = 0;
        let unlisten = () => {};
        let finished = false;

        host.logTime('READY_WAIT_START',
            `strategy=${PREMAP_STRATEGY} kind=${req.kind} target=${fmt(req.target)} cap=${cap} hard=${hard} ` +
            `resized_ms_before_wait=${(host.nowMs() - startedAt).toFixed(1)} first_frame_already=${watch.firstFrameAtMs !== null}`,
            window);

        const finish = (which, outcome) => {
            if (finished)
                return;
            finished = true;
            host.removeTimer(timer);
            unlisten();
            watch.stop();
            const elapsedMs = host.nowMs() - startedAt;
            const final = host.frameSize(window);
            const { label, commit } = PREMAP_LABELS[which];
            const done = { ...outcome, label };
            host.logTime('COMMIT_RESOLVED',
                `reason=${commit} finalFrame=${fmt(final)} target=${fmt(req.target)} safe=${fmt(req.target)}`,
                window);
            host.logTime('READY_WAIT_RESOLVED',
                `strategy=${PREMAP_STRATEGY} outcome=${done.outcome} elapsed_ms=${elapsedMs.toFixed(1)} ` +
                `waited_ms=${done.waited_ms ?? 0} reason=${label} first_frames=${watch.frames} ` +
                `finalFrame=${fmt(final)} target=${fmt(req.target)}`,
                window);
            resolve({ outcome: done, final, elapsedMs });
        };

        const reached = () => finish('firstFrame', {
            outcome: 'reached',
            size: host.frameSize(window),
            waited_ms: Math.round((watch.firstFrameAtMs ?? host.nowMs()) - startedAt),
        });

        if (watch.stopped) {
            finish('unmanaged', { outcome: 'skipped', reason: PREMAP_LABELS.unmanaged.label });
            return;
        }
        if (!watch.hasSignal) {
            finish('noSignal', { outcome: 'skipped', reason: PREMAP_LABELS.noSignal.label });
            return;
        }
        if (watch.firstFrameAtMs !== null) {
            reached();
            return;
        }
        unlisten = watch.listen(kind => {
            if (kind === 'first-frame')
                reached();
            else
                finish('unmanaged', { outcome: 'skipped', reason: PREMAP_LABELS.unmanaged.label });
        });
        const remaining = Math.max(1, Math.ceil(startedAt + cap - host.nowMs()));
        timer = host.addTimer(remaining, () => {
            timer = 0;
            finish(capIsHard ? 'hard' : 'cap', {
                outcome: 'timed-out',
                last_size: host.frameSize(window),
                waited_ms: Math.round(host.nowMs() - startedAt),
                phase: capIsHard ? 'hard' : 'cap',
            });
        });
    });
}

