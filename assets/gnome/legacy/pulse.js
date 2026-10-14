/**
 * spawn-at — VTE / terminal cursor heartbeat and focus pulse tracking (Legacy GNOME 42-44).
 */

const { GLib } = imports.gi;

const PULSE_LEAVE_IDLE_MS = 100;
const PULSE_REFOCUS_CEILING_MS = 250;
const WAKE_IDLE_MS = 120;
const WAKE_QUIET_MS = 30;
const WAKE_CEILING_MS = 300;

var FocusPulse = class FocusPulse {
    constructor(helper, commitLatch) {
        this._helper = helper;
        this._commitLatch = commitLatch;
        this._vtePidCache = new Map();
    }

    isVteCandidate(window) {
        if (!window)
            return false;

        let pid = -1;
        try { pid = window.get_pid ? window.get_pid() : -1; } catch (_e) {}
        if (pid > 0 && this._vtePidCache.has(pid))
            return this._vtePidCache.get(pid);

        let isVte = false;

        if (pid > 0) {
            try {
                const [ok, contents] = GLib.file_get_contents(`/proc/${pid}/maps`);
                if (ok) {
                    const text = new TextDecoder().decode(contents);
                    if (text.includes('libvte'))
                        isVte = true;
                }
            } catch (_e) {}

            if (!isVte) {
                try {
                    const [ok, commBytes] = GLib.file_get_contents(`/proc/${pid}/comm`);
                    if (ok) {
                        const comm = new TextDecoder().decode(commBytes).trim().toLowerCase();
                        if (comm.includes('gnome-terminal') || comm === 'terminator' ||
                            comm === 'tilix' || comm === 'guake') {
                            isVte = true;
                        }
                    }
                } catch (_e) {}
            }
        }

        if (!isVte) {
            const ids = this._getWindowIdentifiers(window).map(id => id.toLowerCase());
            const vteKeywords = ['gnome-terminal', 'terminator', 'tilix', 'guake', 'xfce4-terminal', 'vte'];
            for (const id of ids) {
                if (vteKeywords.some(kw => id.includes(kw))) {
                    isVte = true;
                    break;
                }
            }
        }

        if (pid > 0) {
            if (this._vtePidCache.size >= 128)
                this._vtePidCache.clear();
            this._vtePidCache.set(pid, isVte);
        }

        return isVte;
    }

    _getWindowIdentifiers(window) {
        const list = [];
        try {
            const wmClass = window.get_wm_class?.();
            if (wmClass) list.push(wmClass);
        } catch (_e) {}
        try {
            const appId = window.get_gtk_application_id?.() || window.get_sandboxed_app_id?.();
            if (appId) list.push(appId);
        } catch (_e) {}
        return list;
    }

    async execute(ctx) {
        const { window, actor } = ctx;
        const winId = window.get_id ? window.get_id() : -1;
        const startFrame = window.get_frame_rect ? window.get_frame_rect() : { x: 0, y: 0, width: 0, height: 0 };
        const startBuf = window.get_buffer_rect ? window.get_buffer_rect() : startFrame;

        this._helper.logTime('FOCUS_PULSE_INIT',
            `windowId=${winId} preFrame=(${startFrame.x},${startFrame.y},${startFrame.width}x${startFrame.height}) ` +
            `bufferRect=(${startBuf.x},${startBuf.y},${startBuf.width}x${startBuf.height}) ` +
            `requestedSize=(${ctx.targetW}x${ctx.targetH}) opacity=${actor?.opacity}`,
            window);

        this._helper.defocusWindow(window, 'desktop');
        this._helper.logTime('FOCUS_PULSE_DEFOCUSED', `windowId=${winId}`, window);

        const resized = await this._waitForSizeChangedOrTimeout(window, PULSE_LEAVE_IDLE_MS);
        if (this._helper.isDisabled && this._helper.isDisabled())
            return;

        if (!resized) {
            this._helper.logTime('FOCUS_PULSE_EARLY_EXIT',
                `windowId=${winId} reason="no size-changed within ${PULSE_LEAVE_IDLE_MS}ms"`,
                window);
            this._helper.focusWindow(window);
            await this._waitForWindowCondition(
                window, ['notify::appears-focused', 'focus'],
                () => Boolean(window.has_focus?.()), PULSE_REFOCUS_CEILING_MS);
            this._helper.logTime('FOCUS_PULSE_EARLY_EXIT_DONE', `windowId=${winId}`, window);
            return;
        }

        this._helper.logTime('FOCUS_PULSE_FULL_EXECUTION_START', `windowId=${winId}`, window);
        await this._commitLatch.waitForSizeQuiet(window, 0, WAKE_QUIET_MS, WAKE_CEILING_MS);
        if (this._helper.isDisabled && this._helper.isDisabled())
            return;
        this._helper.cloak(actor);

        this._helper.focusWindow(window);
        this._helper.logTime('FOCUS_PULSE_REFOCUSED', `windowId=${winId}`, window);

        const focused = await this._waitForWindowCondition(
            window, ['notify::appears-focused', 'focus'],
            () => Boolean(window.has_focus?.()), PULSE_REFOCUS_CEILING_MS);
        this._helper.logTime('FOCUS_PULSE_FOCUS_CONFIRMED', `focused=${focused}`, window);

        await this._commitLatch.waitForSizeQuiet(window, WAKE_IDLE_MS, WAKE_QUIET_MS, WAKE_CEILING_MS);

        const endFrame = window.get_frame_rect ? window.get_frame_rect() : { x: 0, y: 0, width: 0, height: 0 };
        const endBuf = window.get_buffer_rect ? window.get_buffer_rect() : endFrame;
        const deltaW = endFrame.width - startFrame.width;
        const deltaH = endFrame.height - startFrame.height;

        this._helper.logTime('FOCUS_PULSE_FULL_EXECUTION_DONE',
            `windowId=${winId} postFrame=(${endFrame.x},${endFrame.y},${endFrame.width}x${endFrame.height}) ` +
            `bufferRect=(${endBuf.x},${endBuf.y},${endBuf.width}x${endBuf.height}) ` +
            `delta=(${deltaW}x${deltaH}) deltaH=${deltaH}`,
            window);
    }

    _waitForSizeChangedOrTimeout(window, timeoutMs) {
        return new Promise(resolve => {
            let finished = false;
            let timerId = 0;
            let sigId = 0;

            const finish = ok => {
                if (finished)
                    return;
                finished = true;
                this._helper.removeTimer(timerId);
                this._helper.safeDisconnect(window, sigId);
                resolve(ok);
            };

            sigId = this._helper.tryConnect(window, 'size-changed', () => finish(true));
            timerId = this._helper.addTimer(timeoutMs, () => finish(false));
        });
    }

    _waitForWindowCondition(window, signals, predicate, ceilingMs) {
        return new Promise(resolve => {
            try {
                if (predicate()) {
                    resolve(true);
                    return;
                }
            } catch (_e) {}

            let finished = false;
            let ceiling = 0;
            const ids = [];

            const finish = ok => {
                if (finished)
                    return;
                finished = true;
                this._helper.removeTimer(ceiling);
                ids.forEach(id => this._helper.safeDisconnect(window, id));
                resolve(ok);
            };

            const check = () => {
                try {
                    if (predicate())
                        finish(true);
                } catch (_e) {}
            };

            for (const sig of signals)
                ids.push(this._helper.tryConnect(window, sig, check));
            ceiling = this._helper.addTimer(ceilingMs, () => finish(false));
        });
    }
};

if (typeof module !== 'undefined' && module.exports) {
    module.exports = { FocusPulse };
}
