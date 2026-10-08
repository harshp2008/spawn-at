import test from 'node:test';
import assert from 'node:assert/strict';

/**
 * Pure anchor math extracted directly from GNOME extension logic.
 * Characterizes the exact mathematics in extension.esm.js before modularization.
 */
function computeAnchoredPosition(bounds, frame, minSize, payload) {
    const effW = Math.max(frame.width, (minSize && minSize[0]) || 0);
    const effH = Math.max(frame.height, (minSize && minSize[1]) || 0);

    const mt = Number(payload.margin_top || 0);
    const mb = Number(payload.margin_bottom || 0);
    const ml = Number(payload.margin_left || 0);
    const mr = Number(payload.margin_right || 0);

    const minX = bounds.x + ml;
    const minY = bounds.y + mt;
    let maxX = bounds.x + bounds.width - mr - effW;
    let maxY = bounds.y + bounds.height - mb - effH;

    const oversized = effW > bounds.width - ml - mr || effH > bounds.height - mt - mb;
    if (maxX < minX)
        maxX = minX;
    if (maxY < minY)
        maxY = minY;

    const rawX = Number(payload.screen_anchor_x || 0)
        - Math.round(Number(payload.pivot_u || 0) * effW) + Number(payload.offset_x || 0);
    const rawY = Number(payload.screen_anchor_y || 0)
        - Math.round(Number(payload.pivot_v || 0) * effH) + Number(payload.offset_y || 0);

    const clamp = payload.clamp !== false;
    const x = clamp ? Math.max(minX, Math.min(rawX, maxX)) : rawX;
    const y = clamp ? Math.min(maxY, Math.max(minY, rawY)) : rawY;

    return {
        x, y, rawX, rawY, effW, effH, oversized, bounds,
        range: { minX, maxX, minY, maxY },
    };
}

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
