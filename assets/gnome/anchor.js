/**
 * spawn-at — pure anchor and bounding box calculation.
 * 
 * Contains NO gi:// imports. Can be executed and tested in standard Node.js or GJS.
 */

/**
 * Computes target window frame coordinates and bounding clamping from placement payload.
 *
 * @param {object} bounds - Available screen or workarea rectangle { x, y, width, height }
 * @param {object} frame - Current window frame dimensions { width, height }
 * @param {number[]} [minSize] - Optional toolkit minimum dimensions [minW, minH]
 * @param {object} payload - Placement intent parameters
 * @returns {object} Calculated coordinates, clamped positions, and bounding ranges
 */
export function computeAnchoredPosition(bounds, frame, minSize, payload) {
    const minW = (minSize && minSize[0]) || 0;
    const minH = (minSize && minSize[1]) || 0;
    const effW = Math.max(frame.width || 0, minW);
    const effH = Math.max(frame.height || 0, minH);

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
        x,
        y,
        rawX,
        rawY,
        effW,
        effH,
        minW,
        minH,
        oversized,
        bounds,
        range: { minX, maxX, minY, maxY },
    };
}
