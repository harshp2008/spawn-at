---
title: "6.33 Does the sync counter move on repaint"
---

Does the X sync counter move when a client repaints? Live check with the GTK4 client. The extended sync counter is usable as an experimental signal for this GTK4 client only.

| Field | Value |
| :-- | :-- |
| **ID** | TEST_LOG 6.33. Follows [6.32](/spawn-at/experiments/6-32-gtk4-client-legacy-rule/); the next step is [6.34](/spawn-at/experiments/6-34-self-sent-sync-request/). |
| **Date** | 2026-10-14. |
| **Build** | Tag: none recorded. Tree: HEAD `3190488`. Nothing in the Shell, no extension change, no install. The output of `spawn-at --version` is not recorded for this section. |
| **Verdict** | Usable as an experimental signal for clients that drive an extended-sync counter on their own (confirmed for GTK4 4.14 here); not shown for any other client. The log's limits: (1) the basic counter never moved, so a client with only the basic counter (Chromium and Electron advertise one) would not give a signal unless it updates it on its own (hypothesis: it does not); (2) the counter says the client finished a frame, not that the compositor has it; (3) the extension cannot read XSync counters from GJS. The log sketches a design with the CLI watching the counter and telling the extension over D-Bus, marks it a hypothesis, and says "not shipped". |
| **Raw results** | Not recorded: the log names no results directory for this section. The two scripts are named as gitignored files in `scratch/`. Nothing of this section is in the repository. |

## What was tested

**Question**

When the GTK4 client repaints at a new size, does one of its sync counters step, and does a step
arrive after the client's final paint and after the legacy reveal, so that it could serve as a ready
signal?

**Setup and environment**

GNOME on Wayland. The GTK4 client of [6.32](/spawn-at/experiments/6-32-gtk4-client-legacy-rule/)
(`scratch/p33_syncclient.py`) through the legacy rule, plus a throwaway C watcher
(`scratch/p34_watch.c`) that polled `XSyncQueryCounter` about every 0.3 ms on `CLOCK_MONOTONIC`, found
each new window from the root `CreateNotify`, and logged `ConfigureNotify`. Neither file is in the
repository. Idle: 20 runs per spec (40 runs). Loaded: 22 `xtask burn-cpu` threads after a 70 s warm-up,
20 per spec (40 runs). Slow client: `--paint-delay-ms 100`, 10 per spec idle (20 runs). 100 runs, all
exit 0, all windows found.

**Process note**

The section first stopped after a 4-run smoke. The journal showed gnome-shell assertions at 11:13:32
(5 pairs of `cogl_framebuffer_set_viewport: assertion 'width > 0 && height > 0' failed` and
`_st_create_shadow_pipeline_from_actor: assertion 'clutter_actor_has_allocation (actor)' failed`),
two seconds after the first smoke spawn was armed. In the same second another application was
launched from the Shell. The Shell kept its process id and there was no crash signal. Under the stop
rule then in force no batch was run. **The project owner then changed the stop rule**: stop only on a gnome-shell
crash or restart, signal 5/6/11, an "uncatchable exception", a Mutter window or focus assertion
(`meta_window_*`, `focus_window`), or any line naming spawn-at or the extension; harmless St/Cogl
criticals from other apps are logged with their time and the run goes on. The journal checks before
and after each batch found only the same 5 + 5 St/Cogl lines of 11:13:32 and nothing matching the stop
rule.

**Commands run**

Exact command lines not recorded. The spawn path is the one of 6.32 (legacy rule, `SetLogging(true)`
for the batch only); the watcher ran for the whole batch.

## Result

The client has two counters. **Counter A (the basic counter) never changed in any of the 100 runs.**
**Counter B (GTK4's extended-sync frame counter) steps by 2 per completed frame**; its value at the
resize call was 1 in all 100 runs. Value 2 is the completion of the initial frame at the old size,
value 4 the completion of the frame that carries the repaint at the new size.

| Cell | Runs | First step with value >= 4, vs the final paint: before / median after / worst after | That step vs the legacy reveal | Runs where it never came |
| :-- | :-: | :-- | :-: | :-: |
| idle smaller | 20 | 0 / +2.4 / +3.6 ms | 0 before, 20 after (median +2.9 ms) | 0 |
| idle larger | 20 | 0 / +4.6 / +7.6 ms | 0 before, 20 after (+8.7 ms) | 0 |
| loaded smaller | 20 | 0 / +15.9 / +127.9 ms | 0 before, 20 after (+76.9 ms) | 0 |
| loaded larger | 20 | 0 / +19.5 / +129.0 ms | 0 before, 20 after (+62.9 ms) | 0 |
| slow smaller | 10 | 0 / +3.4 / +4.5 ms | 0 before, 10 after (+103.8 ms) | 0 |
| slow larger | 10 | 0 / +5.2 / +8.0 ms | 0 before, 10 after (+109.2 ms) | 0 |

- The counter reached a settled even value after the client's final paint in 100 of 100 runs and
  never before the paint. The legacy reveal came first in 100 of 100 runs.
- Reacting to the first step of any value would have been wrong in 20 of 20 idle runs per spec, 10 and
  16 of 20 loaded, and 10 of 10 slow (the first step is the old-size initial frame). Reacting to the
  first step with value >= 4 was wrong in 0 of 100.
- A rule using only the counter value at the resize call (wait for B >= v0 + 3 when v0 is odd) also
  held, offline: reached in 100 of 100 runs, 0 before the final paint. The even-v0 case was never
  exercised (hypothesis).
- With the slow client the counter tracked the delayed paint: the step followed the 100 ms delayed
  paint by 3.4 to 5.2 ms while the legacy reveal came about 104 to 109 ms earlier.
- In the 4-run smoke the first step of counter B came 2.7 to 5.5 ms before the final paint; the log
  reads that first step as the old-size frame still in progress.

## Not verified

Any client other than this GTK4 one; a Qt, Chromium or Electron counter; the even-v0 case; the
delay between the client's step and the Shell's texture update; visible flashes (no screen capture);
whether the odd values would add information (the poll missed them). Under load the watcher's
timestamps are upper bounds by the scheduling delay of a starved process.

## Sources

- `docs/TEST_LOG.md` section 6.33 (lines 1669-1721)
- [X11 and XWayland](/spawn-at/environments/x11-xwayland/)
