/**
 * spawn-at — ready-wait state machine (JS port of crates/spawn-at-core/src/ready_wait.rs).
 *
 * Decides when a window has answered a size request, from size samples and time alone. It reads
 * no clock and starts no timer: the caller passes the current time in milliseconds with every call
 * and schedules a timer for the deadline the machine returns. It imports nothing, so it runs the
 * same under GJS (gnome-shell) and Node (tests/ready_wait_test.js).
 *
 * NOT WIRED: extension.js does not import this file and the installer does not ship it yet.
 *
 * Rules (docs/DESIGN_READY_WAIT.md section 4.3, identical to the Rust machine):
 * - Non-resizable, fullscreen or tiled is `skipped`; checked first.
 * - Target equal to the current size is `reached` at once.
 * - Otherwise wait for a sample equal to the target: `reached`.
 * - The first sample that differs from the previous size is the window's answer. If it is not
 *   the target, the quiet rule takes over: `adjusted` once `quiet_ms` pass without another change;
 *   every further change restarts that clock.
 * - No answer within `cap_ms` of the start: `timed-out`, phase `cap`.
 * - `hard_ms` after the start ends the wait in any phase: `timed-out`, phase `hard`.
 * A sample equal to the previous size is not an answer and is ignored.
 *
 * Ordering: a sample at the same millisecond as a deadline is applied before the timer fires; a
 * sample after a deadline is handled as if the timer had fired on time and the outcome reports the
 * deadline; when the quiet deadline equals the hard deadline the hard limit wins.
 *
 * Sizes are `[w, h]` arrays. Actions are `{kind: 'wait', deadline_ms}` or `{kind: 'done', outcome}`;
 * outcomes are `{outcome: 'reached'|'adjusted'|'timed-out'|'skipped', ...}` exactly as in the golden
 * scripts under tests/ready_wait_vectors/.
 */

/** The single JS copy of the timing constants; tests/ready_wait_vectors/policy.json keeps it equal to the Rust default. */
export const DEFAULT_POLICY = Object.freeze({ cap_ms: 250, quiet_ms: 30, hard_ms: 1500 });

/**
 * @param {object} [partial] - policy fields to override
 * @returns {{cap_ms: number, quiet_ms: number, hard_ms: number}} a full policy; missing fields take the defaults
 */
export function mergePolicy(partial = {}) {
    return { ...DEFAULT_POLICY, ...partial };
}

/** @returns {boolean} whether two `[w, h]` sizes are equal */
function sameSize(a, b) {
    return a[0] === b[0] && a[1] === b[1];
}

/** The ready-wait state machine for one size request. */
export class ReadyWait {
    /**
     * Starts a wait at time `now`.
     *
     * @param {object} policy - `{cap_ms, quiet_ms, hard_ms}`; use `mergePolicy` to fill defaults
     * @param {object} request - `{target, current, resizable, fullscreen, tiled}`; `resizable` defaults to true
     * @param {number} now - time of the request, ms on the caller's clock
     * @returns {{machine: ReadyWait, action: object}} the machine and its first action: `done` for a
     *     skipped or already-satisfied request, else `wait` until the cap
     */
    static start(policy, request, now) {
        const machine = new ReadyWait(policy, request, now);
        const resizable = request.resizable === undefined ? true : request.resizable;
        let reason = null;
        if (!resizable)
            reason = 'not-resizable';
        else if (request.fullscreen)
            reason = 'fullscreen';
        else if (request.tiled)
            reason = 'tiled';
        if (reason !== null)
            return { machine, action: machine._finish({ outcome: 'skipped', reason }) };
        if (sameSize(request.current, request.target)) {
            return {
                machine,
                action: machine._finish({ outcome: 'reached', size: [...request.target], waited_ms: 0 }),
            };
        }
        return { machine, action: machine._waiting() };
    }

    constructor(policy, request, now) {
        this._policy = policy;
        this._target = [...request.target];
        this._started = now;
        this._last = [...request.current];
        // 'exact' = no answer yet; 'quiet' = answered, `_since` is the time of the latest change; 'done'.
        this._phase = 'exact';
        this._since = 0;
        this._outcome = null;
    }

    /**
     * A size sample at time `now`.
     *
     * @param {number[]} size - `[w, h]`
     * @param {number} now - ms on the caller's clock
     * @returns {object} the next action
     */
    onSample(size, now) {
        if (this._phase === 'done')
            return { kind: 'done', outcome: this._outcome };
        if (now > this._deadline())
            return this._expire();
        if (sameSize(size, this._target)) {
            return this._finish({
                outcome: 'reached',
                size: [...size],
                waited_ms: now - this._started,
            });
        }
        if (!sameSize(size, this._last)) {
            this._last = [...size];
            this._phase = 'quiet';
            this._since = now;
        }
        return this._waiting();
    }

    /**
     * The time `now` was reached with no new sample (the caller's timer for the last deadline).
     *
     * @param {number} now - ms on the caller's clock
     * @returns {object} the next action
     */
    onTick(now) {
        if (this._phase === 'done')
            return { kind: 'done', outcome: this._outcome };
        if (now >= this._deadline())
            return this._expire();
        return this._waiting();
    }

    /** The earlier of the hard limit and the phase's own limit (cap before an answer, quiet window after one). */
    _deadline() {
        const hard = this._started + this._policy.hard_ms;
        const own = this._phase === 'quiet'
            ? this._since + this._policy.quiet_ms
            : this._started + this._policy.cap_ms;
        return Math.min(own, hard);
    }

    _waiting() {
        return { kind: 'wait', deadline_ms: this._deadline() };
    }

    /** Ends the wait because the current deadline passed; the outcome reports the deadline. */
    _expire() {
        const deadline = this._deadline();
        const waited = deadline - this._started;
        const hardFired = deadline >= this._started + this._policy.hard_ms;
        let outcome;
        if (hardFired)
            outcome = { outcome: 'timed-out', last_size: [...this._last], waited_ms: waited, phase: 'hard' };
        else if (this._phase === 'exact')
            outcome = { outcome: 'timed-out', last_size: [...this._last], waited_ms: waited, phase: 'cap' };
        else
            outcome = { outcome: 'adjusted', size: [...this._last], waited_ms: waited };
        return this._finish(outcome);
    }

    _finish(outcome) {
        this._phase = 'done';
        this._outcome = outcome;
        return { kind: 'done', outcome };
    }
}
