import test from 'node:test';
import assert from 'node:assert/strict';

import {
    PREMAP_STRATEGY, PREMAP_DEFAULT_CAP_MS, PREMAP_HARD_MS, PREMAP_LABELS, PremapWatch, findPremapPlan,
    waitForPremapFirstFrame,
} from '../assets/gnome/modern/x11_premap.js';
import { readyReason } from '../assets/gnome/modern/commit.js';

// The experimental x11-premap wait with a fake host and a manual clock. No GJS API is used.

function makeHost({ hasSignal = true, texture = { width: 800, height: 600 } } = {}) {
    let now = 1000;
    let nextId = 1;
    const timers = new Map();
    const handlers = new Map();
    const host = {
        logs: [], timers, handlers,
        nowMs: () => now,
        logTime: (tag, extra) => host.logs.push({ tag, extra }),
        tryConnect: (target, signal, cb) => { const id = nextId++; handlers.set(id, { target, signal, cb }); return id; },
        safeDisconnect: (_t, id) => { handlers.delete(id); },
        addTimer: (ms, cb) => { const id = nextId++; timers.set(id, { at: now + ms, cb }); return id; },
        removeTimer: id => { timers.delete(id); },
        signalExists: () => hasSignal,
        frameSize: () => [800, 600],
        textureSize: () => texture,
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
const PLAN = { w: 800, h: 600 };
const REQ = { kind: 'XWayland', target: [800, 600] };

test('findPremapPlan: needs a size request and strategy x11-premap on the wait; reads the cap', () => {
    const ops = (strategy, extra = {}) => [
        { name: 'Cloak', args: {} }, { name: 'SetSize', args: { w: 800, h: 600 } },
        { name: 'WaitForCommit', args: { timeout_ms: 300, strategy, ...extra } },
    ];
    assert.deepEqual(findPremapPlan(ops(PREMAP_STRATEGY)), { w: 800, h: 600, capMs: undefined });
    assert.deepEqual(findPremapPlan(ops(PREMAP_STRATEGY, { cap_ms: 90 })), { w: 800, h: 600, capMs: 90 });
    assert.equal(findPremapPlan(ops('legacy')), null);
    assert.equal(findPremapPlan(ops(undefined)), null);
    assert.equal(findPremapPlan(ops('x11-hold-damage')), null);
    assert.equal(findPremapPlan(ops(PREMAP_STRATEGY).filter(o => o.name !== 'SetSize')), null);
    assert.equal(findPremapPlan([{ name: 'SetSize', args: { w: 0, h: 5 } }, { name: 'WaitForCommit', args: { strategy: PREMAP_STRATEGY } }]), null);
    assert.equal(findPremapPlan(null), null, 'garbage never throws');
});

test('a first-frame before the resize call is logged but does not count; the one after it does', () => {
    const host = makeHost();
    const w = new PremapWatch(host, WIN, ACTOR, PLAN);
    assert.equal(w.start(), true);
    host.emit('first-frame', 1001);
    assert.equal(w.firstFrameAtMs, null);
    w.resizeIssued();
    host.emit('first-frame', 1012);
    assert.equal(w.firstFrameAtMs, 1012);
    const lines = host.logs.filter(l => l.tag === 'X11_PREMAP_FIRST_FRAME').map(l => l.extra);
    assert.match(lines[0], /^n=1 after_resize=false since_resize_ms=na texture=\(800x600\) /);
    assert.match(lines[1], /^n=2 after_resize=true since_resize_ms=\d+\.\d texture=\(800x600\) /);
});

test('the first-frame after the resize ends the wait as reached with its own label', async () => {
    const host = makeHost();
    const w = new PremapWatch(host, WIN, ACTOR, PLAN);
    w.start();
    w.resizeIssued();
    host.advance(1021); // the batch starts 21 ms later
    const p = waitForPremapFirstFrame(host, WIN, w, REQ);
    host.emit('first-frame', 1030);
    const r = await p;
    assert.equal(r.outcome.outcome, 'reached');
    assert.equal(r.outcome.label, 'x11-premap-first-frame');
    assert.equal(r.outcome.waited_ms, 30);
    assert.equal(host.handlers.size, 0, 'handlers disconnected');
    assert.equal(host.timers.size, 0, 'cap timer removed');
    assert.match(host.logs.find(l => l.tag === 'READY_WAIT_RESOLVED').extra,
        /^strategy=x11-premap outcome=reached elapsed_ms=30\.0 waited_ms=30 reason=x11-premap-first-frame first_frames=1 /);
    assert.match(host.logs.find(l => l.tag === 'COMMIT_RESOLVED').extra, /^reason=X11_PREMAP_FIRST_FRAME /);
    assert.match(host.logs.find(l => l.tag === 'READY_WAIT_START').extra,
        /^strategy=x11-premap kind=XWayland target=\(800x600\) cap=250 hard=1500 resized_ms_before_wait=21\.0 first_frame_already=false$/);
});

test('a first-frame that already came ends the wait at once', async () => {
    const host = makeHost();
    const w = new PremapWatch(host, WIN, ACTOR, PLAN);
    w.start();
    w.resizeIssued();
    host.emit('first-frame', 1008);
    host.advance(1021);
    const r = await waitForPremapFirstFrame(host, WIN, w, REQ);
    assert.equal(r.outcome.outcome, 'reached');
    assert.equal(r.outcome.waited_ms, 8);
    assert.match(host.logs.find(l => l.tag === 'READY_WAIT_START').extra, / first_frame_already=true$/);
});

test('no first-frame: the cap counts from the resize call and ends as timed-out/cap; cap_ms replaces 250', async () => {
    const host = makeHost();
    const w = new PremapWatch(host, WIN, ACTOR, PLAN);
    w.start();
    w.resizeIssued();
    host.advance(1021);
    const p = waitForPremapFirstFrame(host, WIN, w, REQ);
    host.advance(1249);
    assert.equal(host.timers.size, 1);
    host.advance(1250);
    const r = await p;
    assert.equal(r.outcome.outcome, 'timed-out');
    assert.equal(r.outcome.phase, 'cap');
    assert.equal(r.outcome.label, 'x11-premap-cap');
    assert.equal(r.outcome.waited_ms, 250);
    assert.equal(PREMAP_DEFAULT_CAP_MS, 250);

    const h2 = makeHost();
    const w2 = new PremapWatch(h2, WIN, ACTOR, PLAN);
    w2.start();
    w2.resizeIssued();
    const p2 = waitForPremapFirstFrame(h2, WIN, w2, { ...REQ, capMs: 90 });
    h2.advance(1090);
    assert.equal((await p2).outcome.waited_ms, 90);
});

test('a cap at or above 1500 ends as timed-out/hard and larger caps are clamped', async () => {
    for (const capMs of [1500, 9999]) {
        const host = makeHost();
        const w = new PremapWatch(host, WIN, ACTOR, PLAN);
        w.start();
        w.resizeIssued();
        const p = waitForPremapFirstFrame(host, WIN, w, { ...REQ, capMs });
        host.advance(2500);
        const r = await p;
        assert.equal(r.outcome.phase, 'hard');
        assert.equal(r.outcome.label, 'x11-premap-hard');
        assert.equal(r.outcome.waited_ms, 1500);
    }
    assert.equal(PREMAP_HARD_MS, 1500);
});

test('window gone, or no first-frame signal: skipped, no timer, no handler left', async () => {
    const host = makeHost();
    const w = new PremapWatch(host, WIN, ACTOR, PLAN);
    w.start();
    w.resizeIssued();
    const p = waitForPremapFirstFrame(host, WIN, w, REQ);
    [...host.handlers.values()].find(h => h.signal === 'unmanaged').cb();
    const r = await p;
    assert.equal(r.outcome.outcome, 'skipped');
    assert.equal(r.outcome.label, 'x11-premap-unmanaged');
    assert.equal(host.handlers.size, 0);
    assert.equal(host.timers.size, 0);

    const none = makeHost({ hasSignal: false });
    const w2 = new PremapWatch(none, WIN, ACTOR, PLAN);
    assert.equal(w2.start(), false);
    w2.resizeIssued();
    const q = await waitForPremapFirstFrame(none, WIN, w2, REQ);
    assert.equal(q.outcome.label, 'x11-premap-no-first-frame-signal');
    assert.equal(none.timers.size, 0);
});

test('a host that throws never throws into the first-frame emission, and the watch can still stop', () => {
    const host = makeHost();
    host.textureSize = () => { throw new Error('no texture'); };
    const w = new PremapWatch(host, WIN, ACTOR, PLAN);
    w.start();
    w.resizeIssued();
    assert.doesNotThrow(() => host.emit('first-frame', 1005));
    assert.equal(w.firstFrameAtMs, 1005);
    assert.match(host.logs.find(l => l.tag === 'X11_PREMAP_FIRST_FRAME').extra, /texture=\(na\)/);
    w.stop();
    w.stop();
    assert.equal(host.handlers.size, 0);
    host.logTime = () => { throw new Error('log failed'); };
    const w2 = new PremapWatch(host, WIN, ACTOR, PLAN);
    w2.start();
    assert.doesNotThrow(() => host.emit('first-frame', 1100));
});

test('labels reach the ResizeOutcome reason; spelling matches the Rust value', () => {
    assert.equal(readyReason({ outcome: 'reached', label: 'x11-premap-first-frame' }), 'x11-premap-first-frame');
    assert.equal(readyReason({ outcome: 'timed-out', phase: 'cap', label: 'x11-premap-cap' }), 'x11-premap-cap');
    for (const v of Object.values(PREMAP_LABELS)) assert.ok(v.label.startsWith('x11-premap'));
    assert.equal(PREMAP_STRATEGY, 'x11-premap');
});
