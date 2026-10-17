---
title: "6.14 Mutter API leads and an arrival-wait simulation"
---

## Title

Mutter API leads and an "arrival wait" simulation (read-only, nothing implemented)

## ID

TEST_LOG 6.14. Follows [6.13](/spawn-at/experiments/6-13-cloak-starves-frame-callbacks/).

## Date

2026-10-12.

## Build

Not recorded: no commit hash and no version string. Nothing was implemented.

## Question

Which Mutter APIs on GNOME 46 could tell the extension when a client's resize has arrived, or pin a
window's anchor corner in the same operation? And would a rule that waits until the frame equals the
requested size, with a cap, avoid more flashes per millisecond of cloak than the fixed grace of 6.12?

## Setup and environment

`Meta-14.typelib` (mutter 46.2) introspected with `gjs`; the installed `libmutter-14-dev` headers
searched with grep; upstream files read through a fetch tool whose summaries the log calls
unverified; and an offline simulation on the stored data (the 320 stored loaded runs, the 320 idle
runs, and the 120 fixture-trace runs of [6.13](/spawn-at/experiments/6-13-cloak-starves-frame-callbacks/)).
The log tags every statement as [typelib], [headers], [web], [sim] or [inference].

The simulated rule: instead of the 30 ms quiet rule, wait until the frame first equals the requested
size (a `size-changed` sample at that size), with a cap C; on the cap, fall back to today's rule.

## Commands run

`gjs` introspection of `Meta-14.typelib`, grep of the mutter headers, and the offline simulation over
stored `report.json` data. Exact command lines not recorded.

## Raw result location

Not in the repository (the log's header says raw results and scripts are not).

## Result

**API on GNOME 46 (confirmed by typelib and headers).** `Meta.Window` has the signals
`workspace-changed`, `focus`, `raised`, `unmanaging`, `unmanaged`, `position-changed`,
`monitor-changed`, `shown`, `size-changed`, `highest-scale-monitor-changed`, and no `configure`. A
`configure` signal exists only on `Meta.WaylandSurface`, with no arguments. `add_external_constraint`,
`remove_external_constraint`, `get_min_size` and `get_max_size` are not present on 46.

**Upstream (indirect, from summaries):** those four functions are absent from `window.h` at tags 47.0,
48.0 and 49.4 and present at 50.0 and `main`, so the log's hypothesis is that they came with mutter 50.
A constraint that recomputes the position from the size could in principle pin the anchor in the same
operation as a late client resize; this is the log's inference and cannot be tested on 46.

**A latent defect noted in the extension (read, not run):** the extension's `_getMinSize` calls
`window.get_min_size?.()` and reads `size[0]`, `size[1]`. On 46 the method is missing, so it returns
`[0,0]`; the log infers that on GNOME 50 GJS would return `[true, width, height]` and the code would
read the width into the height slot, so it needs `[ok, w, h]` handling before it is relied on.

**Simulation, stored loaded runs (22 threads):** a fixed grace of G = 50, 100, 150, 200 and 300 ms avoided
66, 109, 118, 124 and 130 of 134 flashes at a mean extra of 28.5 to 51.9 ms. An arrival wait with the
minimum unknown avoided 55, 102 and 123 flashes for C = 100, 150 and 250 at a mean extra of 30.9, 61.2
and 120.0 ms; with the minimum known, the same flashes at a mean extra of 10.7, 19.1 and 27.7 ms; a
hybrid (arrival for compliant sizes, today's answer-then-quiet when the app answers with a different
size) at 23.0, 33.5 and 43.4 ms. At idle the fixed grace costs a mean 0.7 ms; the arrival wait with
the minimum unknown costs 32 to 117 ms on the 180 below-minimum specs.

The log lists edge cases that break "wait until frame equals target" (an app that ignores the request,
terminals that snap to a size grid, an app that resizes twice, a request the compositor clamps,
maximised or fullscreen windows, X11 clients, an unknown minimum) with a proposed fallback for each.

## Verdict

"Do not build the arrival wait with the cap as the only stop." The log's recommendation: wait for
equality only for requests that can be met; treat the first `size-changed` to any other size as the
answer and hand over to today's quiet rule; a cap of 150 ms (102 of 134 loaded flashes avoided at a
mean 33.5 ms extra with the hybrid, 0 cost at idle); a learned per-app minimum in memory. External
constraints and `get_min_size` would need GNOME 50 and are to be revisited there.

## Not verified

No code was written. The constraint path, GJS's handling of the interface and of out-arguments, and
the upstream version claims were not verified beyond the summaries. The simulation uses the fixture and
stored runs only: an app that never changes its frame is not in the data, arrival times are
approximations, the minimum is taken from the final size, and it assumes the app's timing is
unchanged by a longer cloak.

## Sources

- `docs/TEST_LOG.md` section 6.14 (lines 443-528)
