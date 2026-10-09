import test from 'node:test';
import assert from 'node:assert/strict';

import { computeAnchoredPosition } from '../assets/gnome/anchor.js';

/**
 * Pure instruction normalization logic extracted from extension.esm.js.
 */
function normalizeInstruction(inst) {
    if (typeof inst === 'string')
        return { name: inst, args: {} };
    if (inst && typeof inst === 'object') {
        const name = Object.keys(inst)[0];
        if (name)
            return { name, args: inst[name] ?? {} };
    }
    return null;
}

test('extension characterization: anchor math center on 1920x1080', () => {
    const bounds = { x: 0, y: 40, width: 1920, height: 1040 };
    const frame = { width: 800, height: 600 };
    const payload = {
        screen_anchor_x: 960,
        screen_anchor_y: 560,
        pivot_u: 0.5,
        pivot_v: 0.5,
        offset_x: 0,
        offset_y: 0,
        margin_top: 16,
        margin_bottom: 16,
        margin_left: 16,
        margin_right: 16,
    };

    const res = computeAnchoredPosition(bounds, frame, [0, 0], payload);
    assert.equal(res.rawX, 960 - 400); // 560
    assert.equal(res.rawY, 560 - 300); // 260
    assert.equal(res.x, 560);
    assert.equal(res.y, 260);
    assert.equal(res.oversized, false);
});

test('extension characterization: anchor math clamping out-of-bounds target', () => {
    const bounds = { x: 0, y: 0, width: 1920, height: 1080 };
    const frame = { width: 400, height: 300 };
    const payload = {
        screen_anchor_x: 2500, // far off-screen right
        screen_anchor_y: -500, // far off-screen top
        pivot_u: 0,
        pivot_v: 0,
        offset_x: 0,
        offset_y: 0,
        margin_top: 20,
        margin_bottom: 20,
        margin_left: 20,
        margin_right: 20,
    };

    const res = computeAnchoredPosition(bounds, frame, [0, 0], payload);
    assert.equal(res.rawX, 2500);
    assert.equal(res.rawY, -500);
    assert.equal(res.x, 1920 - 20 - 400); // 1500 (clamped to maxX)
    assert.equal(res.y, 20);              // 20 (clamped to minY)
});

test('extension: clamp false bypasses bounding constraints', () => {
    const bounds = { x: 0, y: 0, width: 1920, height: 1080 };
    const frame = { width: 400, height: 300 };
    const payload = {
        screen_anchor_x: 2500,
        screen_anchor_y: -500,
        pivot_u: 0,
        pivot_v: 0,
        offset_x: 0,
        offset_y: 0,
        margin_top: 20,
        margin_bottom: 20,
        margin_left: 20,
        margin_right: 20,
        clamp: false,
    };

    const res = computeAnchoredPosition(bounds, frame, [0, 0], payload);
    assert.equal(res.rawX, 2500);
    assert.equal(res.rawY, -500);
    assert.equal(res.x, 2500); // not clamped
    assert.equal(res.y, -500); // not clamped
});

test('extension characterization: oversized window top-left priority', () => {
    const bounds = { x: 100, y: 100, width: 500, height: 400 };
    const frame = { width: 600, height: 500 }; // larger than bounds
    const payload = {
        screen_anchor_x: 350,
        screen_anchor_y: 300,
        pivot_u: 0.5,
        pivot_v: 0.5,
        offset_x: 0,
        offset_y: 0,
        margin_top: 10,
        margin_bottom: 10,
        margin_left: 10,
        margin_right: 10,
    };

    const res = computeAnchoredPosition(bounds, frame, [0, 0], payload);
    assert.equal(res.oversized, true);
    // When oversized, maxX < minX -> maxX = minX = bounds.x + ml = 110
    assert.equal(res.range.minX, 110);
    assert.equal(res.range.maxX, 110);
    assert.equal(res.range.minY, 110);
    assert.equal(res.range.maxY, 110);
    assert.equal(res.x, 110);
    assert.equal(res.y, 110);
});

test('extension characterization: normalizeInstruction variants', () => {
    assert.deepEqual(normalizeInstruction('Snapshot'), { name: 'Snapshot', args: {} });
    assert.deepEqual(normalizeInstruction('Cloak'), { name: 'Cloak', args: {} });
    assert.deepEqual(normalizeInstruction('Uncloak'), { name: 'Uncloak', args: {} });

    assert.deepEqual(
        normalizeInstruction({ SetSize: { w: 1024, h: 768 } }),
        { name: 'SetSize', args: { w: 1024, h: 768 } }
    );

    assert.deepEqual(
        normalizeInstruction({ WaitForCommit: { timeout_ms: 300 } }),
        { name: 'WaitForCommit', args: { timeout_ms: 300 } }
    );

    assert.equal(normalizeInstruction(null), null);
    assert.equal(normalizeInstruction(undefined), null);
    assert.equal(normalizeInstruction(42), null);
    assert.equal(normalizeInstruction({}), null);
});

import { validateInstructions, validateInstruction } from '../assets/gnome/validator.js';

test('validator: valid full golden instruction pipeline', () => {
    const raw = [
        'Cloak',
        { SetSize: { w: 800, h: 600 } },
        { WaitForCommit: { timeout_ms: 300 } },
        {
            SetPositionAnchored: {
                screen_anchor_x: 1895,
                screen_anchor_y: 60,
                pivot_u: 1.0,
                pivot_v: 0.0,
                offset_x: 10,
                offset_y: -5,
                area: 'workarea',
                margin_top: 20,
                margin_bottom: 0,
                margin_left: 0,
                margin_right: 25,
                clamp: true,
            },
        },
        'Uncloak',
    ];

    const validated = validateInstructions(raw);
    assert.equal(validated.length, 5);
    assert.equal(validated[0].name, 'Cloak');
    assert.equal(validated[1].name, 'SetSize');
    assert.equal(validated[1].args.w, 800);
    assert.equal(validated[1].args.h, 600);
    assert.equal(validated[2].name, 'WaitForCommit');
    assert.equal(validated[2].args.timeout_ms, 300);
    assert.equal(validated[3].name, 'SetPositionAnchored');
    assert.equal(validated[3].args.screen_anchor_x, 1895);
    assert.equal(validated[3].args.clamp, true);
    assert.equal(validated[4].name, 'Uncloak');
});

test('validator: JSON string input support', () => {
    const jsonStr = JSON.stringify(['Snapshot', 'Cloak', 'Uncloak', 'DestroySnapshot']);
    const validated = validateInstructions(jsonStr);
    assert.equal(validated.length, 4);
});

test('validator: rejects malformed and adversarial inputs', () => {
    // Non-array
    assert.throws(() => validateInstructions(null), /must be an Array/);
    assert.throws(() => validateInstructions({}), /must be an Array/);
    assert.throws(() => validateInstructions(''), /Unexpected end of JSON input|must be an Array/);
    assert.throws(() => validateInstructions([]), /cannot be empty/);

    // Unknown operation
    assert.throws(() => validateInstructions(['UnknownOp']), /Unknown instruction name/);
    assert.throws(() => validateInstructions([{ MaliciousOp: {} }]), /Unknown instruction name/);

    // Missing arguments on parameterized ops
    assert.throws(() => validateInstructions(['SetSize']), /requires an arguments object/);
    assert.throws(() => validateInstructions(['WaitForCommit']), /requires an arguments object/);
    assert.throws(() => validateInstructions(['SetPositionAnchored']), /requires an arguments object/);

    // Invalid dimensions
    assert.throws(() => validateInstructions([{ SetSize: { w: -10, h: 500 } }]), /invalid dimensions/);
    assert.throws(() => validateInstructions([{ SetSize: { w: NaN, h: 500 } }]), /invalid dimensions/);
    assert.throws(() => validateInstructions([{ SetSize: { w: Infinity, h: 500 } }]), /invalid dimensions/);

    // Invalid timeouts
    assert.throws(() => validateInstructions([{ WaitForCommit: { timeout_ms: -5 } }]), /invalid timeout_ms/);
    assert.throws(() => validateInstructions([{ WaitForCommit: { timeout_ms: 100_000 } }]), /invalid timeout_ms/);

    // Invalid areas
    assert.throws(() => validateInstructions([{
        SetPositionAnchored: {
            screen_anchor_x: 0, screen_anchor_y: 0,
            pivot_u: 0, pivot_v: 0,
            area: 'invalid_area',
        },
    }]), /unknown area/);
});
