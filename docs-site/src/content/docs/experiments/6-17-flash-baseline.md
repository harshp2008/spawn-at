---
title: "6.17 Flash baseline before the ready-wait change"
---

## Title

BASELINE before the ready-wait change: flash on native Wayland, XWayland and X11

## ID

TEST_LOG 6.17. The comparison point for [6.18](/spawn-at/experiments/6-18-ready-wait-live-validation/).

## Date

2026-10-13.

## Build

Binary commit `0756167`, the catalog commit that added `placement/top-left-smaller` (36 tests); the
log gives no version string. The log's emphasis: no product or extension code was touched, and nothing
in it is a result of the ready-wait change. `bottom-right-larger` ran with a debug `spawn-at`; the
"release" batch used a release fixture with a debug copy of `spawn-at`, because the harness requires the
binary's commit to equal `HEAD`.

## Question

How often does a window flash on native Wayland, on XWayland and on X11 under the rule that existed
before the ready-wait change?

## Setup and environment

`XDG_SESSION_TYPE=wayland`, GNOME Shell 46, 22 CPUs, 60.00 Hz. Every run is one isolated `run`
invocation, specs interleaved, 1.5 s apart, 10 runs per spec per batch; all exit 0 and `auto-pass`.

| batch | fixture | `spawn-at` | load (1 min) at run start |
| :--- | :--- | :--- | :--- |
| idle, debug | debug | debug | 1.38 - 2.91 |
| idle, release | release | debug copy | 1.05 - 2.37 |
| loaded | debug | debug | 20.53 - 24.49 (mean 22.7), after 22 `burn-cpu` threads and a 70 s warm-up |
| XWayland | debug (`--protocol x11`) | debug | 1.67 - 1.96 |

Specs: `bottom-right-larger`, `bottom-right-smaller`, `top-left-smaller` and a delayed-paint control
(throwaway specs in `scratch/`).

## Commands run

`spawn-at-verify run` per spec, and `cargo build --release --bin fixture`. Full command lines not
recorded.

## Raw result location

Not in the repository. The log states the scratch specs and batch scripts are not committed, and that
the raw per-run JSON stayed in the session scratchpad.

## Result

- **Idle, native Wayland: 0 numeric flashes in all 80 runs** (10 debug and 10 release runs for each of
  four specs). In the debug `bottom-right-larger` runs `COMMIT_RESOLVED reason=AT_TARGET` came 12.9 to
  20.6 ms after `SET_SIZE_REQUEST`: the extension waited for the client and revealed at the new size.
- **The delayed-paint control does not work on Wayland:** 0 flashes in 20 of 20 runs, whereas on X11 it
  flashed 10 of 10 (6.15). `CLOAK` came 430.6 to 444.3 ms (debug) after `WINDOW_CREATED`, so the 400 ms
  paint delay moved the point at which the extension saw the window. The log's hypothesis is that on
  Wayland the surface is only mapped once its first buffer is committed.
- **Loaded, 22 threads, debug:**

| spec | runs with a numeric flash | numeric events | certain / possible | lead median / max, ms | runs with an old warning |
| :--- | :---: | :---: | :---: | :--- | :---: |
| `bottom-right-smaller` | 6 / 10 | 11 | 8 / 3 | 29.7 / 78.9 | 6 |
| `bottom-right-larger` | 9 / 10 | 16 | 14 / 2 | 32.1 / 60.0 | 9 |

  Together 15 of 20 runs (75%) were corrected after the reveal. Clock uncertainty under load was 6.3 to
  17.2 ms. The log compares this with 6.12 (91 of 120, 76%, agrees) and 6.13 (10 of 40, 25%, does not
  agree) and does not establish why.
- **XWayland, `bottom-right-larger` with `--protocol x11`, 10 debug runs, idle:** recorded as
  `proto=XWayland` in 10 of 10; the frame rect jumped to the target at once and `COMMIT_RESOLVED` came
  0.1 to 0.3 ms after the request; 1 of 10 runs had a numeric flash (lead 6.4 ms, visible probability
  0.38).

| | native Wayland | XWayland | native X11 (copied from 6.15) |
| :--- | :--- | :--- | :--- |
| `bottom-right-larger`, idle, debug: flash runs | 0 / 10 | 1 / 10 | 9 / 10, 10 / 10 |
| `bottom-right-larger`, idle, release fixture | 0 / 10 | not run | 0 / 10 |
| `bottom-right-larger`, 22 threads, debug | 9 / 10 | not run | not run |
| old warnings, idle | 0 of 60 | 0 of 10 | 0 of 90 |

## Verdict

A baseline, not a verdict. The log records that the X11 column is not re-measured (this session is
Wayland), and the knowledge base notes the XWayland rate of the old rule varied between logins: 1 of
10 here and 9 of 10 and 10 of 10 in [6.18](/spawn-at/experiments/6-18-ready-wait-live-validation/);
its explanation is a hypothesis.

## Not verified

XWayland under load, with the release fixture, or for any spec but `bottom-right-larger`;
`top-left-smaller` under load; native X11 in this session (needs a log-out); a release-built
`spawn-at`; any screen capture; real toolkit apps.

## Sources

- `docs/TEST_LOG.md` section 6.17 (lines 634-681)
- `docs/KNOWLEDGE.md` ("Dead ends", the XWayland flash rate depends on the login)
