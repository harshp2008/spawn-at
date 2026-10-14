import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync, readdirSync } from 'node:fs';
import { register } from 'node:module';

// Tests that the extension drives the ready-wait machine the way the golden scripts say, with a
// mock window that delivers `size-changed` samples and a virtual clock for every timer. Nothing
// here talks to Mutter: it shows the wiring (which samples, which clock, which strategy), not how
// a real compositor times its configure/commit answers.

register('./gi_loader.js', import.meta.url);

globalThis.global = {
    stage: {
        add_child: () => {},
        remove_child: () => {},
        connect: () => 1,
        disconnect: () => {},
        get_key_focus: () => null,
        set_key_focus: () => {},
    },
    display: {
        connect: () => 1,
        disconnect: () => {},
        get_n_monitors: () => 1,
        get_monitor_geometry: () => ({ x: 0, y: 0, width: 1920, height: 1080 }),
        get_workspace_manager: () => ({
            get_active_workspace: () => ({
                get_work_area_for_monitor: () => ({ x: 0, y: 40, width: 1920, height: 1040 }),
            }),
        }),
        get_tab_list: () => [],
        unset_input_focus: () => {},
    },
    window_manager: {
        connect: () => 1,
        disconnect: () => {},
        get_window_actor_for_meta_window: () => null,
    },
    backend: {
        get_monitor_manager: () => null,
    },
    get_current_time: () => Date.now(),
    get_pointer: () => [500, 500],
};

const { default: SpawnAtExtension } = await import('../assets/gnome/modern/extension.js');
const { CommitLatch } = await import('../assets/gnome/modern/commit.js');
const { mergePolicy } = await import('../assets/gnome/modern/ready_wait.js');
const { validateInstructions } = await import('../assets/gnome/modern/validator.js');
const { default: Meta } = await import('gi://Meta');

// ---------------------------------------------------------------------------------------------
// Virtual clock. Timers and scripted window changes are ordered by due time; at the same time a
// scripted change goes first (the machine's rule: a sample on a deadline beats the timer).
// ---------------------------------------------------------------------------------------------

function createSim() {
    let now = 0;
    let seq = 0;
    const pending = new Map();
    const flush = () => new Promise(resolve => setImmediate(resolve));
    return {
        now: () => now,
        add(ms, callback) {
            const id = ++seq;
            pending.set(id, { id, due: now + ms, priority: 1, callback });
            return id;
        },
        remove(id) {
            return pending.delete(id);
        },
        /** A scripted change at absolute virtual time `at`. */
        at(at, callback) {
            const id = ++seq;
            pending.set(id, { id, due: at, priority: 0, callback });
        },
        /** Runs the clock until `promise` settles; fails if it can never settle. */
        async run(promise) {
            let settled = false;
            const done = promise.then(value => { settled = true; return value; },
                error => { settled = true; throw error; });
            done.catch(() => {});
            for (let step = 0; step < 100000; step++) {
                await flush();
                if (settled)
                    return done;
                let next = null;
                for (const timer of pending.values()) {
                    if (!next || timer.due < next.due ||
                        (timer.due === next.due && (timer.priority < next.priority ||
                            (timer.priority === next.priority && timer.id < next.id))))
                        next = timer;
                }
                assert.ok(next, 'nothing left to run but the promise is still pending');
                pending.delete(next.id);
                now = Math.max(now, next.due);
                next.callback();
            }
            throw new Error('virtual clock did not settle');
        },
    };
}

// A window whose size only changes when the test says so (`respond`), like a Wayland client that
// answers a configure later. `kind: 'x11'` makes move_resize_frame set the size at once, like the
// window manager does for X11 windows.
function createWindow({ kind = 'wayland', start = [400, 300], resizable = true, fullscreen = false, tiled = false,
    hasClientType = true } = {}) {
    let frame = { x: 50, y: 90, width: start[0], height: start[1] };
    const handlers = new Map();
    let nextId = 1;
    const connected = [];
    const window = {
        get_frame_rect: () => ({ ...frame }),
        get_buffer_rect: () => ({ ...frame }),
        get_id: () => 301,
        get_pid: () => 7301,
        get_wm_class: () => 'verify.ready.wiring',
        get_monitor: () => 0,
        get_work_area_current_monitor: () => ({ x: 0, y: 40, width: 1920, height: 1040 }),
        get_work_area_for_monitor: () => ({ x: 0, y: 40, width: 1920, height: 1040 }),
        allows_resize: () => resizable,
        is_fullscreen: () => fullscreen,
        get_tile_match: () => (tiled ? {} : null),
        move_resize_frame: (_u, x, y, w, h) => {
            frame = kind === 'x11' ? { x, y, width: w, height: h } : { ...frame, x, y };
        },
        move_frame: (_u, x, y) => { frame = { ...frame, x, y }; },
        connect: (signal, callback) => {
            const id = nextId++;
            handlers.set(id, { signal, callback });
            connected.push(signal);
            return id;
        },
        disconnect: id => { handlers.delete(id); },
        has_focus: () => true,
        raise: () => {},
        activate: () => {},
        unminimize: () => {},
        delete: () => {},
        /** The app's answer: sets the frame size and delivers `size-changed`. */
        respond(w, h) {
            frame = { ...frame, width: w, height: h };
            for (const handler of [...handlers.values()]) {
                if (handler.signal === 'size-changed')
                    handler.callback();
            }
        },
        connectedSignals: connected,
        handlerCount: () => handlers.size,
    };
    if (hasClientType)
        window.get_client_type = () => (kind === 'wayland' ? 0 : 1);
    return window;
}

function createActor() {
    const signals = [];
    const handlers = new Map();
    let nextId = 1;
    return {
        opacity: 255,
        width: 800,
        height: 600,
        get_stage: () => globalThis.global.stage,
        is_destroyed: () => false,
        get_texture: () => ({ get_texture: () => ({ get_width: () => 800, get_height: () => 600 }) }),
        connect: (signal, callback) => {
            signals.push(signal);
            const id = nextId++;
            handlers.set(id, { signal, callback });
            return id;
        },
        disconnect: id => { handlers.delete(id); },
        /** Delivers `signal` to the handlers connected for it, like the compositor. */
        emit: signal => {
            for (const handler of [...handlers.values()])
                if (handler.signal === signal) handler.callback();
        },
        handlerCount: () => handlers.size,
        remove_all_transitions: () => {},
        signals,
    };
}

// ---------------------------------------------------------------------------------------------
// 1. The golden scripts of tests/ready_wait_vectors/ through CommitLatch.waitForReady.
// ---------------------------------------------------------------------------------------------

const VECTORS = new URL('./ready_wait_vectors/', import.meta.url);
const scriptFiles = readdirSync(new URL('scripts/', VECTORS)).filter(name => name.endsWith('.json')).sort();

for (const file of scriptFiles) {
    const script = JSON.parse(readFileSync(new URL(`scripts/${file}`, VECTORS), 'utf8'));
    test(`wiring drives the machine like golden script ${file}`, async () => {
        const sim = createSim();
        const request = script.request;
        const window = createWindow({
            start: request.current,
            resizable: request.resizable !== false,
            fullscreen: Boolean(request.fullscreen),
            tiled: Boolean(request.tiled),
        });
        const logs = [];
        const latch = new CommitLatch({
            logTime: (tag, extra) => logs.push(`${tag} ${extra}`),
            addTimer: (ms, callback) => sim.add(ms, callback),
            removeTimer: id => sim.remove(id),
            tryConnect: (target, signal, callback) => target.connect(signal, callback),
            safeDisconnect: (target, id) => { if (id) target.disconnect(id); },
            nowMs: () => sim.now(),
        });
        for (const event of script.events)
            sim.at(event.at, () => window.respond(event.size[0], event.size[1]));

        const result = await sim.run(latch.waitForReady(window, {
            kind: 'NativeWayland',
            target: request.target,
            before: request.current,
            resizable: request.resizable !== false,
            fullscreen: Boolean(request.fullscreen),
            tiled: Boolean(request.tiled),
            startedAtMs: 0,
            policy: mergePolicy(script.policy),
        }));
        assert.deepEqual(result.outcome, script.expect);
        assert.ok(logs.some(line => line.startsWith('READY_WAIT_START ')));
        assert.ok(logs.some(line => line.startsWith('READY_WAIT_RESOLVED ')));
        assert.ok(logs.some(line => line.startsWith('COMMIT_RESOLVED ')));
        // no signal handler survives the wait; a skipped or already-satisfied request never connects one
        assert.equal(window.handlerCount(), 0);
        const immediate = script.expect.outcome === 'skipped' ||
            (request.target[0] === request.current[0] && request.target[1] === request.current[1]);
        if (immediate)
            assert.deepEqual(window.connectedSignals, []);
        else
            assert.deepEqual(window.connectedSignals, ['size-changed']);
    });
}

// ---------------------------------------------------------------------------------------------
// 2. The extension: strategy choice, samples, D-Bus signal order, anchoring for the outcome size.
// ---------------------------------------------------------------------------------------------

// Bottom-right, 16 px margins, workarea 0,40 1920x1040: x = 1904 - w, y = 1064 - h.
const BOTTOM_RIGHT = {
    intended_w: 0, intended_h: 0,
    screen_anchor_x: 1904, screen_anchor_y: 1064,
    pivot_u: 1.0, pivot_v: 1.0, offset_x: 0, offset_y: 0,
    area: 'workarea',
    margin_top: 16, margin_bottom: 16, margin_left: 16, margin_right: 16,
    clamp: true,
};
const WITH_SIZE = JSON.stringify(['Cloak', { SetSize: { w: 800, h: 600 } }, { WaitForCommit: { timeout_ms: 300 } },
    { SetPositionAnchored: BOTTOM_RIGHT }, 'Uncloak']);
const WITHOUT_SIZE = JSON.stringify(['Cloak', { WaitForCommit: { timeout_ms: 300 } },
    { SetPositionAnchored: BOTTOM_RIGHT }, 'Uncloak']);

/** Runs one batch on a fresh extension with the virtual clock; `script(sim, window)` schedules the app's answers. */
function makeRig() {
    const sim = createSim();
    const ext = new SpawnAtExtension({ path: '/dummy' });
    ext.enable();
    ext._addTimer = (ms, callback) => sim.add(ms, callback);
    ext._removeTimer = id => sim.remove(id);
    ext.nowMs = () => sim.now();
    const emitted = [];
    ext._dbusManager = { emitSignal: (name, params) => emitted.push({ name, params }), export() {}, unexport() {} };
    const logs = [];
    const orig = ext.logTime.bind(ext);
    ext.logTime = (tag, extra, win) => { logs.push({ tag, extra }); orig(tag, extra, win); };
    return { sim, ext, emitted, logs };
}

/** Runs one batch on `rig` (a new window each time); the result holds only what this batch logged and emitted. */
async function runOn(rig, windowOptions, script = () => {}, batch = WITH_SIZE) {
    const { sim, ext } = rig;
    const logFrom = rig.logs.length;
    const emitFrom = rig.emitted.length;
    const startedAt = sim.now();

    const window = createWindow(windowOptions);
    const actor = createActor();
    script(sim, window);
    await sim.run(ext._runBatch(window, actor, validateInstructions(batch), 'tgt-1'));
    const frame = window.get_frame_rect();
    const logs = rig.logs.slice(logFrom);
    const emitted = rig.emitted.slice(emitFrom);
    const line = tag => logs.find(entry => entry.tag === tag)?.extra;
    const outcomeSignal = emitted.find(entry => entry.name === 'ResizeOutcome');
    return {
        logs, emitted, frame, window, actor, line,
        names: emitted.map(entry => entry.name),
        outcome: outcomeSignal?.params,
        elapsed: sim.now() - startedAt,
    };
}

async function runBatch(windowOptions, script = () => {}, batch = WITH_SIZE) {
    const rig = makeRig();
    const result = await runOn(rig, windowOptions, script, batch);
    rig.ext.disable();
    return result;
}

const rect = f => [f.x, f.y, f.width, f.height];

test('native Wayland, fast app: reached at 20 ms, signal before SpawnClaimed, anchored for 800x600', async () => {
    const r = await runBatch({}, (sim, w) => sim.at(20, () => w.respond(800, 600)));
    assert.match(r.line('READY_WAIT_START'), /^strategy=ready-wait kind=NativeWayland target=\(800x600\) current=\(400x300\) cap=250 quiet=30 hard=1500$/);
    assert.match(r.line('READY_WAIT_RESOLVED'), /^strategy=ready-wait outcome=reached elapsed_ms=20\.0 waited_ms=20 reason=at-target /);
    assert.match(r.line('COMMIT_RESOLVED'), /^reason=AT_TARGET finalFrame=\(800x600\)/);
    assert.deepEqual(r.names, ['ResizeOutcome', 'SpawnClaimed']);
    assert.equal(r.outcome.format, '(ssuuus)');
    assert.deepEqual(r.outcome.value, ['tgt-1', 'reached', 800, 600, 20, 'at-target']);
    assert.deepEqual(rect(r.frame), [1104, 464, 800, 600]);
});

test('native Wayland, slow app: answering at 220 ms (inside the 250 ms cap) is still reached', async () => {
    const r = await runBatch({}, (sim, w) => sim.at(220, () => w.respond(800, 600)));
    assert.deepEqual(r.outcome.value, ['tgt-1', 'reached', 800, 600, 220, 'at-target']);
    assert.deepEqual(rect(r.frame), [1104, 464, 800, 600]);
});

test('native Wayland, refusing app: no answer by the cap is TimedOut at 250 ms, placed for the size it has', async () => {
    const r = await runBatch({});
    assert.match(r.line('READY_WAIT_RESOLVED'), /outcome=timed-out .* waited_ms=250 reason=cap /);
    assert.match(r.line('COMMIT_RESOLVED'), /^reason=TIMEOUT_NO_CHANGE finalFrame=\(400x300\)/);
    assert.deepEqual(r.outcome.value, ['tgt-1', 'timed-out', 400, 300, 250, 'cap']);
    assert.deepEqual(rect(r.frame), [1504, 764, 400, 300]);
});

test('native Wayland, adjusted app: 494x292 at 40 ms, quiet for 30 ms, placed for 494x292 (not for the old size)', async () => {
    const r = await runBatch({}, (sim, w) => sim.at(40, () => w.respond(494, 292)));
    assert.deepEqual(r.outcome.value, ['tgt-1', 'adjusted', 494, 292, 70, 'quiet']);
    assert.match(r.line('COMMIT_RESOLVED'), /^reason=SIZE_SETTLED finalFrame=\(494x292\)/);
    assert.deepEqual(rect(r.frame), [1410, 772, 494, 292]);
    // the reveal saw no size change after positioning: nothing needed a late correction
    assert.equal(r.logs.some(entry => entry.tag === 'LATE_RESIZE_CORRECTED'), false);
});

test('native Wayland, restless app that never settles: the hard limit ends the wait at 1500 ms and the batch still reveals', async () => {
    const r = await runBatch({}, (sim, w) => {
        for (let t = 10, i = 0; t < 1700; t += 20, i++)
            sim.at(t, () => w.respond(500 + (i % 2), 300));
    });
    assert.match(r.line('READY_WAIT_RESOLVED'), /outcome=timed-out .* waited_ms=1500 reason=hard /);
    assert.match(r.line('COMMIT_RESOLVED'), /^reason=HARD_CAP /);
    assert.equal(r.outcome.value[1], 'timed-out');
    assert.equal(r.outcome.value[5], 'hard');
    assert.deepEqual(r.names, ['ResizeOutcome', 'SpawnClaimed']);
});

test('native Wayland, window that does not allow resize: Skipped at once, the frame is left as it is', async () => {
    const r = await runBatch({ resizable: false });
    assert.deepEqual(r.outcome.value, ['tgt-1', 'skipped', 400, 300, 0, 'not-resizable']);
    assert.equal(r.elapsed < 250, true, `the wait did not run to the cap (virtual ${r.elapsed} ms)`);
    assert.match(r.line('COMMIT_RESOLVED'), /^reason=SKIPPED /);
});

test('native Wayland, fullscreen and tiled windows are Skipped', async () => {
    const fullscreen = await runBatch({ fullscreen: true });
    assert.deepEqual(fullscreen.outcome.value, ['tgt-1', 'skipped', 400, 300, 0, 'fullscreen']);
    const tiled = await runBatch({ tiled: true });
    assert.deepEqual(tiled.outcome.value, ['tgt-1', 'skipped', 400, 300, 0, 'tiled']);
});

test('native Wayland: the actor allocation is not a sample (it started the 30 ms timer too early)', async () => {
    const r = await runBatch({}, (sim, w) => sim.at(20, () => w.respond(800, 600)));
    assert.equal(r.actor.signals.includes('notify::allocation'), false);
});

test('native Wayland: feature detection by presence, a window without allows_resize is treated as resizable', async () => {
    const sim = createSim();
    const ext = new SpawnAtExtension({ path: '/dummy' });
    ext.enable();
    ext._addTimer = (ms, callback) => sim.add(ms, callback);
    ext._removeTimer = id => sim.remove(id);
    ext.nowMs = () => sim.now();
    const window = createWindow();
    delete window.allows_resize;
    sim.at(20, () => window.respond(800, 600));
    const logs = [];
    const orig = ext.logTime.bind(ext);
    ext.logTime = (tag, extra, win) => { logs.push({ tag, extra }); orig(tag, extra, win); };
    await sim.run(ext._runBatch(window, createActor(), validateInstructions(WITH_SIZE), 'tgt-1'));
    ext.disable();
    assert.match(logs.find(e => e.tag === 'READY_WAIT_RESOLVED').extra, /outcome=reached/);
});

test('X11 window: keeps the original rule (strategy=legacy), no ResizeOutcome, allocation still watched', async () => {
    const r = await runBatch({ kind: 'x11' });
    assert.match(r.line('READY_WAIT_START'), /^strategy=legacy kind=X11 why=not-native-wayland timeoutMs=300$/);
    assert.match(r.line('READY_WAIT_RESOLVED'), /^strategy=legacy outcome=none elapsed_ms=\d+\.\d reason=AT_TARGET$/);
    assert.match(r.line('COMMIT_RESOLVED'), /^reason=AT_TARGET finalFrame=\(800x600\)/);
    assert.equal(r.names.includes('ResizeOutcome'), false);
    assert.deepEqual(r.names, ['SpawnClaimed']);
    assert.equal(r.actor.signals.includes('notify::allocation'), true);
    assert.deepEqual(rect(r.frame), [1104, 464, 800, 600]);
});

test('XWayland window (X11 client under a Wayland compositor): legacy strategy, labelled XWayland', async () => {
    const original = Meta.is_wayland_compositor;
    Meta.is_wayland_compositor = () => true;
    try {
        const r = await runBatch({ kind: 'x11' });
        assert.match(r.line('READY_WAIT_START'), /^strategy=legacy kind=XWayland why=not-native-wayland /);
        assert.deepEqual(r.names, ['SpawnClaimed']);
    } finally {
        Meta.is_wayland_compositor = original;
    }
});

test('native Wayland window without a size request keeps the original rule', async () => {
    const r = await runBatch({}, () => {}, WITHOUT_SIZE);
    assert.match(r.line('READY_WAIT_START'), /^strategy=legacy kind=NativeWayland why=no-size-request /);
    assert.deepEqual(r.names, ['SpawnClaimed']);
});

test('a window that cannot say what it is (no get_client_type) keeps the original rule', async () => {
    const r = await runBatch({ hasClientType: false, kind: 'x11' });
    assert.match(r.line('READY_WAIT_START'), /^strategy=legacy kind=unknown /);
    assert.deepEqual(r.names, ['SpawnClaimed']);
});

// ---------------------------------------------------------------------------------------------
// 3. Per-spawn override (`strategy`, `cap_ms` in the WaitForCommit arguments): honoured for the
//    spawn that carries it, ignored when invalid, and no trace of it when absent.
// ---------------------------------------------------------------------------------------------

const withWait = extra => JSON.stringify(['Cloak', { SetSize: { w: 800, h: 600 } },
    { WaitForCommit: { timeout_ms: 300, ...extra } }, { SetPositionAnchored: BOTTOM_RIGHT }, 'Uncloak']);
const logTrace = r => r.logs.map(entry => `${entry.tag} ${entry.extra}`);

test('override: an instruction without the fields logs no READY_WAIT_OVERRIDE and starts with cap=250', async () => {
    const r = await runBatch({}, (sim, w) => sim.at(20, () => w.respond(800, 600)));
    assert.equal(r.line('READY_WAIT_OVERRIDE'), undefined);
    assert.match(r.line('READY_WAIT_START'), / cap=250 quiet=30 hard=1500$/);
});

test('override: strategy "auto" is the same as no override, line for line', async () => {
    const script = (sim, w) => sim.at(20, () => w.respond(800, 600));
    const plain = await runBatch({}, script);
    const auto = await runBatch({}, script, withWait({ strategy: 'auto' }));
    assert.deepEqual(logTrace(auto), logTrace(plain));
    assert.deepEqual(auto.names, plain.names);
    assert.deepEqual(auto.outcome.value, plain.outcome.value);
    assert.deepEqual(rect(auto.frame), rect(plain.frame));
});

test('override legacy: a native Wayland window takes the original rule and says why', async () => {
    const r = await runBatch({}, (sim, w) => sim.at(20, () => w.respond(800, 600)), withWait({ strategy: 'legacy' }));
    assert.equal(r.line('READY_WAIT_OVERRIDE'), 'strategy=legacy cap_ms=none unused=none ignored=none');
    assert.equal(r.line('READY_WAIT_START'), 'strategy=legacy kind=NativeWayland why=override-legacy timeoutMs=300');
    assert.match(r.line('READY_WAIT_RESOLVED'), /^strategy=legacy outcome=none .* reason=AT_TARGET$/);
    assert.deepEqual(r.names, ['SpawnClaimed']);
    assert.deepEqual(rect(r.frame), [1104, 464, 800, 600]);
});

test('override legacy: a refusing app ends on the old idle timeout (300 ms), not the 250 ms cap', async () => {
    const legacy = await runBatch({}, () => {}, withWait({ strategy: 'legacy' }));
    assert.match(legacy.line('COMMIT_RESOLVED'), /^reason=TIMEOUT_NO_CHANGE /);
    assert.ok(legacy.elapsed >= 300 && legacy.elapsed < 1500, `legacy ended at ${legacy.elapsed} ms`);
    // The batch also spends the frame wait (50 ms) after the resize wait, so the 250 ms is read
    // from the outcome (`waited_ms`), not from the length of the batch.
    const auto = await runBatch({});
    assert.equal(auto.outcome.value[4], 250);
});

test('override ready-wait: the machine as under auto, with the override recorded', async () => {
    const r = await runBatch({}, (sim, w) => sim.at(20, () => w.respond(800, 600)), withWait({ strategy: 'ready-wait' }));
    assert.equal(r.line('READY_WAIT_OVERRIDE'), 'strategy=ready-wait cap_ms=none unused=none ignored=none');
    assert.match(r.line('READY_WAIT_START'), /^strategy=ready-wait kind=NativeWayland .* cap=250 /);
    assert.deepEqual(r.outcome.value, ['tgt-1', 'reached', 800, 600, 20, 'at-target']);
});

test('override ready-wait on an X11 window: still the original rule, reported as unused', async () => {
    const r = await runBatch({ kind: 'x11' }, () => {}, withWait({ strategy: 'ready-wait' }));
    assert.equal(r.line('READY_WAIT_OVERRIDE'), 'strategy=ready-wait cap_ms=none unused=strategy(ready-wait:not-native-wayland) ignored=none');
    assert.match(r.line('READY_WAIT_START'), /^strategy=legacy kind=X11 why=not-native-wayland /);
    assert.deepEqual(r.names, ['SpawnClaimed']);
});

test('override cap_ms: a smaller cap turns a 100 ms answer into TimedOut at the new cap', async () => {
    const answer = (sim, w) => sim.at(100, () => w.respond(800, 600));
    const r = await runBatch({}, answer, withWait({ cap_ms: 60 }));
    assert.equal(r.line('READY_WAIT_OVERRIDE'), 'strategy=none cap_ms=60 unused=none ignored=none');
    assert.match(r.line('READY_WAIT_START'), / cap=60 quiet=30 hard=1500$/);
    assert.deepEqual(r.outcome.value, ['tgt-1', 'timed-out', 400, 300, 60, 'cap']);
    // The same app under the default cap is reached.
    const plain = await runBatch({}, answer);
    assert.deepEqual(plain.outcome.value, ['tgt-1', 'reached', 800, 600, 100, 'at-target']);
});

test('override cap_ms: a larger cap waits for a 300 ms answer that the default cap would miss', async () => {
    const answer = (sim, w) => sim.at(300, () => w.respond(800, 600));
    const r = await runBatch({}, answer, withWait({ cap_ms: 400 }));
    assert.match(r.line('READY_WAIT_START'), / cap=400 quiet=30 hard=1500$/);
    assert.deepEqual(r.outcome.value, ['tgt-1', 'reached', 800, 600, 300, 'at-target']);
    const plain = await runBatch({}, answer);
    assert.deepEqual(plain.outcome.value, ['tgt-1', 'timed-out', 400, 300, 250, 'cap']);
});

test('override cap_ms together with legacy: the cap has nothing to apply to and is reported as unused', async () => {
    const r = await runBatch({}, () => {}, withWait({ strategy: 'legacy', cap_ms: 60 }));
    assert.equal(r.line('READY_WAIT_OVERRIDE'), 'strategy=legacy cap_ms=60 unused=cap_ms(legacy-has-no-cap) ignored=none');
    assert.match(r.line('READY_WAIT_START'), /^strategy=legacy kind=NativeWayland why=override-legacy /);
    assert.ok(r.elapsed >= 300, `legacy ended at ${r.elapsed} ms, not at the unused cap`);
});

test('override: invalid values are ignored, named in the log, and the spawn runs with the defaults', async () => {
    const answer = (sim, w) => sim.at(20, () => w.respond(800, 600));
    const r = await runBatch({}, answer, withWait({ strategy: 'fast', cap_ms: 0 }));
    assert.equal(r.line('READY_WAIT_OVERRIDE'), 'strategy=none cap_ms=none unused=none ignored=strategy="fast";cap_ms=0');
    assert.match(r.line('READY_WAIT_START'), /^strategy=ready-wait kind=NativeWayland .* cap=250 quiet=30 hard=1500$/);
    assert.deepEqual(r.outcome.value, ['tgt-1', 'reached', 800, 600, 20, 'at-target']);
    // A valid field next to an invalid one is still used.
    const mixed = await runBatch({}, answer, withWait({ strategy: 'legacy', cap_ms: 9999 }));
    assert.match(mixed.line('READY_WAIT_START'), /why=override-legacy/);
    assert.match(mixed.line('READY_WAIT_OVERRIDE'), /ignored=cap_ms=9999$/);
});

test('override: scoped to the spawn that carries it, the next spawn on the same extension is back to the defaults', async () => {
    const rig = makeRig();
    const first = await runOn(rig, {}, () => {}, withWait({ strategy: 'legacy', cap_ms: 60 }));
    assert.match(first.line('READY_WAIT_START'), /why=override-legacy/);
    const second = await runOn(rig, {});
    assert.equal(second.line('READY_WAIT_OVERRIDE'), undefined);
    assert.match(second.line('READY_WAIT_START'), /^strategy=ready-wait kind=NativeWayland .* cap=250 quiet=30 hard=1500$/);
    assert.equal(second.outcome.value[4], 250);
    rig.ext.disable();
});

// ---------------------------------------------------------------------------------------------
// 4. Observe-only X11 logging (x11_observe.js) inside the extension: no work with logging off, no
//    change of outcome with it on, and no handler left after the window's watch ends.
// ---------------------------------------------------------------------------------------------

const { default: GObject } = await import('gi://GObject');

/** Runs an x11-kind batch on a fresh rig and lets the 500 ms tail of the watch pass. */
async function runX11(rig, { logging, windowOptions = { kind: 'x11' }, script = () => {} } = {}) {
    if (logging)
        rig.ext.SetLogging(true);
    const { sim, ext } = rig;
    const logFrom = rig.logs.length;
    const window = createWindow(windowOptions);
    const actor = createActor();
    script(sim, window, actor);
    await sim.run(ext._runBatch(window, actor, validateInstructions(WITH_SIZE), 'tgt-1'));
    const afterBatch = {
        activeCount: ext._x11Observer.activeCount,
        actorHandlers: actor.handlerCount(),
        windowHandlers: window.handlerCount(),
    };
    await sim.run(new Promise(resolve => ext._addTimer(600, resolve)));
    return {
        window, actor, afterBatch,
        logs: rig.logs.slice(logFrom),
        frame: window.get_frame_rect(),
        after: { activeCount: ext._x11Observer.activeCount, actorHandlers: actor.handlerCount(), windowHandlers: window.handlerCount() },
    };
}

test('x11 observation: logging off adds no work (no handler on the actor, no timer, no X11_OBS line)', async () => {
    const rig = makeRig();
    const timersBefore = rig.ext._timerIds.size;
    const r = await runX11(rig, { logging: false });
    assert.equal(r.actor.signals.some(s => s === 'damaged' || s === 'first-frame'), false, r.actor.signals.join());
    assert.equal(r.logs.some(l => l.tag.startsWith('X11_OBS')), false);
    assert.equal(r.afterBatch.activeCount, 0);
    assert.equal(rig.ext._x11Observer.handlerCount, 0);
    assert.equal(rig.ext._timerIds.size, timersBefore + 0, 'no timer of the observer is pending');
    rig.ext.disable();
});

test('x11 observation: logging on writes the joinable lines, connects damaged and first-frame, and leaves no handler after the tail', async () => {
    const saved = GObject.signal_lookup;
    GObject.signal_lookup = () => 7; // the signals exist (decided at runtime, not by version)
    try {
        const rig = makeRig();
        const r = await runX11(rig, {
            logging: true,
            script: (sim, _w, actor) => {
                sim.at(5, () => actor.emit('damaged'));
                sim.at(6, () => actor.emit('first-frame'));
            },
        });
        const tags = r.logs.map(l => l.tag).filter(t => t.startsWith('X11_OBS'));
        assert.equal(tags[0], 'X11_OBS_START');
        assert.deepEqual(tags.filter(t => t === 'X11_OBS_SIGNAL').length, 2);
        const marks = r.logs.filter(l => l.tag === 'X11_OBS_MARK').map(l => l.extra.split(' ')[0]);
        assert.deepEqual(marks, ['what=set_size', 'what=commit_resolved', 'what=reveal']);
        assert.equal(tags.at(-1), 'X11_OBS_END');
        assert.match(r.logs.find(l => l.tag === 'X11_OBS_END').extra, /reason=reveal\+500ms .*damaged=1 first_frame=1$/);
        assert.match(r.logs.find(l => l.tag === 'X11_OBS_START').extra, /^kind=X11 wayland=false client_type=1 id=301 /);
        assert.match(r.logs.find(l => l.tag === 'X11_OBS_SIGNAL').extra, /texture=\(800x600\) texture_via=shaped>multi actor=\(800x600\) frame=\(/);
        // Connected during the batch, gone after the tail.
        assert.equal(r.afterBatch.activeCount, 1);
        assert.equal(r.afterBatch.actorHandlers > 0, true);
        assert.equal(r.after.activeCount, 0);
        assert.equal(r.after.actorHandlers, 0);
        assert.equal(rig.ext._x11Observer.handlerCount, 0);
        rig.ext.disable();
    } finally {
        GObject.signal_lookup = saved;
    }
});

test('x11 observation: the batch gives the same frame, the same wait lines and the same signals with logging on as off', async () => {
    const off = await runX11(makeRig(), { logging: false });
    const on = await runX11(makeRig(), { logging: true });
    assert.deepEqual(on.frame, off.frame);
    const strip = logs => logs.filter(l => !l.tag.startsWith('X11_OBS') && l.tag !== 'SET_LOGGING')
        .map(l => `${l.tag} ${l.extra}`.replace(/elapsed_ms=[0-9.]+/g, ''));
    // `_logActorTrace` lines (CLOAK, AFTER_SET_SIZE, UNCLOAK, ...) were already skipped when logging
    // is off; every line the off run logs is in the on run, in the same order and with the same text.
    const onLines = strip(on.logs);
    let at = 0;
    for (const line of strip(off.logs)) {
        at = onLines.indexOf(line, at);
        assert.notEqual(at, -1, `missing from the logging-on run: ${line}`);
        at += 1;
    }
    assert.equal(on.logs.filter(l => l.tag.startsWith('X11_OBS')).length > 0, true);
});

test('x11 observation: damaged missing is logged once as a note and does not stop the batch', async () => {
    const saved = GObject.signal_lookup;
    GObject.signal_lookup = (name) => (name === 'damaged' ? 0 : 7);
    try {
        const rig = makeRig();
        const a = await runX11(rig, { logging: true });
        const b = await runX11(rig, { logging: true });
        const notes = [...a.logs, ...b.logs].filter(l => l.tag === 'X11_OBS_NOTE');
        assert.equal(notes.length, 1);
        assert.equal(notes[0].extra, 'damaged unavailable');
        assert.deepEqual(a.actor.signals.filter(s => s === 'damaged'), []);
        rig.ext.disable();
    } finally {
        GObject.signal_lookup = saved;
    }
});

test('x11 observation: a native Wayland window is never watched, even with logging on', async () => {
    const rig = makeRig();
    const r = await runX11(rig, { logging: true, windowOptions: { kind: 'wayland' } });
    assert.equal(r.logs.some(l => l.tag.startsWith('X11_OBS')), false);
    rig.ext.disable();
});

test('x11 observation: disabling the extension ends a running watch and disconnects it', async () => {
    const saved = GObject.signal_lookup;
    GObject.signal_lookup = () => 7;
    try {
        const rig = makeRig();
        rig.ext.SetLogging(true);
        const window = createWindow({ kind: 'x11' });
        const actor = createActor();
        await rig.sim.run(rig.ext._runBatch(window, actor, validateInstructions(WITH_SIZE), 'tgt-1'));
        assert.equal(rig.ext._x11Observer.activeCount, 1);
        rig.ext.disable();
        assert.equal(rig.ext._x11Observer.activeCount, 0);
        assert.equal(actor.handlerCount(), 0);
    } finally {
        GObject.signal_lookup = saved;
    }
});
