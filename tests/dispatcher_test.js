import test from 'node:test';
import assert from 'node:assert/strict';
import { register } from 'node:module';

// Register the custom gi:// and resource:/// module loader stub
register('./gi_loader.js', import.meta.url);

// Mock global GNOME Shell environment before importing extension.js
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
const { validateInstructions } = await import('../assets/gnome/modern/validator.js');

function createMockWindowAndActor() {
    let frameRect = { x: 100, y: 100, width: 640, height: 480 };
    const window = {
        get_frame_rect: () => ({ ...frameRect }),
        get_buffer_rect: () => ({ ...frameRect }),
        get_id: () => 101,
        get_pid: () => 5432,
        get_wm_class: () => 'org.gnome.TextEditor',
        get_client_type: () => 0,
        get_monitor: () => 0,
        get_work_area_current_monitor: () => ({ x: 0, y: 40, width: 1920, height: 1040 }),
        get_work_area_for_monitor: () => ({ x: 0, y: 40, width: 1920, height: 1040 }),
        move_resize_frame: (_userOp, x, y, w, h) => {
            frameRect = { x, y, width: w, height: h };
        },
        move_frame: (_userOp, x, y) => {
            frameRect.x = x;
            frameRect.y = y;
        },
        connect: () => 1,
        disconnect: () => {},
        has_focus: () => true,
        raise: () => {},
        activate: () => {},
        unminimize: () => { window.minimized = false; },
        delete: () => {
            window._deleted = true;
        },
    };

    const actor = {
        opacity: 255,
        get_stage: () => globalThis.global.stage,
        is_destroyed: () => false,
        connect: () => 1,
        disconnect: () => {},
        remove_all_transitions: () => {},
    };

    return { window, actor };
}

test('dispatcher: executes Rust golden entry instruction pipeline in exact order', async () => {
    // Golden JSON from mechanics.rs test_golden_build_instructions_for_entry
    const goldenEntryJson = JSON.stringify([
        "Cloak",
        { SetSize: { w: 800, h: 600 } },
        { WaitForCommit: { timeout_ms: 300 } },
        {
            SetPositionAnchored: {
                intended_w: 800,
                intended_h: 600,
                screen_anchor_x: 1895,
                screen_anchor_y: 60,
                pivot_u: 1.0,
                pivot_v: 0.0,
                offset_x: 10,
                offset_y: -5,
                area: "workarea",
                margin_top: 20,
                margin_bottom: 0,
                margin_left: 0,
                margin_right: 25,
                clamp: true,
            },
        },
        "Uncloak",
    ]);

    const ext = new SpawnAtExtension({ path: '/dummy' });
    ext.enable();

    // Hook logTime to record executed BATCH_STEP operations
    const executedSteps = [];
    const origLogTime = ext.logTime.bind(ext);
    ext.logTime = (tag, extra, win) => {
        if (tag === 'BATCH_STEP') {
            executedSteps.push(extra.replace('inst=', ''));
        }
        origLogTime(tag, extra, win);
    };

    const validated = validateInstructions(goldenEntryJson);
    const { window, actor } = createMockWindowAndActor();

    await ext._runBatch(window, actor, validated);

    assert.deepEqual(executedSteps, [
        'Cloak',
        'SetSize',
        'WaitForCommit',
        'SetPositionAnchored',
        'Uncloak',
    ]);

    ext.disable();
});

test('dispatcher: executes Rust golden resize-only instruction pipeline without move', async () => {
    // Golden JSON from mechanics.rs test_golden_instruction_batch_resize_only
    const goldenResizeOnlyJson = JSON.stringify([
        "Cloak",
        { SetSize: { w: 500, h: 400 } },
        { WaitForCommit: { timeout_ms: 300 } },
        "Uncloak",
    ]);

    const ext = new SpawnAtExtension({ path: '/dummy' });
    ext.enable();

    const executedSteps = [];
    const origLogTime = ext.logTime.bind(ext);
    ext.logTime = (tag, extra, win) => {
        if (tag === 'BATCH_STEP') {
            executedSteps.push(extra.replace('inst=', ''));
        }
        origLogTime(tag, extra, win);
    };

    const validated = validateInstructions(goldenResizeOnlyJson);
    const { window, actor } = createMockWindowAndActor();

    await ext._runBatch(window, actor, validated);

    assert.deepEqual(executedSteps, [
        'Cloak',
        'SetSize',
        'WaitForCommit',
        'Uncloak',
    ]);

    ext.disable();
});

test('dispatcher: executes Rust golden transform instruction pipeline in exact order', async () => {
    // Golden JSON from mechanics.rs test_golden_transform_batch_instructions
    const goldenTransformJson = JSON.stringify([
        "Snapshot",
        "Cloak",
        { SetSize: { w: 500, h: 400 } },
        { WaitForCommit: { timeout_ms: 500 } },
        {
            SetPositionAnchored: {
                intended_w: 500,
                intended_h: 400,
                screen_anchor_x: 960,
                screen_anchor_y: 540,
                pivot_u: 0.5,
                pivot_v: 0.5,
                offset_x: 0,
                offset_y: 0,
                area: "screen",
                margin_top: 16,
                margin_bottom: 16,
                margin_left: 16,
                margin_right: 16,
                clamp: true,
            },
        },
        "Uncloak",
        "DestroySnapshot",
    ]);

    const ext = new SpawnAtExtension({ path: '/dummy' });
    ext.enable();

    const executedSteps = [];
    ext.logTime = (tag, extra) => {
        if (tag === 'BATCH_STEP') {
            executedSteps.push(extra.replace('inst=', ''));
        }
    };

    const validated = validateInstructions(goldenTransformJson);
    const { window, actor } = createMockWindowAndActor();

    await ext._runBatch(window, actor, validated);

    assert.deepEqual(executedSteps, [
        'Snapshot',
        'Cloak',
        'SetSize',
        'WaitForCommit',
        'SetPositionAnchored',
        'Uncloak',
        'DestroySnapshot',
    ]);

    ext.disable();
});

test('dispatcher: executes Rust golden position-only entry pipeline without WaitForCommit', async () => {
    // Golden JSON from mechanics.rs test_golden_instruction_batch_pos_only
    const goldenPosOnlyJson = JSON.stringify([
        "Cloak",
        {
            SetPositionAnchored: {
                intended_w: 0,
                intended_h: 0,
                screen_anchor_x: 100,
                screen_anchor_y: 200,
                pivot_u: 0.0,
                pivot_v: 0.0,
                offset_x: 0,
                offset_y: 0,
                area: "workarea",
                margin_top: 16,
                margin_bottom: 16,
                margin_left: 16,
                margin_right: 16,
                clamp: true,
            },
        },
        "Uncloak",
    ]);

    const ext = new SpawnAtExtension({ path: '/dummy' });
    ext.enable();

    const executedSteps = [];
    const origLogTime = ext.logTime.bind(ext);
    ext.logTime = (tag, extra, win) => {
        if (tag === 'BATCH_STEP') {
            executedSteps.push(extra.replace('inst=', ''));
        }
        origLogTime(tag, extra, win);
    };

    const validated = validateInstructions(goldenPosOnlyJson);
    const { window, actor } = createMockWindowAndActor();

    await ext._runBatch(window, actor, validated);

    assert.deepEqual(executedSteps, [
        'Cloak',
        'SetPositionAnchored',
        'Uncloak',
    ]);

    ext.disable();
});

test('dispatcher: executes Rust golden position-only transform pipeline without WaitForCommit', async () => {
    // Golden JSON from mechanics.rs test_golden_transform_batch_pos_only_instructions
    const goldenTransformPosOnlyJson = JSON.stringify([
        "Snapshot",
        "Cloak",
        {
            SetPositionAnchored: {
                intended_w: 500,
                intended_h: 400,
                screen_anchor_x: 960,
                screen_anchor_y: 540,
                pivot_u: 0.5,
                pivot_v: 0.5,
                offset_x: 0,
                offset_y: 0,
                area: "screen",
                margin_top: 16,
                margin_bottom: 16,
                margin_left: 16,
                margin_right: 16,
                clamp: true,
            },
        },
        "Uncloak",
        "DestroySnapshot",
    ]);

    const ext = new SpawnAtExtension({ path: '/dummy' });
    ext.enable();

    const executedSteps = [];
    ext.logTime = (tag, extra) => {
        if (tag === 'BATCH_STEP') {
            executedSteps.push(extra.replace('inst=', ''));
        }
    };

    const validated = validateInstructions(goldenTransformPosOnlyJson);
    const { window, actor } = createMockWindowAndActor();

    await ext._runBatch(window, actor, validated);

    assert.deepEqual(executedSteps, [
        'Snapshot',
        'Cloak',
        'SetPositionAnchored',
        'Uncloak',
        'DestroySnapshot',
    ]);

    ext.disable();
});

test('dispatcher: unknown instruction causes immediate test failure rather than silent ignore', async () => {
    const ext = new SpawnAtExtension({ path: '/dummy' });
    ext.enable();

    const { window, actor } = createMockWindowAndActor();
    const maliciousBatch = [
        { name: 'UnknownOpcode', args: {} },
    ];

    await assert.rejects(
        async () => {
            await ext._runBatch(window, actor, maliciousBatch);
        },
        {
            message: '[SpawnAt] Unknown instruction "UnknownOpcode"',
        }
    );

    ext.disable();
});

test('dispatcher: emits SpawnClaimed signal on success with target_id and placement geometry', async () => {
    const ext = new SpawnAtExtension({ path: '/dummy' });
    ext.enable();

    let emittedSignal = null;
    ext._dbusManager = {
        emitSignal: (name, params) => {
            emittedSignal = { name, params };
        },
        unexport: () => {},
    };

    const { window, actor } = createMockWindowAndActor();
    const batch = [
        { name: 'Cloak', args: {} },
        { name: 'SetSize', args: { w: 800, h: 600 } },
        { name: 'WaitForCommit', args: { timeout_ms: 50 } },
        {
            name: 'SetPositionAnchored',
            args: {
                intended_w: 800,
                intended_h: 600,
                screen_anchor_x: 500,
                screen_anchor_y: 500,
                pivot_u: 0.5,
                pivot_v: 0.5,
                offset_x: 0,
                offset_y: 0,
                area: 'workarea',
                margin_top: 0,
                margin_bottom: 0,
                margin_left: 0,
                margin_right: 0,
                clamp: true,
            },
        },
        { name: 'Uncloak', args: {} },
    ];

    await ext._runBatch(window, actor, batch, 'org.gnome.Terminal');

    assert.ok(emittedSignal, 'SpawnClaimed signal should be emitted');
    assert.equal(emittedSignal.name, 'SpawnClaimed');
    assert.equal(emittedSignal.params.value[0], 'org.gnome.Terminal'); // target_id
    assert.equal(emittedSignal.params.value[1], true); // success
    assert.equal(emittedSignal.params.value[2], 101); // window_id
    assert.equal(emittedSignal.params.value[7], false); // size_raised
    assert.equal(emittedSignal.params.value[8], ''); // error

    ext.disable();
});

test('dispatcher: emits SpawnClaimed signal with failure status on error', async () => {
    const ext = new SpawnAtExtension({ path: '/dummy' });
    ext.enable();

    let emittedSignal = null;
    ext._dbusManager = {
        emitSignal: (name, params) => {
            emittedSignal = { name, params };
        },
        unexport: () => {},
    };

    const { window, actor } = createMockWindowAndActor();
    const maliciousBatch = [
        { name: 'UnknownOpcode', args: {} },
    ];

    ext._enqueueBatch(window, actor, maliciousBatch, 'org.gnome.Terminal');
    // Allow mutex queue execution to complete
    await new Promise(r => setTimeout(r, 20));

    assert.ok(emittedSignal, 'SpawnClaimed failure signal should be emitted');
    assert.equal(emittedSignal.name, 'SpawnClaimed');
    assert.equal(emittedSignal.params.value[0], 'org.gnome.Terminal'); // target_id
    assert.equal(emittedSignal.params.value[1], false); // success = false
    assert.ok(emittedSignal.params.value[8].includes('Unknown instruction'), 'error should detail failure');

    ext.disable();
});

test('dispatcher: ProtocolVersion property is 5', () => {
    const ext = new SpawnAtExtension({ path: '/dummy' });
    assert.equal(ext.ProtocolVersion, 5);
});

test('dispatcher: GetWindows returns protocol 5 metadata (monitor, state, workspace)', () => {
    const ext = new SpawnAtExtension({ path: '/dummy' });
    const { window } = createMockWindowAndActor();
    window.get_workspace = () => ({ index: () => 1 });
    window.minimized = false;
    ext._getMRUWindows = () => [window];

    const jsonStr = ext.GetWindows();
    const windows = JSON.parse(jsonStr);
    assert.equal(windows.length, 1);
    assert.equal(windows[0].monitor, 0);
    assert.equal(windows[0].state, 'normal');
    assert.equal(windows[0].workspace, 1);
});

test('dispatcher: GetLayout returns schema_version 1 monitors', () => {
    const ext = new SpawnAtExtension({ path: '/dummy' });
    const jsonStr = ext.GetLayout();
    const layout = JSON.parse(jsonStr);
    assert.equal(layout.schema_version, 1);
    assert.ok(Array.isArray(layout.monitors));
    assert.equal(layout.monitors.length, 1);
    const m = layout.monitors[0];
    assert.equal(m.index, 0);
    assert.equal(m.primary, true);
    assert.deepEqual(m.screen, { x: 0, y: 0, w: 1920, h: 1080 });
    assert.deepEqual(m.workarea, { x: 0, y: 40, w: 1920, h: 1040 });
});

test('dispatcher: CloseWindow deletes target window gracefully', () => {
    const ext = new SpawnAtExtension({ path: '/dummy' });
    const { window } = createMockWindowAndActor();

    // Stub _findWindow to return our mock window
    ext._findWindow = (t) => (t === '101' || t === 'org.gnome.TextEditor' ? window : null);

    assert.equal(ext.CloseWindow('999'), false, 'Non-existent window returns false');
    assert.equal(window._deleted, undefined);

    assert.equal(ext.CloseWindow('101'), true, 'Existing window by id returns true');
    assert.equal(window._deleted, true, 'window.delete() must be invoked');
});

test('dispatcher: unmaximizes window before placement and re-asserts if client re-maximized', async () => {
    const ext = new SpawnAtExtension({ path: '/dummy' });
    ext.enable();

    const { window, actor } = createMockWindowAndActor();
    let unmaximizeCount = 0;
    let isMax = true;
    window.is_maximized = () => isMax;
    window.unmaximize = () => {
        unmaximizeCount++;
        isMax = false;
    };

    const batch = [
        { name: 'Cloak', args: {} },
        { name: 'SetSize', args: { w: 600, h: 400 } },
        { name: 'WaitForCommit', args: { timeout_ms: 10 } },
        {
            name: 'SetPositionAnchored',
            args: {
                intended_w: 600,
                intended_h: 400,
                screen_anchor_x: 200,
                screen_anchor_y: 200,
                pivot_u: 0.0,
                pivot_v: 0.0,
                offset_x: 0,
                offset_y: 0,
                area: 'workarea',
                margin_top: 0,
                margin_bottom: 0,
                margin_left: 0,
                margin_right: 0,
                clamp: true,
            },
        },
        { name: 'Uncloak', args: {} },
    ];

    await ext._runBatch(window, actor, batch, 'test-app');
    assert.ok(unmaximizeCount >= 1, 'Should unmaximize maximized window before placement');

    ext.disable();
});

test('dispatcher (modern): structured trace logging captures snapshot, actor geometry, and window state transitions', async () => {
    const ext = new SpawnAtExtension({ path: '/dummy' });
    ext.enable();

    const { window, actor } = createMockWindowAndActor();
    actor.x = 100;
    actor.y = 100;
    actor.width = 640;
    actor.height = 480;
    actor.opacity = 255;
    actor.get_parent = () => ({
        constructor: { name: 'MetaWindowActor' },
        add_child: () => {},
    });
    window.get_compositor_private = () => actor;
    window.minimize = () => { window.minimized = true; };
    window.unminimize = () => { window.minimized = false; };
    window.raise = () => {};
    window.activate = () => {};

    const prevTabList = globalThis.global.display.get_tab_list;
    globalThis.global.display.get_tab_list = () => [window];

    const logs = [];
    const origError = console.error;
    console.error = (...args) => {
        logs.push(args.join(' '));
    };

    try {
        ext.SetLogging(true);

        const batch = [
            { name: 'Snapshot', args: {} },
            { name: 'Cloak', args: {} },
            { name: 'SetSize', args: { w: 800, h: 600 } },
            { name: 'WaitForCommit', args: { timeout_ms: 10 } },
            {
                name: 'SetPositionAnchored',
                args: {
                    intended_w: 800,
                    intended_h: 600,
                    screen_anchor_x: 200,
                    screen_anchor_y: 200,
                    pivot_u: 0.0,
                    pivot_v: 0.0,
                    offset_x: 0,
                    offset_y: 0,
                    area: 'workarea',
                    margin_top: 0,
                    margin_bottom: 0,
                    margin_left: 0,
                    margin_right: 0,
                    clamp: true,
                },
            },
            { name: 'Uncloak', args: {} },
            { name: 'DestroySnapshot', args: {} },
        ];

        await ext._runBatch(window, actor, batch, 'test-app');

        ext._focusWindowObject(window);
        ext.SetWindowState('101', 'minimize');
        ext.SetWindowState('101', 'unminimize');

        ext.SetLogging(false);
    } finally {
        console.error = origError;
        globalThis.global.display.get_tab_list = prevTabList;
        ext.disable();
    }

    const timeLogs = logs.filter(l => l.includes('[spawn-at-time]'));

    // 1. Snapshot created & destroyed
    const snapshotCreated = timeLogs.find(l => l.includes('SNAPSHOT_CREATED'));
    assert.ok(snapshotCreated, 'Should emit SNAPSHOT_CREATED trace');
    assert.match(snapshotCreated, /visible=(true|false)/, 'SNAPSHOT_CREATED should report clone visibility');
    assert.match(snapshotCreated, /pos=\(\d+,\d+\)/, 'SNAPSHOT_CREATED should report clone position');
    assert.match(snapshotCreated, /size=\(\d+x\d+\)/, 'SNAPSHOT_CREATED should report clone size');
    assert.match(snapshotCreated, /opacity=\d+/, 'SNAPSHOT_CREATED should report clone opacity');
    assert.match(snapshotCreated, /parent=\w+/, 'SNAPSHOT_CREATED should report parent name');

    const snapshotDestroyed = timeLogs.find(l => l.includes('SNAPSHOT_DESTROYED'));
    assert.ok(snapshotDestroyed, 'Should emit SNAPSHOT_DESTROYED trace');

    // 2. Cloak, SetSize, SetPositionAnchored, Uncloak actor states
    const cloakLog = timeLogs.find(l => l.includes('| CLOAK '));
    assert.ok(cloakLog, 'Should emit CLOAK trace');
    assert.match(cloakLog, /opacity=0 frame=\(100,100,640x480\)/);

    const setSizeLog = timeLogs.find(l => l.includes('| AFTER_SET_SIZE '));
    assert.ok(setSizeLog, 'Should emit AFTER_SET_SIZE trace');
    assert.match(setSizeLog, /opacity=0 frame=\(100,100,800x600\)/);

    const setPosLog = timeLogs.find(l => l.includes('| AFTER_SET_POSITION_ANCHORED '));
    assert.ok(setPosLog, 'Should emit AFTER_SET_POSITION_ANCHORED trace');
    assert.match(setPosLog, /opacity=0 frame=\(200,200,800x600\)/);

    const uncloakLog = timeLogs.find(l => l.includes('| UNCLOAK '));
    assert.ok(uncloakLog, 'Should emit UNCLOAK trace');
    assert.match(uncloakLog, /opacity=255 frame=\(200,200,800x600\)/);

    // 3. Raise, activate, minimize, unminimize
    const raiseLog = timeLogs.find(l => l.includes('| RAISE '));
    assert.ok(raiseLog, 'Should emit RAISE trace');
    assert.match(raiseLog, /opacity=255 frame=\(200,200,800x600\)/);

    const activateLog = timeLogs.find(l => l.includes('| ACTIVATE '));
    assert.ok(activateLog, 'Should emit ACTIVATE trace');
    assert.match(activateLog, /opacity=255 frame=\(200,200,800x600\)/);

    const minimizeLog = timeLogs.find(l => l.includes('| MINIMIZE '));
    assert.ok(minimizeLog, 'Should emit MINIMIZE trace');
    assert.match(minimizeLog, /opacity=255 frame=\(200,200,800x600\)/);

    const unminimizeLog = timeLogs.find(l => l.includes('| UNMINIMIZE '));
    assert.ok(unminimizeLog, 'Should emit UNMINIMIZE trace');
    assert.match(unminimizeLog, /opacity=255 frame=\(200,200,800x600\)/);
});

test('dispatcher (modern): snapshot gating for minimized window and pre-reveal unminimize sequence', async () => {
    const ext = new SpawnAtExtension({ path: '/dummy' });
    ext.enable();

    const { window, actor } = createMockWindowAndActor();
    window.minimized = true;
    window.unminimize = () => { window.minimized = false; };
    window.raise = () => {};
    window.activate = () => {};
    actor.get_parent = () => ({
        constructor: { name: 'MetaWindowActor' },
        add_child: () => {},
    });
    window.get_compositor_private = () => actor;

    const prevTabList = globalThis.global.display.get_tab_list;
    globalThis.global.display.get_tab_list = () => [window];

    const logs = [];
    const origError = console.error;
    console.error = (...args) => { logs.push(args.join(' ')); };

    try {
        ext.SetLogging(true);
        const batch = [
            { name: 'Snapshot', args: {} },
            { name: 'Cloak', args: {} },
            {
                name: 'SetPositionAnchored',
                args: {
                    intended_w: 640,
                    intended_h: 480,
                    screen_anchor_x: 200,
                    screen_anchor_y: 200,
                    pivot_u: 0.0,
                    pivot_v: 0.0,
                    offset_x: 0,
                    offset_y: 0,
                    area: 'workarea',
                    margin_top: 0,
                    margin_bottom: 0,
                    margin_left: 0,
                    margin_right: 0,
                    clamp: true,
                },
            },
            { name: 'Uncloak', args: {} },
            { name: 'DestroySnapshot', args: {} },
        ];

        await ext._runBatch(window, actor, batch, 'test-minimized');
    } finally {
        console.error = origError;
        globalThis.global.display.get_tab_list = prevTabList;
        ext.disable();
    }

    const timeLogs = logs.filter(l => l.includes('[spawn-at-time]'));
    const skipLog = timeLogs.find(l => l.includes('SNAPSHOT_SKIPPED'));
    assert.ok(skipLog, 'Should emit SNAPSHOT_SKIPPED trace');
    assert.match(skipLog, /reason="minimized"/);

    const snapshotCreated = timeLogs.find(l => l.includes('SNAPSHOT_CREATED'));
    assert.equal(snapshotCreated, undefined, 'Must not create snapshot clone for minimized window');

    const preRevealUnminimize = timeLogs.find(l => l.includes('PRE_REVEAL_UNMINIMIZE'));
    assert.ok(preRevealUnminimize, 'Should emit PRE_REVEAL_UNMINIMIZE trace prior to uncloak');

    const raiseIdx = timeLogs.findIndex(l => l.includes('| RAISE '));
    const activateIdx = timeLogs.findIndex(l => l.includes('| ACTIVATE '));
    const uncloakIdx = timeLogs.findIndex(l => l.includes('| UNCLOAK ') && l.includes('opacity=255'));

    assert.ok(raiseIdx !== -1, 'RAISE should be logged');
    assert.ok(activateIdx !== -1, 'ACTIVATE should be logged');
    assert.ok(uncloakIdx !== -1, 'UNCLOAK should be logged');
    assert.ok(raiseIdx < uncloakIdx, 'RAISE must occur before UNCLOAK');
    assert.ok(activateIdx < uncloakIdx, 'ACTIVATE must occur before UNCLOAK');
    assert.equal(window.minimized, false, 'Window must be unminimized');
    assert.equal(actor.opacity, 255, 'Actor opacity must be 255 after reveal');
});

test('dispatcher (modern): snapshot gating for fully covered window (single and union)', async () => {
    const ext = new SpawnAtExtension({ path: '/dummy' });
    ext.enable();

    // 1. Single covering window
    const { window: winA, actor: actA } = createMockWindowAndActor();
    winA.get_frame_rect = () => ({ x: 100, y: 100, width: 400, height: 300 });
    winA.get_compositor_private = () => actA;
    actA.get_parent = () => ({ add_child: () => {} });

    const { window: winB, actor: actB } = createMockWindowAndActor();
    winB.get_frame_rect = () => ({ x: 50, y: 50, width: 600, height: 500 });
    winB.get_compositor_private = () => actB;
    actB.opacity = 255;
    actB.visible = true;

    const mockWs = {
        list_windows: () => [winA, winB],
        get_work_area_for_monitor: () => ({ x: 0, y: 40, width: 1920, height: 1040 }),
    };

    const prevWsMgr = globalThis.global.display.get_workspace_manager;
    const prevSort = globalThis.global.display.sort_windows_by_stacking;
    const prevTabList = globalThis.global.display.get_tab_list;

    globalThis.global.display.get_workspace_manager = () => ({
        get_active_workspace: () => mockWs,
    });
    globalThis.global.display.sort_windows_by_stacking = () => [winA, winB];
    globalThis.global.display.get_tab_list = () => [winA, winB];

    const logs = [];
    const origError = console.error;
    console.error = (...args) => { logs.push(args.join(' ')); };

    try {
        ext.SetLogging(true);
        const batch = [
            { name: 'Snapshot', args: {} },
            { name: 'Cloak', args: {} },
            {
                name: 'SetPositionAnchored',
                args: {
                    intended_w: 400,
                    intended_h: 300,
                    screen_anchor_x: 200,
                    screen_anchor_y: 200,
                    pivot_u: 0.0,
                    pivot_v: 0.0,
                    offset_x: 0,
                    offset_y: 0,
                    area: 'workarea',
                    margin_top: 0,
                    margin_bottom: 0,
                    margin_left: 0,
                    margin_right: 0,
                    clamp: true,
                },
            },
            { name: 'Uncloak', args: {} },
            { name: 'DestroySnapshot', args: {} },
        ];

        await ext._runBatch(winA, actA, batch, 'test-covered-single');

        const timeLogs = logs.filter(l => l.includes('[spawn-at-time]'));
        const skipLog = timeLogs.find(l => l.includes('SNAPSHOT_SKIPPED'));
        assert.ok(skipLog, 'Should emit SNAPSHOT_SKIPPED trace');
        assert.match(skipLog, /reason="covered"/);

        const preRevealRaise = timeLogs.find(l => l.includes('PRE_REVEAL_RAISE'));
        assert.ok(preRevealRaise, 'Should emit PRE_REVEAL_RAISE for covered window');

        const raiseIdx = timeLogs.findIndex(l => l.includes('| RAISE '));
        const uncloakIdx = timeLogs.findIndex(l => l.includes('| UNCLOAK ') && l.includes('opacity=255'));
        assert.ok(raiseIdx < uncloakIdx, 'RAISE must precede UNCLOAK');

        // 2. Union of covering windows
        logs.length = 0;
        const { window: winLeft, actor: actLeft } = createMockWindowAndActor();
        winLeft.get_frame_rect = () => ({ x: 100, y: 100, width: 200, height: 300 });
        winLeft.get_compositor_private = () => actLeft;
        actLeft.opacity = 255;
        actLeft.visible = true;

        const { window: winRight, actor: actRight } = createMockWindowAndActor();
        winRight.get_frame_rect = () => ({ x: 300, y: 100, width: 200, height: 300 });
        winRight.get_compositor_private = () => actRight;
        actRight.opacity = 255;
        actRight.visible = true;

        mockWs.list_windows = () => [winA, winLeft, winRight];
        globalThis.global.display.sort_windows_by_stacking = () => [winA, winLeft, winRight];

        await ext._runBatch(winA, actA, batch, 'test-covered-union');
        const unionLogs = logs.filter(l => l.includes('[spawn-at-time]'));
        const unionSkip = unionLogs.find(l => l.includes('SNAPSHOT_SKIPPED'));
        assert.ok(unionSkip, 'Should emit SNAPSHOT_SKIPPED for union coverage');
        assert.match(unionSkip, /reason="covered"/);

        // 3. Partial coverage (conservative: snapshot kept)
        logs.length = 0;
        mockWs.list_windows = () => [winA, winLeft];
        globalThis.global.display.sort_windows_by_stacking = () => [winA, winLeft];

        await ext._runBatch(winA, actA, batch, 'test-covered-partial');
        const partialLogs = logs.filter(l => l.includes('[spawn-at-time]'));
        const createdLog = partialLogs.find(l => l.includes('SNAPSHOT_CREATED'));
        assert.ok(createdLog, 'Snapshot must be created for partially covered window');
    } finally {
        console.error = origError;
        globalThis.global.display.get_workspace_manager = prevWsMgr;
        globalThis.global.display.sort_windows_by_stacking = prevSort;
        globalThis.global.display.get_tab_list = prevTabList;
        ext.disable();
    }
});

test('dispatcher (modern): snapshot gating for inactive workspace window', async () => {
    const ext = new SpawnAtExtension({ path: '/dummy' });
    ext.enable();

    const { window, actor } = createMockWindowAndActor();
    const activeWs = { list_windows: () => [window], get_work_area_for_monitor: () => ({ x: 0, y: 40, width: 1920, height: 1040 }) };

    window.located_on_workspace = (ws) => ws !== activeWs;
    window.is_on_all_workspaces = () => false;
    actor.get_parent = () => ({ add_child: () => {} });
    window.get_compositor_private = () => actor;

    const prevWsMgr = globalThis.global.display.get_workspace_manager;
    globalThis.global.display.get_workspace_manager = () => ({
        get_active_workspace: () => activeWs,
    });

    const logs = [];
    const origError = console.error;
    console.error = (...args) => { logs.push(args.join(' ')); };

    try {
        ext.SetLogging(true);
        const batch = [
            { name: 'Snapshot', args: {} },
            { name: 'Cloak', args: {} },
            { name: 'Uncloak', args: {} },
        ];
        await ext._runBatch(window, actor, batch, 'test-inactive-ws');
    } finally {
        console.error = origError;
        globalThis.global.display.get_workspace_manager = prevWsMgr;
        ext.disable();
    }

    const timeLogs = logs.filter(l => l.includes('[spawn-at-time]'));
    const skipLog = timeLogs.find(l => l.includes('SNAPSHOT_SKIPPED'));
    assert.ok(skipLog, 'Should emit SNAPSHOT_SKIPPED for window on inactive workspace');
    assert.match(skipLog, /reason="hidden_or_inactive_workspace"/);
    assert.equal(actor.opacity, 255, 'Actor must be uncloaked');
});

test('dispatcher (modern): cloak/uncloak balance and snapshot destruction on error and disable', async () => {
    const ext = new SpawnAtExtension({ path: '/dummy' });
    ext.enable();

    const { window, actor } = createMockWindowAndActor();
    actor.get_parent = () => ({ add_child: () => {} });
    window.get_compositor_private = () => actor;

    // 1. Error mid-batch ensures uncloak and snapshot destruction in finally
    const invalidBatch = [
        { name: 'Snapshot', args: {} },
        { name: 'Cloak', args: {} },
        { name: 'NonExistentInstruction', args: {} },
    ];

    await assert.rejects(
        async () => ext._runBatch(window, actor, invalidBatch, 'test-error'),
        /Unknown instruction/
    );

    assert.equal(actor.opacity, 255, 'Actor opacity must be restored to 255 on error mid-batch');
    assert.equal(ext._cloakManager.snapshots.has(actor), false, 'Snapshot must be destroyed on error');
    assert.equal(ext._cloakManager.cloakedActors.has(actor), false, 'Actor must be removed from cloaked set');

    // 2. Uncloak balance on disable()
    ext._cloak(actor);
    assert.equal(actor.opacity, 0, 'Actor should be cloaked');
    assert.equal(ext._cloakManager.cloakedActors.has(actor), true);

    ext.disable();
    assert.equal(actor.opacity, 255, 'Actor must be uncloaked on extension disable()');
    assert.equal(ext._cloakManager.cloakedActors.size, 0);
    assert.equal(ext._cloakManager.watchdogs.size, 0);
    assert.equal(ext._cloakManager.snapshots.size, 0);
});

test('dispatcher (modern): defers FocusWindow to batch reveal resulting in exactly one RAISE/ACTIVATE pair', async () => {
    const ext = new SpawnAtExtension({ path: '/dummy' });
    ext.enable();

    const { window, actor } = createMockWindowAndActor();
    window.minimized = true;
    actor.get_parent = () => ({ add_child: () => {} });
    window.get_compositor_private = () => actor;

    const prevTabList = globalThis.global.display.get_tab_list;
    globalThis.global.display.get_tab_list = () => [window];

    const logs = [];
    const origError = console.error;
    console.error = (...args) => { logs.push(args.join(' ')); };

    try {
        ext.SetLogging(true);
        const batch = [
            'Snapshot',
            'Cloak',
            { SetSize: { w: 500, h: 400 } },
            { WaitForCommit: { timeout_ms: 20 } },
            'Uncloak',
        ];

        // ExecuteBatch enqueues asynchronously and returns immediately over D-Bus
        ext.ExecuteBatch(String(window.get_id()), JSON.stringify(batch));

        // CLI calls FocusWindow immediately after ExecuteBatch returns:
        const focusRes = ext.FocusWindow(String(window.get_id()));
        assert.equal(focusRes, true, 'FocusWindow on pending batch must report success');

        // Allow batch and microtasks to complete
        await new Promise(r => setTimeout(r, 60));
    } finally {
        console.error = origError;
        globalThis.global.display.get_tab_list = prevTabList;
        ext.disable();
    }

    const timeLogs = logs.filter(l => l.includes('[spawn-at-time]'));
    const deferredLog = timeLogs.find(l => l.includes('FOCUS_DEFERRED'));
    assert.ok(deferredLog, 'Must emit FOCUS_DEFERRED when batch is active or queued');

    const raiseLogs = timeLogs.filter(l => l.includes('| RAISE '));
    const activateLogs = timeLogs.filter(l => l.includes('| ACTIVATE '));

    // Must fire exactly ONCE: inside batch uncloak
    assert.equal(raiseLogs.length, 1, 'Deferred FocusWindow must result in exactly one RAISE before reveal');
    assert.equal(activateLogs.length, 1, 'Deferred FocusWindow must result in exactly one ACTIVATE before reveal');
});

test('dispatcher (modern): failed batch executes deferred focus fallback and does not leave window cloaked', async () => {
    const ext = new SpawnAtExtension({ path: '/dummy' });
    ext.enable();

    const { window, actor } = createMockWindowAndActor();
    actor.get_parent = () => ({ add_child: () => {} });
    window.get_compositor_private = () => actor;
    window.move_resize_frame = () => {
        throw new Error('Simulated compositor failure during batch');
    };

    const prevTabList = globalThis.global.display.get_tab_list;
    globalThis.global.display.get_tab_list = () => [window];

    const logs = [];
    const origError = console.error;
    console.error = (...args) => { logs.push(args.join(' ')); };

    try {
        ext.SetLogging(true);
        const batch = [
            'Snapshot',
            'Cloak',
            { SetSize: { w: 500, h: 400 } },
            'Uncloak',
        ];

        ext.ExecuteBatch(String(window.get_id()), JSON.stringify(batch));
        ext.FocusWindow(String(window.get_id()));

        await new Promise(r => setTimeout(r, 60));
    } finally {
        console.error = origError;
        globalThis.global.display.get_tab_list = prevTabList;
        ext.disable();
    }

    assert.equal(actor.opacity, 255, 'Actor must be uncloaked even if batch failed');
    const timeLogs = logs.filter(l => l.includes('[spawn-at-time]'));
    const fallbackLog = timeLogs.find(l => l.includes('DEFERRED_FOCUS_FALLBACK'));
    assert.ok(fallbackLog, 'Must execute DEFERRED_FOCUS_FALLBACK on batch failure');
    const raiseLogs = timeLogs.filter(l => l.includes('| RAISE '));
    const activateLogs = timeLogs.filter(l => l.includes('| ACTIVATE '));
    assert.equal(raiseLogs.length, 1, 'Deferred focus fallback must raise window once');
    assert.equal(activateLogs.length, 1, 'Deferred focus fallback must activate window once');
});

test('dispatcher (modern): FocusWindow on window without pending batch focuses immediately', async () => {
    const ext = new SpawnAtExtension({ path: '/dummy' });
    ext.enable();

    const { window, actor } = createMockWindowAndActor();
    actor.get_parent = () => ({ add_child: () => {} });
    window.get_compositor_private = () => actor;

    const prevTabList = globalThis.global.display.get_tab_list;
    globalThis.global.display.get_tab_list = () => [window];

    const logs = [];
    const origError = console.error;
    console.error = (...args) => { logs.push(args.join(' ')); };

    try {
        ext.SetLogging(true);
        const res = ext.FocusWindow(String(window.get_id()));
        assert.equal(res, true, 'FocusWindow must succeed');
    } finally {
        console.error = origError;
        globalThis.global.display.get_tab_list = prevTabList;
        ext.disable();
    }

    const timeLogs = logs.filter(l => l.includes('[spawn-at-time]'));
    const raiseLogs = timeLogs.filter(l => l.includes('| RAISE '));
    const activateLogs = timeLogs.filter(l => l.includes('| ACTIVATE '));
    assert.equal(raiseLogs.length, 1, 'Immediate focus must raise window once');
    assert.equal(activateLogs.length, 1, 'Immediate focus must activate window once');
    assert.equal(timeLogs.some(l => l.includes('FOCUS_DEFERRED')), false, 'Must not defer focus when no batch exists');
});



