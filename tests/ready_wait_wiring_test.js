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
    return {
        opacity: 255,
        get_stage: () => globalThis.global.stage,
        is_destroyed: () => false,
        connect: signal => { signals.push(signal); return 1; },
        disconnect: () => {},
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
async function runBatch(windowOptions, script = () => {}, batch = WITH_SIZE) {
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

    const window = createWindow(windowOptions);
    const actor = createActor();
    script(sim, window);
    await sim.run(ext._runBatch(window, actor, validateInstructions(batch), 'tgt-1'));
    const frame = window.get_frame_rect();
    ext.disable();
    const line = tag => logs.find(entry => entry.tag === tag)?.extra;
    const outcomeSignal = emitted.find(entry => entry.name === 'ResizeOutcome');
    return {
        logs, emitted, frame, window, actor, line,
        names: emitted.map(entry => entry.name),
        outcome: outcomeSignal?.params,
        elapsed: sim.now(),
    };
}

const rect = f => [f.x, f.y, f.width, f.height];

test('native Wayland, fast app: reached at 20 ms, signal before SpawnClaimed, anchored for 800x600', async () => {
    const r = await runBatch({}, (sim, w) => sim.at(20, () => w.respond(800, 600)));
    assert.match(r.line('READY_WAIT_START'), /^strategy=ready-wait kind=NativeWayland target=\(800x600\) current=\(400x300\) cap=150 quiet=30 hard=1500$/);
    assert.match(r.line('READY_WAIT_RESOLVED'), /^strategy=ready-wait outcome=reached elapsed_ms=20\.0 waited_ms=20 reason=at-target /);
    assert.match(r.line('COMMIT_RESOLVED'), /^reason=AT_TARGET finalFrame=\(800x600\)/);
    assert.deepEqual(r.names, ['ResizeOutcome', 'SpawnClaimed']);
    assert.equal(r.outcome.format, '(ssuuus)');
    assert.deepEqual(r.outcome.value, ['tgt-1', 'reached', 800, 600, 20, 'at-target']);
    assert.deepEqual(rect(r.frame), [1104, 464, 800, 600]);
});

test('native Wayland, slow app: answering at 120 ms (inside the 150 ms cap) is still reached', async () => {
    const r = await runBatch({}, (sim, w) => sim.at(120, () => w.respond(800, 600)));
    assert.deepEqual(r.outcome.value, ['tgt-1', 'reached', 800, 600, 120, 'at-target']);
    assert.deepEqual(rect(r.frame), [1104, 464, 800, 600]);
});

test('native Wayland, refusing app: no answer by the cap is TimedOut at 150 ms, placed for the size it has', async () => {
    const r = await runBatch({});
    assert.match(r.line('READY_WAIT_RESOLVED'), /outcome=timed-out .* waited_ms=150 reason=cap /);
    assert.match(r.line('COMMIT_RESOLVED'), /^reason=TIMEOUT_NO_CHANGE finalFrame=\(400x300\)/);
    assert.deepEqual(r.outcome.value, ['tgt-1', 'timed-out', 400, 300, 150, 'cap']);
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
    assert.equal(r.elapsed < 150, true, `the wait did not run to the cap (virtual ${r.elapsed} ms)`);
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
