/**
 * Strict instruction validator with allow-listed operations and bounded numbers (Legacy GNOME 42-44).
 */

const ALLOWED_OPS = new Set([
    'Snapshot',
    'Cloak',
    'DestroySnapshot',
    'SetSize',
    'WaitForCommit',
    'SetPositionAnchored',
    'Uncloak',
]);

const MAX_OPS = 64;
const MAX_STR_LEN = 128;
const MAX_COORD = 1_000_000;
const MAX_DIM = 65_535;
const MAX_MARGIN = 10_000;
const MAX_TIMEOUT_MS = 60_000;

function isFiniteNumber(val, min, max) {
    return typeof val === 'number' && Number.isFinite(val) && val >= min && val <= max;
}

var validateInstruction = function(op) {
    if (typeof op === 'string') {
        if (!ALLOWED_OPS.has(op))
            throw new Error(`Unknown instruction name: "${op}"`);
        if (op === 'SetSize' || op === 'WaitForCommit' || op === 'SetPositionAnchored')
            throw new Error(`Instruction "${op}" requires an arguments object`);
        return { name: op, args: {} };
    }

    if (!op || typeof op !== 'object' || Array.isArray(op))
        throw new Error('Instruction must be a string or single-key object');

    const keys = Object.keys(op);
    if (keys.length !== 1)
        throw new Error(`Instruction object must contain exactly one key, found ${keys.length}`);

    const name = keys[0];
    if (!ALLOWED_OPS.has(name))
        throw new Error(`Unknown instruction name: "${name}"`);

    const args = op[name];
    if (!args || typeof args !== 'object' || Array.isArray(args))
        throw new Error(`Arguments for instruction "${name}" must be an object`);

    switch (name) {
        case 'Snapshot':
        case 'Cloak':
        case 'DestroySnapshot':
            return { name, args: {} };

        case 'SetSize': {
            const { w, h } = args;
            if (!isFiniteNumber(w, 1, MAX_DIM) || !isFiniteNumber(h, 1, MAX_DIM))
                throw new Error(`SetSize invalid dimensions: w=${w}, h=${h}`);
            return { name, args: { w: Math.round(w), h: Math.round(h) } };
        }

        case 'WaitForCommit': {
            const { timeout_ms } = args;
            if (!isFiniteNumber(timeout_ms, 0, MAX_TIMEOUT_MS))
                throw new Error(`WaitForCommit invalid timeout_ms: ${timeout_ms}`);
            return { name, args: { timeout_ms: Math.round(timeout_ms) } };
        }

        case 'SetPositionAnchored': {
            const {
                screen_anchor_x, screen_anchor_y,
                pivot_u, pivot_v,
                offset_x, offset_y,
                area,
                margin_top, margin_bottom, margin_left, margin_right,
                clamp,
            } = args;

            if (!isFiniteNumber(screen_anchor_x, -MAX_COORD, MAX_COORD) ||
                !isFiniteNumber(screen_anchor_y, -MAX_COORD, MAX_COORD))
                throw new Error('SetPositionAnchored invalid screen_anchor coordinates');

            if (!isFiniteNumber(pivot_u, -100, 100) || !isFiniteNumber(pivot_v, -100, 100))
                throw new Error('SetPositionAnchored invalid pivot coordinates');

            const offX = offset_x !== undefined ? offset_x : 0;
            const offY = offset_y !== undefined ? offset_y : 0;
            if (!isFiniteNumber(offX, -MAX_COORD, MAX_COORD) || !isFiniteNumber(offY, -MAX_COORD, MAX_COORD))
                throw new Error('SetPositionAnchored invalid offsets');

            let validArea = undefined;
            if (area !== undefined && area !== null) {
                if (typeof area !== 'string' || area.length > MAX_STR_LEN)
                    throw new Error('SetPositionAnchored invalid area');
                if (area !== 'workarea' && area !== 'screen')
                    throw new Error(`SetPositionAnchored unknown area: "${area}"`);
                validArea = area;
            }

            const mt = margin_top !== undefined ? margin_top : 0;
            const mb = margin_bottom !== undefined ? margin_bottom : 0;
            const ml = margin_left !== undefined ? margin_left : 0;
            const mr = margin_right !== undefined ? margin_right : 0;
            for (const [mName, mVal] of [['margin_top', mt], ['margin_bottom', mb], ['margin_left', ml], ['margin_right', mr]]) {
                if (!isFiniteNumber(mVal, -MAX_MARGIN, MAX_MARGIN))
                    throw new Error(`SetPositionAnchored invalid ${mName}: ${mVal}`);
            }

            return {
                name,
                args: {
                    screen_anchor_x: Math.round(screen_anchor_x),
                    screen_anchor_y: Math.round(screen_anchor_y),
                    pivot_u: Number(pivot_u),
                    pivot_v: Number(pivot_v),
                    offset_x: Math.round(offX),
                    offset_y: Math.round(offY),
                    area: validArea,
                    margin_top: Math.round(mt),
                    margin_bottom: Math.round(mb),
                    margin_left: Math.round(ml),
                    margin_right: Math.round(mr),
                    clamp: clamp !== false,
                },
            };
        }

        case 'Uncloak': {
            const wake = args.wake !== undefined ? Boolean(args.wake) : true;
            return { name, args: { wake } };
        }
    }
};

var validateInstructions = function(input) {
    let raw = input;
    if (typeof raw === 'string') {
        if (raw.length > 65_536)
            throw new Error('Instruction JSON exceeds maximum payload length');
        raw = JSON.parse(raw);
    }

    if (!Array.isArray(raw))
        throw new Error('Instructions must be an Array');

    if (raw.length === 0)
        throw new Error('Instructions array cannot be empty');

    if (raw.length > MAX_OPS)
        throw new Error(`Instructions array exceeds maximum length of ${MAX_OPS}`);

    return raw.map(op => validateInstruction(op));
};

if (typeof module !== 'undefined' && module.exports) {
    module.exports = { validateInstruction, validateInstructions };
}
