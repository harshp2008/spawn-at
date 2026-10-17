---
title: "6.34 Self-sent sync request"
---

## Title

Answering a sync request that we send ourselves: smoke (stopped after the smoke because counter A never
moved)

## ID

TEST_LOG 6.34. Follows [6.33](/spawn-at/experiments/6-33-sync-counter-moves-on-repaint/).

## Date

2026-10-14.

## Build

Tag: none recorded. Tree: HEAD `a1394ca`. Nothing in the Shell; no extension or binary change; no
install. The output of `spawn-at --version` is not recorded for this section. The journal before and
after showed only the 11:13:32 St/Cogl criticals described in 6.33, no stop-rule line, and the same
gnome-shell process throughout.

## Question

If a watcher sends the client an `_NET_WM_SYNC_REQUEST` message itself, after the window reaches its
target size, does the client's basic sync counter (counter A) move? The log fixed the decision rule in
advance: "A never moves" is BAD.

## Setup and environment

The GTK4 client of 6.32 (`scratch/p33_syncclient.py`) and a throwaway C sender and watcher
(`scratch/p35_watch.c`, gitignored, not committed). When the watcher saw the client window's
`ConfigureNotify` reach the target size, it sent one `ClientMessage` with `XSendEvent` (event mask
`NoEventMask`, propagate false): `message_type = WM_PROTOCOLS`, format 32, `data[0] =
_NET_WM_SYNC_REQUEST`, `data[1] = CurrentTime`, `data[2..3]` = N split in low and high 32 bits,
`data[4] = 0`, with N = (last value of counter A) + 1, which was 1 in every run. It logged the send
time, every change of counters A and B, and the client's event file gave the final `FIRST_PAINT`.
Smoke only: 2 runs per spec, idle, legacy rule, send delay 0 (4 runs).

## Commands run

Exact command lines not recorded.

## Raw result location

Not recorded: the log names no results directory for this section. The sender and watcher are
gitignored files in `scratch/`. Nothing of it is in the repository.

## Result

Counter A did not move in any of the 4 runs, before or after the send. The send happened 0.07 to 1.25 ms
after the target-size `ConfigureNotify` and 2.1 to 5.9 ms before the client's final `FIRST_PAINT`; the
reveal came between 3.2 ms before and 4.2 ms after the send. Counter B behaved as in 6.33 (the first
step after the send was +5.8 to +12.2 ms later, after the final paint). The client wrote no error
lines and did not crash.

| Run | Configure to send | Send vs final paint | Send vs reveal | Counter A changes | Counter B steps after the send (first, ms) |
| :-- | :-: | :-: | :-: | :-: | :-: |
| larger 1 | 0.87 ms | -2.05 ms | +1.37 ms | 0 | 4 (+6.6) |
| larger 2 | 1.25 ms | -5.87 ms | +4.19 ms | 0 | 4 (+12.2) |
| smaller 1 | 0.08 ms | -2.85 ms | -3.16 ms | 0 | 2 (+5.8) |
| smaller 2 | 0.07 ms | -3.19 ms | -2.93 ms | 0 | 2 (+6.5) |

## Verdict

"BAD for this send timing" under the decision rules fixed in the task prompt of that day ("A never
moves"). The large batches (idle and loaded, the slow client, the already-painted case, the
send-delay comparison) were not run, as that prompt required when counter A never changes.

## Not verified

Why counter A did not move. The log's reading is a hypothesis, not checked against GTK's source (not on
that machine): the sync protocol orders the request before the resize, a toolkit keeps the value as
"pending" and associates it with the next configure it processes, so a request sent after the
configure may simply have stayed pending; if so the request would have to be sent before the resize,
a different design that was not tried. Whether Mutter sends sync requests to these XWayland windows
is not observed: counter A never moving is consistent with Mutter sending none, and equally with the
client not answering a request outside that order. No X protocol trace tool (`xtrace`, `xscope`) was
installed, and `strace` cannot attach to Xwayland; `strace` on the client itself was not done.

## Sources

- `docs/TEST_LOG.md` section 6.34 (lines 1724-1743)
- [X11 and XWayland](/spawn-at/environments/x11-xwayland/)
