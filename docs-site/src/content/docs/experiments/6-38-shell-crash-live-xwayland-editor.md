---
title: "6.38 Two Shell crashes closing a GTK app on XWayland"
---

GNOME Shell aborted twice, 34 minutes apart, 6 ms after a graceful close of a focused `gnome-text-editor` window that was forced onto X11 (XWayland). The assertion was the same both times. The cause is **not established**: a focused close alone does not explain it, and the crash did not reproduce in a private Shell ([6.41](/spawn-at/experiments/6-41-bisect-private-shell/)).

| Field | Value |
| :-- | :-- |
| **ID** | X11-FLASH-02 crash notes; not in TEST_LOG. Follows [6.35](/spawn-at/experiments/6-35-ld-preload-geometry-shim/). |
| **Date** | 2026-10-29: 12:32:34 and 13:06:53 (+05:30), same boot. |
| **Build** | GNOME Shell 46.0, Mutter 46.2-1ubuntu0.24.04.18, spawn-at 0.2.4, extension identical to `assets/gnome/modern/`. Tree: branch `exp/x11-flash-02` from `7c053b9`. |
| **Verdict** | Not decidable. Same assertion twice; trigger unknown. |
| **Raw results** | Journal excerpts and run records in the repository (Evidence below). The crash dump in `/var/crash` was not opened. |

## At a glance

| | Crash 1 | Crash 2 |
| :-- | :-- | :-- |
| Time | 12:32:34 | 13:06:53 |
| App in flight | gnome-text-editor, forced onto X11 | same |
| Step | graceful close (`spawn-at close`) | same |
| Focus of the window | spawned focused | **focused** at close, although spawned with `--no-focus` |
| Assertion | `meta_window_unmanage ... focus_window != window` | identical |
| Preceding warnings | timestamp-0 `_NET_ACTIVE_WINDOW`, bad-serial ping, failed focus | same three kinds |
| Time since batch start | about 21 s | about 6.5 s |
| Extension busy at the abort | no | no |

## What was tested

**Question**

Not a planned test: these were the first live batches of the real-application measurement (XWayland, with the X-side observer). The page records what the journal and the run records show.

**Setup and environment**

A GNOME Wayland session, one monitor. The driver spawned the app with spawn-at, waited 2.5 s, then asked the window to close. The X-side observer was attached to Xwayland. Crash 1 was run 2 of a probe batch (after two Chrome runs); crash 2 was run 0 of an editor batch.

## Result

**Measured.** The journal shows, in both crashes, in this order: a warning that a client sent `_NET_ACTIVE_WINDOW` with a timestamp of 0 (a few tens of milliseconds before the extension saw the window), a bad-serial ping warning at the moment of the close, `Failed to focus window W... while avoiding 0x...`, then `libmutter:ERROR ... meta_window_unmanage: assertion failed: (window->display->focus_window != window)` and signal 6. The abort came 6 ms after the extension logged the close. The extension had finished its steps and armed its anchor 2.5 s earlier, so it was **not** mid-operation.

**Same assertion, different state.** The fallback focus target was a Wayland window each time (`W1043`, then `W6`).

**Hypotheses.** None is concluded.

| H | Suspect | Status |
| :-- | :-- | :-- |
| H1 | gnome-text-editor specifically | supported, not exclusive: both crashes were the editor |
| H2 | any GTK app forced onto X11 | supported; only the editor was seen to send the timestamp-0 message |
| H3 | the observer | contradicted as the sole cause: 80 xterm runs under it were clean |
| H4 | the extension's legacy path | contradicted as the trigger: idle at both aborts |
| H5 | Shell already degraded | not decided: odd warnings (invalid geometry, size accounting, stack_position, infinite scale) appeared at 12:11, before any batch |
| H6 | closing a focused XWayland window | **contradicted as sufficient** |

**H6 and the xterm runs.** All 80 live xterm closes were focused (`focused_at_close` true in every record of both xterm batches) and none crashed the Shell. A focused close of an XWayland window is therefore not enough.

**Not caused by the 12:11 state alone.** The same warnings came back in the Shell started at 12:33 without any batch running, and 80 xterm closes in that Shell were clean.

**Observer.** Read from its source: it selects events and creates damage objects, and sends no focus request, no `_NET_ACTIVE_WINDOW` and takes no server grab. It saw about 527 lines per xterm run and 431 lines in the single editor run.

## Not verified

- Who sent the timestamp-0 message. It names the editor's window and arrived 4 ms after the window was created; the app is the likely sender (inferred, not shown).
- Whether the warnings at 12:11 came from another client.
- Upstream (second-hand, from web search snippets): the assertion belongs to a known class in which Mutter unmanages a window that is still the focus window because handing focus to another window failed; no report was found that ties it to Mutter 46.2 and no fix version was found. The Ubuntu changelog of the installed package has no entry about it.
- Anything on a Shell other than 46.0, on X11 sessions, or with other apps.

## Sources

- [Crash 1 note](https://github.com/harshp2008/spawn-at/blob/main/docs/evidence/x11-flash-02/crash-2026-10-29.md) and its [run records and trimmed journal](https://github.com/harshp2008/spawn-at/blob/main/docs/evidence/x11-flash-02/crash-2026-10-29/)
- [Crash 2 note](https://github.com/harshp2008/spawn-at/blob/main/docs/evidence/x11-flash-02/crash-2026-10-29-second.md)
- [Plan and state](https://github.com/harshp2008/spawn-at/blob/main/docs/evidence/x11-flash-02/PLAN.md)
- [The 80 live xterm run records](https://github.com/harshp2008/spawn-at/blob/main/docs/evidence/x11-flash-02/data/xterm-live/) (`focused_at_close` in every `runs.jsonl` line)
- `docs/evidence/x11-flash-02/tools/src/obs.rs` (the observer's requests)
- The links point into the repository on GitHub and resolve once the branch is merged.
