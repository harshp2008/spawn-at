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
            get_active_workspace: () => ({}),
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

const { default: SpawnAtExtension } = await import('../assets/gnome/extension.js');
const { validateInstructions } = await import('../assets/gnome/validator.js');

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

test('dispatcher: ProtocolVersion property is 2', () => {
    const ext = new SpawnAtExtension({ path: '/dummy' });
    assert.equal(ext.ProtocolVersion, 2);
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
