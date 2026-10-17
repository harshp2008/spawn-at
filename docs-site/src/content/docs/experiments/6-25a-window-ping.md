---
title: "6.25a Window ping"
---

## Title

Is a window ping observable? (X11 investigation, stage 1, phase 1a)

## ID

TEST_LOG 6.25a (the first part of section 6.25 of `docs/TEST_LOG.md`).

## Date

2026-10-14.

## Build

Tag: none recorded. Tree: branch `refactor/v0.2.0-beta.1`, HEAD `b09e27f`, tree clean, extension
ACTIVE, installed files identical to `assets/gnome/modern`. The output of `spawn-at --version` is
not recorded for this section.

## Question

Is a window ping observable (the log's phase 1a question)? The wider question of the
investigation, as the [X11 and XWayland](/spawn-at/environments/x11-xwayland/) page puts it, was how
to tell that an X11 client has painted at its new size.

## Setup and environment

`gjs` with the `Meta-14` typelib, run outside the Shell. Introspection only: no window was pinged.
The log's label for the whole section: fixture only, nothing installed, no default changed.

## Commands run

`gjs` introspection of the `Meta-14` typelib. Exact command line not recorded.

## Raw result location

`scratch/p26/` (gitignored). Not in the repository.

## Result

- `Meta.Window.check_alive(timestamp: guint32) -> void` exists. `Meta.Window` has the read-only
  boolean property `is-alive`. `Meta.prefs_get_check_alive_timeout()` returned 5000 ms. `Meta.Window`
  also has `can_close`, `delete` and `kill`; `Meta.Display` has `close`.
- No signal or property reports a pong or its time. `Meta.Window` has 10 signals
  (`workspace-changed`, `focus`, `raised`, `unmanaging`, `unmanaged`, `position-changed`,
  `monitor-changed`, `shown`, `size-changed`, `highest-scale-monitor-changed`); none concerns a ping.
  `Meta.Display` has 29, none concerns a ping.
- The only observable is `notify::is-alive`, which changes only when a window fails to answer within
  the 5000 ms timeout. That when-it-flips statement is a hypothesis in the log: it was read from the
  property's existence, not run.
- A reply that arrives in time leaves no trace the extension can read. `check_alive` is an action
  that makes Mutter send a ping and may bring up the "not responding" dialog for a hung
  application. It is not an observation.

## Verdict

No usable ping/pong API. Nothing was added for it (the log's phase 3b: ping/pong logging, nothing
added).

## Not verified

When Mutter flips `is-alive`. Anything inside a running Shell: the introspection was done outside
it.

## Sources

- `docs/TEST_LOG.md` section 6.25, "Phase 1a" (lines 1363-1368) and "Phase 3b" (line 1391)
- [X11 and XWayland](/spawn-at/environments/x11-xwayland/) ("What was tried to make X11 flicker-free")
