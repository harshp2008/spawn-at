// xflash-probe: ground truth for "was the window flashing?", read from the compositor's own paint
// loop. Loaded ONLY in the private headless Shell (tools/psh-start.sh with PROBE=1). It changes
// nothing about windows: it connects signals, reads getters and writes JSON lines to
// $PSH_SB/probe/probe.jsonl (and PNG files to $PSH_SB/probe/px/ when pixels are on).
//
// For every NORMAL window, from window-created until PROBE_AFTER_REVEAL_MS after the first stage
// paint in which the window is visible, it writes one `f` line per stage `after-paint` and one `e`
// line per window/actor event. Times are ms on CLOCK_MONOTONIC, the clock spawn-at's own log calls
// mono_ms. "Visible" = the actor and every ancestor have `visible` set and the effective paint
// opacity is above 0 (is_mapped() reads false at after-paint in the headless Shell, see README).
//
// Config: $PSH_SB/probe/config.json, read at every window-created:
//   { "pixels": "none" | "png" }
// `png`: for every `f` line with vis=true an idle callback (never inside the paint signal) asks
// Shell.Screenshot.screenshot_area for the window's frame rect, into a memory stream, and writes
// the PNG to px/<window>-<frame no>.png. A `px` line records the state at the moment of the call.
// Clutter.Stage.read_pixels is NOT used: its typelib return array has no length and GJS aborts the
// Shell on it (docs/evidence/x11-flash-03/crash-1-read-pixels.md).
import GLib from 'gi://GLib';
import Gio from 'gi://Gio';
import Meta from 'gi://Meta';
import Shell from 'gi://Shell';

import {Extension} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';

const PROBE_AFTER_REVEAL_MS = 300;
const PROBE_HARD_CAP_MS = 8000;
const MAX_ROWS_PER_WINDOW = 600;
const MAX_SHOTS_PER_WINDOW = 40;
const FLUSH_MS = 150;

Gio._promisify(Shell.Screenshot.prototype, 'screenshot_area', 'screenshot_area_finish');

const nowMs = () => GLib.get_monotonic_time() / 1000;
const r3 = v => Math.round(v * 1000) / 1000;
const rect = r => (r ? [r.x, r.y, r.width, r.height] : null);

function textureSize(actor) {
    try {
        const m = actor.get_texture().get_texture();
        return [m.get_width(), m.get_height()];
    } catch (e) {
        return null;
    }
}

export default class XflashProbe extends Extension {
    enable() {
        const sb = GLib.getenv('PSH_SB');
        this._dir = sb ? `${sb}/probe` : null;
        if (!this._dir)
            return;
        GLib.mkdir_with_parents(`${this._dir}/px`, 0o755);
        this._outPath = `${this._dir}/probe.jsonl`;
        this._buf = [];
        this._trackers = new Map();
        this._seq = 0;
        this._frameNo = 0;

        this._flushId = GLib.timeout_add(GLib.PRIORITY_LOW, FLUSH_MS, () => {
            this._flush();
            return GLib.SOURCE_CONTINUE;
        });
        this._createdId = global.display.connect('window-created', (_d, win) => this._onCreated(win));
        this._paintId = global.stage.connect('after-paint', () => this._onPaint());
        this._emit({k: 'probe-start', t: r3(nowMs()),
            stage_mapped: global.stage.is_mapped(), window_group_visible: global.window_group.visible});

        // The private Shell starts in the Overview (as a fresh GNOME session does), which hides
        // Meta.WindowGroup: windows would then be drawn only as Overview clones. Close it, so the
        // window actors are what gets painted, as on a desktop where a window is being placed.
        let tries = 0;
        this._desktopId = GLib.timeout_add(GLib.PRIORITY_DEFAULT, 500, () => {
            tries++;
            try {
                if (Main.overview.visible)
                    Main.overview.hide();
            } catch (e) {
                console.error(`[xflash-probe] overview.hide: ${e}`);
            }
            const ok = global.window_group.visible && !Main.overview.visible;
            if (ok || tries >= 40) {
                this._emit({k: 'desktop', t: r3(nowMs()), tries, ok, overview: Main.overview.visible,
                    window_group_visible: global.window_group.visible});
                this._desktopId = 0;
                return GLib.SOURCE_REMOVE;
            }
            return GLib.SOURCE_CONTINUE;
        });
    }

    disable() {
        if (this._createdId)
            global.display.disconnect(this._createdId);
        if (this._paintId)
            global.stage.disconnect(this._paintId);
        if (this._flushId)
            GLib.source_remove(this._flushId);
        if (this._desktopId)
            GLib.source_remove(this._desktopId);
        for (const tr of [...(this._trackers?.values() ?? [])])
            this._finish(tr, 'disabled');
        this._flush();
        this._createdId = this._paintId = this._flushId = this._desktopId = 0;
        this._trackers = null;
    }

    _emit(obj) {
        this._buf?.push(JSON.stringify(obj));
    }

    _flush() {
        if (!this._buf || this._buf.length === 0)
            return;
        const text = `${this._buf.join('\n')}\n`;
        this._buf = [];
        try {
            const f = Gio.File.new_for_path(this._outPath);
            const s = f.append_to(Gio.FileCreateFlags.NONE, null);
            s.write_all(new TextEncoder().encode(text), null);
            s.close(null);
        } catch (e) {
            console.error(`[xflash-probe] flush failed: ${e}`);
        }
    }

    _readConfig() {
        try {
            const [ok, bytes] = GLib.file_get_contents(`${this._dir}/config.json`);
            if (ok)
                return JSON.parse(new TextDecoder().decode(bytes));
        } catch (_e) { /* no config: defaults */ }
        return {};
    }

    _onCreated(win) {
        let type;
        try { type = win.get_window_type(); } catch (_e) { return; }
        if (type !== Meta.WindowType.NORMAL)
            return;
        const id = win.get_id();
        const cfg = this._readConfig();
        const tr = {
            win, id, cfg,
            seq: ++this._seq,
            createdAt: nowMs(),
            firstVisibleAt: null,
            rows: 0,
            shots: 0,
            ids: [],
            actorIds: [],
            actor: null,
            blocker: '',
            done: false,
        };
        this._trackers.set(id, tr);
        let cls = '', title = '', client = null;
        try { cls = win.get_wm_class() ?? ''; title = win.get_title() ?? ''; client = win.get_client_type(); } catch (_e) { /* ok */ }
        this._emit({k: 'win', w: id, t: r3(tr.createdAt), seq: tr.seq, cls, title, client, cfg});
        this._hookWindow(tr);
        tr.endTimer = GLib.timeout_add(GLib.PRIORITY_DEFAULT, PROBE_HARD_CAP_MS, () => {
            tr.endTimer = 0;
            this._finish(tr, 'hard-cap');
            return GLib.SOURCE_REMOVE;
        });
    }

    _actor(tr) {
        if (!tr.actor && !tr.done) {
            try { tr.actor = tr.win.get_compositor_private(); } catch (_e) { tr.actor = null; }
            if (tr.actor)
                this._hookActor(tr);
        }
        return tr.actor;
    }

    _snap(tr) {
        const out = {fr: rect(tr.win.get_frame_rect()), bf: rect(tr.win.get_buffer_rect?.() ?? null)};
        const a = tr.actor;
        if (a) {
            out.op = a.opacity;
            out.shown = a.visible;
            try { out.pop = a.get_paint_opacity(); } catch (_e) { /* actor gone */ }
            out.tex = textureSize(a);
        }
        return out;
    }

    _event(tr, ev, extra = {}) {
        if (tr.done)
            return;
        this._emit({k: 'e', w: tr.id, t: r3(nowMs()), ev, ...this._snap(tr), ...extra});
    }

    _hookWindow(tr) {
        const w = tr.win;
        const c = (sig, ev) => {
            try { tr.ids.push(w.connect(sig, () => this._event(tr, ev))); } catch (_e) { /* signal absent */ }
        };
        c('size-changed', 'win-size-changed');
        c('position-changed', 'win-position-changed');
        try {
            tr.ids.push(w.connect('unmanaged', () => {
                // The actor is disposed by now: read nothing from it.
                tr.actor = null;
                this._emit({k: 'e', w: tr.id, t: r3(nowMs()), ev: 'win-unmanaged'});
                this._finish(tr, 'unmanaged');
            }));
        } catch (_e) { /* signal absent */ }
        this._actor(tr);
        this._event(tr, 'created', {actor: Boolean(tr.actor)});
    }

    _hookActor(tr) {
        const a = tr.actor;
        const c = (sig, ev) => {
            try { tr.actorIds.push(a.connect(sig, () => this._event(tr, ev))); } catch (_e) { /* signal absent */ }
        };
        c('notify::opacity', 'actor-opacity');
        c('notify::visible', 'actor-visible');
        c('first-frame', 'first-frame');
        c('damaged', 'damaged');
        this._event(tr, 'actor-found');
    }

    _isVisible(tr) {
        const a = tr.actor;
        if (!a)
            return false;
        // Not is_mapped(): at after-paint the window actor reads mapped=false in the headless Shell
        // while it is shown and painted (seen in the first xterm run), so visible means the actor and
        // every ancestor have `visible` set and the effective (ancestor-multiplied) opacity is above 0.
        try {
            for (let p = a; p; p = p.get_parent()) {
                if (!p.visible) {
                    tr.blocker = p === a ? 'actor' : p.constructor.name;
                    return false;
                }
            }
            tr.blocker = a.get_paint_opacity() > 0 ? '' : 'opacity0';
            return a.get_paint_opacity() > 0;
        } catch (_e) {
            return false;
        }
    }

    _onPaint() {
        if (!this._trackers || this._trackers.size === 0)
            return;
        const t = nowMs();
        const n = ++this._frameNo;
        for (const tr of [...this._trackers.values()]) {
            if (tr.done)
                continue;
            this._actor(tr);
            const vis = this._isVisible(tr);
            if (vis && tr.firstVisibleAt === null) {
                tr.firstVisibleAt = t;
                if (tr.endTimer)
                    GLib.source_remove(tr.endTimer);
                tr.endTimer = GLib.timeout_add(GLib.PRIORITY_DEFAULT, PROBE_AFTER_REVEAL_MS + 40, () => {
                    tr.endTimer = 0;
                    this._finish(tr, 'after-reveal');
                    return GLib.SOURCE_REMOVE;
                });
            }
            if (tr.firstVisibleAt !== null && t - tr.firstVisibleAt > PROBE_AFTER_REVEAL_MS) {
                this._finish(tr, 'after-reveal');
                continue;
            }
            if (tr.rows++ >= MAX_ROWS_PER_WINDOW)
                continue;
            const row = {k: 'f', w: tr.id, t: r3(t), n, ...this._snap(tr), vis};
            if (!vis && tr.blocker)
                row.blk = tr.blocker;
            this._emit(row);
            if (vis && tr.cfg.pixels === 'png' && tr.shots < MAX_SHOTS_PER_WINDOW) {
                tr.shots++;
                // Not inside the paint signal: an offscreen paint from after-paint could re-enter the
                // paint machinery. The idle runs in the same main-loop turn, before the next frame.
                GLib.idle_add(GLib.PRIORITY_DEFAULT, () => {
                    this._shoot(tr, n);
                    return GLib.SOURCE_REMOVE;
                });
            }
        }
    }

    async _shoot(tr, n) {
        if (tr.done || !tr.actor)
            return;
        const tReq = nowMs();
        const snap = this._snap(tr);
        const fr = tr.win.get_frame_rect();
        const x = Math.max(0, fr.x), y = Math.max(0, fr.y);
        const w = Math.min(fr.width, global.stage.width - x), h = Math.min(fr.height, global.stage.height - y);
        if (w <= 0 || h <= 0) {
            this._emit({k: 'px', w: tr.id, n, t_req: r3(tReq), err: 'empty-rect', ...snap});
            return;
        }
        const file = `${this._dir}/px/${tr.id}-${n}.png`;
        const line = {k: 'px', w: tr.id, n, t_req: r3(tReq), rect: [x, y, w, h], file: `px/${tr.id}-${n}.png`, ...snap};
        try {
            const stream = Gio.MemoryOutputStream.new_resizable();
            await new Shell.Screenshot().screenshot_area(x, y, w, h, stream);
            stream.close(null);
            const bytes = stream.steal_as_bytes();
            line.t_done = r3(nowMs());
            line.bytes = bytes.get_size();
            Gio.File.new_for_path(file).replace_contents_bytes_async(bytes, null, false,
                Gio.FileCreateFlags.NONE, null, null);
        } catch (e) {
            line.err = String(e).slice(0, 160);
        }
        this._emit(line);
    }

    _finish(tr, reason) {
        if (tr.done)
            return;
        this._emit({k: 'end', w: tr.id, t: r3(nowMs()), reason, rows: tr.rows, shots: tr.shots,
            first_visible: tr.firstVisibleAt === null ? null : r3(tr.firstVisibleAt)});
        tr.done = true;
        if (tr.endTimer)
            GLib.source_remove(tr.endTimer);
        tr.endTimer = 0;
        for (const id of tr.ids) { try { tr.win.disconnect(id); } catch (_e) { /* gone */ } }
        for (const id of tr.actorIds) { try { tr.actor?.disconnect(id); } catch (_e) { /* gone */ } }
        this._trackers?.delete(tr.id);
        this._flush();
    }
}
