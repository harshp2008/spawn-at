/**
 * spawn-at — EXPERIMENT: hold the cloak of an XWayland/X11 window for a second `damaged`.
 *
 * Off by default and never chosen by the extension itself: it runs only when the instruction
 * carries `strategy: "x11-hold-damage"` (the hidden `--wait-strategy` flag) and the window is an
 * XWayland or X11 window with a size request (docs/DESIGN_READY_WAIT.md, section 13). A native
 * Wayland window, or any spawn without that value, never reaches this module.
 *
 * Why: on XWayland the first `damaged` after the size request is the shell's own (TEST_PLAN 6.23),
 * and the client's paint was never seen before the reveal while the window was cloaked, but nobody
 * had tried to keep it cloaked and wait. The rule: ignore the first `damaged`, end the wait on the
 * second, or at the cap. HYPOTHESIS until run in a Shell: the second one arrives while cloaked.
 *
 * No `gi://` import: everything the shell provides comes in through `host`, so the logic is tested
 * in plain Node (tests/x11_hold_test.js).
 */

/** The value of `strategy` that selects this experiment (spelling of `wait_override.rs`). */
export const HOLD_STRATEGY = 'x11-hold-damage';

/** Cap used when the instruction has no `cap_ms`; the same 250 ms as the ready-wait. */
export const HOLD_DEFAULT_CAP_MS = 250;

/** The wait never outlasts this, as for the ready-wait. */
export const HOLD_HARD_MS = 1500;

/** The `damaged` that ends the wait: the first is ignored. */
export const HOLD_REVEAL_ON = 2;

/** True for the window kinds this experiment applies to (`_windowKind` in extension.js). */
export function isHoldTarget(kind) {
    return kind === 'XWayland' || kind === 'X11';
}

/**
 * The distinct labels of the three ways the wait ends (the `reason=` of READY_WAIT_RESOLVED and of
 * the ResizeOutcome signal), and the `COMMIT_RESOLVED` reason that goes with each.
 */
export const HOLD_LABELS = {
    second: { label: 'x11-second-damaged', commit: 'X11_HOLD_SECOND_DAMAGED' },
    cap: { label: 'x11-cap', commit: 'X11_HOLD_CAP' },
    hard: { label: 'x11-hard', commit: 'X11_HOLD_HARD' },
    unmanaged: { label: 'x11-unmanaged', commit: 'SKIPPED' },
    noSignal: { label: 'x11-no-damaged-signal', commit: 'SKIPPED' },
};

const fmt = s => (s && Number.isFinite(s[0]) && Number.isFinite(s[1])) ? `(${s[0]}x${s[1]})` : '(na)';

/**
 * Keeps the (already cloaked) window as it is and waits.
 *
 * `host` provides: nowMs(), logTime(tag, extra, window), tryConnect(target, signal, cb) -> id,
 * safeDisconnect(target, id), addTimer(ms, cb) -> id, removeTimer(id), signalExists(actor, name),
 * frameSize(window) -> [w, h].
 *
 * @param {object} host
 * @param {object} window - the Meta.Window
 * @param {object} actor - its window actor
 * @param {{kind: string, target: number[], capMs?: number}} req - `target` is the size asked for
 * @returns {Promise<{outcome: object, final: number[], elapsedMs: number}>} `outcome` is shaped
 *   like an outcome of ready_wait.js (`reached`, `timed-out` or `skipped`) plus a `label`
 */
export function holdForSecondDamage(host, window, actor, req) {
    return new Promise(resolve => {
        const startedAt = host.nowMs();
        const hard = HOLD_HARD_MS;
        const cap = Math.min(Number.isInteger(req.capMs) && req.capMs >= 1 ? req.capMs : HOLD_DEFAULT_CAP_MS, hard);
        const capIsHard = cap >= hard;
        let seen = 0;
        let damagedId = 0;
        let unmanagedId = 0;
        let timer = 0;
        let finished = false;

        host.logTime('READY_WAIT_START',
            `strategy=${HOLD_STRATEGY} kind=${req.kind} target=${fmt(req.target)} cap=${cap} hard=${hard} ` +
            `reveal_on_damaged=${HOLD_REVEAL_ON}`,
            window);

        const finish = (which, outcome) => {
            if (finished)
                return;
            finished = true;
            host.removeTimer(timer);
            host.safeDisconnect(actor, damagedId);
            host.safeDisconnect(window, unmanagedId);
            const elapsedMs = host.nowMs() - startedAt;
            const final = host.frameSize(window);
            const { label, commit } = HOLD_LABELS[which];
            const done = { ...outcome, label };
            host.logTime('COMMIT_RESOLVED',
                `reason=${commit} finalFrame=${fmt(final)} target=${fmt(req.target)} safe=${fmt(req.target)}`,
                window);
            host.logTime('READY_WAIT_RESOLVED',
                `strategy=${HOLD_STRATEGY} outcome=${done.outcome} elapsed_ms=${elapsedMs.toFixed(1)} ` +
                `waited_ms=${done.waited_ms ?? 0} reason=${label} damaged_seen=${seen} ` +
                `finalFrame=${fmt(final)} target=${fmt(req.target)}`,
                window);
            resolve({ outcome: done, final, elapsedMs });
        };

        if (!host.signalExists(actor, 'damaged')) {
            finish('noSignal', { outcome: 'skipped', reason: HOLD_LABELS.noSignal.label });
            return;
        }

        damagedId = host.tryConnect(actor, 'damaged', () => {
            if (finished)
                return;
            seen += 1;
            host.logTime('X11_HOLD_DAMAGED',
                `n=${seen} ignored=${seen < HOLD_REVEAL_ON} since_start_ms=${(host.nowMs() - startedAt).toFixed(1)}`,
                window);
            if (seen >= HOLD_REVEAL_ON) {
                finish('second', {
                    outcome: 'reached',
                    size: host.frameSize(window),
                    waited_ms: Math.round(host.nowMs() - startedAt),
                });
            }
        });
        unmanagedId = host.tryConnect(window, 'unmanaged',
            () => finish('unmanaged', { outcome: 'skipped', reason: HOLD_LABELS.unmanaged.label }));
        timer = host.addTimer(cap, () => {
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
