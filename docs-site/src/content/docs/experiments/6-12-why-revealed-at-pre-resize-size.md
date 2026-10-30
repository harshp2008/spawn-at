---
title: "6.12 Why the window is revealed at its old size"
---

Why the window is revealed at its pre-resize size: an investigation (read-only, nothing implemented). The mechanism was found; the log recommends accepting the residual and no fix was built.

| Field | Value |
| :-- | :-- |
| **ID** | TEST_LOG 6.12. Follows [6.11.1](/spawn-at/experiments/6-11-1-final-state-oracle/). |
| **Date** | 2026-10-12. |
| **Build** | Not recorded: the section gives no commit hash and no version string. The log says no product code changed. It used stored runs from the batch of 6.11.1 ("g3") and new live sampling. |
| **Verdict** | Decision by the log's own rule (short tail: options B + C; long tail: option E if validated, else accept): the loaded p99 is 150 to 370 ms, option E (a scheduler-aware settle) is not validated, so "D: accept" was the recommendation, with the residual being the post-reveal warning already shown by the verify tool. Simulated cost of a bounded grace: fixing 83 to 130 of the 134 loaded flashes would add 25.9 to 35.1 ms median extra cloak (p95 77.8 to 230 ms) and would break the provisional 100 ms budget on an app that never answers. No fix was implemented. The project owner stopped the run at about 19:3x; no phase was rerun. |
| **Raw results** | `scratch/` and `verify-results/` (gitignored); the sampler is not committed. Not in the repository. |

## What was tested

**Question**

Why is the window revealed at its pre-resize size, with the app's resize landing after the reveal? The
log's hypotheses: H1, the 30 ms quiet clock starts at the first sample, not at an answer; H2, a request
below the app's minimum is slower than a legal one; H3, shrink against grow is the discriminator.

**Setup and environment**

GNOME 46 on Wayland, mutter 46.2. Three data sources: (1) 640 stored runs from 6.11.1 (320 idle, 320
loaded); (2) live sampling with a throwaway Rust program in `scratch/` that read `/proc/<pid>/stat`
and `schedstat` about every 1 ms, with the clock tied to the journal by 16 `SetLogging` echoes per run
(one sample costs 1.2 to 1.6 microseconds idle); (3) introspection of `Meta-14` with `gjs`. Live levels:
idle (84 runs), 1 busy thread (32), 11 threads (120), 22 threads (120). Real apps, 3 runs each, idle and
11 threads: `gnome-text-editor` (30x20), `nautilus`, `gnome-terminal` and `firefox` (snap).

**Commands run**

Exact command lines not recorded. By the log: `gjs` introspection of the `Meta-14` typelib and
`Meta-14.gir`; the `/proc` sampler; `burn-cpu` for the thread levels; `gnome-shell --headless
--wayland --no-x11 --wayland-display=NAME --virtual-monitor 1920x1080` inside `dbus-run-session` for
the nested-shell trial.

## Result

- Mechanism, from `assets/gnome/modern/commit.js` and the 640 stored runs: the latch starts its 30 ms
  quiet timer on any `allocation` sample when the idle timer is still pending, even if the frame size
  did not change. In all 640 runs the first non-initial sample was an `allocation` at the initial
  400x300, never a `size-changed`.
- `SIZE_SETTLED` on a frame other than the final one occurred in 205 of 320 loaded runs and 3 of 320
  idle runs. Of the 205, 134 were corrected after the reveal, 62 were caught by
  `LATE_RESIZE_CORRECTED` before it, and 9 were not observed.
- H1 supported as the mechanism. H2 not supported: the classes have the same idle response time and
  flash rate (38% against 39% loaded). H3 not supported as stated: grow flashes most because the app
  needs longer to answer (17 ms against about 10 ms at idle).
- A premise corrected: `bottom-right-larger` is the worst class in the stored loaded runs (20 of 20
  settled early, 18 post-reveal corrections), and shows no stale-print warnings only because
  `spawn-at` printed the rectangle after the grow landed.
- Delay from `SIZE_SETTLED` to the app's real resize, in runs corrected after the reveal: median 50.6
  ms, p99 307.6 ms, maximum 350.0 ms (stored loaded); median 9.8 ms idle live; median 39.0 ms with 22
  threads live. The loaded tail is long.
- Live idle stale rate was 65 to 84%, far above the 1% in stored idle runs. The cause is not
  established (hypothesis: cold CPU frequency state).
- Whether the app was asleep at `SIZE_SETTLED` does not separate stale from non-stale runs, so a
  scheduler-aware settle would not tell them apart.
- Real apps: `gnome-text-editor`, `nautilus` and `gnome-terminal` settled by `SIZE_SETTLED` and none
  flashed; `firefox` (snap) flashed 6 of 6 (corrected 135 to 312 ms after the request). Chrome was
  never claimed, so there is no Chrome timing.
- A screencast run (one window) showed the Shell main loop stalled twice, 355 ms before the first
  sample and 185 ms for a 30 ms timer.
- Signals on GNOME 46 (`Meta-14`): `Meta.Window` has `size-changed`, `position-changed`, `shown` and
  `unmanaged`; `Meta.WindowActor` has `damaged`, `first-frame` and `thawed`. Not tested live: that
  `damaged` or the texture's `size-changed` fires on a client commit after the request.
- **Incident recorded in the log.** A nested-shell experiment wrote the `org.gnome.shell
  enabled-extensions` setting into the real dconf database, because the D-Bus-activated dconf service
  in the private session did not inherit `XDG_CONFIG_HOME`. The value held only `spawn-at@harsh.local`
  for a while and is back to the original 25-entry list, compared by eye against the value captured at
  the start. The log says its author did not restore it and that how it came back is not established.
  The nested shell itself started in about 1 s but the extension was not shown to load there.

**Correction (2026-10-31).** The bullet above says Chrome was never claimed, so there is no Chrome
timing. That was true for this run, but it is not true now. In run 04, the texture gate matched Chrome
in 20 of 20 runs, after a median of 9 to 13 ms and at most 58 ms, so Chrome timing exists. Chrome's
remaining problem is blank frames at the right size (MEASURED,
`docs/evidence/x11-flash-04/findings.md`, on branch `exp/x11-flash-03`).

## Not verified

Why the idle stale rate differed from the stored runs; that the first allocation is the app's own
initial buffer (hypothesis); real apps at 22 threads; Chrome timing; the nested-shell route; the
signals' firing on a client commit.

## Sources

- `docs/TEST_LOG.md` section 6.12 (lines 285-366)
