# Golden scripts for the ready-wait state machine

Language-neutral scripts run by two implementations of the same machine:

- Rust: `crates/spawn-at-core/src/ready_wait.rs`, test `crates/spawn-at-core/tests/ready_wait_vectors.rs`
- JS: `assets/gnome/modern/ready_wait.js`, test `tests/ready_wait_test.js` (`node --test tests/*.js`)

Design: `docs/DESIGN_READY_WAIT.md` sections 2.2 and 4.3. A change to the rules edits the scripts
first; a divergence between the two implementations fails a test.

## Files

- `policy.json`: the default `Policy`. Both implementations assert that their built-in default equals it.
- `scripts/*.json`: one scenario each. A test runs every file in the directory in name order.

## Script format

```json
{
  "name": "...", "description": "...",
  "policy":  { "cap_ms": 150, "quiet_ms": 30, "hard_ms": 1500 },   // optional, missing fields use the defaults
  "request": { "target": [w, h], "current": [w, h], "resizable": true, "fullscreen": false, "tiled": false },
  "events":  [ { "at": ms, "size": [w, h] } ],                     // size samples, ascending "at", ms from the request
  "expect":  { "outcome": "reached", "size": [w, h], "waited_ms": n }
}
```

`resizable` defaults to true, `fullscreen` and `tiled` to false. `expect` is the outcome object
exactly as the machine reports it (`reached`, `adjusted`, `timed-out`, `skipped`; see `ready_wait.rs`).

## Driver (virtual clock; both implementations do exactly this)

```text
(machine, action) = start(policy, request, now = 0)
for event in events:
    while action is Wait{deadline} and deadline < event.at:    # timers strictly before the event fire first
        action = machine.on_tick(deadline)
    if action is Done: break                                   # later events are never seen
    action = machine.on_sample(event.size, event.at)           # a sample on a deadline beats the timer
while action is Wait{deadline}:                                # no more input: let the timers run
    action = machine.on_tick(deadline)
outcome = action.outcome                                       # compare with "expect"
```

## Scenarios

| File | Pins |
| :--- | :--- |
| 01 | fast compliant app; an unchanged first sample is not an answer |
| 02 | slow app answering before the cap |
| 03 | slow app answering after the cap: TimedOut(Cap) |
| 04 | a sample exactly on the cap beats the timer |
| 05 | refusing app (minimum equals current): TimedOut(Cap) |
| 06 | adjusted to the app's minimum |
| 07 | adjusted by a terminal grid snap |
| 08 | target already equal: Reached at 0 |
| 09, 09b, 09c | non-resizable, fullscreen, tiled: Skipped (skip check comes first) |
| 10 | resizes twice, second size is the target |
| 10b | resizes twice, quiet clock restarts on the second change |
| 10c | a change on the quiet boundary beats the timer |
| 10d | an unchanged sample does not restart the quiet clock |
| 11 | grows beyond the work area: AdjustedTo |
| 12 | hung app, cap longer than the hard limit: TimedOut(Hard) |
| 13 | restless app never quiet: TimedOut(Hard) |
| 14 | quiet deadline equal to the hard limit: the hard limit wins |
| 15 | policy override (`quiet_ms` 100) |
