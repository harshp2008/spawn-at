# X11-FLASH-05 (deepseek): cold vs warm, calculator size, the blank frame, native Wayland

Branch `exp/x11-flash-05-ds`, based on `origin/exp/x11-flash-03` (`27af7b1`). Started 2026-10-30
~20:32 IST. Read `findings.md` first: verdict, tables, MEASURED / READ / GUESS. Then
`checkpoint-A.md` to `checkpoint-D.md` for what ran, per task.

## Setup, in one table

| Item | Value |
| :-- | :-- |
| Live GNOME Shell | pid **5787**, start time **9315** (`/proc/5787/stat` field 22). Recorded before the first cell, after every cell group and at the end; never changed. |
| Private Shell | GNOME Shell 46.0, `--headless --wayland --virtual-monitor 1920x1080`, own D-Bus / Wayland socket / Xwayland / keyfile gsettings, DING not loaded, only `spawn-at@harsh.local` and `xflash-probe@local`, Overview closed by the probe. Started by `tools/psh-start.sh`; one at a time. |
| CLI | built from PR #95 head **`777867c`** (`docs: CHANGELOG...`; code `01bb587`) into `scratch/xflash05/target-fix/debug/spawn-at`. `spawn-at --version`: DEV BUILD 0.2.4, `Previous Commit ID: 777867c`. |
| Extension | the same `777867c` tree, `assets/gnome/modern`, loaded with `EXT_SRC` (tool change 1). sha256 per file in each cell's `ext-sha256.txt`; they equal X11-FLASH-04's `ext-trees/after-sha256.txt` (commit.js `3d5ff2…`, extension.js `8b176b…`). |
| Tools | `docs/evidence/x11-flash-03/tools` from `origin/exp/x11-flash-03`, plus the changes listed below. |
| Probe | `xflash-probe@local`, capture mode (`{"pixels":"png"}`) for every cell. Per painted frame: actor visibility, frame rect, buffer rect, texture size, and a `Shell.Screenshot.screenshot_area` PNG of the frame rect. |
| Bad frame | visible frame before the first good one. **texture** reading: good = frame and texture have the final size. **pixels**: also not blank (≥ half transparent or mostly black). `stale` = the first bad frame has the old texture size; `blank` = the texture is already the final size but the pixels are blank. |
| Cell sizes | editor 640x420 and 1100x760; calculator 900x700; eog / Qt / Chrome their large defaults; native cells also small+large. |

The two controls of RUNBOOK-03b pass in this session: positive (no-wait patch, xterm) **10 of 10
runs bad** (`data/C-pos-xterm`), negative (native editor) **0 of 20** (`data/N-neg-editor`).

## Tool changes (only under `docs/evidence/`)

1. `x11-flash-03/tools/psh-start.sh`: `EXT_SRC` selects the extension tree copied into the sandbox
   (default unchanged, `assets/gnome/modern`) and writes `$SB/ext-sha256.txt`. Why: load the PR #95
   tree from a separate worktree without changing `assets/` in this branch.
2. `x11-flash-03/tools/src/run.rs`: `XFLASH_KEEP_PIDS` (comma separated) is skipped by the stray
   check. Why: a warm cell keeps one pre-existing window and process of the same app for the whole
   batch; it is deliberate, not a leftover. Everything else in the sandbox is still reported.
3. `src/run.rs`: `XFLASH_RESET_EDITOR_STATE=1` deletes
   `$XDG_DATA_HOME/org.gnome.TextEditor/session.gvariant` before every run. Why: the editor stores
   its last `default-window-size` there; a fixed-size cold cell stops being a size-mismatch test
   after its first run (measured: only run 1 waits 63–80 ms at the gate, the other 19 wait <1 ms).
4. `src/run.rs`: app `eog-image` (`eog --new-instance {IMAGE}`, `XFLASH_EOG_IMAGE`). Why: Task D
   needs eog with a real image (a 3000x2000 gradient made with ImageMagick in scratch).
5. `src/run.rs`: app `google-chrome-warm` (`--user-data-dir={CHROME_PROFILE}`, `XFLASH_CHROME_PROFILE`,
   `--new-window`). Why: a warm Chrome needs one profile for the whole cell so the next launch talks
   to the running browser instead of starting a new one.
6. `src/run.rs` + `src/obs.rs` + `Cargo.toml` (`x11rb` feature `sync`): `XFLASH_SYNC=1` makes the
   observer pass `--sync`. The observer reads `_NET_WM_SYNC_REQUEST_COUNTER` when it appears and
   polls its 64-bit value after every X event; it also dumps `WM_PROTOCOLS` in each `props` record.
   Why: Task C candidate 2, the `_NET_WM_SYNC_REQUEST` protocol.
7. New `x11-flash-05/tools/gate.sh` (on/off via `[wait] strategy = "legacy"`),
   `warm-cell.sh` (start one keeper, run one pcell with `XFLASH_KEEP_PIDS`, close the keeper by
   window id and the recorded pids only), `collect-cell.sh` (copy a cell to `data/`, gzip the ext
   and X observer logs).

No change to `src/`, `assets/` or any product file. The tools tree has two pre-existing warnings
(`analyze.rs` dead `wh`, unused `Observer.root`); this session adds none.

## Cells

| Prefix | What |
| :-- | :-- |
| `C-pos-xterm`, `N-neg-editor` | controls |
| `A-editor-<size>-<gate|legacy>-<cold|warm>` | Task A, 8 cells of 20 runs |
| `A-editor-<size>-<gate|legacy>-coldreset` | Task A, controlled cold start (state deleted per run), 10 runs |
| `X-eog-*`, `X-qt-*`, `X-chrome-*` | Task A cross-check, 4 runs each (2 sizes x 2 runs) |
| `B-calc-<gate|legacy|native>-cold`, `B-calc-keyfile-*` | Task B, 40 runs each (keyfile 20 and 10) |
| `C-sync-*` | Task C, 2 runs each with the XSync counter observer |
| `D-*` | Task D native Wayland: eog-image, Qt, Chrome 20 runs, editor warm 10, editor cold = `N-neg-editor` (20), calculator native = `B-calc-native-cold` (40) |

Each `data/<cell>/` has `runs.jsonl`, `runs.csv`, `probe-runs.csv`, `probe-summary.md`,
`probe.jsonl.gz`, `ext.log.gz`, `xobs.jsonl.gz`, `summary.md`, `analyze.txt`, `meta.json`,
`end.json`, `progress.jsonl`, `binary.txt`, `ext-sha256.txt`. The driver logs are in `logs/`; the
sample frames are in `samples/`.

## Reproduce one cell

```
cd scratch/xflash05/wt
export SPAWN_AT_BIN_DIR=../target-fix/debug          # absolute path in practice
EXT_SRC="$PWD/../wt-fix/assets/gnome/modern" PROBE=1 PROBE_CFG='{"pixels":"png"}' \
  sh docs/evidence/x11-flash-03/tools/psh-start.sh; sleep 4
sh docs/evidence/x11-flash-05/tools/gate.sh on|off
sh docs/evidence/x11-flash-03/tools/pcell.sh LABEL APP --sizes WxH,WxH --reps N --seed S
# warm: sh docs/evidence/x11-flash-05/tools/warm-cell.sh LABEL APP CLASS 'KEEPER CMD' ...
sh docs/evidence/x11-flash-03/tools/psh-stop.sh end
```

## Caveats that apply to every number

- The private Shell has **no vblank**: painted frames are not 60 Hz. `bad_ms` is the time the bad
  state was the latest painted state; `p60 = min(1, bad_ms/16.667)` estimates the chance a 60 Hz
  refresh lands inside it. Durations are comparable only within capture mode (all cells here).
- Capture mode stretches the bad window of fast apps (03b: eog 11.6 ms vs 1.9 ms passive). No
  passive cells were run in 05; durations quoted from 04 are marked READ.
- Every app in a batch runs with `GSETTINGS_BACKEND=memory`, so it starts from the schema defaults.
  This matters for the editor (session file, not GSettings) and the calculator (see Task B).
- The live session was never used. No live app was launched, closed or focused.
