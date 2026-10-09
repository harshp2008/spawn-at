import test from 'node:test';
import assert from 'node:assert/strict';

/**
 * Mock Clutter.Actor for unit testing cloak mechanics and invariants.
 */
class MockActor {
    constructor() {
        this.opacity = 255;
        this.visible = true;
        this._destroyed = false;
        this._listeners = new Map();
        this._stage = {};
    }

    remove_all_transitions() {}
    set_offscreen_redirect() {}
    set_easing_duration() {}
    show() { this.visible = true; }
    is_destroyed() { return this._destroyed; }
    get_stage() { return this._destroyed ? null : this._stage; }

    connect(signal, cb) {
        if (!this._listeners.has(signal))
            this._listeners.set(signal, new Set());
        this._listeners.get(signal).add(cb);
        return cb;
    }

    disconnect(id) {
        for (const set of this._listeners.values())
            set.delete(id);
    }
}

class MockStage {
    constructor() {
        this.listeners = new Set();
    }
    connect(signal, cb) {
        this.listeners.add(cb);
        return cb;
    }
    disconnect(cb) {
        this.listeners.delete(cb);
    }
    emitAfterUpdate() {
        for (const cb of [...this.listeners])
            cb();
    }
}

import { CloakManager } from '../assets/gnome/cloak.js';

test('cloak: standard cloak and uncloak flow', () => {
    const stage = new MockStage();
    const cm = new CloakManager(stage);
    const actor = new MockActor();

    assert.equal(actor.opacity, 255);
    cm.cloak(actor);
    assert.equal(actor.opacity, 0);
    assert.equal(cm.stageListenerId !== null, true);

    cm.uncloak(actor);
    assert.equal(actor.opacity, 255);
    assert.equal(cm.stageListenerId, null);
});

test('cloak: watchdog uncloaks unconditionally on timeout', () => {
    let triggeredCb = null;
    const mockTimers = {
        setTimeout: (cb, ms) => {
            triggeredCb = cb;
            return 42;
        },
        clearTimeout: () => {},
    };

    const stage = new MockStage();
    const cm = new CloakManager(stage, mockTimers);
    const actor = new MockActor();

    cm.cloak(actor);
    assert.equal(actor.opacity, 0);

    // Trigger watchdog callback
    assert.equal(typeof triggeredCb, 'function');
    triggeredCb();

    assert.equal(actor.opacity, 255);
    assert.equal(cm.cloakedActors.size, 0);
    assert.equal(cm.stageListenerId, null);
});

test('cloak: disable() unconditionally uncloaks all cloaked actors', () => {
    const stage = new MockStage();
    const cm = new CloakManager(stage);
    const actor1 = new MockActor();
    const actor2 = new MockActor();

    cm.cloak(actor1);
    cm.cloak(actor2);
    assert.equal(actor1.opacity, 0);
    assert.equal(actor2.opacity, 0);

    cm.disable();
    assert.equal(actor1.opacity, 255);
    assert.equal(actor2.opacity, 255);
    assert.equal(cm.cloakedActors.size, 0);
    assert.equal(cm.stageListenerId, null);
});

test('cloak: destroyed actor during after-update is pruned and uncloaked', () => {
    const stage = new MockStage();
    const cm = new CloakManager(stage);
    const actor = new MockActor();

    cm.cloak(actor);
    assert.equal(cm.cloakedActors.size, 1);

    // Simulate actor destruction
    actor._destroyed = true;
    stage.emitAfterUpdate();

    assert.equal(cm.cloakedActors.size, 0);
    assert.equal(cm.stageListenerId, null);
});

test('cloak: batch error finally block guarantees uncloak', () => {
    const stage = new MockStage();
    const cm = new CloakManager(stage);
    const actor = new MockActor();

    const ctx = { revealed: false };
    try {
        try {
            cm.cloak(actor);
            throw new Error('Simulated D-Bus or Mutter failure');
        } finally {
            if (!ctx.revealed)
                cm.uncloak(actor);
        }
    } catch (_expected) {}

    assert.equal(actor.opacity, 255);
    assert.equal(cm.cloakedActors.size, 0);
});
