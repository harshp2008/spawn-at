import test from 'node:test';
import assert from 'node:assert/strict';
import { register } from 'node:module';

register('./gi_loader.js', import.meta.url);

// Same mocked Shell globals as dispatcher_test.js; none of these tests maps a window.
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

// A launch key as `spawn-at spawn` builds it (src/commands/spawn.rs `launch_key`): the pid, the
// milliseconds since the epoch and a `_TIME0` part. A GTK app writes the whole string into the
// window's _NET_STARTUP_ID, and the extension reads it back with window.get_startup_id().
const KEY_A = 'spawn-at-4242-1793282000000_TIME0';
const KEY_B = 'spawn-at-4242-1793282000001_TIME0';
const arm = w => JSON.stringify([{ SetSize: { w, h: 100 } }]);
const windowWithStartupId = id => ({
    get_startup_id: () => id,
    get_wm_class: () => 'unrelated-class',
});

function newExtension() {
    const ext = new SpawnAtExtension({ path: '/dummy' });
    ext.enable();
    return ext;
}

test('launch key: a window whose startup id is the key claims exactly that entry', () => {
    const ext = newExtension();
    // Several entries in one batch are armed under their own keys (driver.rs: target = entry.key).
    ext.ArmSpawnEntry(KEY_A, KEY_A, arm(101));
    ext.ArmSpawnEntry(KEY_B, KEY_B, arm(102));
    const win = windowWithStartupId(KEY_B);
    assert.equal(ext._findArmedKey(ext._getIdentifiers(win)), KEY_B);
    assert.ok(ext._claimInstructions(win));
    assert.equal(ext._getState(win).instructions[0].args.w, 102);
    // The other entry is still armed.
    assert.deepEqual([...ext._armedSpawns.keys()], [KEY_A]);
    ext.disable();
});

test('launch key: a startup id of another launch claims nothing', () => {
    const ext = newExtension();
    ext.ArmSpawnEntry(KEY_A, KEY_A, arm(101));
    const win = windowWithStartupId('spawn-at-4243-1793282000000_TIME0');
    assert.equal(ext._findArmedKey(ext._getIdentifiers(win)), null);
    ext.disable();
});

test('launch key: DisarmEntry withdraws the entry under the same key', () => {
    const ext = newExtension();
    ext.ArmSpawnEntry(KEY_A, KEY_A, arm(101));
    assert.equal(ext.DisarmEntry(KEY_A, KEY_A), true);
    assert.equal(ext._armedSpawns.size, 0);
    ext.disable();
});
