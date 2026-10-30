# X11-FIX-02: the texture gate, measured

Branch `exp/x11-flash-03` (evidence only). The product code is on `fix/x11-texture-gate` (commit
`01bb587` code, tests and test counts; `777867c` docs), draft PR, not here. Read `findings.md` first.

Everything ran in a private headless GNOME Shell 46.0 (Xwayland), one Shell and one cell at a time,
DING not loaded, Overview closed by the probe, tools from `exp/x11-flash-03` (`xflash`, the probe
extension, `pcell.sh`) in a throwaway worktree. The CLI for every cell, before and after, is built
from `main` at `0984e11` (PR #94 merged): `Previous Commit ID: 0984e11` in every `data/*/binary.txt`.
Live Shell pid 2718897, start time 81373971: recorded before the first cell, checked after every cell
(`session.sh` stops on any change) and at the end; never changed. No stop condition was hit: no
private-Shell exit, no `meta_window_`, `focus_window`, assertion or `[SpawnAt]` line in either Shell
log (`logs/shell/`), no signal 5/6/11.

| What | Path |
| :-- | :-- |
| write-up: verdict, tables, measured against reasoned, not run, decisions | `findings.md` |
| per cell: `runs.jsonl`, `runs.csv`, `probe-runs.csv`, `probe-summary.md`, `summary.md`, `analyze.txt`, `meta.json`, `end.json`, `progress.jsonl`, `binary.txt`, `probe.jsonl.gz`, `ext.log.gz` (the extension's log of the cell, with the `X11_GATE_*` lines) | `data/<cell>/` |
| the tables of `findings.md` as produced from the cells, and the same as JSON | `tables/` |
| the full trace of 10 native-Wayland scenarios through `_runBatch` on main and on the branch, and their sha256 | `native-trace/` |
| the two extension trees that were loaded (sha256 per file) and the patch between them | `ext-trees/` |
| the driver's output per session, the `pcell` output per cell, both private Shell logs | `logs/` |
| two captured frames: a cap hit at 1700 px (what the user sees), an editor window that never got its final-size buffer | `samples/` |

Cell names: `bef-` = main's extension, `aft-` = the gated extension; `cap-` = capture mode (PNG per
visible frame), `pas-` = passive (no pixels, durations), `nat-` = native Wayland control, `sent1`
and `sent2` = eog 1100x760 sentinels at the start and end of a session, `aft-off-*` = same Shell,
gate off through `[wait] strategy = "legacy"` in `SPAWN_AT_CONFIG`, `aft-capdemo-*` = same Shell,
`[wait] cap_ms = 5`.

The scripts that drove the cells and built the tables (`session.sh`, a node summariser) are throwaway
and are not committed; the definitions of every column are in `findings.md`, and every number can be
recomputed from `data/`.
