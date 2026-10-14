import test from 'node:test';
import assert from 'node:assert/strict';

import {
    PROBE_STRATEGY, MAX_PROBES, MIN_GAP_MS, CLIP_SIDE, X11PixelProbe, clipPoints, describePixels, formatSample, hashBytes,
} from '../assets/gnome/modern/x11_probe.js';

// The experimental, observe-only x11-pixel-probe with a fake host and a manual clock. No GJS API is used.

const MAGENTA = [0xff, 0x00, 0xff, 0xff];
const GREEN = [0x00, 0xff, 0x00, 0xff];
const px = (rgba, n = 4) => Uint8Array.from(Array.from({ length: n }, () => rgba).flat());

function makeHost({ logging = true, tex = { width: 800, height: 600 }, read = () => px(GREEN), hasSignals = true } = {}) {
    let now = 5000;
    let nextId = 1;
    const timers = new Map();
    const handlers = new Map();
    const host = {
        logs: [], timers, handlers, reads: 0, inHandler: false,
        isLoggingEnabled: () => logging,
        setLogging: v => { logging = v; },
        nowMs: () => now,
        logTime: (tag, extra) => host.logs.push({ tag, extra }),
        tryConnect: (target, signal, cb) => { const id = nextId++; handlers.set(id, { target, signal, cb }); return id; },
        safeDisconnect: (_t, id) => { handlers.delete(id); },
        addTimer: (ms, cb) => { const id = nextId++; timers.set(id, { at: now + ms, cb }); return id; },
        removeTimer: id => { timers.delete(id); },
        signalExists: () => hasSignals,
        textureSize: () => tex,
        readbackReady: () => ({ ok: true }),
        readPixels: (_a, clip) => { host.reads += 1; if (host.inHandler) throw new Error('read inside the signal handler'); return read(clip); },
        advance(t) {
            for (;;) {
                const due = [...timers.entries()].filter(([, v]) => v.at <= t).sort((a, b) => a[1].at - b[1].at)[0];
                if (!due) break;
                now = due[1].at; timers.delete(due[0]); due[1].cb();
            }
            now = t;
        },
        emit(signal, t) {
            host.advance(t);
            host.inHandler = true;
            try { for (const h of [...handlers.values()]) if (h.signal === signal) h.cb(); } finally { host.inHandler = false; }
        },
    };
    return host;
}

const WIN = { id: 'win' };
const ACTOR = { id: 'actor' };

test('clipPoints: four 2x2 clips at 1/4 and 3/4, always inside the texture', () => {
    assert.deepEqual(clipPoints(800, 600), [
        { x: 200, y: 150, width: 2, height: 2 }, { x: 600, y: 150, width: 2, height: 2 },
        { x: 200, y: 450, width: 2, height: 2 }, { x: 600, y: 450, width: 2, height: 2 },
    ]);
    for (const c of clipPoints(3, 3)) assert.ok(c.x >= 0 && c.y >= 0 && c.x + c.width <= 3 && c.y + c.height <= 3, JSON.stringify(c));
    assert.equal(CLIP_SIDE, 2);
});

test('describePixels: transparent, uniform and mixed are told apart; the hash follows the bytes', () => {
    assert.deepEqual(describePixels(px([0, 0, 0, 0])), { rgba: '#00000000', uniform: true, cls: 'transparent', hash: hashBytes(px([0, 0, 0, 0])) });
    assert.equal(describePixels(px(MAGENTA)).rgba, '#ff00ffff');
    assert.equal(describePixels(px(MAGENTA)).cls, 'uniform');
    assert.equal(describePixels(px(GREEN)).cls, 'uniform');
    assert.notEqual(describePixels(px(MAGENTA)).hash, describePixels(px(GREEN)).hash);
    const mixed = Uint8Array.from([...MAGENTA, ...GREEN, ...MAGENTA, ...MAGENTA]);
    assert.equal(describePixels(mixed).cls, 'mixed');
    assert.equal(describePixels(mixed).uniform, false);
    assert.equal(describePixels(new Uint8Array(0)).cls, 'empty');
    assert.equal(hashBytes(Uint8Array.from([1, 2, 3])), hashBytes(Uint8Array.from([1, 2, 3])));
});

test('logging off: nothing is connected, no timer, no read, no line', () => {
    const host = makeHost({ logging: false });
    const p = new X11PixelProbe(host);
    assert.equal(p.start(WIN, ACTOR), false);
    assert.equal(host.handlers.size, 0);
    assert.equal(host.timers.size, 0);
    assert.equal(host.reads, 0);
    assert.equal(host.logs.length, 0);
});

test('a damaged queues a timer and the pixels are read from the timer, never inside the handler', () => {
    const host = makeHost();
    const p = new X11PixelProbe(host);
    assert.equal(p.start(WIN, ACTOR), true);
    host.emit('damaged', 5010);
    assert.equal(host.reads, 0, 'no read yet');
    assert.equal(host.timers.size, 1);
    host.advance(5011);
    assert.equal(host.reads, 4);
    const line = host.logs.find(l => l.tag === 'X11_PROBE_SAMPLE').extra;
    assert.match(line, /^signal=damaged seq=1 mono_ms=5010\.000 texture=\(800x600\) cost_ms=0\.000 s0=#00ff00ff\/uniform\/h:[0-9a-f]{8} s1=#00ff00ff\/uniform\/h:/);
});

test('rate limit: not within the minimum gap, one at a time, and at most MAX_PROBES per window', () => {
    const host = makeHost();
    const p = new X11PixelProbe(host);
    p.start(WIN, ACTOR);
    host.emit('damaged', 6000);
    host.emit('damaged', 6001); // timer for the first still pending
    host.advance(6002);
    host.emit('first-frame', 6003); // inside MIN_GAP_MS of the probe at 6000
    assert.equal(host.logs.filter(l => l.tag === 'X11_PROBE_SAMPLE').length, 1);
    for (let i = 0; i < 40; i++) {
        host.emit('damaged', 7000 + i * (MIN_GAP_MS + 2));
        host.advance(7000 + i * (MIN_GAP_MS + 2) + 1);
    }
    assert.equal(host.logs.filter(l => l.tag === 'X11_PROBE_SAMPLE').length, MAX_PROBES);
    p.stopAll();
    assert.match(host.logs.at(-1).extra, new RegExp(`^reason=disabled probes=${MAX_PROBES} skipped=\\d+ total_cost_ms=`));
});

test('a read that throws is logged as its own error text; an unreadable texture is logged; nothing throws', () => {
    const host = makeHost({ read: () => { throw new Error('get_image returned null'); } });
    const p = new X11PixelProbe(host);
    p.start(WIN, ACTOR);
    host.emit('damaged', 5100);
    assert.doesNotThrow(() => host.advance(5101));
    assert.match(host.logs.find(l => l.tag === 'X11_PROBE_SAMPLE').extra, /s0=err:get_image_returned_null s1=err:get_image_returned_null/);

    const h2 = makeHost({ tex: { error: 'shaped>multi:boom' } });
    const p2 = new X11PixelProbe(h2);
    p2.start(WIN, ACTOR);
    h2.emit('damaged', 5100);
    h2.advance(5101);
    assert.match(h2.logs.find(l => l.tag === 'X11_PROBE_SAMPLE').extra, /texture=\(na\) .* s0=err:texture:shaped>multi:boom$/);
    assert.equal(h2.reads, 0);

    const h3 = makeHost();
    h3.logTime = () => { throw new Error('log failed'); };
    const p3 = new X11PixelProbe(h3);
    assert.doesNotThrow(() => { p3.start(WIN, ACTOR); h3.emit('damaged', 5100); h3.advance(5101); });
});

test('the reveal ends the watch 500 ms later; unmanaged and disable end it at once; no handler or timer is left', () => {
    const host = makeHost();
    const p = new X11PixelProbe(host);
    p.start(WIN, ACTOR);
    assert.ok(p.handlerCount > 0);
    host.emit('damaged', 5100);
    p.reveal(WIN);
    host.advance(5300);
    assert.equal(p.activeCount, 1);
    host.advance(5601);
    assert.equal(p.activeCount, 0);
    assert.equal(p.handlerCount, 0);
    assert.equal(host.handlers.size, 0);
    assert.equal(host.timers.size, 0);
    assert.match(host.logs.at(-1).extra, /^reason=reveal\+500ms probes=1 /);

    const h2 = makeHost();
    const p2 = new X11PixelProbe(h2);
    p2.start(WIN, ACTOR);
    [...h2.handlers.values()].find(h => h.signal === 'unmanaged').cb();
    assert.equal(h2.handlers.size, 0);
    assert.match(h2.logs.at(-1).extra, /^reason=unmanaged /);
});

test('formatSample keeps one word per field and the strategy name is the Rust spelling', () => {
    const line = formatSample({ signal: 'first-frame', seq: 2, monoMs: 12.5, texture: { width: 4, height: 4 }, costMs: 1.25, samples: [{ error: 'a b c' }, describePixels(px(MAGENTA))] });
    assert.match(line, /^signal=first-frame seq=2 mono_ms=12\.500 texture=\(4x4\) cost_ms=1\.250 s0=err:a_b_c s1=#ff00ffff\/uniform\/h:/);
    assert.equal(PROBE_STRATEGY, 'x11-pixel-probe');
});
