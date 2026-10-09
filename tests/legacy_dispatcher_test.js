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

test('dispatcher (legacy): ProtocolVersion property is 2', () => {
    const ext = new SpawnAtExtension();
    assert.equal(ext.ProtocolVersion, 2);
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
