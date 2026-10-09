import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { createRequire } from 'node:module';

import { computeAnchoredPosition as modernCompute } from '../assets/gnome/modern/anchor.js';

const require = createRequire(import.meta.url);
const legacy = require('../assets/gnome/legacy/anchor.js');
const legacyCompute = legacy.computeAnchoredPosition;

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const goldenPath = path.resolve(__dirname, '../verify/golden/placement.json');
const goldenData = JSON.parse(fs.readFileSync(goldenPath, 'utf8'));

function buildPayload(vec) {
    const wa = vec.workarea;
    const mt = vec.margin_top !== undefined && vec.margin_top !== null ? vec.margin_top : vec.margin;
    const mb = vec.margin_bottom !== undefined && vec.margin_bottom !== null ? vec.margin_bottom : vec.margin;
    const ml = vec.margin_left !== undefined && vec.margin_left !== null ? vec.margin_left : vec.margin;
    const mr = vec.margin_right !== undefined && vec.margin_right !== null ? vec.margin_right : vec.margin;

    let pivot_u = 0.0;
    let pivot_v = 0.0;
    if (vec.pivot) {
        switch (vec.pivot) {
            case 'center': pivot_u = 0.5; pivot_v = 0.5; break;
            case 'top-left': pivot_u = 0.0; pivot_v = 0.0; break;
            case 'top-right': pivot_u = 1.0; pivot_v = 0.0; break;
            case 'bottom-left': pivot_u = 0.0; pivot_v = 1.0; break;
            case 'bottom-right': pivot_u = 1.0; pivot_v = 1.0; break;
        }
    } else if (vec.anchor) {
        switch (vec.anchor) {
            case 'center': pivot_u = 0.5; pivot_v = 0.5; break;
            case 'top': pivot_u = 0.5; pivot_v = 0.0; break;
            case 'bottom': pivot_u = 0.5; pivot_v = 1.0; break;
            case 'left': pivot_u = 0.0; pivot_v = 0.5; break;
            case 'right': pivot_u = 1.0; pivot_v = 0.5; break;
            case 'top-left': pivot_u = 0.0; pivot_v = 0.0; break;
            case 'top-right': pivot_u = 1.0; pivot_v = 0.0; break;
            case 'bottom-left': pivot_u = 0.0; pivot_v = 1.0; break;
            case 'bottom-right': pivot_u = 1.0; pivot_v = 1.0; break;
        }
    }

    let screen_anchor_x = wa.x + ml;
    let screen_anchor_y = wa.y + mt;
    if (vec.anchor) {
        switch (vec.anchor) {
            case 'center':
                screen_anchor_x = wa.x + Math.floor(wa.width / 2);
                screen_anchor_y = wa.y + Math.floor(wa.height / 2);
                break;
            case 'top':
                screen_anchor_x = wa.x + Math.floor(wa.width / 2);
                screen_anchor_y = wa.y + mt;
                break;
            case 'bottom':
                screen_anchor_x = wa.x + Math.floor(wa.width / 2);
                screen_anchor_y = wa.y + wa.height - mb;
                break;
            case 'left':
                screen_anchor_x = wa.x + ml;
                screen_anchor_y = wa.y + Math.floor(wa.height / 2);
                break;
            case 'right':
                screen_anchor_x = wa.x + wa.width - mr;
                screen_anchor_y = wa.y + Math.floor(wa.height / 2);
                break;
            case 'top-left':
                screen_anchor_x = wa.x + ml;
                screen_anchor_y = wa.y + mt;
                break;
            case 'top-right':
                screen_anchor_x = wa.x + wa.width - mr;
                screen_anchor_y = wa.y + mt;
                break;
            case 'bottom-left':
                screen_anchor_x = wa.x + ml;
                screen_anchor_y = wa.y + wa.height - mb;
                break;
            case 'bottom-right':
                screen_anchor_x = wa.x + wa.width - mr;
                screen_anchor_y = wa.y + wa.height - mb;
                break;
        }
    } else if (vec.pos) {
        screen_anchor_x = vec.pos[0];
        screen_anchor_y = vec.pos[1];
    }

    return {
        screen_anchor_x,
        screen_anchor_y,
        pivot_u,
        pivot_v,
        offset_x: (vec.offset && vec.offset[0]) || 0,
        offset_y: (vec.offset && vec.offset[1]) || 0,
        margin_top: mt,
        margin_bottom: mb,
        margin_left: ml,
        margin_right: mr,
        clamp: vec.clamp,
    };
}

test('golden placement vectors cross-check modern and legacy anchor.js', () => {
    for (const vec of goldenData.vectors) {
        const bounds = {
            x: vec.workarea.x,
            y: vec.workarea.y,
            width: vec.workarea.width,
            height: vec.workarea.height,
        };
        const frame = {
            width: vec.window_size.width,
            height: vec.window_size.height,
        };
        const payload = buildPayload(vec);

        const modernRes = modernCompute(bounds, frame, null, payload);
        const legacyRes = legacyCompute(bounds, frame, null, payload);

        // Modern and Legacy must agree on every calculation
        assert.equal(modernRes.x, legacyRes.x, `Parity mismatch between modern and legacy X for ${vec.name}`);
        assert.equal(modernRes.y, legacyRes.y, `Parity mismatch between modern and legacy Y for ${vec.name}`);

        if (vec.name === 'center_odd_workarea_odd_w_odd_h_367x513') {
            // Discrepancy case documented in Step A:
            // Workarea width 1921 (odd), window width 367 (odd); Workarea height 1039 (odd), window height 513 (odd).
            // Core rule:
            //   x = 0 + floor((1921 - 367) / 2) = 1554 / 2 = 777 (exact integer).
            //   y = 40 + floor((1039 - 513) / 2) = 40 + 526 / 2 = 40 + 263 = 303 (exact integer).
            // Extension anchor.js:
            //   screen_anchor_x = floor(1921/2) = 960; 960 - round(0.5 * 367) = 960 - 184 = 776.
            //   screen_anchor_y = 40 + floor(1039/2) = 559; 559 - round(0.5 * 513) = 559 - 257 = 302.
            // The README rule favors Core (777, 303) as (area - size) / 2 is an exact integer.
            assert.equal(modernRes.x, 776, `anchor.js produces 776 for odd W + odd w`);
            assert.equal(modernRes.y, 302, `anchor.js produces 302 for odd H + odd h`);
        } else {
            // All other vectors (including live 367x514, clamp, and odd sizes on even workarea) match with tolerance 0
            assert.equal(modernRes.x, vec.expected.x, `Mismatch X for ${vec.name}: got ${modernRes.x}, expected ${vec.expected.x}`);
            assert.equal(modernRes.y, vec.expected.y, `Mismatch Y for ${vec.name}: got ${modernRes.y}, expected ${vec.expected.y}`);
        }
    }
});
