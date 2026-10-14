import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync, readdirSync } from 'node:fs';
import { DEFAULT_POLICY, ReadyWait, mergePolicy } from '../assets/gnome/modern/ready_wait.js';

// Runs the golden scripts of tests/ready_wait_vectors/ (the same files the Rust test runs) through
// the JS port of the ready-wait machine, with the virtual-clock driver from that directory's README.

const VECTORS = new URL('./ready_wait_vectors/', import.meta.url);
const MODULE = new URL('../assets/gnome/modern/ready_wait.js', import.meta.url);

/** Runs `events` (`{at, size}`) against a fresh machine and returns the outcome. */
function drive(policy, request, events) {
    let { machine, action } = ReadyWait.start(policy, request, 0);
    for (const event of events) {
        while (action.kind === 'wait' && action.deadline_ms < event.at)
            action = machine.onTick(action.deadline_ms);
        if (action.kind === 'done')
            break;
        action = machine.onSample(event.size, event.at);
    }
    while (action.kind === 'wait')
        action = machine.onTick(action.deadline_ms);
    return action.outcome;
}

const scriptFiles = readdirSync(new URL('scripts/', VECTORS))
    .filter(name => name.endsWith('.json'))
    .sort();

test('all required scenarios are present', () => {
    assert.ok(scriptFiles.length >= 20, `found ${scriptFiles.length} scripts`);
    for (const prefix of ['01_', '02_', '03_', '04_', '05_', '06_', '07_', '08_', '09_', '09b_', '09c_',
        '10_', '10b_', '10c_', '10d_', '11_', '12_', '13_', '14_', '15_'])
        assert.ok(scriptFiles.some(f => f.startsWith(prefix)), `missing scenario ${prefix}`);
});

for (const file of scriptFiles) {
    const script = JSON.parse(readFileSync(new URL(`scripts/${file}`, VECTORS), 'utf8'));
    test(`golden script ${file}: ${script.name}`, () => {
        const outcome = drive(mergePolicy(script.policy), script.request, script.events);
        assert.deepEqual(outcome, script.expect);
    });
}

test('the default policy equals the shared policy.json (one source of the numbers)', () => {
    const shared = JSON.parse(readFileSync(new URL('policy.json', VECTORS), 'utf8'));
    assert.deepEqual({ ...DEFAULT_POLICY }, shared);
    assert.deepEqual(shared, { cap_ms: 250, quiet_ms: 30, hard_ms: 1500 });
    assert.deepEqual(mergePolicy({ quiet_ms: 100 }), { cap_ms: 250, quiet_ms: 100, hard_ms: 1500 });
    assert.ok(Object.isFrozen(DEFAULT_POLICY));
});

// The cases below mirror the unit tests in crates/spawn-at-core/src/ready_wait.rs.

const BIG = [800, 600];
const SMALL = [400, 300];
const req = (extra = {}) => ({ target: BIG, current: SMALL, ...extra });

test('skip is checked before the equal-size check and in a fixed order', () => {
    const base = { target: SMALL, current: SMALL, resizable: false, fullscreen: true, tiled: true };
    const reason = r => ReadyWait.start(DEFAULT_POLICY, r, 0).action.outcome.reason;
    assert.equal(reason(base), 'not-resizable');
    assert.equal(reason({ ...base, resizable: true }), 'fullscreen');
    assert.equal(reason({ ...base, resizable: true, fullscreen: false }), 'tiled');
});

test('the first deadline is the cap, counted from the start time', () => {
    const { action } = ReadyWait.start(DEFAULT_POLICY, req(), 1000);
    assert.deepEqual(action, { kind: 'wait', deadline_ms: 1250 });
});

test('a sample equal to the starting size is not an answer', () => {
    const outcome = drive(DEFAULT_POLICY, req(), [{ at: 5, size: SMALL }, { at: 60, size: SMALL }]);
    assert.deepEqual(outcome, { outcome: 'timed-out', last_size: SMALL, waited_ms: 250, phase: 'cap' });
});

test('done is sticky', () => {
    const { machine } = ReadyWait.start(DEFAULT_POLICY, req(), 0);
    const first = machine.onSample(BIG, 10);
    assert.deepEqual(first, { kind: 'done', outcome: { outcome: 'reached', size: BIG, waited_ms: 10 } });
    assert.deepEqual(machine.onSample([1, 1], 20), first);
    assert.deepEqual(machine.onTick(5000), first);
});

test('a sample after an expired cap does not become a late Reached', () => {
    const { machine } = ReadyWait.start(DEFAULT_POLICY, req(), 0);
    assert.deepEqual(machine.onSample(BIG, 400).outcome,
        { outcome: 'timed-out', last_size: SMALL, waited_ms: 250, phase: 'cap' });
});

test('the hard limit wins a tie with the quiet deadline', () => {
    const events = Array.from({ length: 74 }, (_, k) => ({
        at: 10 + 20 * k,
        size: k % 2 === 0 ? [410, 310] : [420, 320],
    }));
    assert.equal(events[73].at, 1470);
    assert.deepEqual(drive(DEFAULT_POLICY, req(), events),
        { outcome: 'timed-out', last_size: [420, 320], waited_ms: 1500, phase: 'hard' });
});

test('a cap beyond the hard limit reports the hard limit', () => {
    assert.deepEqual(drive(mergePolicy({ cap_ms: 5000 }), req(), []),
        { outcome: 'timed-out', last_size: SMALL, waited_ms: 1500, phase: 'hard' });
});

test('waited_ms is relative to the start', () => {
    const { machine } = ReadyWait.start(DEFAULT_POLICY, req(), 10000);
    assert.deepEqual(machine.onSample(BIG, 10040).outcome, { outcome: 'reached', size: BIG, waited_ms: 40 });
});

test('the module is portable: no imports and no host APIs', () => {
    const source = readFileSync(MODULE, 'utf8')
        .split('\n')
        .filter(line => !line.trimStart().startsWith('*') && !line.trimStart().startsWith('/*') && !line.trimStart().startsWith('//'))
        .join('\n');
    for (const forbidden of [/\bimport\b/, /\brequire\s*\(/, /gi:\/\//, /resource:\/\//, /\bprocess\./,
        /\bsetTimeout\b/, /\bsetInterval\b/, /\bDate\.now\b/, /\bperformance\./, /\bGLib\b/, /\bconsole\./, /\bglobalThis\b/])
        assert.doesNotMatch(source, forbidden);
});
