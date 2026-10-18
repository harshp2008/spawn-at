---
title: "6.29 Pixel probe retry: the Shell crashed"
---

`x11-pixel-probe`, live retry with the `Mtk.Rectangle` fix: stopped after phase 1, GNOME Shell crashed in the batch. The probe worked in the smoke run, then the batch crashed GNOME Shell; the probe must not be used.

| Field | Value |
| :-- | :-- |
| **ID** | TEST_LOG 6.29. Follows [6.28](/spawn-at/experiments/6-28-premap-and-pixel-probe-live/). |
| **Date** | 2026-10-14. The crash is logged at 09:59:04 on that date. |
| **Build** | Tag: none recorded. Tree: HEAD `2d8fc5e`. The installed extension included the `Mtk.Rectangle` fix, commit `02ae11e`, "the pixel probe passes a real Mtk.Rectangle as the get_image clip (not installed, not yet run)". The output of `spawn-at --version` is not recorded for this section. Session: GNOME on Wayland, gnome-shell pid 553983 (started 09:57:09), extension ACTIVE, protocol 6. |
| **Verdict** | The probe as built can crash the Shell and must not be used. It was removed together with the other X11 experiments. |
| **Raw results** | `scratch/p30r/`, with the discarded batch in `scratch/p30r/idle_DISCARDED_shell_crashed/` (gitignored). Not in the repository. |

## What was tested

**Question**

Does the pixel probe work once the clip is a real `Mtk.Rectangle`, and can it tell a blank new buffer
from a painted one?

**Setup and environment**

Fixture only: `--protocol x11 --state-colors`. A smoke of 3 idle `larger` spawns (09:58:25 to
09:58:32), then an idle batch of 20 runs per spec with arm `x11-pixel-probe`, logging switched on by
the batch script, started at 09:59:04.

**Commands run**

Exact command lines not recorded.

## Result

**Phase 1, the smoke, worked in 3 of 3 runs.** The `Mtk.Rectangle` clip was accepted, `get_image` returned
a surface for the cloaked actor, and the PNG round trip worked (a valid 2x2 PNG with four `#00FF00`
pixels). At `first-frame` (texture 400x300) the samples were a blank, fully transparent buffer in 3 of
3 runs; at `damaged` (texture 800x600) they were the fixture's green in 2 of 2 runs that had one.
Readback cost was 2.26 to 2.68 ms per probe in warm runs and **89.8 ms for the first probe after the
extension started**, a stall of the Shell's main loop (a one-time cost the log attributes to lazy
imports and the first GPU readback). A logged class `mixed` for the green samples was a decoding bug in
the probe's code (the PNG was solid); it was not fixed in the tree.

**Phase 2, the batch, crashed GNOME Shell at 09:59:04**, in the first second, at its first run. The
journal showed `SET_LOGGING enabled=true`, `ARM_SPAWN`, `WINDOW_CREATED`, then
`static JSObject* CairoSurface::from_c_ptr(JSContext*, cairo_surface_t*): assertion 'surface' failed`,
`Call to function "_addTimer/id<" (GLib.SourceFunc) terminated with uncatchable exception` and
`GNOME Shell crashed with signal 5`. systemd reported the Shell's main process exited with
`code=dumped, status=5/TRAP`. **The session was restarted (a new gnome-shell pid 558192 at
09:59:18/19) and the project owner's session applications were lost.**

The cause, from the log: the probe's timer callback called `WindowActor.get_image(clip)`, which
returned a NULL `cairo_surface_t` (`may_return_null=true` in the typelib). GJS asserts and aborts on a
NULL cairo surface instead of giving JS `null`, so the probe's `if (!surface)` check and its
`try/catch` never ran. Why the surface was null is a hypothesis in the log: probably the first probe of
a fresh window before its texture had content.

**Valid runs: 0.** The batch script recorded 23 runs, but the first of them started after the crash:
runs 1 to 4 went to a dead Shell and the rest to the new session, where no extension-claimed window was
found. None is data. The loaded batch never started.

**A process slip, recorded by the log's author.** Before the first spawn the journal of that Shell held
one Mutter line, `surface_state_changed: assertion 'wl_window->has_last_sent_configuration' failed`, 6 s
after the Shell started. The author called it a baseline and went on; the log says that under the rule
"stop if the journal shows a gnome-shell assertion" the author should have stopped and asked. It was
not the crash: the Shell lived until 09:59:04.

## Not verified

Whether the probe separates blank from painted buffers at scale (no valid batch exists); why the
surface was null; whether the pixel readback is cheap in warm state beyond 4 smoke samples.

## Sources

- `docs/TEST_LOG.md` section 6.29 (lines 1535-1554)
- `git log` for `02ae11e`
- [X11 and XWayland](/spawn-at/environments/x11-xwayland/) ("What was tried to make X11 flicker-free")
