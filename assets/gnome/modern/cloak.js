/**
 * spawn-at — actor opacity cloaking and snapshot management.
 *
 * Implements hard visual cloaking (opacity 0 + Clutter offscreen redirect disable)
 * with a mandatory per-actor hard watchdog (6000ms), dynamic after-update stage
 * listener, and unconditional cleanup on disable.
 */

export class CloakManager {
    /**
     * @param {object} stage - Clutter.Stage instance (e.g. global.stage)
     * @param {object} [timers] - Optional timer overrides for testing { setTimeout, clearTimeout }
     * @param {object} [options] - Injected Clutter/Meta dependencies
     */
    constructor(stage, timers, options = {}) {
        this.stage = stage;
        this.timers = timers || {
            setTimeout: (cb, ms) => setTimeout(cb, ms),
            clearTimeout: id => clearTimeout(id),
        };
        this.Clutter = options.Clutter || (typeof globalThis !== 'undefined' && globalThis.Clutter ? globalThis.Clutter : null);
        this.cloakedActors = new Set();
        this.watchdogs = new Map();
        this.snapshots = new Map();
        this.stageListenerId = null;
    }

    /**
     * Hard-cloaks a Clutter actor to zero opacity.
     *
     * @param {object} actor - Clutter.Actor to cloak
     */
    cloak(actor) {
        if (!actor)
            return;

        this.cloakedActors.add(actor);
        this.ensureStageListener();

        // Safety watchdog: unconditionally uncloak if timeout expires
        if (!this.watchdogs.has(actor)) {
            const timerId = this.timers.setTimeout(() => {
                if (this.cloakedActors.has(actor)) {
                    if (typeof console !== 'undefined' && console.warn)
                        console.warn('[spawn-at] Watchdog deadline reached for cloaked actor; uncloaking unconditionally.');
                    this.uncloak(actor);
                }
            }, 6000);
            this.watchdogs.set(actor, timerId);
        }

        try {
            actor.remove_all_transitions?.();

            if (typeof actor.set_offscreen_redirect === 'function' && this.Clutter?.OffscreenRedirect)
                actor.set_offscreen_redirect(this.Clutter.OffscreenRedirect.NEVER);

            actor.opacity = 0;

            if (!actor._spawnAtOpacityId && typeof actor.connect === 'function') {
                actor._spawnAtOpacityId = actor.connect('notify::opacity', () => {
                    if (this.cloakedActors.has(actor) && actor.opacity !== 0) {
                        actor.remove_all_transitions?.();
                        actor.opacity = 0;
                    }
                });
            }
        } catch (_e) {}
    }

    /**
     * Restores a Clutter actor to fully visible state (opacity 255).
     *
     * @param {object} actor - Clutter.Actor to uncloak
     */
    uncloak(actor) {
        if (!actor)
            return;

        this.cloakedActors.delete(actor);
        if (this.watchdogs.has(actor)) {
            this.timers.clearTimeout(this.watchdogs.get(actor));
            this.watchdogs.delete(actor);
        }
        this.maybeDisconnectStageListener();

        try {
            if (actor._spawnAtOpacityId && typeof actor.disconnect === 'function') {
                actor.disconnect(actor._spawnAtOpacityId);
                delete actor._spawnAtOpacityId;
            }
            actor.remove_all_transitions?.();
            if (typeof actor.set_offscreen_redirect === 'function' && this.Clutter?.OffscreenRedirect)
                actor.set_offscreen_redirect(this.Clutter.OffscreenRedirect.AUTOMATIC_FOR_OPACITY);
            if (typeof actor.set_easing_duration === 'function')
                actor.set_easing_duration(0);
            actor.opacity = 255;
            if (actor.visible === false && typeof actor.show === 'function')
                actor.show();
        } catch (_e) {}
    }

    /** Connects stage after-update listener only while cloaked set is non-empty. */
    ensureStageListener() {
        if (!this.stageListenerId && this.cloakedActors.size > 0 && this.stage?.connect) {
            this.stageListenerId = this.stage.connect('after-update', () => {
                if (this.cloakedActors.size === 0) {
                    this.maybeDisconnectStageListener();
                    return;
                }
                for (const actor of [...this.cloakedActors]) {
                    if (actor.is_destroyed?.() || (actor.get_stage && !actor.get_stage())) {
                        this.uncloak(actor);
                        continue;
                    }
                    if (actor.opacity !== 0) {
                        actor.remove_all_transitions?.();
                        actor.opacity = 0;
                    }
                }
            });
        }
    }

    /** Disconnects stage after-update listener when cloaked set becomes empty. */
    maybeDisconnectStageListener() {
        if (this.cloakedActors.size === 0 && this.stageListenerId && this.stage?.disconnect) {
            this.stage.disconnect(this.stageListenerId);
            this.stageListenerId = null;
        }
    }

    /** Creates a Clutter.Clone snapshot of the actor. */
    createSnapshot(actor) {
        if (!actor || this.snapshots.has(actor) || !this.Clutter?.Clone)
            return;
        try {
            const parent = actor.get_parent?.();
            if (!parent)
                return;
            const clone = new this.Clutter.Clone({ source: actor, x: actor.x, y: actor.y });
            parent.add_child?.(clone);
            this.snapshots.set(actor, clone);
        } catch (_e) {}
    }

    /** Destroys an existing snapshot clone. */
    destroySnapshot(actor) {
        const clone = this.snapshots.get(actor);
        if (!clone)
            return;
        try {
            clone.destroy?.();
        } catch (_e) {}
        this.snapshots.delete(actor);
    }

    /** Unconditionally uncloaks all actors and tears down all listeners during disable(). */
    disable() {
        for (const actor of [...this.cloakedActors])
            this.uncloak(actor);
        for (const t of this.watchdogs.values())
            this.timers.clearTimeout(t);
        this.watchdogs.clear();

        for (const actor of [...this.snapshots.keys()])
            this.destroySnapshot(actor);
        this.snapshots.clear();

        this.maybeDisconnectStageListener();
    }
}
