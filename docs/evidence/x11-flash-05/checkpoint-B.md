# Checkpoint B: gnome-calculator ends at 360x492

Task B, measured 2026-10-30 ~21:36–22:05 in the private Shell (live pair 5787 / 9315, unchanged).
The cell app is `gnome-calculator` (`GDK_BACKEND=x11`, `GSETTINGS_BACKEND=memory`), requested
900x700, gate on / gate off / native Wayland, 40 runs each. Warm is not possible (below).

## Numbers (MEASURED, capture mode)

| Variant | runs | final 900x700 | final 360x492 | other | bad texture | bad pixels (blank) | worst blank ms |
| :-- | --: | --: | --: | :-- | --: | --: | --: |
| X11, gate on | 40 | 35 | **5** | - | 0 | **5** | 37.8 |
| X11, gate off (`legacy`) | 40 | 31 | **8** | 1 (360x503) | **37** | 37 | 221.7 |
| native Wayland | 40 | 40 | 0 | - | 0 | 0 | - |

- The wrong final size is a **race the gate neither causes nor prevents**: 5 of 40 with the gate,
  9 of 40 without it (binomial spread; 04 measured 20 of 80 and 21 of 80 across its cells).
- The wrong runs are 360x492 (the app's natural size), one 360x503. There is nothing "minimum"
  about the lost 900x700: a minimum can only enlarge.
- In these 40 gated runs **no wrong-size run changed size after the reveal** (`size_changed_after`
  is 0 in all 40): the shrink landed before the gate resolved, and the gate then matched the
  *new* 360x492 buffer rect. The 03b observation of a shrink ~23 ms after the reveal was 3 of 40
  runs in a different session; it is a rarer tail of the same race.
- Gate off: 37 of 40 runs have a bad **texture** frame (2 painted frames, 71–222 ms) — the old-size
  calculator content in the 900x700 frame. The gate removes that class completely; the blank frames
  at the final size (5 of 40) remain.

## Warm: not possible (MEASURED)

With a calculator window open (pid 234607, 360x492), a second `gnome-calculator --new-window`
(pid 234739) exited after ~0 s; `query windows` still showed only the first window. The app is
single-instance and activates the existing window instead of creating one, so a warm batch would
have no new window for spawn-at to claim. `--new-window` is not a real option of this build.

## GSettings (MEASURED)

- The schema is **`org.gnome.calculator`** (the task's `org.gnome.Calculator` does not exist and
  `gsettings` fails with "No such schema").
- `window-size` default is **`(-1, -1)`**, described as "Window size (width and height) of the last
  closed window"; `window-position` default `(-1, -1)`. The **live user's dconf has `(900, 700)`**.
- `gsettings list-recursively org.gnome.calculator` before and after every variant is byte-identical
  in the sandbox: `window-size (-1, -1)`. The app runs with `GSETTINGS_BACKEND=memory` in the
  batch, so nothing it writes reaches the keyfile. The batch's environment, not the app, decides
  this: the same app launched through `psh` (keyfile backend) writes its closed size.

## The app restores a saved size (MEASURED)

Manual tests (`logs/B-manual.log`, `logs/B-manual2.log`, script in `logs/manual-calc.sh`), keyfile
backend:

| Test | Result |
| :-- | :-- |
| saved `(900, 700)`, launch **without** spawn-at, read at 2 s | **360x700**: the saved height is applied, the width is clamped to the app's 360 |
| close that window | the app writes `window-size (360, 700)` |
| relaunch | 360x700 again (restore works) |
| saved `(900,700)` keyfile cell, 20 runs at 900x700 | **20 of 20 final 900x700** (0 wrong) |
| saved `(800,600)` keyfile cell, 10 runs at 900x700 | **10 of 10 final 900x700** (0 wrong) |
| saved `(360,700)`, 6 manual spawn-at runs at 900x700 | 6 of 6 final 900x700 |
| no saved size (`memory` backend, `-1,-1`) | 5 of 40 wrong (gate on), 9 of 40 (gate off) |

So with **any** saved size present the natural-size reset did not happen in 36 runs; with no saved
size it did in 5–9 of 40. The live session has a saved `(900,700)`, so the sandbox's 25% rate must
not be assumed for the live session. That is the single most important caveat of Task B.

## Order matters (MEASURED)

- Launch at the natural 360x492, wait 2 s (map handling done), then `spawn-at transform --id … --size 900 700`:
  the size sticks at 900x700 at +0.5 s and +2 s.
- Six consecutive `spawn-at spawn … 900 700` runs with a saved size: all 900x700.
- We did not test a *delayed* pre-map SetSize inside spawn-at (no product change). The late-resize
  correction test is the cheap proxy, and it works.

## Candidate causes (each with the test that would confirm or kill it)

| # | Cause | Label | Test | Result |
| :-- | :-- | :-- | :-- | :-- |
| 1 | App applies its natural 360x492 at first map; it races spawn-at's pre-map SetSize | **MEASURED** (rate + the keyfile arms) | save any size, re-run the cell | 0 of 30 wrong with a saved size vs 5 of 40 with none |
| 2 | The lost size is a minimum or a clamp | **MEASURED false** | final 360x492 < 900x700 | impossible for a minimum |
| 3 | The app restores a stale/saved size that is not the request | **MEASURED** (it does restore) but it is not the cause of 360x492 in the sandbox, where no saved size exists | keyfile arms | with a saved size the rate is 0 |
| 4 | The order (size before vs after the app's map handling) decides | **MEASURED**: a size after map handling wins; the sandbox's reset happens at map time, before the reveal | transform 2 s after map | sticks |
| 5 | The gate's cap / polling lets the app reset first | **MEASURED no**: the 5 wrong runs had no post-reveal change | `size_changed_after`, `L_last_ms` | 0 of 40 |
| 6 | A second launch / warm state changes the rate | **not applicable** (no warm window) | second launch | no window |

## Options for spawn-at-side prevention or correction (not implemented)

1. **Re-apply the size once after the reveal** (only when the window is not at the request).
   Cheap, uses the existing `transform` path, and a late size demonstrably sticks. Risk: a visible
   second resize / anchor jump for the exact apps that reset; needs a settle delay and a one-shot cap.
2. **Watch `size-changed` after the reveal and correct once** if the app moved away from the
   requested size, with the anchor kept. Risk: loops for apps that legitimately resize (video,
   dialogs); the current anchor listener already fires only on `size-changed`.
3. **Wait for the app's first map-time resize before SetSize** (a longer quiet window). Risk: more
   hidden time for every X11 app, against the "no flash, fast" goal; the reset can also come later.
4. **Set `org.gnome.calculator window-size` before launch** (app-specific GSettings write). Risk:
   invasive, changes user state, only helps this app; not acceptable as a general rule.
5. **Do nothing and document**: the sandbox rate is produced by `GSETTINGS_BACKEND=memory`; with the
   live saved size the measured rate is 0 of 36. Risk: unverified live, and the user's live report
   says the 25% happened there too.

There is no evidence for a prevention that is both general and invisible. Option 1/2 are the only
general corrections; both buy a second placement instead of a hidden wait.
