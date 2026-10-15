// Node tests for the extension's DefocusWindow and SetWindowState D-Bus handlers, and for the
// active-workspace limit of _findWindow that both rely on (audit proposal P3, gaps 1, 2 and 14 in
// docs/TEST_COVERAGE_AUDIT.md). The extension is loaded with mocked GNOME objects exactly like
// tests/dispatcher_test.js; nothing here touches a real shell.
import test from 'node:test';
import assert from 'node:assert/strict';
import { register } from 'node:module';

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
    get_current_time: () => 12345,
    get_pointer: () => [500, 500],
};

const { default: SpawnAtExtension } = await import('../assets/gnome/modern/extension.js');

// A window that appends "<name>:<call>" to the shared `log` for every call the handlers can make.
function makeWindow(log, name, id, props = {}) {
    const w = {
        _name: name,
        minimized: false,
        skip_taskbar: false,
        _maximized: false,
        get_id: () => id,
        get_pid: () => 9000 + id,
        get_wm_class: () => props.wmClass ?? `class-${name}`,
        get_gtk_application_id: () => props.appId ?? '',
        get_compositor_private: () => null,
        is_maximized: () => w._maximized,
        raise: () => log.push(`${name}:raise`),
        activate: () => log.push(`${name}:activate`),
        maximize: () => { w._maximized = true; log.push(`${name}:maximize`); },
        unmaximize: () => { w._maximized = false; log.push(`${name}:unmaximize`); },
        minimize: () => { w.minimized = true; log.push(`${name}:minimize`); },
        unminimize: () => { w.minimized = false; log.push(`${name}:unminimize`); },
        connect: () => 1,
        disconnect: () => {},
        ...props.extra,
    };
    return w;
}

// Runs `fn(ext, log)` with global.display.get_tab_list returning `windows` for every workspace,
// then restores the global mocks and disables the extension.
function withExtension(windows, fn) {
    const log = [];
    const display = globalThis.global.display;
    const stage = globalThis.global.stage;
    const prev = {
        tab: display.get_tab_list,
        unset: display.unset_input_focus,
        key: stage.set_key_focus,
    };
    display.get_tab_list = () => windows(log);
    display.unset_input_focus = (t) => log.push(`unset_input_focus(${t})`);
    stage.set_key_focus = (a) => log.push(a ? 'set_key_focus(dummy)' : 'set_key_focus(null)');
    const ext = new SpawnAtExtension({ path: '/dummy' });
    ext.enable();
    try {
        return fn(ext, log);
    } finally {
        ext.disable();
        display.get_tab_list = prev.tab;
        display.unset_input_focus = prev.unset;
        stage.set_key_focus = prev.key;
    }
}

// ---------------------------------------------------------------------------
// DefocusWindow
// ---------------------------------------------------------------------------

test('DefocusWindow desktop mode: clears input focus and key focus, raises and activates nothing', () => {
    let wins;
    withExtension((log) => (wins ??= [makeWindow(log, 'A', 101), makeWindow(log, 'B', 102)]), (ext, log) => {
        assert.ok(ext._dummy, 'the extension keeps a dummy actor to park key focus on');
        const res = ext.DefocusWindow('101', 'desktop', '');
        assert.equal(res, true);
        assert.deepEqual(log, ['unset_input_focus(12345)', 'set_key_focus(dummy)']);
    });
});

test('DefocusWindow window mode: raises then activates the named destination, not the source', () => {
    let wins;
    withExtension((log) => (wins ??= [
        makeWindow(log, 'A', 101),
        makeWindow(log, 'B', 102, { wmClass: 'org.gnome.Terminal' }),
        makeWindow(log, 'C', 103),
    ]), (ext, log) => {
        const res = ext.DefocusWindow('101', 'window', 'org.gnome.Terminal');
        assert.equal(res, true);
        assert.deepEqual(log, ['B:raise', 'B:activate']);
    });
});

test('DefocusWindow window mode: destination can be a window id', () => {
    let wins;
    withExtension((log) => (wins ??= [makeWindow(log, 'A', 101), makeWindow(log, 'B', 102)]), (ext, log) => {
        assert.equal(ext.DefocusWindow('101', 'window', '102'), true);
        assert.deepEqual(log, ['B:raise', 'B:activate']);
    });
});

test('DefocusWindow window mode: unknown destination falls back to the most recently used other window', () => {
    let wins;
    withExtension((log) => (wins ??= [makeWindow(log, 'A', 101), makeWindow(log, 'B', 102)]), (ext, log) => {
        assert.equal(ext.DefocusWindow('101', 'window', 'no-such-class'), true);
        assert.deepEqual(log, ['B:raise', 'B:activate']);
    });
});

test('DefocusWindow window mode with an empty destination behaves like the MRU fallback', () => {
    let wins;
    withExtension((log) => (wins ??= [makeWindow(log, 'A', 101), makeWindow(log, 'B', 102)]), (ext, log) => {
        assert.equal(ext.DefocusWindow('101', 'window', ''), true);
        assert.deepEqual(log, ['B:raise', 'B:activate']);
    });
});

test('DefocusWindow mru mode: skips the source, minimized windows and skip_taskbar windows', () => {
    let wins;
    withExtension((log) => (wins ??= [
        makeWindow(log, 'A', 101),
        makeWindow(log, 'MIN', 102, { extra: { minimized: true } }),
        makeWindow(log, 'SKIP', 103, { extra: { skip_taskbar: true } }),
        makeWindow(log, 'D', 104),
    ]), (ext, log) => {
        assert.equal(ext.DefocusWindow('101', 'mru', ''), true);
        assert.deepEqual(log, ['D:raise', 'D:activate']);
    });
});

test('DefocusWindow with no other usable window falls back to clearing input focus', () => {
    let wins;
    withExtension((log) => (wins ??= [
        makeWindow(log, 'A', 101),
        makeWindow(log, 'MIN', 102, { extra: { minimized: true } }),
    ]), (ext, log) => {
        assert.equal(ext.DefocusWindow('101', 'mru', ''), true);
        assert.deepEqual(log, ['unset_input_focus(12345)', 'set_key_focus(dummy)']);
    });
});

test('DefocusWindow of an unknown target returns false and changes nothing', () => {
    let wins;
    withExtension((log) => (wins ??= [makeWindow(log, 'A', 101)]), (ext, log) => {
        assert.equal(ext.DefocusWindow('999', 'desktop', ''), false);
        assert.deepEqual(log, []);
    });
});

// ---------------------------------------------------------------------------
// SetWindowState
// ---------------------------------------------------------------------------

test('SetWindowState maximize and unmaximize call the window once each and return true', () => {
    let wins;
    withExtension((log) => (wins ??= [makeWindow(log, 'A', 101)]), (ext, log) => {
        assert.equal(ext.SetWindowState('101', 'maximize'), true);
        assert.equal(ext.SetWindowState('101', 'unmaximize'), true);
        assert.deepEqual(log, ['A:maximize', 'A:unmaximize']);
    });
});

test('SetWindowState minimize and unminimize call the window once each and return true', () => {
    let wins;
    withExtension((log) => (wins ??= [makeWindow(log, 'A', 101)]), (ext, log) => {
        assert.equal(ext.SetWindowState('101', 'minimize'), true);
        assert.equal(ext.SetWindowState('101', 'unminimize'), true);
        assert.deepEqual(log, ['A:minimize', 'A:unminimize']);
    });
});

test('SetWindowState restore on a minimized, maximized window unminimizes first, then unmaximizes', () => {
    let wins;
    withExtension((log) => (wins ??= [makeWindow(log, 'A', 101, { extra: { minimized: true, _maximized: true } })]), (ext, log) => {
        assert.equal(ext.SetWindowState('101', 'restore'), true);
        assert.deepEqual(log, ['A:unminimize', 'A:unmaximize']);
    });
});

test('SetWindowState restore on a maximized window only unmaximizes', () => {
    let wins;
    withExtension((log) => (wins ??= [makeWindow(log, 'A', 101, { extra: { _maximized: true } })]), (ext, log) => {
        assert.equal(ext.SetWindowState('101', 'restore'), true);
        assert.deepEqual(log, ['A:unmaximize']);
    });
});

test('SetWindowState restore on a minimized window only unminimizes', () => {
    let wins;
    withExtension((log) => (wins ??= [makeWindow(log, 'A', 101, { extra: { minimized: true } })]), (ext, log) => {
        assert.equal(ext.SetWindowState('101', 'restore'), true);
        assert.deepEqual(log, ['A:unminimize']);
    });
});

test('SetWindowState restore on a normal window does nothing but still returns true', () => {
    let wins;
    withExtension((log) => (wins ??= [makeWindow(log, 'A', 101)]), (ext, log) => {
        assert.equal(ext.SetWindowState('101', 'restore'), true);
        assert.deepEqual(log, []);
    });
});

test('SetWindowState with an unknown state string returns false and changes nothing', () => {
    let wins;
    withExtension((log) => (wins ??= [makeWindow(log, 'A', 101)]), (ext, log) => {
        assert.equal(ext.SetWindowState('101', 'fullscreen'), false);
        assert.equal(ext.SetWindowState('101', ''), false);
        assert.deepEqual(log, []);
    });
});

test('SetWindowState of an unknown target returns false and changes nothing', () => {
    let wins;
    withExtension((log) => (wins ??= [makeWindow(log, 'A', 101)]), (ext, log) => {
        assert.equal(ext.SetWindowState('999', 'maximize'), false);
        assert.deepEqual(log, []);
    });
});

test('SetWindowState resolves the target by pid and by wm class as well as by id', () => {
    let wins;
    withExtension((log) => (wins ??= [makeWindow(log, 'A', 101, { wmClass: 'org.gnome.TextEditor' })]), (ext, log) => {
        assert.equal(ext.SetWindowState('9101', 'minimize'), true, 'pid 9000 + id');
        assert.equal(ext.SetWindowState('org.gnome.TextEditor', 'unminimize'), true, 'wm class');
        assert.deepEqual(log, ['A:minimize', 'A:unminimize']);
    });
});

// ---------------------------------------------------------------------------
// _findWindow sees only the active workspace (audit gap 14)
// ---------------------------------------------------------------------------

test('FocusWindow, DefocusWindow and SetWindowState return false for a window on another workspace', () => {
    const log = [];
    const onWs1 = makeWindow(log, 'ONE', 101);
    const onWs2 = makeWindow(log, 'TWO', 202);
    const ws1 = { name: 'ws1', get_work_area_for_monitor: () => ({ x: 0, y: 40, width: 1920, height: 1040 }) };
    const ws2 = { name: 'ws2', get_work_area_for_monitor: () => ({ x: 0, y: 40, width: 1920, height: 1040 }) };
    let active = ws1;

    const display = globalThis.global.display;
    const prev = { tab: display.get_tab_list, mgr: display.get_workspace_manager, unset: display.unset_input_focus };
    display.get_workspace_manager = () => ({ get_active_workspace: () => active });
    display.get_tab_list = (_type, ws) => (ws === ws1 ? [onWs1] : [onWs2]);
    display.unset_input_focus = () => log.push('unset_input_focus');
    const ext = new SpawnAtExtension({ path: '/dummy' });
    ext.enable();
    try {
        assert.equal(ext.FocusWindow('202'), false);
        assert.equal(ext.DefocusWindow('202', 'desktop', ''), false);
        assert.equal(ext.SetWindowState('202', 'maximize'), false);
        assert.deepEqual(log, [], 'no window method and no focus clearing for the other workspace');

        assert.equal(ext.SetWindowState('101', 'minimize'), true, 'the active workspace window is found');

        active = ws2;
        assert.equal(ext.SetWindowState('202', 'minimize'), true, 'found once its workspace is active');
        assert.equal(ext.SetWindowState('101', 'minimize'), false, 'and the first one is now out of reach');
        assert.deepEqual(log, ['ONE:minimize', 'TWO:minimize']);
    } finally {
        ext.disable();
        display.get_tab_list = prev.tab;
        display.get_workspace_manager = prev.mgr;
        display.unset_input_focus = prev.unset;
    }
});
