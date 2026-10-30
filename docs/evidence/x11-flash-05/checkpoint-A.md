# Checkpoint A: editor cold vs warm

Task A, measured 2026-10-30 ~20:51–22:13 in the private Shell (live pair 5787 / 9315 before and
after every group, unchanged). Gate on = default PR #95 extension; gate off = same extension and
CLI with `[wait] strategy = "legacy"` (the texture gate is skipped, `waitForCommit` still runs).

## What ran

| Cell | Size | Mode | Runs | Keeper (warm) |
| :-- | :-- | :-- | --: | :-- |
| `A-editor-640-gate-cold` | 640x420 | gate on, cold | 20 | - |
| `A-editor-1100-gate-cold` | 1100x760 | gate on, cold | 20 | - |
| `A-editor-640-gate-warm` | 640x420 | gate on, warm | 20 | 1100x760 |
| `A-editor-1100-gate-warm` | 1100x760 | gate on, warm | 20 | 1100x760 |
| `A-editor-640-legacy-cold` | 640x420 | gate off, cold | 20 | - |
| `A-editor-1100-legacy-cold` | 1100x760 | gate off, cold | 20 | - |
| `A-editor-640-legacy-warm` | 640x420 | gate off, warm | 20 | 1100x760 |
| `A-editor-1100-legacy-warm` | 1100x760 | gate off, warm | 20 | 1100x760 |
| `A-editor-640-gate-coldreset` | 640x420 | gate on, cold, state deleted per run | 10 | - |
| `A-editor-1100-gate-coldreset` | 1100x760 | gate on, cold, state deleted per run | 10 | - |
| `A-editor-640-legacy-coldreset` | 640x420 | gate off, cold, state deleted per run | 10 | - |
| `A-editor-1100-legacy-coldreset` | 1100x760 | gate off, cold, state deleted per run | 10 | - |

The keeper is one `gnome-text-editor --new-window` X11 window, started with `warm-cell.sh`, kept
open for the whole cell, its pid in `XFLASH_KEEP_PIDS`, closed only at the end. The keeper's
geometry is in `logs/keeper-A-editor-*-warm.log`; it is the size the editor restored from
`$XDG_DATA_HOME/org.gnome.TextEditor/session.gvariant` (1100x760 in these cells).

## Editor results (MEASURED, capture mode)

| Cell | runs | bad texture | bad pixels | stale | blank | worst bad_ms | gate wait med / max ms |
| :-- | --: | --: | --: | --: | --: | --: | :-- |
| 640 gate cold | 20 | 0 | 0 | 0 | 0 | 0 | 0.0 / 80.3 (run 1 only) |
| 1100 gate cold | 20 | 0 | 0 | 0 | 0 | 0 | 0.0 / 63.5 (run 1 only) |
| 640 gate warm | 20 | 0 | 0 | 0 | 0 | 0 | 17.3 / 39.8 |
| 1100 gate warm | 20 | 0 | **3** | 0 | 3 | 74.7 | 0.1 / 0.3 |
| 640 gate coldreset | 10 | 0 | 0 | 0 | 0 | 0 | 62.4 / 76.9 |
| 1100 gate coldreset | 10 | 0 | **2** | 0 | 2 | 17.4 | 67.5 / 251.3 |
| 640 legacy cold | 20 | 1 | 1 | 1 | 0 | 61.9 | - |
| 1100 legacy cold | 20 | 1 | 1 | 1 | 0 | 73.0 | - |
| 640 legacy warm | 20 | **17** | **17** | 17 | 0 | 61.4 | - |
| 1100 legacy warm | 20 | 0 | **9** | 0 | 9 | 57.5 | - |
| 640 legacy coldreset | 10 | **10** | **10** | 10 | 0 | 70.5 | - |
| 1100 legacy coldreset | 10 | **10** | **10** | 10 | 0 | 81.5 | - |

Reading:

1. **The editor persists its last window size** in `$XDG_DATA_HOME/org.gnome.TextEditor/session.gvariant`
   (`default-window-size`, 1100x760 observed). In a fixed-size cold cell, run 1 has to resize from
   the previous size and the gate waits 63–80 ms; runs 2–20 start at the requested size and the gate
   matches in <1 ms. That is the whole difference between run 1 and the rest (MEASURED, gate waits
   and `session.gvariant`).
2. **Gate off, every run that has to resize shows the old-size texture for about 2 painted frames
   (60–82 ms).** That is 10 of 10 at both sizes with the state deleted per run, 17 of 20 warm at 640,
   and the single mismatching cold run in each fixed-size legacy cell. This is the class the texture
   gate removes.
3. **Gate on removes every stale-size frame: 0 bad texture in 100 gated editor runs** (four cells
   of 20 and two of 10). One 1100 cold-reset run hit the 250 ms cap (waited 251.3 ms, texture
   822x642 against buffer 1222x882); it was not counted bad because the texture arrived before the
   probe's first captured visible frame. The remaining bad pixels are
   **blank frames at the correct final texture size**: 3 of 20 in the 1100 warm cell (65–75 ms), 2 of
   10 in the 1100 cold-reset cell (10.8 / 17.4 ms), 0 at 640. That is the same class the user saw in
   the live session at 640; in this private Shell it is at 1100, not 640.
4. **Cold vs warm is not a clean split in this setup.** Warm launch, gate off: 17 of 20 bad at 640
   (stale) and 9 of 20 at 1100 (blank). Warm launch, gate on: 0 at 640, 3 of 20 at 1100. Cold launch
   (fixed size) after run 1: 0 and 1. The dominant variable is whether the new window has to resize
   from a different size, not whether the process existed before.

## The `1100 gate warm = 3/20 blank` result

`A-editor-1100-gate-warm` had a keeper at 1100x760, i.e. no size mismatch, and the gate matched in
0.0–0.3 ms in 20 of 20 runs. Yet the first visible frame was blank in 3 runs, for 65–75 ms, with the
texture already 1222x882 (the final size). `A-editor-1100-legacy-warm` (same keeper size, gate off,
different cell) is 9 of 20 blank for 6–58 ms. The gate is associated with fewer blank runs but not
zero, and the mechanism is not established: the gate's extra `await` (microtask) may allow the
client's first content to land before the uncloak, or the two cells differ for unrelated reasons
(different times; 03b saw 3–8 of 10 drift between eog batches). **GUESS**, not established.

## Cross-check: eog, Qt, Chrome (4 runs per cell, 2 sizes x 2; coarse)

| App | gate cold | gate warm | legacy cold | legacy warm |
| :-- | :-- | :-- | :-- | :-- |
| eog (GTK3), keeper 540x450 | 0 stale, 0 blank | 0, 0 | **2 stale** (16.2 ms) | 0, 0 |
| Qt assistant 5.15, keeper 1105x722 | 0, **3 blank** (23.4) | 0, **2 blank** (12.4) | 0, **2 blank** (16.4) | **1 stale** (23.3) |
| Chrome, keeper 901x984, one profile | 0, **2 blank** (30.5) | - | 0, **3 blank** (34.0) | - |
| calculator (Task B) | 0, **5 blank** (37.8) of 40 | not possible | **37 stale** of 40 | not possible |

- **eog**: the bad frame is the old-size texture and depends on the size, not on cold/warm (the
  1100 request at 4 runs, 2 bad when the gate is off; the keeper is 540x450).
- **Qt**: blank at the final texture size in all four conditions; the gate has no effect. No
  cold/warm split.
- **Chrome**: warm with one profile still blank at the final size in 2 of 4 (gate on) and 3 of 4
  (gate off). The cold side is READ from X11-FLASH-04: 18 of 20 blank before and after the gate.
  No cold/warm split; the gate cannot see it.
- **calculator**: warm is not possible. A second launch with a running calculator's profile exits
  and only activates the existing window (MEASURED: `--new-window` produced no new window); the
  batch would have nothing to place. Cold numbers are Task B's.

## Unexplained / caveats

- Why the gate-on warm 1100 cell still has 3 blank frames and the legacy warm cell has 9: GUESS above.
- The live 640x420 pure-black frame was **not reproduced** in the private Shell: 0 of 50 gated 640
  runs (20 cold, 20 warm, 10 cold-reset). It is reproduced in kind, not at that size: 3 of 20 at
  1100 warm and 2 of 10 at 1100 cold-reset. The live desktop has a real vblank and a real GPU; this
  Shell has neither.
- Run 1 of each fixed-size cold cell is the only real resize test; the cells are kept because the
  user asked for them, and the cold-reset cells are the controlled replacement.
- Capture mode inflates durations; no passive cells.
