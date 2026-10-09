/**
 * spawn-at — Mutter commit & frame presentation detection (Legacy GNOME 42-44).
 */

const COMMIT_QUIET_MS = 30;
const COMMIT_MIN_TIMEOUT_MS = 120;
const COMMIT_HARD_CAP_MS = 1500;

var CommitLatch = class CommitLatch {
    constructor(helper) {
        this._helper = helper;
    }

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
                resolve();
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
};

if (typeof module !== 'undefined' && module.exports) {
    module.exports = { CommitLatch };
}
