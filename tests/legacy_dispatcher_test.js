import test from 'node:test';
import assert from 'node:assert/strict';
import { register } from 'node:module';
import fs from 'node:fs';

// Register custom loader
register('./gi_loader.js', import.meta.url);

// Mock legacy GNOME 42-44 Shell environment
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
        get_tab_list: () => [],
        unset_input_focus: () => {},
    },
    // Legacy GNOME 42-44 workspace_manager directly on global
    workspace_manager: {
        get_active_workspace: () => ({
            get_work_area_for_monitor: () => ({ x: 0, y: 40, width: 1920, height: 1040 }),
        }),
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

// Mock legacy GJS imports
globalThis.imports = {
    gi: {
        Gio: (await import('gi://Gio')).default,
        Meta: (await import('gi://Meta')).default,
        GLib: (await import('gi://GLib')).default,
        Clutter: (await import('gi://Clutter')).default,
    },
    misc: {
        extensionUtils: {
            getCurrentExtension: () => null,
        },
    },
};

const { SpawnAtExtension } = await import('../assets/gnome/legacy/extension.js');
const { validateInstructions } = await import('../assets/gnome/legacy/validator.js');
const { DBUS_IFACE: legacyIface } = await import('../assets/gnome/legacy/dbus.js');
const { DBUS_IFACE: modernIface } = await import('../assets/gnome/modern/dbus.js');

function createMockWindowAndActor() {
    let frameRect = { x: 100, y: 100, width: 640, height: 480 };
    const window = {
        get_frame_rect: () => ({ ...frameRect }),
        get_buffer_rect: () => ({ ...frameRect }),
        get_id: () => 101,
        get_pid: () => 5432,
        get_wm_class: () => 'org.gnome.TextEditor',
        get_gtk_application_id: () => '',
        get_sandboxed_app_id: () => '',
        get_window_type: () => 0,
        get_client_type: () => 0,
        get_monitor: () => 0,
        get_work_area_current_monitor: () => ({ x: 0, y: 40, width: 1920, height: 1040 }),
        get_work_area_for_monitor: () => ({ x: 0, y: 40, width: 1920, height: 1040 }),
        has_focus: () => true,
        move_resize_frame: (_userOp, x, y, w, h) => {
            frameRect = { x, y, width: w, height: h };
        },
        move_frame: (_userOp, x, y) => {
            frameRect.x = x;
            frameRect.y = y;
        },
        connect: () => 1,
        disconnect: () => {},
        unmaximize: () => {},
        is_maximized: () => false,
        raise: () => {},
        activate: () => {},
        unminimize: () => { window.minimized = false; },
        delete: () => {
            window._deleted = true;
        },
    };

    const actor = {
        opacity: 255,
        connect: () => 1,
        disconnect: () => {},
        remove_all_transitions: () => {},
        set_offscreen_redirect: () => {},
        get_stage: () => globalThis.global.stage,
        is_destroyed: () => false,
    };

    return { window, actor };
}

test('contract: interface XML parity between modern and legacy D-Bus definitions', () => {
    assert.equal(legacyIface.trim(), modernIface.trim(), 'Legacy and modern D-Bus interface XML must match character-for-character');
});

test('dispatcher (legacy): ProtocolVersion property is 5', () => {
    const ext = new SpawnAtExtension();
    assert.equal(ext.ProtocolVersion, 5);
});

test('dispatcher (legacy): GetWindows returns protocol 5 metadata (monitor, state, workspace)', () => {
    const ext = new SpawnAtExtension();
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

test('dispatcher (legacy): GetLayout returns schema_version 1 monitors', () => {
    const ext = new SpawnAtExtension();
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

test('dispatcher (legacy): CloseWindow deletes target window gracefully', () => {
    const ext = new SpawnAtExtension();
    const { window } = createMockWindowAndActor();

    ext._findWindow = (t) => (t === '101' || t === 'org.gnome.TextEditor' ? window : null);

    assert.equal(ext.CloseWindow('999'), false, 'Non-existent window returns false');
    assert.equal(window._deleted, undefined);

    assert.equal(ext.CloseWindow('101'), true, 'Existing window by id returns true');
    assert.equal(window._deleted, true, 'window.delete() must be invoked');
});

test('dispatcher (legacy): executes Rust golden entry instruction pipeline', async () => {
    const ext = new SpawnAtExtension();
    ext.enable();

    const rawInstructions = [
        "Snapshot",
        "Cloak",
        { "SetSize": { "w": 800, "h": 600 } },
        { "WaitForCommit": { "timeout_ms": 120 } },
        {
            "SetPositionAnchored": {
                "screen_anchor_x": 1904,
                "screen_anchor_y": 1064,
                "pivot_u": 1.0,
                "pivot_v": 1.0,
                "offset_x": 0,
                "offset_y": 0,
                "area": "workarea",
                "margin_top": 16,
                "margin_bottom": 16,
                "margin_left": 16,
                "margin_right": 16,
                "clamp": true
            }
        },
        { "Uncloak": { "wake": true } }
    ];

    const validated = validateInstructions(rawInstructions);
    const { window, actor } = createMockWindowAndActor();

    await ext._runBatch(window, actor, validated, 'golden-entry-test');

    const finalRect = window.get_frame_rect();
    assert.equal(finalRect.width, 800);
    assert.equal(finalRect.height, 600);
    ext.disable();
});

test('dispatcher (legacy): emits SpawnClaimed signal on success with target_id and placement geometry', async () => {
    const ext = new SpawnAtExtension();
    ext.enable();

    let emittedSignal = null;
    let emittedArgs = null;
    ext._dbusManager.emitSignal = (name, args) => {
        emittedSignal = name;
        emittedArgs = args;
    };

    const rawInstructions = [
        "Cloak",
        { "SetSize": { "w": 600, "h": 400 } },
        { "WaitForCommit": { "timeout_ms": 50 } },
        {
            "SetPositionAnchored": {
                "screen_anchor_x": 200,
                "screen_anchor_y": 200,
                "pivot_u": 0.0,
                "pivot_v": 0.0,
                "clamp": true
            }
        },
        { "Uncloak": { "wake": true } }
    ];

    const validated = validateInstructions(rawInstructions);
    const { window, actor } = createMockWindowAndActor();

    await ext._runBatch(window, actor, validated, 'target-claim-test-legacy');

    assert.equal(emittedSignal, 'SpawnClaimed');
    assert(emittedArgs !== null);
    ext.disable();
});

test('dispatcher (legacy): unmaximizes window before placement and re-asserts if client re-maximized', async () => {
    const ext = new SpawnAtExtension();
    ext.enable();

    const { window, actor } = createMockWindowAndActor();
    let unmaximizeCalls = 0;
    let isMaximizedState = true;

    window.unmaximize = () => {
        unmaximizeCalls++;
        isMaximizedState = false;
    };
    window.is_maximized = () => isMaximizedState;

    const rawInstructions = [
        "Cloak",
        { "SetSize": { "w": 600, "h": 400 } },
        { "WaitForCommit": { "timeout_ms": 50 } },
        {
            "SetPositionAnchored": {
                "screen_anchor_x": 200,
                "screen_anchor_y": 200,
                "pivot_u": 0.0,
                "pivot_v": 0.0,
                "clamp": true
            }
        },
        { "Uncloak": { "wake": true } }
    ];

    const validated = validateInstructions(rawInstructions);
    await ext._runBatch(window, actor, validated, 'unmax-test-legacy');

    assert(unmaximizeCalls >= 1, `Expected unmaximize to be called, got ${unmaximizeCalls}`);
    ext.disable();
});

test('dispatcher (legacy): structured trace logging captures snapshot, actor geometry, and window state transitions', async () => {
    const ext = new SpawnAtExtension();
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

        await ext._runBatch(window, actor, batch, 'test-app-legacy');

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

test('dispatcher (legacy): snapshot gating for minimized window and pre-reveal unminimize sequence', async () => {
    const ext = new SpawnAtExtension();
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

test('dispatcher (legacy): snapshot gating for fully covered window (single and union)', async () => {
    const ext = new SpawnAtExtension();
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

    const prevGlobalWsMgr = globalThis.global.workspace_manager;
    const prevWsMgr = globalThis.global.display.get_workspace_manager;
    const prevSort = globalThis.global.display.sort_windows_by_stacking;
    const prevTabList = globalThis.global.display.get_tab_list;

    globalThis.global.workspace_manager = {
        get_active_workspace: () => mockWs,
    };
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
        globalThis.global.workspace_manager = prevGlobalWsMgr;
        globalThis.global.display.get_workspace_manager = prevWsMgr;
        globalThis.global.display.sort_windows_by_stacking = prevSort;
        globalThis.global.display.get_tab_list = prevTabList;
        ext.disable();
    }
});

test('dispatcher (legacy): snapshot gating for inactive workspace window', async () => {
    const ext = new SpawnAtExtension();
    ext.enable();

    const { window, actor } = createMockWindowAndActor();
    const activeWs = { list_windows: () => [window], get_work_area_for_monitor: () => ({ x: 0, y: 40, width: 1920, height: 1040 }) };

    window.located_on_workspace = (ws) => ws !== activeWs;
    window.is_on_all_workspaces = () => false;
    actor.get_parent = () => ({ add_child: () => {} });
    window.get_compositor_private = () => actor;

    const prevGlobalWsMgr = globalThis.global.workspace_manager;
    const prevWsMgr = globalThis.global.display.get_workspace_manager;
    globalThis.global.workspace_manager = {
        get_active_workspace: () => activeWs,
    };
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
        globalThis.global.workspace_manager = prevGlobalWsMgr;
        globalThis.global.display.get_workspace_manager = prevWsMgr;
        ext.disable();
    }

    const timeLogs = logs.filter(l => l.includes('[spawn-at-time]'));
    const skipLog = timeLogs.find(l => l.includes('SNAPSHOT_SKIPPED'));
    assert.ok(skipLog, 'Should emit SNAPSHOT_SKIPPED for window on inactive workspace');
    assert.match(skipLog, /reason="hidden_or_inactive_workspace"/);
    assert.equal(actor.opacity, 255, 'Actor must be uncloaked');
});

test('dispatcher (legacy): cloak/uncloak balance and snapshot destruction on error and disable', async () => {
    const ext = new SpawnAtExtension();
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


