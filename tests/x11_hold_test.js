import test from 'node:test';
import assert from 'node:assert/strict';

import {
    HOLD_STRATEGY, HOLD_DEFAULT_CAP_MS, HOLD_HARD_MS, HOLD_LABELS, isHoldTarget, holdForSecondDamage,
} from '../assets/gnome/modern/x11_hold.js';
import { readyReason } from '../assets/gnome/modern/commit.js';

// The experimental x11-hold-damage wait with a fake host and a manual clock. No GJS API is used.

function makeHost({ hasDamaged = true } = {}) {
    let now = 0;
    let nextId = 1;
    const timers = new Map();
    const handlers = new Map();
    const host = {
        logs: [],
        timers,
        handlers,
        nowMs: () => now,
        logTime: (tag, extra) => host.logs.push({ tag, extra }),
        tryConnect: (target, signal, cb) => { const id = nextId++; handlers.set(id, { target, signal, cb }); return id; },
        safeDisconnect: (_t, id) => { handlers.delete(id); },
        addTimer: (ms, cb) => { const id = nextId++; timers.set(id, { at: now + ms, cb }); return id; },
        removeTimer: id => { timers.delete(id); },
        signalExists: () => hasDamaged,
        frameSize: () => [800, 600],
        /** Moves the clock to `t`, running due timers on the way. */
        advance(t) {
            for (;;) {
                const due = [...timers.entries()].filter(([, v]) => v.at <= t).sort((a, b) => a[1].at - b[1].at)[0];
                if (!due) break;
                now = due[1].at;
                timers.delete(due[0]);
                due[1].cb();
            }
            now = t;
        },
        emit(signal, t) { host.advance(t); for (const h of [...handlers.values()]) if (h.signal === signal) h.cb(); },
    };
    return host;
}

const WIN = { id: 'win' };
const ACTOR = { id: 'actor' };
const REQ = { kind: 'XWayland', target: [800, 600] };

test('the first damaged is ignored and the second ends the wait as reached, with its own label', async () => {
    const host = makeHost();
    const p = holdForSecondDamage(host, WIN, ACTOR, REQ);
    host.emit('damaged', 5);
    assert.equal(host.timers.size, 1, 'still waiting after the first damaged');
    host.emit('damaged', 40);
    const r = await p;
    assert.equal(r.outcome.outcome, 'reached');
    assert.equal(r.outcome.label, 'x11-second-damaged');
    assert.equal(r.outcome.waited_ms, 40);
    assert.deepEqual(r.final, [800, 600]);
    assert.equal(host.handlers.size, 0, 'both handlers disconnected');
    assert.equal(host.timers.size, 0, 'the cap timer is gone');
    const done = host.logs.find(l => l.tag === 'READY_WAIT_RESOLVED').extra;
    assert.match(done, /^strategy=x11-hold-damage outcome=reached elapsed_ms=40\.0 waited_ms=40 reason=x11-second-damaged damaged_seen=2 /);
    assert.match(host.logs.find(l => l.tag === 'COMMIT_RESOLVED').extra, /^reason=X11_HOLD_SECOND_DAMAGED finalFrame=\(800x600\)/);
    assert.match(host.logs.find(l => l.tag === 'READY_WAIT_START').extra, /^strategy=x11-hold-damage kind=XWayland target=\(800x600\) cap=250 hard=1500 /);
});

test('with only the first damaged the wait ends at the cap as timed-out/cap', async () => {
    const host = makeHost();
    const p = holdForSecondDamage(host, WIN, ACTOR, REQ);
    host.emit('damaged', 5);
    host.advance(249);
    assert.equal(host.timers.size, 1);
    host.advance(250);
    const r = await p;
    assert.equal(r.outcome.outcome, 'timed-out');
    assert.equal(r.outcome.phase, 'cap');
    assert.equal(r.outcome.label, 'x11-cap');
    assert.equal(r.outcome.waited_ms, 250);
    assert.match(host.logs.find(l => l.tag === 'READY_WAIT_RESOLVED').extra, /outcome=timed-out .* reason=x11-cap damaged_seen=1 /);
    assert.match(host.logs.find(l => l.tag === 'COMMIT_RESOLVED').extra, /^reason=X11_HOLD_CAP /);
    assert.equal(host.handlers.size, 0);
});

test('no damaged at all: cap, and a caller cap replaces the 250 ms default', async () => {
    const host = makeHost();
    const p = holdForSecondDamage(host, WIN, ACTOR, { ...REQ, capMs: 90 });
    host.advance(90);
    const r = await p;
    assert.equal(r.outcome.waited_ms, 90);
    assert.match(host.logs.find(l => l.tag === 'READY_WAIT_RESOLVED').extra, /damaged_seen=0 /);
    assert.equal(HOLD_DEFAULT_CAP_MS, 250);
});

test('a cap at or above the hard limit ends as timed-out/hard at 1500 ms and is clamped', async () => {
    for (const capMs of [1500, 9999]) {
        const host = makeHost();
        const p = holdForSecondDamage(host, WIN, ACTOR, { ...REQ, capMs });
        host.advance(1499);
        assert.equal(host.timers.size, 1);
        host.advance(1500);
        const r = await p;
        assert.equal(r.outcome.phase, 'hard');
        assert.equal(r.outcome.label, 'x11-hard');
        assert.match(host.logs.find(l => l.tag === 'READY_WAIT_START').extra, / cap=1500 hard=1500 /);
    }
    assert.equal(HOLD_HARD_MS, 1500);
});

test('the window going away ends the wait as skipped; a missing damaged signal never waits', async () => {
    const host = makeHost();
    const p = holdForSecondDamage(host, WIN, ACTOR, REQ);
    host.advance(10);
    [...host.handlers.values()].find(h => h.signal === 'unmanaged').cb();
    const r = await p;
    assert.equal(r.outcome.outcome, 'skipped');
    assert.equal(r.outcome.label, 'x11-unmanaged');
    assert.equal(host.handlers.size, 0);

    const none = makeHost({ hasDamaged: false });
    const q = await holdForSecondDamage(none, WIN, ACTOR, REQ);
    assert.equal(q.outcome.outcome, 'skipped');
    assert.equal(q.outcome.label, 'x11-no-damaged-signal');
    assert.equal(none.timers.size, 0, 'no timer was started');
    assert.equal(none.handlers.size, 0);
});

test('a damaged after the wait has ended changes nothing', async () => {
    const host = makeHost();
    const p = holdForSecondDamage(host, WIN, ACTOR, REQ);
    host.emit('damaged', 5);
    host.emit('damaged', 6);
    const r = await p;
    const lines = host.logs.length;
    assert.equal(host.handlers.size, 0);
    assert.equal(r.outcome.label, 'x11-second-damaged');
    assert.equal(host.logs.length, lines);
});

test('labels reach the ResizeOutcome reason only for this path: other outcomes keep their reason', () => {
    assert.equal(readyReason({ outcome: 'reached', label: 'x11-second-damaged' }), 'x11-second-damaged');
    assert.equal(readyReason({ outcome: 'timed-out', phase: 'cap', label: 'x11-cap' }), 'x11-cap');
    assert.equal(readyReason({ outcome: 'reached' }), 'at-target');
    assert.equal(readyReason({ outcome: 'adjusted' }), 'quiet');
    assert.equal(readyReason({ outcome: 'timed-out', phase: 'hard' }), 'hard');
    assert.equal(readyReason({ outcome: 'skipped', reason: 'tiled' }), 'tiled');
    for (const v of Object.values(HOLD_LABELS)) assert.ok(v.label.startsWith('x11-'));
});

test('only XWayland and X11 windows are targets; the strategy name is the Rust spelling', () => {
    assert.equal(isHoldTarget('XWayland'), true);
    assert.equal(isHoldTarget('X11'), true);
    assert.equal(isHoldTarget('NativeWayland'), false);
    assert.equal(isHoldTarget('unknown'), false);
    assert.equal(HOLD_STRATEGY, 'x11-hold-damage');
});
