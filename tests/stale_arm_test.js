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
const { DBUS_IFACE } = await import('../assets/gnome/modern/dbus.js');

// An instruction list that says which arm produced it: SetSize with a distinct width.
const arm = w => JSON.stringify([{ SetSize: { w, h: 100 } }]);

// The wm_class the app really has differs from the arm key (the issue's
// `win11-clipboard-history-bin` against the key `win11-clipboard-history`).
const TARGET = 'win11-clipboard-history';
const windowOf = wmClass => ({ get_wm_class: () => wmClass });

function newExtension() {
    const ext = new SpawnAtExtension({ path: '/dummy' });
    ext.enable();
    return ext;
}

// The width of the SetSize the window was given, or null when nothing claimed it.
function claimedWidth(ext, window) {
    if (!ext._claimInstructions(window))
        return null;
    return ext._getState(window).instructions[0].args.w;
}

test('stale arm: the D-Bus XML declares ArmSpawnEntry (sss) and DisarmEntry (ss -> b), ArmSpawn stays (ss)', () => {
    const method = name => DBUS_IFACE.match(new RegExp(`<method name="${name}">([\\s\\S]*?)</method>`))[1];
    const args = body => [...body.matchAll(/<arg type="(\w)" name="(\w+)" direction="(in|out)"\/>/g)]
        .map(m => `${m[3]}:${m[1]}:${m[2]}`);

    assert.deepEqual(args(method('ArmSpawn')), ['in:s:target_id', 'in:s:instructions_json']);
    assert.deepEqual(args(method('ArmSpawnEntry')),
        ['in:s:target_id', 'in:s:entry_key', 'in:s:instructions_json']);
    assert.deepEqual(args(method('DisarmEntry')),
        ['in:s:target_id', 'in:s:entry_key', 'out:b:disarmed']);
    assert.deepEqual(args(method('DisarmSpawn')), ['in:s:target_id', 'out:b:disarmed']);
});

test('stale arm: visible-then-hide, the withdrawn arm is not served to the next window', () => {
    const ext = newExtension();

    // Press 1: the app is visible, forwards to its running instance, no window is created.
    ext.ArmSpawnEntry(TARGET, 'spawn-at-1-A', arm(185));
    // The CLI gave up and withdrew exactly its own arm.
    assert.equal(ext.DisarmEntry(TARGET, 'spawn-at-1-A'), true);
    assert.equal(ext._hasArmedSpawns(), false);

    // Press 2: the window is re-shown and a new Meta window appears.
    ext.ArmSpawnEntry(TARGET, 'spawn-at-2-B', arm(985));
    assert.equal(claimedWidth(ext, windowOf(`${TARGET}-bin`)), 985);
    ext.disable();
});

test('stale arm: without a disarm the older arm is served first (the bug as reported)', () => {
    const ext = newExtension();
    ext.ArmSpawnEntry(TARGET, 'spawn-at-1-A', arm(185));
    ext.ArmSpawnEntry(TARGET, 'spawn-at-2-B', arm(985));
    assert.equal(claimedWidth(ext, windowOf(`${TARGET}-bin`)), 185);
    ext.disable();
});

test('stale arm: hidden re-show still claims its own arm, and leaves no entry behind', () => {
    const ext = newExtension();
    ext.ArmSpawnEntry(TARGET, 'spawn-at-1-A', arm(185));
    assert.equal(claimedWidth(ext, windowOf(`${TARGET}-bin`)), 185);
    assert.equal(ext._armedSpawns.size, 0, 'a claimed entry leaves the queue');
    // The CLI sees the claim; a late disarm of the claimed key is a harmless no-op.
    assert.equal(ext.DisarmEntry(TARGET, 'spawn-at-1-A'), false);
    ext.disable();
});

test('stale arm: concurrent launches keep arm order, and one disarm leaves the others armed', () => {
    const ext = newExtension();
    ext.ArmSpawnEntry(TARGET, 'k-1', arm(101));
    ext.ArmSpawnEntry(TARGET, 'k-2', arm(102));
    ext.ArmSpawnEntry(TARGET, 'k-3', arm(103));

    assert.equal(ext.DisarmEntry(TARGET, 'k-2'), true);
    assert.equal(ext._armedSpawns.get(TARGET).length, 2);

    assert.equal(claimedWidth(ext, windowOf(TARGET)), 101);
    assert.equal(claimedWidth(ext, windowOf(TARGET)), 103);
    assert.equal(claimedWidth(ext, windowOf(TARGET)), null);
    ext.disable();
});

test('stale arm: DisarmEntry refuses an unknown key, a wrong target, an empty key and a repeat', () => {
    const ext = newExtension();
    ext.ArmSpawnEntry(TARGET, 'k-1', arm(101));
    // An arm from an old CLI has no key; an empty key must not match it.
    ext.ArmSpawn(TARGET, arm(102));

    assert.equal(ext.DisarmEntry(TARGET, 'nope'), false);
    assert.equal(ext.DisarmEntry('other-app', 'k-1'), false);
    assert.equal(ext.DisarmEntry(TARGET, ''), false);
    assert.equal(ext.DisarmEntry('', 'k-1'), false);
    assert.equal(ext._armedSpawns.get(TARGET).length, 2, 'nothing was removed by the refused calls');

    assert.equal(ext.DisarmEntry(TARGET, 'k-1'), true);
    assert.equal(ext.DisarmEntry(TARGET, 'k-1'), false);
    assert.equal(claimedWidth(ext, windowOf(TARGET)), 102, 'the unkeyed arm survived');
    ext.disable();
});

test('stale arm: disarming the last entry drops the queue and the window hold', () => {
    const ext = newExtension();
    ext.ArmSpawnEntry(TARGET, 'k-1', arm(101));
    assert.equal(ext._hasArmedSpawns(), true);
    assert.equal(ext.DisarmEntry(TARGET, 'k-1'), true);
    assert.equal(ext._armedSpawns.has(TARGET), false);
    assert.equal(ext._hasArmedSpawns(), false);
    ext.disable();
});

test('stale arm: race, the window is claimed before the disarm arrives', () => {
    const ext = newExtension();
    ext.ArmSpawnEntry(TARGET, 'k-1', arm(101));
    const window = windowOf(TARGET);

    assert.equal(claimedWidth(ext, window), 101);
    // The CLI timed out at the same moment and sends its disarm second: it finds nothing...
    assert.equal(ext.DisarmEntry(TARGET, 'k-1'), false);
    // ...and the window keeps the placement it was given.
    assert.equal(ext._getState(window).instructions[0].args.w, 101);
    ext.disable();
});

test('stale arm: race, the disarm arrives before the window', () => {
    const ext = newExtension();
    ext.ArmSpawnEntry(TARGET, 'k-1', arm(101));

    assert.equal(ext.DisarmEntry(TARGET, 'k-1'), true);
    const window = windowOf(TARGET);
    assert.equal(claimedWidth(ext, window), null, 'the window is left unplaced, as after a timeout');
    ext.disable();
});

test('stale arm: ArmSpawn with two arguments (an old CLI) and DisarmSpawn behave as before', () => {
    const ext = newExtension();
    ext.ArmSpawn(TARGET, arm(101));
    ext.ArmSpawn(TARGET, arm(102));
    assert.equal(ext._armedSpawns.get(TARGET).length, 2);
    assert.equal(claimedWidth(ext, windowOf(TARGET)), 101);

    // DisarmSpawn still clears the whole queue for the key, keyed entries included.
    ext.ArmSpawnEntry(TARGET, 'k-1', arm(103));
    assert.equal(ext.DisarmSpawn(TARGET), true);
    assert.equal(ext._armedSpawns.has(TARGET), false);
    ext.disable();
});

test('stale arm: keyed arms respect the 16-entry cap, and the dropped one cannot be disarmed', () => {
    const ext = newExtension();
    for (let i = 0; i < 17; i++)
        ext.ArmSpawnEntry(TARGET, `k-${i}`, arm(200 + i));
    assert.equal(ext._armedSpawns.get(TARGET).length, 16);
    assert.equal(ext.DisarmEntry(TARGET, 'k-0'), false, 'the oldest entry was dropped by the cap');
    assert.equal(ext.DisarmEntry(TARGET, 'k-16'), true);
    ext.disable();
});

test('stale arm: invalid instructions in ArmSpawnEntry queue nothing', () => {
    const ext = newExtension();
    ext.ArmSpawnEntry(TARGET, 'k-1', '{not json');
    assert.equal(ext._armedSpawns.has(TARGET), false);
    assert.equal(ext.DisarmEntry(TARGET, 'k-1'), false);
    ext.disable();
});

test('stale arm: the 15 s expiry still removes a keyed arm nobody claimed or withdrew', async () => {
    const ext = newExtension();
    ext.ArmSpawnEntry(TARGET, 'k-1', arm(101));
    assert.equal(ext._armedSpawns.has(TARGET), true);
    // The GLib mock in gi_loader.js caps every timer at 5 ms.
    await new Promise(resolve => setTimeout(resolve, 30));
    assert.equal(ext._armedSpawns.has(TARGET), false);
    assert.equal(ext.DisarmEntry(TARGET, 'k-1'), false);
    ext.disable();
});

test('stale arm: ArmSpawnEntry with an empty target arms the wildcard like ArmSpawn does', () => {
    const ext = newExtension();
    ext.ArmSpawnEntry('', 'k-1', arm(101));
    assert.notEqual(ext._wildcardTarget, null);
    assert.equal(claimedWidth(ext, windowOf('anything')), 101);
    ext.disable();
});
