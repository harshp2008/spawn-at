# X11-FLASH-03: ground-truth probe, controls, per-toolkit measurements

Branch `exp/x11-flash-03` (from `origin/exp/x11-flash-02`). Everything ran in the private headless
Shell (`tools/psh*`), DING not loaded, one run at a time. Live Shell pid 2718897 / start 81373971,
checked before and after each batch, never changed. No change to `src/` or `assets/`.

State after 03b: Tasks 1 to 5 done. Read `findings.md` first (verdict, tables, measured against
reasoned), then `RUNBOOK-03b.md` (the full matrix for a cheaper agent). Binaries: the old-id CLI is
built from this tree (== main), the fixed-id CLI from PR #94 head `703dfb3` into a scratch target
dir; neither is merged into this branch. Three private-Shell aborts happened and are written up:
`crash-1-read-pixels.md` (a GJS typelib assertion from `Clutter.Stage.read_pixels`),
`crash-2-focus-assertion.md` (03a) and `crash-3-reproduction.md` (the permitted reproduction in 03b).

## Files

| Path | What |
| :-- | :-- |
| `tools/probe/xflash-probe@local/` | the probe extension (private Shell only) |
| `tools/controls/nowait.patch` | positive control: patch applied to the sandbox COPY of the spawn-at extension (no WaitForCommit, no frame wait) |
| `tools/src/probe.rs` | `xflash probe-analyze`: bad frames per run from `probe.jsonl` + `px/*.png` (5 unit tests); also the reveal timing, `size-changed`/`position-changed` after reveal, and the texture gate columns |
| `tools/pcell.sh`, `probe-collect.sh`, `psh-start.sh` (`PROBE=1`, `CONTROL=nowait`) | run one cell end to end (`SPAWN_AT_BIN_DIR` picks the CLI) |
| `tools/src/run.rs` | adds app definitions (mousepad, evince, gnome-system-monitor, baobab, kitty, xclock, xcalc) and makes the stray-process check ignore processes outside the sandbox |
| `findings.md` | the write-up: verdict, tables, 3a to 3d, the per-toolkit table, measured against reasoned |
| `RUNBOOK-03b.md` | the full matrix, commands and stop conditions, for a cheaper agent |
| `crash-1-read-pixels.md` | `Clutter.Stage.read_pixels` aborts the Shell (GJS assertion); route dropped |
| `crash-2-focus-assertion.md`, `crash-3-reproduction.md` | the focus assertion (03a) and its permitted reproduction (03b) |
| `data/<cell>/` | per cell: `runs.jsonl`, `runs.csv` (R, D, A, L from the 02 analyser), `probe-runs.csv`, `probe-summary.md`, `probe.jsonl.gz`, `binary.txt`, logs. Cell names: `pos-*`/`neg-*` controls, `val-*`, `A-*` Task A, `rep-*` repeatability, `pert-*`/`pas-*` capture against passive, `sweep-*` size sweep, `calc-x11`, `tk-*`, `T1-*` native, `S-*` smoke |
| `samples/` | captured frames: a black bad frame of the positive control, its good successor, a good first frame of the main extension, Qt's black first frame at the final size |

## The probe, in one paragraph

A second extension in the private Shell connects `Clutter.Stage::after-paint`. For each NORMAL window,
from `window-created` to 300 ms after the first painted frame in which it is visible, it writes per
painted frame: monotonic time, whether the window actor and all its ancestors are shown with paint
opacity above 0, frame rect, buffer rect, and the size of the surface texture (via
`actor.get_texture().get_texture()`). For every visible frame an idle callback (not the paint signal)
asks `Shell.Screenshot.screenshot_area` for the frame rect into a memory stream; the PNG goes to disk
and `xflash probe-analyze` decodes it. The Shell starts in the Overview, which hides
`Meta.WindowGroup`, so the probe closes the Overview first (this is what makes new windows take
focus, see crash 2).

Limits to keep in mind: the headless Shell has no vblank, so painted frames are not 60 Hz; the
analyser therefore also reports `bad_ms` (how long the bad state was the latest painted state) and
`p60 = min(1, bad_ms / 16.67)`. A capture is taken in the same main-loop turn as the paint, not read
from the scanout buffer.
