# X11-FLASH-03a: ground-truth probe (STOPPED at a Mutter focus assertion)

Branch `exp/x11-flash-03` (from `origin/exp/x11-flash-02`). Everything ran in the private headless
Shell (`tools/psh*`), DING not loaded, one run at a time. Live Shell pid 2718897 / start 81373971,
checked before and after each batch, never changed. No change to `src/` or `assets/`.

State: Task 1 done, Task 2 controls done and behaving, validation done for xterm and eog, **stopped**
in the gnome-text-editor (forced X11) validation cell by a Mutter focus assertion in the private
Shell (`crash-2-focus-assertion.md`), as the maintainer's rule requires. Tasks 3 to 5 not started.

## Files

| Path | What |
| :-- | :-- |
| `tools/probe/xflash-probe@local/` | the probe extension (private Shell only) |
| `tools/controls/nowait.patch` | positive control: patch applied to the sandbox COPY of the spawn-at extension (no WaitForCommit, no frame wait) |
| `tools/src/probe.rs` | `xflash probe-analyze`: bad frames per run from `probe.jsonl` + `px/*.png` (3 unit tests) |
| `tools/pcell.sh`, `probe-collect.sh`, `psh-start.sh` (`PROBE=1`, `CONTROL=nowait`) | run one cell end to end |
| `crash-1-read-pixels.md` | `Clutter.Stage.read_pixels` aborts the Shell (GJS assertion); route dropped |
| `crash-2-focus-assertion.md` | the stop |
| `data/<cell>/` | per cell: `runs.jsonl`, `runs.csv` (R, D, A, L from the 02 analyser), `probe-runs.csv`, `probe-summary.md`, `probe.jsonl.gz`, logs |
| `samples/` | three captured frames (a black bad frame of the positive control, its good successor, a good first frame of the main extension) |

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
