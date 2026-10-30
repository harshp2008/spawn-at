# Checkpoint C: what the blank frame is, and which signal marks the first full paint

Task C, measured 2026-10-30 ~22:04–22:12 (plus the cells of Tasks A, B and D). Live pair 5787 / 9315,
unchanged.

## What is on screen, and for how long (MEASURED)

| App / cell | bad runs | first visible frame | duration (capture) | sample |
| :-- | --: | :-- | --: | :-- |
| editor X11 1100 gate warm | 3 of 20 | frame 1100x760, texture 1222x882 (already final), **pure black** | 65–75 ms | `samples/editor-1100-gate-warm-blank-first-frame.png` (mean 0, max 0) |
| editor X11 1100 gate cold-reset | 2 of 10 | same, blank at final texture size | 10.8 / 17.4 ms | - |
| Qt assistant X11 gate cold | 3 of 4 | frame 700x500, texture final, near-black with a few non-black pixels | 4.9–23.4 ms | `samples/qt-gate-cold-blank-first-frame.png` (mean 0.00045) |
| Chrome X11 warm gate | 2 of 4 | 700x480/1200x800, texture final, near-black | 12–30 ms | `samples/chrome-warm-gate-blank-first-frame.png` (mean 0.0024) |
| calculator X11 900x700 gate | 5 of 40 | frame 900x700, texture final, blank | up to 37.8 ms | - |
| editor X11 640 gate warm / cold / cold-reset | 0 of 50 | - | - | - |

The good frame after the editor blank is a fully painted window
(`samples/editor-1100-gate-warm-good-first-frame.png`). For the editor the blank is a **pure black
rectangle exactly at the final frame rect**; for Qt and Chrome it is near-black, not transparent.

Relation to the client's first final-size commit: in all of these the compositor's texture is
already the final size at the first visible frame (`gate_open_first=true` in `probe-runs.csv`), i.e.
the client has committed a final-size buffer, and that buffer is blank. The old-size class is a
different thing: the first buffer is the previous size (eog 16 ms, editor 60–82 ms, calculator
71–222 ms with the gate off) and the texture reading catches it.

## Signal candidates

### 1. X damage after the final-size configure (MEASURED from the X observer + probe join)

R1 = first damage after the first size change that follows the resize, +1 frame. R2 = end of the
damage burst, +1 frame. The `extra` is relative to the reveal (negative = the rule fires before the
current reveal; `fix` = the rule's reveal would be after the client's last change; the exact
definitions are the 03b ones in `tools/src/analyze.rs`).

| Cell | bad runs | class | D per run | R1 fired / fixed / extra ms | R2 fired / fixed / extra ms |
| :-- | --: | :-- | --: | :-- | :-- |
| editor 1100 gate warm | 3 | blank | 0.7 | 0 / 0 / - | 0 / 0 / - |
| editor 1100 gate cold-reset | 2 | blank | 0.0 | 2 / 0 / -50.3 | 2 / 0 / -50.3 |
| Qt gate cold | 3 | blank | 11.0 | 2 / 0 / 14.4 | 2 / **2** / 47.4 |
| Qt legacy cold | 2 | blank | 12.0 | 1 / 0 / 16.7 | 1 / **1** / 44.3 |
| calculator gate on | 5 | blank | 0.0 | 5 / 0 / -68.7 | 5 / 0 / -67.8 |
| Chrome warm gate | 2 | blank | 15.5 | 2 / 0 / 3.4 | 2 / 0 / **231.3** |
| eog legacy cold | 2 | stale | 0.5 | 2 / 1 / 18.8 | 2 / **2** / 32.8 |

- For the editor's 3 gate-warm blanks there were **no damage events at all** (D 0–2 over three
  runs): X damage does not mark those frames.
- For the editor cold-reset and the calculator the rules fire *before* the reveal (negative extra):
  they mark the resize, not the first full paint.
- Qt is the one toolkit where the end-of-burst rule (R2) fixes the blank frames, at the cost of
  ~44–47 ms extra delay (2 of 3 and 1 of 1; n is tiny).
- Chrome's R2 would add 231 ms and still not fix (2 of 2).
- The texture gate's own price on stale-size frames is 13.9 ms (04, eog, READ) and 14–20 ms here.

### 2. `_NET_WM_SYNC_REQUEST` (MEASURED + READ)

MEASURED, `xprop` on the private Xwayland (`logs/protocols.log`): **every** app lists it and has a
counter. Window ids 0x600004 (editor), 0x600004 (calculator), 0x600006 (Qt), 0x600007 (eog),
0x600003 (Chrome); GTK windows have two CARDINAL values (e.g. `6291462, 6291463`), Qt/Chrome one.

```
WM_PROTOCOLS(ATOM): protocols  WM_DELETE_WINDOW, WM_TAKE_FOCUS, _NET_WM_PING, _NET_WM_SYNC_REQUEST
_NET_WM_SYNC_REQUEST_COUNTER(CARDINAL) = 6291462, 6291463
```

MEASURED, observer with `--sync` (`data/C-sync-*`, 2 runs per app, 8 windows): the counter is
attached at window creation with value **0**, and **never changes** — not during the programmatic
resize, not at the reveal (`{"event":"value","hi":0,"lo":0}` once per window, no further lines).
The observer polls the 64-bit value after every X event, and the resize produces damage events, so
a change during the window would have been seen.

READ, Mutter 46 source: `src/x11/window-x11.c`, `meta_window_x11_move_resize_internal`, lines
1503–1511: `meta_sync_counter_send_request()` is called **only** inside
`if (mask != 0 && window_drag && window == meta_window_drag_get_window(window_drag) &&
meta_grab_op_is_resizing(...))` — an interactive resize drag. A programmatic
`move_resize_frame` (the spawn-at path) takes the `else` side and sends no request. The request
itself is a `ClientMessage` `_NET_WM_SYNC_REQUEST` with `wait_serial = last + 240`
(`src/x11/meta-sync-counter.c`, `meta_sync_counter_send_request`, line 220; the client advances
the counter, and Mutter's alarm sees it).

Conclusion: the protocol is advertised and the counter exists, but Mutter 46 does not send the
request for this resize, so the counter is not a usable "client has finished the resize" signal
for spawn-at. Confidence: high for the measured counter values in this Shell and for the source
path; the only way this could differ is a Mutter build that sends the request elsewhere (I read
`window-x11.c`, `window.c` and `meta-sync-counter.c` of the gnome-46 branch).

URLs: https://gitlab.gnome.org/GNOME/mutter/-/blob/gnome-46/src/x11/window-x11.c#L1503 and
https://gitlab.gnome.org/GNOME/mutter/-/blob/gnome-46/src/x11/meta-sync-counter.c#L220

### 3. XWayland surface commit count seen from the compositor

**Not run / not found.** The probe already reads the surface texture size and the buffer rect on
every stage paint (`actor.get_texture().get_texture()`); no commit counter is exposed through that
API. A standalone `gjs` in the sandbox cannot load the Meta typelib ("Typelib file for namespace
'Meta' not found"), and I did not add a probe inside the Shell to enumerate Meta.WindowActor /
MetaShapedTexture signals. Whether Mutter exposes a buffer-attach/commit counter at all is
**unknown**; if it does, it would be the signal that matches "a buffer was committed", not "the
buffer has content", so it would still need the pixel check for the blank class.

### 4. Frame callbacks

**Not run.** The headless Shell has no vblank, and X11 clients' frame callbacks (Present/vblank)
are not visible to the extension. The client-side frame callback marks when the client may paint,
not when the paint is complete.

### Per toolkit: which signal fires before the blank frame

| Toolkit | Texture size | X damage | Sync counter | What the blank frame needs |
| :-- | :-- | :-- | :-- | :-- |
| GTK4 editor | already final → opens at 0 ms | absent for the 3 gate-warm blanks; fires early (R1/R2 negative) in the cold-reset blanks | never moves | a content signal; damage is not it |
| Qt 5.15 | already final | R1 +14 ms (no fix), R2 +47 ms (fixes 2/3) | never moves | end of damage burst, or native launch |
| Chromium | already final | R2 +231 ms, no fix | never moves | native launch; X-side has nothing cheap |
| GTK4 calculator | already final | R1/R2 fire before the reveal, no fix | never moves | a content signal; damage is not it |
| GTK3 eog (stale-size only) | fixes it at ~14–20 ms | R2 fixes it at ~33 ms | never moves | the texture gate is enough |

## Extra delay a gate on that signal would add (estimate)

- Damage R2 (end of burst): **~33 ms** eog stale, **~44–47 ms** Qt blank, **~231 ms** Chrome blank
  (and no fix for Chrome), **no fix** for editor/calculator. MEASURED on 1–5 runs per cell; the
  estimates are coarse.
- Sync counter: not a signal (no update), so no delay estimate.
- Texture gate: **13.9 ms** (04 passive eog, READ) / 14–20 ms here (gate wait on the matching runs).

## Not determined

- A compositor signal for "the first final-size buffer has content" — none of the four candidates
  is both available and content-correct for the editor/calculator/Chrome blank class.
- Whether a commit counter exists in Mutter 46's public API (candidate 3).
- The live session's blank frames; the pure black 640 editor frame was not reproduced here.
