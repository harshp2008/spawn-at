---
title: "6.21 Default cap 250 ms and the wire paint time"
---

Default cap 250 ms and the wire paint time: code, tests and a traced loaded A/B/C. Taking the paint time from the protocol dump removes the stamp lag, within the limits of the clock anchor.

| Field | Value |
| :-- | :-- |
| **ID** | TEST_LOG 6.21. Follows [6.20](/spawn-at/experiments/6-20-why-b-and-c-still-flash/). |
| **Date** | 2026-10-13. |
| **Build** | Commits `e9d5c77` (default cap 250) and `8d65a48` (wire paint time in the harness), plus the log section. The fixture (`src/bin/fixture/`) was not changed: `FIRST_PAINT` is still stamped after `present()` returns. The installed extension was not reinstalled and kept its default of 150; every live run passed the cap explicitly (`--wait-cap-ms 150` or `250`). The version string is not recorded. Sections 6.18 to 6.20 are unchanged: their 150 ms figures are what was measured then. |
| **Verdict** | The stamp lag, not the cap, made C look worse than B, and the flashes of the `reached` runs of 6.20 are a late stamp. The log's own limit applies: the flash verdict needs the lead to exceed the clock anchor, which under this load is wide (half-width 12 to 13 ms against 2 ms idle). So "0 wire flashes" means no commit is provably after the reveal; it is not proof that every commit was before it (25 of 40 B runs and 34 of 40 C runs have the commit inside the anchor). |
| **Raw results** | Not in the repository: throwaway scripts (`w21_loaded.sh`, `w21_rows.jq`, `w21_rows.sh`, `ab_batch.sh`, `ab_stats.jq`) and raw JSON stayed in `scratch/` and the session scratchpad. |

## What was tested

**Question**

Does the default cap of 250 ms behave as 6.19 recommended? Does taking the paint time from the fixture's
Wayland protocol dump (the `wl_surface.commit` of the target-size buffer), instead of the fixture's own
`FIRST_PAINT` stamp, remove the stamp lag found in 6.20?

**Setup and environment**

Two changes. (1) The default cap from 150 to 250 ms in `Policy::default()` (`ready_wait.rs`),
`DEFAULT_POLICY` (`ready_wait.js`), `tests/ready_wait_vectors/policy.json`, golden vectors 02 to 05,
the tests that name the cap, and `DESIGN_READY_WAIT.md`; `quiet_ms` 30 and `hard_ms` 1500 unchanged.
(2) `trace/wire.rs` in `spawn-at-verify` reads the fixture's protocol dump: per `wl_surface`, the first
`commit` that follows an `attach` of a buffer of a new size. The dump's timestamp is `CLOCK_REALTIME`
microseconds modulo 2^32; the `TRACE_START` line relates it to `CLOCK_MONOTONIC`, and the paint goes
through the same clock anchor as before. Default on; `--no-wire-paint` opts out. The dump writes about
37 KB per run.

Live: a cost check (idle 30 pairs and loaded 20 pairs, dump off and on) and a traced loaded A/B/C: 22
`xtask burn-cpu` threads after a 70 s warm-up (load1 19.9 at the start, 26.2 at the end),
`bottom-right-smaller` and `bottom-right-larger`, 20 per arm per spec, 120 runs in one login, arm order
rotated. A is `--wait-strategy legacy`; B is `ready-wait --wait-cap-ms 150`; C is `ready-wait
--wait-cap-ms 250`.

**Commands run**

`spawn-at-verify run` with the wire paint time on; `xtask burn-cpu`; `cargo test -p spawn-at-verify
--features tui,fixture`. Full command lines not recorded.

## Result

- Clock conversion check: with no load the commit and the stamp of the same paint agree to 7
  microseconds in both smoke runs (median lag 0.01 ms, maximum 0.02 ms over 30 idle runs).
- The wire paint time has no measurable cost at these sizes (the dump on against off: cloak medians
  differ by 0.5 ms idle; loaded run duration medians by 9 ms).
- Flash runs, loaded, 40 per arm:

| arm | flash on the old stamp | flash on the wire time |
| :--- | ---: | ---: |
| A legacy | 24 | 24 |
| B cap 150 | 4 | 0 |
| C cap 250 | 7 | 0 |

  Against A (Fisher two-sided): wire time B and C 0 of 40 (-100%, p = 7.7e-10); on the old stamp B 4 of 40
  (p = 4.2e-6) and C 7 of 40 (p = 1.9e-4). B against C is not different on either measure. The legacy arm
  gives the same 24 on both measures, so the wire time does not hide real late paints there.
- All 11 runs the old stamp called a flash in B and C were `reached` runs whose stamp came 10.3 to 48.1 ms
  after the commit of the same buffer.
- This batch produced no timed-out run at either cap (6.20: 5 of 50 B runs). 0 of 40 against 5 of 50 is
  Fisher two-sided p = 0.063, which the log calls weak evidence, not proof, that the cap-150 timeouts
  are gone.
- When the app answers, cap 150 and cap 250 cost the same: cloak median 67.5 against 68.3 ms; waited
  55.4 against 57.2 ms.
- Tests: `cargo test -p spawn-at-verify --features tui,fixture` gave 381 passed, 4 ignored (201 lib, +12);
  `spawn-at` 175, `spawn-at-core` 47, `xtask` 10, node 154.

## Not verified

On-screen visibility (no screen capture, no compositor frame time); hostile apps; X11 and XWayland (the
dump is Wayland-only); a release-build fixture; real toolkit apps under the wire time; the new default
cap in a reinstalled Shell; the compositor-side arrival time of a commit (libwayland stamps a request when
it is marshalled, so the wire time is the earliest the commit could have arrived); a harness anchor
narrow enough under load to prove commits earlier than the reveal (hypothesis: more bracketed calls per
run, currently 4).

## Sources

- `docs/TEST_LOG.md` section 6.21 (lines 1062-1146)
