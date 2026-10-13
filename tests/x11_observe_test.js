import test from 'node:test';
import assert from 'node:assert/strict';

import {
    X11Observer, isX11Kind, formatStart, formatMark, formatSignal, formatEnd,
    DAMAGED_UNAVAILABLE, OBSERVE_TAIL_MS,
} from '../assets/gnome/modern/x11_observe.js';

// The observe-only X11 logging: formatting and lifecycle with a fake host. No GJS API is used,
// so this is the whole of the testable part; what a real Shell emits is not shown here.

/** A host that records every call and has a manual clock and timer list. */
function makeHost({ logging = true, signals = { damaged: true, 'first-frame': true } } = {}) {
    let now = 1000;
    let nextId = 1;
    const calls = [];
    const logs = [];
    const connected = new Map(); // id -> { target, signal, cb }
    const timers = new Map();
    const host = {
        calls, logs, connected, timers,
        logging,
        set now(v) { now = v; },
        isLoggingEnabled: () => { calls.push('isLoggingEnabled'); return host.logging; },
        logTime: (tag, extra, window) => { calls.push('logTime'); logs.push({ tag, extra, window }); },
        nowMs: () => now,
        tryConnect: (target, signal, cb) => {
            calls.push('tryConnect');
            const id = nextId++;
            connected.set(id, { target, signal, cb });
            return id;
        },
        safeDisconnect: (target, id) => { calls.push('safeDisconnect'); if (id) connected.delete(id); },
        addTimer: (ms, cb) => { calls.push('addTimer'); const id = nextId++; timers.set(id, { ms, cb }); return id; },
        removeTimer: id => { calls.push('removeTimer'); timers.delete(id); },
        signalExists: (_a, name) => { calls.push('signalExists'); return signals[name] ?? false; },
        windowId: () => 301,
        clientType: () => 1,
        isWayland: () => true,
        frameRect: () => ({ x: 10, y: 20, width: 800, height: 600 }),
        bufferRect: () => ({ x: 9, y: 19, width: 802, height: 602 }),
        textureSize: () => ({ width: 400, height: 300 }),
        actorSize: () => ({ width: 800, height: 600 }),
        fire(signal) {
            for (const c of [...connected.values()])
                if (c.signal === signal) c.cb();
        },
        fireTimers() {
            for (const [id, t] of [...timers]) { timers.delete(id); t.cb(); }
        },
    };
    return host;
}

const WIN = { name: 'window' };
const ACTOR = { name: 'actor' };
const tags = host => host.logs.map(l => l.tag);

test('only XWayland and X11 windows are watched', () => {
    assert.equal(isX11Kind('XWayland'), true);
    assert.equal(isX11Kind('X11'), true);
    for (const k of ['NativeWayland', 'unknown', undefined, null])
        assert.equal(isX11Kind(k), false);
});

test('log lines have a fixed shape with the ids, the sizes and the clock', () => {
    assert.equal(formatStart({
        kind: 'XWayland', wayland: true, clientType: 1, id: 301, monoMs: 1234.5,
        signals: { damaged: true, 'first-frame': false },
    }), 'kind=XWayland wayland=true client_type=1 id=301 mono_ms=1234.500 damaged=available first_frame=unavailable');
    assert.equal(formatMark('set_size', 301, 1000), 'what=set_size id=301 mono_ms=1000.000');
    assert.equal(formatSignal({
        signal: 'damaged', id: 301, monoMs: 1010.25, seq: 2,
        texture: { width: 400, height: 300 }, actor: { width: 800, height: 600 },
        frame: { x: 10, y: 20, width: 800, height: 600 }, buffer: { x: 9, y: 19, width: 802, height: 602 },
    }), 'signal=damaged id=301 mono_ms=1010.250 seq=2 texture=(400x300) actor=(800x600) frame=(10,20,800x600) buffer=(9,19,802x602)');
    assert.equal(formatEnd(301, 'unmanaged', 1500, { damaged: 3, 'first-frame': 1 }),
        'id=301 reason=unmanaged mono_ms=1500.000 damaged=3 first_frame=1');
    // A size that cannot be read is 'na', never an exception or NaN.
    assert.match(formatSignal({ signal: 'first-frame', id: 1, monoMs: 1, seq: 1, texture: null, actor: null, frame: null, buffer: undefined }),
        /texture=\(na\) actor=\(na\) frame=\(na\) buffer=\(na\)$/);
});

test('logging off: nothing is connected, no timer, no log line, no feature check', () => {
    const host = makeHost({ logging: false });
    const obs = new X11Observer(host);
    assert.equal(obs.start(WIN, ACTOR, 'XWayland'), false);
    obs.mark(WIN, 'set_size');
    obs.reveal(WIN);
    obs.stopAll();
    assert.equal(obs.activeCount, 0);
    assert.equal(obs.handlerCount, 0);
    // The only thing the default-off path does is ask whether logging is on.
    assert.deepEqual([...new Set(host.calls)], ['isLoggingEnabled']);
    assert.equal(host.connected.size, 0);
    assert.equal(host.timers.size, 0);
    assert.deepEqual(host.logs, []);
});

test('a native Wayland window is not watched even with logging on', () => {
    const host = makeHost();
    const obs = new X11Observer(host);
    assert.equal(obs.start(WIN, ACTOR, 'NativeWayland'), false);
    assert.equal(host.calls.includes('tryConnect'), false);
    assert.deepEqual(host.logs, []);
});

test('a watch logs start, marks, each signal with sizes, and ends 500 ms after the reveal with no handler left', () => {
    const host = makeHost();
    const obs = new X11Observer(host);
    assert.equal(obs.start(WIN, ACTOR, 'XWayland'), true);
    assert.equal(obs.activeCount, 1);
    // damaged, first-frame and the window's unmanaged
    assert.deepEqual([...host.connected.values()].map(c => c.signal), ['damaged', 'first-frame', 'unmanaged']);
    assert.equal(obs.handlerCount, 2);

    host.now = 1005; obs.mark(WIN, 'set_size');
    host.now = 1012; host.fire('damaged');
    host.now = 1013; host.fire('first-frame');
    host.now = 1040; obs.mark(WIN, 'commit_resolved');
    host.now = 1041; host.fire('damaged');
    host.now = 1050; obs.reveal(WIN);
    assert.equal(host.timers.size, 1);
    assert.equal([...host.timers.values()][0].ms, OBSERVE_TAIL_MS);
    host.now = 1100; host.fire('damaged'); // still inside the tail

    assert.deepEqual(tags(host), [
        'X11_OBS_START', 'X11_OBS_MARK', 'X11_OBS_SIGNAL', 'X11_OBS_SIGNAL', 'X11_OBS_MARK',
        'X11_OBS_SIGNAL', 'X11_OBS_MARK', 'X11_OBS_SIGNAL',
    ]);
    assert.match(host.logs[0].extra, /^kind=XWayland wayland=true client_type=1 id=301 mono_ms=1000\.000 damaged=available first_frame=available$/);
    assert.match(host.logs[2].extra, /^signal=damaged id=301 mono_ms=1012\.000 seq=1 texture=\(400x300\) actor=\(800x600\) frame=\(10,20,800x600\) buffer=\(9,19,802x602\)$/);
    assert.match(host.logs[3].extra, /^signal=first-frame .* seq=2 /);
    assert.equal(host.logs.every(l => l.window === WIN), true);

    host.now = 1550; host.fireTimers();
    assert.equal(host.logs.at(-1).tag, 'X11_OBS_END');
    assert.match(host.logs.at(-1).extra, /^id=301 reason=reveal\+500ms mono_ms=1550\.000 damaged=3 first_frame=1$/);
    // Nothing stays connected, and a late emission logs nothing.
    assert.equal(obs.activeCount, 0);
    assert.equal(obs.handlerCount, 0);
    assert.equal(host.connected.size, 0);
    assert.equal(host.timers.size, 0);
    const n = host.logs.length;
    host.fire('damaged');
    assert.equal(host.logs.length, n);
});

test('the window going away ends the watch at once and disconnects everything', () => {
    const host = makeHost();
    const obs = new X11Observer(host);
    obs.start(WIN, ACTOR, 'X11');
    host.fire('unmanaged');
    assert.equal(host.logs.at(-1).tag, 'X11_OBS_END');
    assert.match(host.logs.at(-1).extra, /reason=unmanaged/);
    assert.equal(host.connected.size, 0);
    assert.equal(obs.activeCount, 0);
});

test('disabling the extension ends every watch', () => {
    const host = makeHost();
    const obs = new X11Observer(host);
    const w2 = { name: 'w2' };
    obs.start(WIN, ACTOR, 'XWayland');
    obs.start(w2, ACTOR, 'X11');
    obs.reveal(WIN);
    obs.stopAll('disabled');
    assert.equal(obs.activeCount, 0);
    assert.equal(host.connected.size, 0);
    assert.equal(host.timers.size, 0);
    assert.equal(host.logs.filter(l => l.tag === 'X11_OBS_END').length, 2);
});

test('damaged missing: noted once, first-frame is still watched; the check is by signal lookup, not version', () => {
    const host = makeHost({ signals: { damaged: false, 'first-frame': true } });
    const obs = new X11Observer(host);
    obs.start(WIN, ACTOR, 'XWayland');
    obs.start({ name: 'w2' }, ACTOR, 'XWayland');
    const notes = host.logs.filter(l => l.tag === 'X11_OBS_NOTE');
    assert.equal(notes.length, 1);
    assert.equal(notes[0].extra, DAMAGED_UNAVAILABLE);
    assert.match(host.logs[0].extra, /damaged=unavailable first_frame=available/);
    assert.deepEqual([...host.connected.values()].filter(c => c.signal !== 'unmanaged').map(c => c.signal),
        ['first-frame', 'first-frame']);
});

test('logging switched off mid-watch: emissions are silent, the end still disconnects', () => {
    const host = makeHost();
    const obs = new X11Observer(host);
    obs.start(WIN, ACTOR, 'XWayland');
    host.logging = false;
    const n = host.logs.length;
    host.fire('damaged');
    obs.mark(WIN, 'commit_resolved');
    host.fire('unmanaged');
    assert.equal(host.logs.length, n);
    assert.equal(host.connected.size, 0);
});

test('a host that throws never throws into the emission, and the watch can still end', () => {
    const host = makeHost();
    const obs = new X11Observer(host);
    obs.start(WIN, ACTOR, 'XWayland');
    host.textureSize = () => { throw new Error('no texture'); };
    assert.doesNotThrow(() => host.fire('damaged'));
    obs.stopAll();
    assert.equal(host.connected.size, 0);
});

test('a second start for the same window is ignored', () => {
    const host = makeHost();
    const obs = new X11Observer(host);
    assert.equal(obs.start(WIN, ACTOR, 'XWayland'), true);
    assert.equal(obs.start(WIN, ACTOR, 'XWayland'), false);
    assert.equal(host.logs.filter(l => l.tag === 'X11_OBS_START').length, 1);
});
