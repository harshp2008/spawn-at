---
title: "6.19 Ready-wait: same-session A/B/C"
---

Ready-wait: same-session A/B/C with the per-spawn override (no code change). The log recommends a cap of 250 ms; the two caps are not separable on flashes.

| Field | Value |
| :-- | :-- |
| **ID** | TEST_LOG 6.19. Follows [6.18](/spawn-at/experiments/6-18-ready-wait-live-validation/); explained by [6.20](/spawn-at/experiments/6-20-why-b-and-c-still-flash/). |
| **Date** | 2026-10-13. |
| **Build** | Debug `spawn-at`, `spawn-at-verify` and `fixture` built from the code of commit `5a2c7be` (`binary_commit` in the report is `5a2c7be`, `is_dirty_build` false). The per-spawn override flags come from commits `44ee6b7` and `80ef74a`. The extension installed at 12:27:22 IST; the Shell process started at 12:30:49 IST. The version string is not recorded. Nothing in the product, extension or harness was changed; the only file written in the repository was the log section. Tests were not run. |
| **Verdict** | The log recommends cap 250, labelled "a recommendation; the decision is the user's": at idle the two caps are indistinguishable and neither is ever reached (largest wait 40.8 ms); under load only 250 meets the 70% criterion because it turns 9 of B's 10 cap timeouts into runs that reached the target. The price is the cloak tail (p95 +34 ms, max +80 ms against 150). The log holds this loosely: B and C are not separable on flashes, both caps leave 41 to 42 of 50 loaded runs over the 100 ms budget, and a cap between 150 and 250 or above 250 was not tried. |
| **Raw results** | Not in the repository: scratch scripts (`ab_batch.sh`, `ab_loaded.sh`, `ab_rows.sh`, `ab_rows.jq`, `ab_stats.sh`, `ab_stats.jq`, `app_ab.sh`, `rw_extract2.sh`) are throwaway and gitignored; the raw per-run JSON (330 fixture runs) and the app logs stayed in the session scratchpad. |

## What was tested

**Question**

In one login and one extension instance, how do the legacy rule and the ready-wait with two caps
compare? Arm A is `--wait-strategy legacy` (the 30 ms rule); arm B is `--wait-strategy ready-wait
--wait-cap-ms 150`; arm C is `--wait-strategy ready-wait --wait-cap-ms 250`. The log's criteria are
those of 6.18.

**Setup and environment**

GNOME Shell 46 on Wayland, 22 CPUs, 60 Hz, one monitor. Preflight confirmed before any measurement: the
session; the extension ACTIVE and byte-identical to the tree (`diff -rq` printed nothing); protocol 6;
the override logged for both flags. The log notes the extension's timing log is off by default and was
turned on with the same in-memory D-Bus call (`SetLogging true`) the harness uses; no gsettings or
dconf key was written.

A is the legacy rule reached through the override, in the same extension that holds the ready-wait
code, not a build from before the change.

Idle: `bottom-right-smaller`, `bottom-right-larger`, `top-left-smaller`, 20 runs per arm per spec (60
per arm, 180 runs). Loaded: 22 `burn-cpu` threads, 70 s warm-up, `bottom-right-*` 20 per arm per spec
and `top-left-smaller` 10 per arm (150 runs); mean load average at run start A 25.80, B 25.86, C 25.77.
Every run was checked against its arm; 0 of 330 failed a check.

**Commands run**

`spawn-at-verify run --dir scratch/flashlib --bin target/debug/spawn-at --glob placement/<spec> --json
<arm flags>`, one process per run, specs and arms interleaved, a 1.5 s sleep after each; `xtask
burn-cpu --threads 22`; for real apps `spawn-at spawn --class <id> --anchor bottom-right --size W H
--margin 16 [override flags] -- <app>`. The first batch script needed fixes (the arm rotation started
at the second arm; a stale-file copy was removed; a wrong binary argument in a trial that was
discarded).

## Result

| | A legacy | B cap 150 | C cap 250 |
| :--- | :--- | :--- | :--- |
| idle flash runs (of 60) | 1 | 0 | 0 |
| idle cloak median / p95 (ms) | 41.2 / 65.7 | 39.8 / 50.5 | 40.6 / 51.0 |
| loaded flash runs (of 50) | 47 | 15 | 11 |
| loaded cloak median / p95 / max (ms) | 95.6 / 133.8 / 140.9 | 131.6 / 167.0 / 183.9 | 129.3 / 200.9 / 263.4 |
| loaded outcomes | SIZE_SETTLED x49, AT_TARGET x1 | reached x40, timed-out (cap) x10 | reached x49, timed-out (cap) x1 |
| loaded runs over 100 ms cloak / over 150 ms | 19 / 0 | 41 / 13 | 42 / 16 |
| relative flash reduction against A | n/a | 68.1% (Fisher p = 1.6e-11) | 76.6% (p = 5.3e-14) |

- B and C are not distinguishable on flashes at this sample size (15 of 50 against 11 of 50, p = 0.49).
- Flash by outcome, loaded: in B, 8 of 40 `reached` runs flashed and 7 of 10 `timed-out` runs; in C, 10
  of 49 `reached` and 1 of 1 `timed-out`.
- The +22 ms idle cloak seen in 6.18 is present in the legacy arm of this login too (idle cloak median
  41.2 ms; the 6.17 baseline login had 19.1 ms), and arm A flashed in 40 of 40 loaded bottom-right
  runs (baseline login: 15 of 20, under a lower mean load). So it came with the login, not with the
  ready-wait rule, and the cross-login comparison of 6.18 understated A's flashing.
- Real apps, A against B (extension log only): gnome-text-editor, no difference; gnome-terminal, the
  legacy arm's cloak was 163 to 209 ms against 68 to 81 ms for the ready-wait in all 5 pairs (the time
  was between `UNCLOAK_4B` and `UNCLOAK_4C`, the forced stage update, in the one window read); Firefox
  157.0.1 (snap, throwaway profile, warm starts), B had 0 corrections in 5, A had one. The CLI
  warning "did not appear" seen in 6.18 did not reproduce (0 of 30).
- Criteria: (1) idle 0 flashes: met for both; (2) loaded flashes down at least 70%: B not met (68.1%,
  one flash fewer would have been 70.2%), C met (76.6%); (3) idle cloak median up by no more than 10
  ms: met for both (-1.4 and -0.6 ms); (4) hung or refusing apps end by the hard limit: not shown.

## Not verified

Hung or refusing apps (the fixture has no flag to stall or refuse a resize; the 1500 ms hard limit and
the `skipped` outcome were never exercised); whether any flash is visible (no screen capture); X11 and
XWayland; a build from before the change; a release build, other hardware or other sessions; real
apps beyond n = 5 per arm and app; why `reached` loaded runs still flash (8 of 40 in B, 10 of 49 in
C); why the login differs from the baseline login.

## Sources

- `docs/TEST_LOG.md` section 6.19 (lines 801-922)
