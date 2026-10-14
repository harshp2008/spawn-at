import test from 'node:test';
import assert from 'node:assert/strict';

// The per-spawn wait override as the instruction validator sees it. The behaviour of the
// extension with these fields is in ready_wait_wiring_test.js (section 3).

const { validateInstruction, validateInstructions } = await import('../assets/gnome/modern/validator.js');
const legacy = await import('../assets/gnome/legacy/validator.js');

const wait = args => validateInstruction({ WaitForCommit: args }).args;

test('no override fields: the validated arguments are exactly {timeout_ms}, as before', () => {
    assert.deepEqual(wait({ timeout_ms: 300 }), { timeout_ms: 300 });
    assert.deepEqual(Object.keys(wait({ timeout_ms: 300 })), ['timeout_ms']);
});

test('strategy legacy and ready-wait are kept, auto adds nothing', () => {
    assert.deepEqual(wait({ timeout_ms: 300, strategy: 'legacy' }), { timeout_ms: 300, strategy: 'legacy' });
    assert.deepEqual(wait({ timeout_ms: 300, strategy: 'ready-wait' }), { timeout_ms: 300, strategy: 'ready-wait' });
    assert.deepEqual(wait({ timeout_ms: 300, strategy: 'auto' }), { timeout_ms: 300 });
});

test('cap_ms is kept from 1 to the hard limit 1500 and nowhere else', () => {
    for (const cap of [1, 60, 150, 1500])
        assert.deepEqual(wait({ timeout_ms: 300, cap_ms: cap }), { timeout_ms: 300, cap_ms: cap });
    for (const bad of [0, -1, 1501, 1.5, '200', null, true, [], {}]) {
        const args = wait({ timeout_ms: 300, cap_ms: bad });
        assert.equal(args.cap_ms, undefined, `cap_ms ${JSON.stringify(bad)} must be dropped`);
        assert.equal(args.ignored.length, 1);
        assert.match(args.ignored[0], /^cap_ms=/);
    }
});

test('invalid strategy values are dropped and named, never thrown', () => {
    for (const bad of ['fast', '', 'Legacy', 'ready_wait', 5, null, true, {}]) {
        const args = wait({ timeout_ms: 300, strategy: bad });
        assert.equal(args.strategy, undefined, `strategy ${JSON.stringify(bad)} must be dropped`);
        assert.equal(args.ignored.length, 1);
        assert.match(args.ignored[0], /^strategy=/);
    }
    assert.deepEqual(wait({ timeout_ms: 300, strategy: 'fast' }).ignored, ['strategy="fast"']);
});

test('one invalid field does not drop the valid one, and both are reported separately', () => {
    assert.deepEqual(wait({ timeout_ms: 300, strategy: 'legacy', cap_ms: 0 }),
        { timeout_ms: 300, strategy: 'legacy', ignored: ['cap_ms=0'] });
    assert.deepEqual(wait({ timeout_ms: 300, strategy: 'x', cap_ms: 90 }),
        { timeout_ms: 300, cap_ms: 90, ignored: ['strategy="x"'] });
});

test('an ignored value is cut to 32 characters so it cannot flood the log', () => {
    const args = wait({ timeout_ms: 300, strategy: 'a'.repeat(500) });
    assert.equal(args.ignored[0].length, 'strategy='.length + 32);
});

test('the override never rescues an invalid timeout_ms: that still rejects the instruction', () => {
    assert.throws(() => wait({ strategy: 'legacy' }), /WaitForCommit invalid timeout_ms/);
    assert.throws(() => wait({ timeout_ms: -1, strategy: 'legacy' }), /WaitForCommit invalid timeout_ms/);
});

test('a whole instruction list with an override validates, in order', () => {
    const list = validateInstructions(JSON.stringify(['Cloak', { SetSize: { w: 800, h: 600 } },
        { WaitForCommit: { timeout_ms: 300, strategy: 'legacy', cap_ms: 200 } }, 'Uncloak']));
    assert.deepEqual(list.map(op => op.name), ['Cloak', 'SetSize', 'WaitForCommit', 'Uncloak']);
    assert.deepEqual(list[2].args, { timeout_ms: 300, strategy: 'legacy', cap_ms: 200 });
});

test('the legacy validator (GNOME 42-44 extension, no ready-wait) drops the fields and still accepts the instruction', () => {
    const op = legacy.validateInstruction({ WaitForCommit: { timeout_ms: 300, strategy: 'legacy', cap_ms: 200 } });
    assert.deepEqual(op.args, { timeout_ms: 300 });
});
