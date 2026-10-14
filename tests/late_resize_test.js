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



// A window that opens at 400x300 (like the verify fixture) and whose frame the test can change
// at a chosen moment. `connect` never delivers a signal, so the reactive anchor (armed at the
// reveal) cannot rescue a resize that landed before it was armed: exactly the gap under test.
function createWindow() {
    let frameRect = { x: 50, y: 90, width: 400, height: 300 };
    const window = {
        get_frame_rect: () => ({ ...frameRect }),
        get_buffer_rect: () => ({ ...frameRect }),
        get_id: () => 201,
        get_pid: () => 7001,
        get_wm_class: () => 'verify.placement.late',
        get_client_type: () => 0,
        get_monitor: () => 0,
        get_work_area_current_monitor: () => ({ x: 0, y: 40, width: 1920, height: 1040 }),
        get_work_area_for_monitor: () => ({ x: 0, y: 40, width: 1920, height: 1040 }),
        move_resize_frame: (_u, x, y, w, h) => { frameRect = { x, y, width: w, height: h }; },
        move_frame: (_u, x, y) => { frameRect.x = x; frameRect.y = y; },
        resizeClient: (w, h) => { frameRect = { ...frameRect, width: w, height: h }; },
        connect: () => 1,
        disconnect: () => {},
        has_focus: () => true,
        raise: () => {},
        activate: () => {},
        unminimize: () => {},
        delete: () => {},
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

// Bottom-right, 16 px margins, workarea 0,40 1920x1040; no SetSize so the commit step is not involved.
const BOTTOM_RIGHT = {
    intended_w: 0, intended_h: 0,
    screen_anchor_x: 1904, screen_anchor_y: 1064,
    pivot_u: 1.0, pivot_v: 1.0, offset_x: 0, offset_y: 0,
    area: 'workarea',
    margin_top: 16, margin_bottom: 16, margin_left: 16, margin_right: 16,
    clamp: true,
};
const BATCH = JSON.stringify(['Cloak', { SetPositionAnchored: BOTTOM_RIGHT }, 'Uncloak']);

// Runs the batch on a fresh window. `onEvent(tag, count, window)` is called for every timing-log
// event and may change the window (as an app that resizes itself late would). Returns the ordered
// event tags, the per-tag counts and the final frame.
async function run(onEvent) {
    const ext = new SpawnAtExtension({ path: '/dummy' });
    ext.enable();
    const { window, actor } = createWindow();
    const tags = [];
    const counts = {};
    let atReveal = null;
    let frameWaits = 0;
    const origWait = ext._commitLatch.waitForFrames.bind(ext._commitLatch);
    ext._commitLatch.waitForFrames = (...args) => {
        frameWaits++;
        return origWait(...args);
    };
    const orig = ext.logTime.bind(ext);
    ext.logTime = (tag, extra, win) => {
        tags.push(tag);
        counts[tag] = (counts[tag] ?? 0) + 1;
        // the first event of the reveal runs in the same tick as the opacity restore
        if (tag === 'UNCLOAK_4D_REVEAL') atReveal = window.get_frame_rect();
        onEvent?.(tag, counts[tag], window);
        orig(tag, extra, win);
    };
    await ext._runBatch(window, actor, validateInstructions(BATCH));
    const frame = window.get_frame_rect();
    ext.disable();
    return { tags, counts, frame, atReveal, frameWaits };
}

const rect = f => [f.x, f.y, f.width, f.height];

test('late resize: with no resize the sequence has no LATE_RESIZE_CORRECTED and the anchor is exact', async () => {
    const { counts, frame, frameWaits } = await run();
    assert.equal(counts.LATE_RESIZE_CORRECTED, undefined);
    assert.equal(counts.REANCHOR_TRIGGERED, undefined);
    // anchored for the 400x300 it has: x = 1904 - 400, y = 1064 - 300
    assert.deepEqual(rect(frame), [1504, 764, 400, 300]);
    // cloak cost of the change when nothing resized: no extra frame wait (the one before reveal)
    assert.equal(frameWaits, 1);
});

test('late resize: a resize landing after UNCLOAK_4C is corrected before the reveal', async () => {
    const { tags, counts, frame, frameWaits } = await run((tag, _n, window) => {
        // the app answers shortly after the 4C check, while the reveal waits for a frame
        if (tag === 'UNCLOAK_4C') setTimeout(() => window.resizeClient(200, 150), 1);
    });
    // anchored for the FINAL 200x150: x = 1904 - 200, y = 1064 - 150
    assert.deepEqual(rect(frame), [1704, 914, 200, 150]);
    assert.equal(counts.LATE_RESIZE_CORRECTED, 1);
    assert.ok(
        tags.indexOf('LATE_RESIZE_CORRECTED') < tags.indexOf('UNCLOAK_4D_REVEAL'),
        'the correction must come before the reveal'
    );
    // the 4C check itself saw no change, so the original path did not fire
    assert.equal(counts.REANCHOR_TRIGGERED, undefined);
    // one correction costs one extra frame wait
    assert.equal(frameWaits, 2);
});

test('late resize: a second resize during the extra frame wait is also corrected, with one extra wait at most', async () => {
    const { counts, frame, frameWaits } = await run((tag, n, window) => {
        if (tag === 'UNCLOAK_4C') setTimeout(() => window.resizeClient(200, 150), 1);
        // the app resizes again while the corrected window waits one more frame
        if (tag === 'LATE_RESIZE_CORRECTED' && n === 1) setTimeout(() => window.resizeClient(300, 200), 1);
    });
    assert.deepEqual(rect(frame), [1604, 864, 300, 200]);
    assert.equal(counts.LATE_RESIZE_CORRECTED, 2);
    assert.equal(frameWaits, 2);
});

test('late resize: a window that never stops resizing is bounded (2 corrections) and still revealed', async () => {
    let w = 200;
    const { counts, tags, atReveal, frameWaits } = await run((tag, _n, window) => {
        if (tag === 'UNCLOAK_4C' || tag === 'LATE_RESIZE_CORRECTED') {
            w += 10;
            setTimeout(() => window.resizeClient(w, 150), 1);
        }
    });
    assert.equal(counts.LATE_RESIZE_CORRECTED, 2);
    // two corrections, but still only one extra frame wait: the second check runs in the
    // reveal's own tick
    assert.equal(frameWaits, 2);
    assert.ok(tags.includes('UNCLOAK_4D_REVEAL'), 'the window must still be revealed');
    // At the reveal the window was anchored for the size it had then: right and bottom edges on
    // the 16 px margin (1920-16, 1080-16).
    assert.equal(atReveal.x + atReveal.width, 1904);
    assert.equal(atReveal.y + atReveal.height, 1064);
});

test('late resize: a resize caught by the original 4C check is not corrected a second time', async () => {
    const { counts, frame } = await run((tag, _n, window) => {
        // the app has already resized when the 4C check runs
        if (tag === 'POSITION_SET') window.resizeClient(200, 150);
    });
    assert.equal(counts.REANCHOR_TRIGGERED, 1);
    assert.equal(counts.LATE_RESIZE_CORRECTED, undefined);
    assert.deepEqual(rect(frame), [1704, 914, 200, 150]);
});
