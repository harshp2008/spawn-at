# X11-FLASH-05 findings

Written 2026-10-30 22:10. Branch `exp/x11-flash-05-ds`. Every number is in `data/<cell>/` (per-run
rows in `probe-runs.csv` and `runs.csv`, summaries in `probe-summary.md`, X-side rules in
`runs.csv`). Tags: **M** = MEASURED in this session's private Shell, **R** = READ from source or
docs, **G** = GUESS / reasoned only. Durations are capture mode; the 03b caveat applies (capture
inflates fast apps). Live Shell pair 5787 / 9315 was recorded before every group and at the end;
unchanged.

## Verdict

1. **The texture gate works for the class it was built for.** In 100 gated editor runs, 40 calculator
   runs, 20 eog runs and the 04 cells there is not one bad **texture-size** frame; without the gate
   every run that has to resize shows the old-size texture for 2 painted frames (60–82 ms editor,
   71–222 ms calculator). **M**
2. **Blank frames at the correct final texture size remain, in every toolkit.** Editor 5 of 100
   gated runs (3 of 20 warm 1100, 2 of 10 cold-reset 1100, 0 of 50 at 640); calculator 5 of 40;
   Qt 3 of 4 in the 05 coarse cell (5 of 20 in 04); Chrome 2 of 4 warm (18 of 20 cold in 04). The
   texture gate cannot see them because the texture is already final. **M**
3. **Cold vs warm is not the split the live observation suggested.** The editor persists its last
   window size in `session.gvariant`; a fixed-size cold cell stops resizing after run 1. The split
   that matters is "does the new window have to resize from a different size", not "did the process
   exist before". Warm with a smaller request (keeper 1100 → 640) was 17 of 20 bad without the gate
   (stale), 0 of 20 with it; warm at the same size was 9 of 20 blank without the gate, 3 of 20 with
   it. **M** The live 640 pure-black frame was **not reproduced** in 50 gated 640 runs.
4. **The calculator's wrong final size is a map-time race, and it is strongly modulated by the
   settings backend.** 5 of 40 with the gate, 9 of 40 without, 0 of 40 native. With any saved
   `window-size` present (keyfile backend) it was 0 of 30; with `GSETTINGS_BACKEND=memory` (no saved
   size) the app applies its natural 360x492. **M** The live user has a saved (900,700), so the
   sandbox's 25% must not be projected onto the live session without a live check.
5. **The `_NET_WM_SYNC_REQUEST` protocol is advertised but dead for spawn-at on Mutter 46.** All
   five apps list it and have a counter; the counter never moves through the programmatic resize.
   Mutter 46 sends the request only for interactive resize drags. **M + R**
6. **Native Wayland is clean for all five apps**: 0 bad frames in 130 runs, 100% requested size,
   100% anchored. **M**
7. **PR #95 should be merged as is, with its documentation corrected.** It removes the larger
   texture-size class and causes no placement regression; it does not fix the blank class, which
   also existed before it. The correction must say blank frames remain for GTK4 (and are the main
   class for Qt and Chromium), and that the gate cannot see them.

## Task A: editor cold vs warm (MEASURED)

Gate on = PR #95 default; gate off = same extension/CLI with `[wait] strategy = "legacy"` (the
texture gate is skipped; `waitForCommit` still runs). 20 runs per cell unless stated.

| Cell | gate | mode | keeper | bad tex | bad px | stale | blank | worst bad_ms | gate wait med/max |
| :-- | :-- | :-- | :-- | --: | --: | --: | --: | --: | :-- |
| 640 cold | on | cold | - | 0 | 0 | 0 | 0 | 0 | 0.0 / 80.3 (run 1) |
| 1100 cold | on | cold | - | 0 | 0 | 0 | 0 | 0 | 0.0 / 63.5 (run 1) |
| 640 warm | on | warm | 1100 | 0 | 0 | 0 | 0 | 0 | 17.3 / 39.8 |
| 1100 warm | on | warm | 1100 | 0 | 3 | 0 | 3 | 74.7 | 0.1 / 0.3 |
| 640 coldreset | on | cold, state deleted | - | 0 | 0 | 0 | 0 | 0 | 62.4 / 76.9 |
| 1100 coldreset | on | cold, state deleted | - | 0 | 2 | 0 | 2 | 17.4 | 67.5 / 251.3 |
| 640 cold | off | cold | - | 1 | 1 | 1 | 0 | 61.9 | - |
| 1100 cold | off | cold | - | 1 | 1 | 1 | 0 | 73.0 | - |
| 640 warm | off | warm | 1100 | 17 | 17 | 17 | 0 | 61.4 | - |
| 1100 warm | off | warm | 1100 | 0 | 9 | 0 | 9 | 57.5 | - |
| 640 coldreset | off | cold, state deleted | - | 10 | 10 | 10 | 0 | 70.5 | - |
| 1100 coldreset | off | cold, state deleted | - | 10 | 10 | 10 | 0 | 81.5 | - |

- The editor **persists `default-window-size`** in `$XDG_DATA_HOME/org.gnome.TextEditor/session.gvariant`
  (the file contained 1100x760). Run 1 of a fixed-size cold cell resizes from the old size and waits
  63–80 ms at the gate; runs 2–20 start at the requested size (<1 ms). The coldreset cells delete the
  file before every run, so every run is the "fresh process, wrong saved size" case. **M**
- The gate removes **every** stale-size frame in this session (0 of 100 gated editor runs; 39 of 100
  legacy editor runs had one). One 1100 cold-reset run hit the 250 ms cap (251.3 ms, texture
  822x642 against buffer 1222x882) and was not counted bad (the texture arrived before the probe's
  first captured visible frame); the 250 ms cap has a tail, as 04 already saw at 318 ms. **M**
- The remaining bad frames are blank at the final size: 3 of 20 at 1100 warm (65–75 ms), 2 of 10 at
  1100 cold-reset (10.8/17.4 ms), none at 640. The user's live 640 black is therefore **not
  reproduced here**; the class is. **M / G** for the reason (real vblank/GPU on the live desktop).
- Cross-check (4 runs per cell, 2 sizes x 2, coarse):
  - eog: gate cold 0, gate warm 0; legacy cold 2 stale (13–16 ms), legacy warm 0. No cold/warm
    split; size drives it. **M**
  - Qt: gate cold 3 blank (5–23 ms), gate warm 2 blank, legacy cold 2 blank, legacy warm 1 stale.
    No cold/warm split; the gate is invisible to it. **M**
  - Chrome (one profile, warm): gate 2 of 4 blank (up to 30 ms), legacy 3 of 4 (up to 34 ms). Cold is
    **R** from 04 (18 of 20 before and after the gate). No cold/warm split.
  - calculator: warm is not possible; cold numbers in Task B.

## Task B: gnome-calculator wrong final size (MEASURED)

Request 900x700, 40 runs per variant:

| Variant | final 900x700 | final 360x492 | other | bad tex | bad px (blank) | worst blank |
| :-- | --: | --: | :-- | --: | --: | --: |
| X11 gate on | 35 | **5** | - | 0 | **5** | 37.8 ms |
| X11 gate off | 31 | **8** | 1 (360x503) | **37** | 37 | 221.7 ms |
| native Wayland | 40 | 0 | - | 0 | 0 | - |

- The gate removes the stale-size class (37 of 40 → 0 of 40) and neither causes nor prevents the
  final-size reset (5 vs 9; 04 measured 20/80 vs 21/80). **M**
- In the 5 wrong gated runs, **no size change happened after the reveal** (`size_changed_after` 0
  of 40): the app's natural-size request reached the WM before the gate resolved and the gate then
  matched the new 360x492 buffer rect. The 03b "23 ms after reveal" was a rarer tail. **M**
- **Warm is not possible**: a second `gnome-calculator` (also with `--new-window`) exits and only
  activates the existing window; no new window for spawn-at. **M**
- **The app does restore a saved size**: schema `org.gnome.calculator`, `window-size` default
  `(-1,-1)` ("last closed window"); the live user has `(900,700)`. With a keyfile backend and a
  saved `(900,700)` the app opens 360x700 (width clamped), writes its closed size back, and **0 of
  30** runs ended at the wrong size; with `GSETTINGS_BACKEND=memory` (no saved size) 5–9 of 40 did.
  The sandbox's 25% is therefore **not** the live configuration; this is the biggest caveat of the
  task. **M**
- **Order matters**: a `spawn-at transform` to 900x700 sent 2 s after the window was mapped sticks
  (900x700 at +0.5 s and +2 s), and 6 spawn-at runs with a saved size all ended 900x700. A
  spawn-at-side correction after the reveal is therefore possible. **M**
- Candidate causes and tests are in `checkpoint-B.md`; the cheap confirmations: (1) natural-size
  map race — confirmed by the keyfile arms; (2) minimum — killed (360x492 < 900x700); (3) saved
  size restore — real, but not the cause of the sandbox rate; (4) order — confirmed by transform;
  (5) gate cap — not the cause (no post-reveal change); (6) warm — not applicable.
- Options (not implemented): re-apply the size once after reveal (visible second resize, cheap,
  works); watch `size-changed` and correct once (loop risk); longer quiet window (hidden time);
  app-specific GSettings write (invasive); document/no fix (unverified live). No general invisible
  prevention is supported by the data.

## Task C: the blank frame and signals (MEASURED + READ)

- **What it is**: the editor's is a **pure black rectangle at the final frame rect**
  (`samples/editor-1100-gate-warm-blank-first-frame.png`: 1100x760, mean 0, max 0). Qt and Chrome
  are near-black (means 0.00045 and 0.0024) with the final texture size. The compositor already has
  a final-size buffer; nothing is painted in it yet. **M**
- **How long**: editor 65–75 ms (gate warm 1100), 10.8/17.4 ms (cold-reset 1100); Qt 5–23 ms;
  Chrome 12–34 ms warm and 150–300+ ms cold (04 **R**); calculator up to 37.8 ms. Capture mode.
- **Signals** (per bad run, from `runs.csv` joined to `probe-runs.csv`; negative extra = rule fires
  before the reveal; small n):

| Cell | bad runs | class | X damage/run | R1 fired/fixed/extra | R2 fired/fixed/extra |
| :-- | --: | :-- | --: | :-- | :-- |
| editor 1100 gate warm | 3 | blank | 0.7 | 0/0/- | 0/0/- |
| editor 1100 cold-reset | 2 | blank | 0.0 | 2/0/-50.3 | 2/0/-50.3 |
| Qt gate cold | 3 | blank | 11.0 | 2/0/+14.4 | 2/**2**/+47.4 |
| calculator gate on | 5 | blank | 0.0 | 5/0/-68.7 | 5/0/-67.8 |
| Chrome warm gate | 2 | blank | 15.5 | 2/0/+3.4 | 2/0/+231.3 |
| eog legacy cold | 2 | stale | 0.5 | 2/1/+18.8 | 2/**2**/+32.8 |

- The editor's gate-warm blanks have **no X damage at all**; the damage rules fire before the
  reveal for the editor cold-reset and the calculator. The texture gate fixes eog's stale class for
  13.9 ms (04 **R**) / 14–20 ms here. R2 fixes Qt's blank frames at ~44–47 ms but adds 231 ms and
  fixes nothing for Chrome.
- **`_NET_WM_SYNC_REQUEST`**: all five apps list it and have a counter (`logs/protocols.log`), but
  the counter stays 0 through the resize in all 8 observed windows (`data/C-sync-*`). Mutter 46
  sends the request only in the interactive-resize branch of
  `meta_window_x11_move_resize_internal` (`src/x11/window-x11.c` lines 1503–1511), not for a
  programmatic `move_resize_frame`; the sender is `meta_sync_counter_send_request`
  (`src/x11/meta-sync-counter.c` line 220). **M + R**
  URLs: https://gitlab.gnome.org/GNOME/mutter/-/blob/gnome-46/src/x11/window-x11.c#L1503 ,
  https://gitlab.gnome.org/GNOME/mutter/-/blob/gnome-46/src/x11/meta-sync-counter.c#L220
- **Commit count / frame callbacks**: not measured. No commit counter is exposed by the probe's API;
  a standalone gjs cannot load the Meta typelib in the sandbox. **G** that no usable content signal
  exists there; **not run**.

## Task D: native Wayland (MEASURED)

| App | runs | bad tex | bad px | final = request | anchored | cloak→reveal med (min/max) |
| :-- | --: | --: | --: | --: | --: | :-- |
| editor cold | 20 | 0 | 0 | 20 | 20 | 3.4 (2.6/52.5) |
| editor warm | 10 | 0 | 0 | 10 | 10 | 4.7 (1.9/8.5) |
| eog + real 3000x2000 image | 20 | 0 | 0 | 20 | 20 | 13.1 (9.3/16.7) |
| Qt assistant | 20 | 0 | 0 | 20 | 20 | 5.8 (3.8/14.9) |
| Chrome (fresh profile) | 20 | 0 | 0 | 20 | 20 | 32.1 (26.9/69.0) |
| calculator cold | 40 | 0 | 0 | 40 | 40 | 11.4 (8.3/20.2) |

0 bad frames in 130 runs; all anchored at x+w=1896, y+h=1056. Capture mode, so the cloak→reveal
values are comparable with 04's passive range (7.9/5.9/37.5/8.8/12.4/7.6 ms) but not identical.
Typing, clipboard and drag-and-drop: **not run** (headless, no seat). Wayland caveats and URLs are
in `checkpoint-D.md`.

## Task E: recommendation per toolkit

Legend: M = MEASURED, R = READ, G = GUESS. "Bad" = runs with a visible bad frame; texture and pixel
columns are separate because the gate only addresses the first. Cold/warm is the measured split in
this Shell; "same" means the bad class appears in both.

| Toolkit (app) | Bad texture, X11 cold / warm | Bad pixels, X11 cold / warm | Fits | Cost | Risk | Known limit |
| :-- | :-- | :-- | :-- | :-- | :-- | :-- |
| GTK3 (eog) | gate 0/20 (04), 0/4 (05) / 0/4; legacy 16/20 (04), 2/4 / 0/4 — **M** | 0 — **M** | texture gate or native | +13.9 ms (04 passive **R**), +14–20 ms here **M** | low; hidden time | larger windows still wait longer; capture only |
| GTK4 (editor) | gate 0/100 **M**; legacy 39/100 **M** | gate 5/100 (3/20 warm 1100, 2/10 cold-reset 1100), 0/50 at 640 **M** | gate for texture + native for the rest; no content signal found | +17–80 ms gate wait **M**; native 3.4–4.7 ms **M** | blank frames remain; live 640 unreproduced **M/G** | texture size cannot see blank content |
| GTK4 (calculator) | gate 0/40 **M**; legacy 37/40 **M** | gate 5/40 **M** | gate for texture; size reset is separate | +50–115 ms (04 passive **R**) | wrong final size 5/40 **M**; live rate unknown (saved size) **G** | app resets at map time; no warm |
| Qt 5.15 (assistant) | gate 0/20 (04 **R**); 0/4 **M** | gate 5/20 (04 **R**), 3/4 (05 **M**) | native Wayland; damage R2 is a costly partial | native 5.8 ms **M**; R2 +47 ms **M** | R2 fixes 2/3, not all; adds delay | blank frames are opaque black at final size |
| Chromium (Chrome) | gate 0/20 (04 **R**) | gate 18/20 cold (04 **R**), 2/4 warm **M** | native Wayland only | native 32.1 ms **M** | X-side has no signal; damage R2 +231 ms and no fix **M** | blank at final size, cold and warm |
| X11-only toolkits (xterm, Swing) | 0 **M** (xterm), 0–2/10 **M** | 0–1 **M** | nothing (or texture gate, free) | 0 | low | not the problem set |

**PR #95: merge as is, with the documentation corrected.** Reasons: it removes the whole
texture-size class (0 bad texture in 100 gated editor runs, 40 calculator, 20 eog; 04: 520+520 runs
with 100% placement), it costs hidden time only (13.9 ms eog, up to ~80 ms editor, 50–115 ms
calculator), and it neither causes nor prevents the blank class (the legacy cells show the blank
class too: 9 of 20 at 1100 same-size warm). The correction: say that blank frames at the correct
final size remain, that they are the main class for Qt and Chromium, and that the gate cannot see
them; add the editor's `session.gvariant` size persistence (a cold fixed-size test is not a resize
test after run 1); add the calculator's map-time size reset as a separate, unfixed bug. Hold back
only if the merge policy requires the blank class to be fixed first; the data do not support that,
because the gate is strictly better than legacy on every class it can see.

## MEASURED vs READ vs GUESS

- MEASURED: every cell number and table row above, the controls, the gate waits, the keyfile arms,
  the transform tests, the xprop and sync observations, the samples' pixel means, the native cells,
  the shell warning/assertion scan.
- READ: 04's cold Qt/Chrome/eog rates and passive timings; the Mutter 46 source lines; GTK/Qt/
  Chromium/eog documentation URLs; 03b's passive-vs-capture perturbation.
- GUESS: why the gate-on 1100 warm cell has 3 of 20 blank while legacy has 9 of 20 (scheduling or
  drift); that the live desktop's vblank/GPU explains the unreproduced 640 black; that a commit
  counter/frame callback would not help without a content test; the live calculator rate.

## Not run

The live GNOME session (read-only pid checks only); a real Xorg session; other GNOME versions; a
loaded machine; passive X11/native cells in 05 (durations are capture); warm calculator (impossible);
Chrome cold/warm with 20 runs (only 4 warm, cold from 04); typing/clipboard/drag-and-drop;
candidate 3 and 4 instrumented; the full RUNBOOK-03b matrix for 03b's app list.

## Incidents

- **Two non-fatal Mutter buffer assertions** in the private Shell log:
  `libmutter-CRITICAL … meta_wayland_buffer_process_damage: assertion 'buffer->resource' failed` at
  21:00:43.505 and 21:37:54.563 (log lines 70897 and 382608 of the 83 MB shell log, gzipped in
  `logs/shell-end/shell.log.gz`). No crash, no `meta_window_*`/`focus_window` assertion, no signal
  5/6/11; the Shell kept running for thousands of runs after each. Not explained; reported because
  it is an assertion in the private Shell.
- 500 runs carry a `shell_warn`: "Buggy client sent a _NET_ACTIVE_WINDOW message with a timestamp
  of 0" and "Tried to ping window … with a bad serial" — the known startup-id TIME0 warnings; no
  abort (the fixed CLI closed 50+ focused editor windows without one).
- The positive-control Shell at first failed because the CLI path was built into the main repo's
  scratch, not the worktree's; no run was measured by that attempt (it errored before a window) and
  the cell was discarded and rerun with the correct path.
- No live-Shell change: pid 5787 / start 9315 at the start, after every group and at the end.

## Decisions for the maintainer

1. Merge PR #95 as is with the doc correction above (or tell me to hold it).
2. If the calculator size bug is worth a product fix, choose between the post-reveal re-apply
   (visible second resize) and doing nothing until a live measurement exists. The sandbox rate is
   not the live rate (saved-size arms: 0 of 30).
3. If the blank frames are worth a follow-up, the only measured partial is the damage-burst-end rule
   for Qt (~47 ms extra, 2 of 3); the editor and calculator need a content signal that does not
   exist in the four candidates, and Chromium needs native launch.
