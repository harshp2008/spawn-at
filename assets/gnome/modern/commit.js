/**
 * spawn-at — Mutter commit & frame presentation detection.
 *
 * Implements geometry synchronization latches:
 * - waitForCommit: listens for allocation changes or target size settlement.
 * - waitForSizeQuiet: waits until client-side resize events settle.
 * - waitForFrames: waits for compositor stage updates before uncloaking.
 * - waitForReady: the ready-wait machine (ready_wait.js) for native Wayland windows.
 */

import { ReadyWait, mergePolicy } from './ready_wait.js';

const COMMIT_QUIET_MS = 30;
const COMMIT_MIN_TIMEOUT_MS = 120;
const COMMIT_HARD_CAP_MS = 1500;

/**
 * The `reason=` text of the READY_WAIT_RESOLVED line: why the machine stopped.
 *
 * @param {object} outcome - an outcome of ready_wait.js
 * @returns {string} `at-target`, `quiet`, `cap`, `hard`, or the skip reason (`not-resizable`, `fullscreen`, `tiled`)
 */
export function readyReason(outcome) {
    // Only the experimental x11-hold-damage path sets a label (x11_hold.js); no ready-wait outcome has one.
    if (outcome.label !== undefined)
        return outcome.label;
    switch (outcome.outcome) {
        case 'reached': return 'at-target';
        case 'adjusted': return 'quiet';
        case 'timed-out': return outcome.phase;
        default: return outcome.reason;
    }
}

/**
 * The `reason=` of the COMMIT_RESOLVED line written for the same wait, so the old tag keeps its
 * vocabulary for the verify analyser (`AT_TARGET`, `SIZE_SETTLED`, `TIMEOUT_NO_CHANGE`, `HARD_CAP`;
 * `SKIPPED` is new).
 *
 * @param {object} outcome - an outcome of ready_wait.js
 * @returns {string} the legacy reason name
 */
export function commitReason(outcome) {
    switch (outcome.outcome) {
        case 'reached': return 'AT_TARGET';
        case 'adjusted': return 'SIZE_SETTLED';
        case 'timed-out': return outcome.phase === 'hard' ? 'HARD_CAP' : 'TIMEOUT_NO_CHANGE';
        default: return 'SKIPPED';
    }
}

export class CommitLatch {
    /**
     * @param {object} helper - Interface providing { logTime, addTimer, removeTimer, tryConnect, safeDisconnect }
     */
    constructor(helper) {
        this._helper = helper;
    }

    /**
     * Waits until the client's frame geometry has settled after a configure request.
     *
     * @param {object} window - Meta.Window
     * @param {object} actor - Clutter.Actor
     * @param {number} timeoutMs - Base idle timeout
     * @param {number|null} targetW - Target width
     * @param {number|null} targetH - Target height
     * @param {number|null} safeW - Safe clamped target width
     * @param {number|null} safeH - Safe clamped target height
     * @returns {Promise<string>} the reason it finished (`AT_TARGET`, `SIZE_SETTLED`, `TIMEOUT_NO_CHANGE`, `HARD_CAP`)
     */
    waitForCommit(window, actor, timeoutMs, targetW = null, targetH = null, safeW = null, safeH = null) {
        return new Promise(resolve => {
            const idleTimeout = Math.max(Number(timeoutMs || 0), COMMIT_MIN_TIMEOUT_MS);
            const hasTarget = targetW !== null && targetH !== null;

            let finished = false;
            let quietTimer = 0;
            let idleTimer = 0;
            let capTimer = 0;
            let actorSigId = 0;
            let windowSigId = 0;

            const start = window.get_frame_rect ? window.get_frame_rect() : { width: 0, height: 0 };
            let lastW = start.width;
            let lastH = start.height;

            const finish = reason => {
                if (finished)
                    return;
                finished = true;
                this._helper.removeTimer(quietTimer);
                this._helper.removeTimer(idleTimer);
                this._helper.removeTimer(capTimer);
                if (actor)
                    this._helper.safeDisconnect(actor, actorSigId);
                this._helper.safeDisconnect(window, windowSigId);

                const finalRect = window.get_frame_rect ? window.get_frame_rect() : { width: 0, height: 0 };
                this._helper.logTime('COMMIT_RESOLVED',
                    `reason=${reason} finalFrame=(${finalRect.width}x${finalRect.height}) ` +
                    `target=(${targetW}x${targetH}) safe=(${safeW}x${safeH})`,
                    window);
                resolve(reason);
            };

            const check = source => {
                if (finished)
                    return;
                const rect = window.get_frame_rect ? window.get_frame_rect() : { width: 0, height: 0 };
                this._helper.logTime('COMMIT_SAMPLE',
                    `source=${source} frame=(${rect.width}x${rect.height}) last=(${lastW}x${lastH})`,
                    window);

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
                    this._helper.removeTimer(idleTimer);
                    idleTimer = 0;
                    this._helper.removeTimer(quietTimer);
                    quietTimer = this._helper.addTimer(COMMIT_QUIET_MS, () => finish('SIZE_SETTLED'));
                } else if (source === 'allocation' && idleTimer) {
                    this._helper.removeTimer(idleTimer);
                    idleTimer = 0;
                    quietTimer = this._helper.addTimer(COMMIT_QUIET_MS, () => finish('SIZE_SETTLED'));
                }
            };

            if (actor)
                actorSigId = this._helper.tryConnect(actor, 'notify::allocation', () => check('allocation'));
            windowSigId = this._helper.tryConnect(window, 'size-changed', () => check('size-changed'));

            idleTimer = this._helper.addTimer(idleTimeout, () => finish('TIMEOUT_NO_CHANGE'));
            capTimer = this._helper.addTimer(COMMIT_HARD_CAP_MS, () => finish('HARD_CAP'));

            check('initial');
        });
    }

    /**
     * Waits for a native Wayland window to answer a size request, using the ready-wait machine
     * (docs/DESIGN_READY_WAIT.md 4.3).
     *
     * The samples are the window FRAME size: one read at the start and one per `size-changed`.
     * The actor's `notify::allocation` is deliberately not used: its first sample is the app's old
     * size, and starting the quiet timer on it is what revealed windows too early (TEST_PLAN 6.12).
     * The machine runs on the helper's `nowMs()` clock and the helper's timers; one timer holds the
     * machine's current deadline and a second one holds the hard limit, so a lost deadline timer
     * cannot hold the cloak past `hard_ms`.
     *
     * @param {object} window - Meta.Window
     * @param {object} request - `{kind, target, before, resizable, fullscreen, tiled, startedAtMs, policy}`:
     *     `target` is the `[w, h]` that was requested after the clamp, `before` the frame size before
     *     the request, `startedAtMs` the `nowMs()` at the request, `policy` optional overrides
     * @returns {Promise<{outcome: object, final: number[], elapsedMs: number}>} the machine's outcome
     *     (see ready_wait.js), the frame size at the end and the milliseconds since the request
     */
    waitForReady(window, request) {
        return new Promise(resolve => {
            const helper = this._helper;
            const policy = mergePolicy(request.policy);
            const startedAt = request.startedAtMs;
            const frame = () => {
                const rect = window.get_frame_rect ? window.get_frame_rect() : { width: 0, height: 0 };
                return [rect.width, rect.height];
            };
            const fmt = size => `(${size[0]}x${size[1]})`;

            let machine = null;
            let finished = false;
            let timer = 0;
            let hardTimer = 0;
            let sigId = 0;
            let last = [...request.before];

            const finish = outcome => {
                if (finished)
                    return;
                finished = true;
                helper.removeTimer(timer);
                helper.removeTimer(hardTimer);
                helper.safeDisconnect(window, sigId);

                const final = frame();
                const elapsedMs = Math.max(0, helper.nowMs() - startedAt);
                const reason = readyReason(outcome);
                helper.logTime('COMMIT_RESOLVED',
                    `reason=${commitReason(outcome)} finalFrame=${fmt(final)} target=${fmt(request.target)} ` +
                    `safe=${fmt(request.target)}`,
                    window);
                helper.logTime('READY_WAIT_RESOLVED',
                    `strategy=ready-wait outcome=${outcome.outcome} elapsed_ms=${elapsedMs.toFixed(1)} ` +
                    `waited_ms=${outcome.waited_ms ?? 0} reason=${reason} finalFrame=${fmt(final)} ` +
                    `target=${fmt(request.target)}`,
                    window);
                resolve({ outcome, final, elapsedMs });
            };

            const apply = action => {
                if (action.kind === 'done') {
                    finish(action.outcome);
                    return;
                }
                helper.removeTimer(timer);
                const delay = Math.max(1, Math.ceil(action.deadline_ms - helper.nowMs()));
                timer = helper.addTimer(delay, () => {
                    timer = 0;
                    apply(machine.onTick(helper.nowMs()));
                });
            };

            const sample = source => {
                if (finished)
                    return;
                const size = frame();
                helper.logTime('COMMIT_SAMPLE',
                    `source=${source} frame=${fmt(size)} last=${fmt(last)}`,
                    window);
                last = size;
                apply(machine.onSample(size, helper.nowMs()));
            };

            helper.logTime('READY_WAIT_START',
                `strategy=ready-wait kind=${request.kind ?? 'NativeWayland'} target=${fmt(request.target)} ` +
                `current=${fmt(request.before)} cap=${policy.cap_ms} quiet=${policy.quiet_ms} hard=${policy.hard_ms}`,
                window);

            const started = ReadyWait.start(policy, {
                target: request.target,
                current: request.before,
                resizable: request.resizable,
                fullscreen: request.fullscreen,
                tiled: request.tiled,
            }, startedAt);
            machine = started.machine;
            if (started.action.kind === 'done') {
                finish(started.action.outcome);
                return;
            }

            sigId = helper.tryConnect(window, 'size-changed', () => sample('size-changed'));
            const hardDelay = Math.max(1, Math.ceil(startedAt + policy.hard_ms - helper.nowMs()));
            hardTimer = helper.addTimer(hardDelay, () => {
                hardTimer = 0;
                apply(machine.onTick(helper.nowMs()));
            });

            sample('initial');
        });
    }

    /**
     * Waits until no size-changed events occur for quietMs, bounded by idleMs and ceilingMs.
     */
    waitForSizeQuiet(window, idleMs, quietMs, ceilingMs) {
        return new Promise(resolve => {
            let finished = false;
            let timer = 0;
            let ceiling = 0;
            let sigId = 0;

            const finish = () => {
                if (finished)
                    return;
                finished = true;
                this._helper.removeTimer(timer);
                this._helper.removeTimer(ceiling);
                this._helper.safeDisconnect(window, sigId);
                resolve();
            };

            const rearm = ms => {
                this._helper.removeTimer(timer);
                timer = this._helper.addTimer(ms, finish);
            };

            sigId = this._helper.tryConnect(window, 'size-changed', () => rearm(quietMs));
            ceiling = this._helper.addTimer(ceilingMs, finish);
            rearm(idleMs);
        });
    }

    /**
     * Waits for count compositor frame updates or ceilingMs timeout.
     */
    waitForFrames(stage, count, ceilingMs) {
        return new Promise(resolve => {
            let finished = false;
            let remaining = count;
            let ceiling = 0;
            let sigId = 0;

            const finish = () => {
                if (finished)
                    return;
                finished = true;
                this._helper.removeTimer(ceiling);
                if (stage)
                    this._helper.safeDisconnect(stage, sigId);
                resolve();
            };

            if (stage?.connect) {
                sigId = this._helper.tryConnect(stage, 'after-update', () => {
                    if (--remaining <= 0)
                        finish();
                });
            }

            ceiling = this._helper.addTimer(ceilingMs, finish);
            try { stage?.queue_redraw?.(); } catch (_e) {}
            if (!sigId)
                finish();
        });
    }
}
