---
title: "6.18 Ready-wait: live validation"
---

Ready-wait: live validation against the 6.17 baseline (no code change). Of the four criteria, one was met, two were not met and one was not shown.

| Field | Value |
| :-- | :-- |
| **ID** | TEST_LOG 6.18. Compares with [6.17](/spawn-at/experiments/6-17-flash-baseline/); followed by [6.19](/spawn-at/experiments/6-19-ready-wait-abc/). |
| **Date** | 2026-10-13. |
| **Build** | Debug `spawn-at` and debug fixture at commit `6ecc537` (the ready-wait wiring). The installed extension contained `ready_wait.js`, installed at 11:18:31 IST, with the project owner logged back in afterwards. The version string is not recorded. Nothing in the product, extension or harness was changed; the only files written were the log section and throwaway scripts in `scratch/`. Tests were not run in this step. |
| **Verdict** | By the log's own table: criterion 1 met, 2 and 3 not met, 4 not shown. The log also notes that against the design's own loaded criterion (flash at most 5 of 100, p95 cloak within the 100 ms budget) neither holds (5 of 20, p95 166.6 ms), and does not use it for the verdict. |
| **Raw results** | Not in the repository. The scratch scripts (`rw_extract.sh`, `rw_stats.sh`, `app_survey.sh`, `baseline_batch.sh`) are throwaway and not committed; the raw per-run JSON stayed in the session scratchpad. |

## What was tested

**Question**

Does the ready-wait reduce flashes against the baseline of 6.17, within criteria proposed beforehand?
The log's criteria: (1) idle stays at 0 flashes; (2) loaded flashes drop by at least 70% against the
baseline 15 of 20; (3) the idle cloak median rises by no more than 10 ms; (4) hung or refusing cases
end by the hard limit.

**Setup and environment**

GNOME Shell 46.0 on Wayland, 22 CPUs, 60 Hz. Preflight confirmed before any measurement: session
`wayland`; the extension ACTIVE; `ProtocolVersion` 6; a native spawn logging `READY_WAIT_START ...
strategy=ready-wait kind=NativeWayland ... cap=150 quiet=30 hard=1500`; and an XWayland spawn logging
`strategy=legacy kind=XWayland why=not-native-wayland`. Same four scratch specs and the same
`scratch/baseline_batch.sh` as 6.17 (one isolated `run` per spec, interleaved, 1.5 s apart). The
baseline cloak durations had not been written down before, so they were recomputed from the baseline
runs' lines in the local session log.

Two caveats the log states first: no same-session before and after exists (the baseline was the
previous login, and the extension has no switch back to the old rule); and the XWayland result shows
that reveal timing can differ between logins on the unchanged legacy path.

**Commands run**

`spawn-at-verify run` per spec; `cargo xtask burn-cpu --threads 22` after a 70 s warm-up;
`spawn-at spawn --class <id> --anchor bottom-right --size W H --margin 16` for the real-app survey;
`spawn-at close --id <id>`. Full command lines not recorded.

## Result

| measure | before (6.17 baseline) | after |
| :--- | :--- | :--- |
| idle flashes (debug) | 0 / 40 | 0 / 40 |
| idle cloak median / p95 (ms) | 19.1 / 21.2 | 41.1 / 50.7 |
| loaded flash runs, `bottom-right-smaller` | 6 / 10 | 0 / 10 |
| loaded flash runs, `bottom-right-larger` | 9 / 10 | 5 / 10 |
| loaded flash runs, the two together | 15 / 20 | 5 / 20 (-66.7%) |
| loaded runs with a correction after the reveal | 15 / 20 | 2 / 20 |
| loaded cloak median / p95 (ms) | 49.9 / 75.8 | 117.7 / 166.6 |
| loaded runs over the 100 ms cloak budget | 0 / 20 | 24 / 30 |
| loaded outcomes | n/a | 27 reached, 3 timed-out (cap), 0 adjusted, 0 skipped |
| XWayland `bottom-right-larger` flash runs | 1 / 10 | 9 / 10 and 10 / 10 (two batches), strategy `legacy` |

- Idle: 40 runs, all `reached` (`at-target`). The app's answer came about 20 ms later than in the
  baseline login (35 ms after the request against 15 ms) and the `UNCLOAK_4C` to `UNCLOAK` gap was
  about 4.5 ms longer; together that is the +22 ms of cloak. Whether this is the login, the load or the
  wiring was not separated.
- Loaded (22 threads, 10 runs each of `bottom-right-smaller`, `bottom-right-larger`,
  `top-left-smaller`): 7 of 30 flashed. Fisher exact, 5 of 20 against 15 of 20: p = 0.0038 (calculated
  in the log, not by the harness). All 3 `timed-out` runs flashed; 4 of 27 `reached` runs flashed. The
  batch ran under about 10% more load than the baseline.
- XWayland did not match the baseline and the log shows it is not the wiring (`git diff 0756167 HEAD --
  assets/gnome/modern/` shows the legacy path calls the same `waitForCommit`). The difference is the
  `UNCLOAK_4C` to `UNCLOAK` time (28 ms before, 9 ms now). The log concludes XWayland should be treated
  as flashing on this machine.
- Real apps (extension log only, no capture; 15 spawns): gnome-terminal (grid snapping), 5 `adjusted`;
  gnome-text-editor, 5 `reached`; Firefox 157.0.1 (snap), a fresh throwaway profile with `--no-remote`,
  1 `timed-out` at the 150 ms cap on the cold first start and 4 `reached`. In 3 of 5 Firefox spawns the
  CLI printed `Timed out waiting for window to map` and exited 0, although the extension log has a
  ready-wait pair for all five; not examined.
- No hard-limit (1500 ms) outcome occurred anywhere. The throwaway Firefox profile was deleted.

| # | criterion | result | met? |
| :-: | :--- | :--- | :-: |
| 1 | idle stays at 0 flashes | 0 of 40 | Met |
| 2 | loaded flashes drop by at least 70% | 66.7%, 3.3 points short | Not met |
| 3 | idle cloak median rises by no more than 10 ms | +22.0 ms | Not met |
| 4 | hung or refusing cases end by the hard limit | no hung or refusing app was run | Not shown |

## Not verified

A same-session before and after; the cause of the +22 ms idle cloak; a hung, refusing or not-resizable
app and the hard limit; the learned-minimum cache; the release fixture and a release `spawn-at`;
100-run cells; native X11; XWayland under load; any screen capture; why 4 of 27 loaded `reached` runs
flashed and why `bottom-right-larger` timed out 3 times in 10; Firefox with a real profile.

## Sources

- `docs/TEST_LOG.md` section 6.18 (lines 685-799)
