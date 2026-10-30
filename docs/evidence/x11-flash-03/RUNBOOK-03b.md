# RUNBOOK-03b: the full matrix with the ground-truth probe

For a cheaper agent. Every step is a command; every stop condition is a test; there are no
judgement calls. If a step says STOP, stop and report the last output. Branch `exp/x11-flash-03`.
Written 2026-10-29 after X11-FLASH-03b; the commands were run on that date except where a step says
"first run" (the new apps in section 5, which were added to `run.rs` but not yet run).

## 0. Rules (all of them, every time)

- One private headless Shell, one batch at a time, never two jobs in parallel (timing matters).
- Everything that touches the Shell goes through `tools/psh*`. Nothing writes outside the repo
  except `/tmp/xflash-priv.*` (made and removed by `psh-start.sh` / `psh-stop.sh`).
- Never `pkill`/`killall`; never signal a process you did not start. A `gnome-text-editor`,
  `xterm` or `eog` process that is NOT in the sandbox (no `PSH_SB` in its environment) is the
  maintainer's own: leave it alone (the batch tool already ignores it).
- No change to `src/` or `assets/`. Tools under `docs/evidence/x11-flash-03/tools/` only.
- Live Shell: record pid and start time first (`LIVE_PID`, `LIVE_START` below) and compare after
  EVERY batch. A difference is STOP.
- STOP, do not retry, on: any private Shell exit (the batch prints `CRASH:` and `pcell: batch failed`),
  `signal 5/6/11`, `uncatchable exception`, a line with `meta_window_` or `focus_window` in the
  Shell log, a log line mentioning `xflash-probe` with `CRITICAL` or `ERROR`, or a live pid change.
  Save the log with `tools/psh-stop.sh NAME`, write `crash-N-*.md`, report.
- Commit after each section (conventional message, no trailers, no session id). Never push `main`,
  never force, never `--tags`.

## 1. Pre-flight and binaries (once)

```
cd "$(git rev-parse --show-toplevel)"
git checkout exp/x11-flash-03
p=$(pgrep -x gnome-shell | head -1); LIVE_PID=$p; LIVE_START=$(awk '{print $22}' /proc/$p/stat); echo "LIVE $LIVE_PID $LIVE_START"
(cd docs/evidence/x11-flash-03/tools && cargo build --offline && cargo test --offline)     # expect 3 passed
# the CLI under test: the fixed launch id (PR #94, commit 703dfb3) unless PR #94 has merged
git worktree add --detach scratch/xflash03/wt-fix 703dfb3
(cd scratch/xflash03/wt-fix && CARGO_TARGET_DIR="$OLDPWD/scratch/xflash03/target-fix" cargo build --locked -p spawn-at)
scratch/xflash03/target-fix/debug/spawn-at --version | sed -n 4p        # expect: Previous Commit ID: 703dfb3
```

`SPAWN_AT_BIN_DIR=$PWD/scratch/xflash03/target-fix/debug` goes in front of every `pcell.sh` below.
The OLD-id CLI (built from this tree, `target-main`, default of `pcell.sh`) MUST NOT be used on a
forced-X11 GTK window: it aborts the Shell on the first close (crash 2/3). It is only ever used in
section 3, cell `A-old-editor`, and only if the maintainer asks for that cell again.

## 2. Start the private Shell with the probe (the only supported start)

```
PROBE=1 PROBE_CFG='{"pixels":"png"}' sh docs/evidence/x11-flash-03/tools/psh-start.sh
sleep 4
SB=$(cat scratch/xflash02/priv/CURRENT)
grep -o "enabled-extensions=.*" $SB/config/glib-2.0/settings/keyfile
```

Expected: `['spawn-at@harsh.local', 'xflash-probe@local']`. Then:

```
docs/evidence/x11-flash-03/tools/psh gdbus call --session --dest org.gnome.Shell.Extensions \
  --object-path /org/gnome/Shell/Extensions --method org.gnome.Shell.Extensions.ListExtensions \
  | grep -o "'xflash-probe@local': {[^}]*" | grep -o "'state': <[0-9.]*>"        # expect <1.0>
grep '"k":"desktop"' $SB/probe/probe.jsonl                                          # expect "ok":true
```

If `state` is not 1.0 or `"ok":false`: STOP. DING must not be loaded (`ding@rastersoft.com` state 6).

Passive mode (log only, no screenshots) is `printf '{"pixels":"none"}\n' > $SB/probe/config.json`;
capture mode is `printf '{"pixels":"png"}\n' > $SB/probe/config.json`. Read at every window creation.

## 3. Positive and negative control (every session, before any measurement)

Positive control needs the patched extension, so it needs its own Shell:

```
sh docs/evidence/x11-flash-03/tools/psh-stop.sh pos
CONTROL=nowait PROBE=1 PROBE_CFG='{"pixels":"png"}' sh docs/evidence/x11-flash-03/tools/psh-start.sh; sleep 4
export SPAWN_AT_BIN_DIR=$PWD/scratch/xflash03/target-fix/debug
sh docs/evidence/x11-flash-03/tools/pcell.sh C-pos-xterm xterm --reps 5 --seed 41
```

Pass: `runs bad px` is 10 of 10 for `xterm`. Fail (any run with 0 bad frames): STOP, the probe is
blind. Then restart WITHOUT `CONTROL` (section 2) and run the negative control:

```
sh docs/evidence/x11-flash-03/tools/pcell.sh C-neg-editor gnome-text-editor --reps 0 --ctl-reps 20 --seed 51
```

Pass: `runs bad px` is 0 of 10 in both rows. Fail: STOP.

## 4. One cell (the general form)

```
export SPAWN_AT_BIN_DIR=$PWD/scratch/xflash03/target-fix/debug
sh docs/evidence/x11-flash-03/tools/pcell.sh LABEL APP [--reps N] [--ctl-reps M] [--sizes SxS,LxL] --seed SEED
```

`--reps N` is N runs per size (small and large). `--ctl-reps M` is M native-Wayland runs in total
(M/2 per size) for apps that have a native mode. Output: `scratch/xflash03/cells/LABEL/`.
The printed table is `probe-summary.md`; the per-run rows are `probe-runs.csv`.

Record, per cell, in `docs/evidence/x11-flash-03/data/LABEL/` (copy these files, nothing else):
`runs.jsonl runs.csv summary.md analyze.txt probe-runs.csv probe-summary.md meta.json end.json
progress.jsonl binary.txt` and `gzip -9 -c probe/probe.jsonl > probe.jsonl.gz`.
From the table copy into the findings: n, runs with a bad frame (`runs bad px`), bad frames and
`bad_ms` median/max, `p60` median, R, D, A>0, L median.

After EVERY cell:

```
echo "live: $(pgrep -x gnome-shell | head -1) $(awk '{print $22}' /proc/2718897/stat)"   # replace 2718897 by $LIVE_PID
```

must print the recorded pair. If it prints anything else: STOP.

## 5. The matrix

Idle only (loaded is not part of 03b). Seeds are fixed so a rerun repeats the order.

Already measured with the probe in this branch (do not repeat unless a section says so):
xterm, eog, gnome-text-editor (forced X11), gnome-calculator, qt-assistant, java-swing,
google-chrome, and native Wayland for editor, calculator, eog, Qt and Chrome. The runbook's cells
below add the apps whose smoke passed (n=1 per size so far).

### 5a. Smoke for every app not yet run with the probe (1 run per size, 2 runs)

For each of `mousepad evince gnome-system-monitor baobab kitty xclock xcalc code firefox nautilus`:

```
sh docs/evidence/x11-flash-03/tools/pcell.sh S-APP APP --reps 1 --seed 5
```

Pass: both runs `status=ok` and the table shows `seen` 2. Otherwise record in
`data/not-measurable.md`: app, the first 3 lines of the text `pcell.sh` printed, and write "not
measurable: <that text>"; do not retry; go on.

Results of this smoke on 2026-10-29 (fixed id, `data/S-*`):

| App | Smoke | Note |
| :-- | :-- | :-- |
| mousepad, evince, gnome-system-monitor, baobab, kitty, xclock, xcalc, nautilus | pass (2 runs, `seen` 2) | the class names in `run.rs` matched |
| firefox | fail: `status=no_window` in both runs | snap; record "not measurable" |
| code (Electron) | opens the window (`win=Some(..)`) but the run ends `status=not_closed`: two graceful closes are ignored and SIGTERM to the window's pid does nothing | the batch stops with `window ... did not close`. One run is all there is: run `psh-batch.sh --app code --out scratch/xflash03/cells/M-code --reps 1 --max-runs 1 --seed 208`, then `probe-collect.sh scratch/xflash03/cells/M-code` and `probe-analyze` by hand (command in section 6 below), then `psh-stop.sh` (which ends the Shell and so the app). Do not run any other cell after it in the same Shell |

If a run ends `not_closed` for any other app: same handling, stop the batch, do not signal.
A `gnome-text-editor`, `xterm` or `eog` of the maintainer's own live session does not block a batch
(the stray check only counts processes carrying this sandbox's `PSH_SB`). Never run `probe-analyze`
or any other `xflash` process while a batch is running: the batch's stray check will see it and refuse
the next run (this cost 7 smoke runs on 2026-10-29).

### 5b. Full cells (10 runs per size; native control where the app has one)

| App (toolkit) | Command | Seed |
| :-- | :-- | --: |
| mousepad (GTK3) | `pcell.sh M-mousepad mousepad --reps 10 --ctl-reps 10` | 201 |
| evince (GTK3) | `pcell.sh M-evince evince --reps 10 --ctl-reps 10` | 202 |
| gnome-system-monitor (GTK4) | `pcell.sh M-sysmon gnome-system-monitor --reps 10 --ctl-reps 10` | 203 |
| baobab (GTK4) | `pcell.sh M-baobab baobab --reps 10 --ctl-reps 10` | 204 |
| kitty (OpenGL terminal) | `pcell.sh M-kitty kitty --reps 10 --ctl-reps 10` | 205 |
| xclock (Xt, X11 only) | `pcell.sh M-xclock xclock --reps 10` | 206 |
| xcalc (Xt, X11 only) | `pcell.sh M-xcalc xcalc --reps 10` | 207 |
| code (Electron) | `pcell.sh M-code code --reps 10 --ctl-reps 10` | 208 |
| firefox (Gecko) | `pcell.sh M-firefox firefox --reps 10 --ctl-reps 10` | 209 |
| nautilus (GTK4) | `pcell.sh M-nautilus nautilus --reps 10 --ctl-reps 10` | 210 |

Only the apps whose smoke passed. `pcell.sh` is `sh docs/evidence/x11-flash-03/tools/pcell.sh`.
A second terminal other than kitty is not installed (`foot` is Wayland-only and has no X11 mode:
skip it, record "foot: no X11 mode"). Native-Wayland-only toolkits are not part of this matrix.

### 5c. Repeat the sentinel (drift check)

Last cell of every session:

```
sh docs/evidence/x11-flash-03/tools/pcell.sh Z-eog-large eog --sizes 1100x760,1100x760 --reps 5 --seed 301
```

Copy `runs bad px` of it next to the three earlier batches (`rep-eog-large-1..3`: 7, 8, 6 of 10).
If it is 0 of 10 or 10 of 10, write "drift: yes" in the report and do not interpret the matrix.

## 5d. Passive companions (durations)

Capture mode stretches the duration of a bad frame for fast apps (eog: 11.6 ms against 1.9 ms
passive) but not the rate. For every app with a bad frame in 5b that is not slow (bad window under
30 ms in capture mode), run it passive too, on the same Shell:

```
SB=$(cat scratch/xflash02/priv/CURRENT)
printf '{"pixels":"none"}\n' > $SB/probe/config.json
sh docs/evidence/x11-flash-03/tools/pcell.sh P-APP APP --reps 10 --seed SEED+100
printf '{"pixels":"png"}\n' > $SB/probe/config.json
```

Quote durations from the passive cell (`reveal_to_tex_final_ms` minus `reveal_to_first_frame_ms` in
`probe-runs.csv`), rates from either. Passive cells have no pixel columns: Qt- and Chrome-like blank
frames are invisible there.

## 6. Analysis (offline, after the last cell)

```
for c in scratch/xflash03/cells/*/; do n=$(basename $c); [ -f $c/probe-summary.md ] && { echo "## $n"; cat $c/probe-summary.md; }; done > scratch/xflash03/all-summaries.md
```

Per toolkit, write one row in `findings.md`: app, toolkit, runs with bad frame / runs (`runs bad px`
and `runs bad tex`), `bad_ms` max (capture) and the passive window (5d), first texture size at
reveal against the final one (from `probe-runs.csv` columns `first_vis_tex`, `final_tex`), whether
the texture gate was open at the first frame (`gate_open_first`), `size_changed_after`,
final size equals requested (from `runs.csv` `final_w final_h req_w req_h`), and
`focused_at_close` count (`grep -c '"focused_at_close":true' runs.jsonl`).

Re-analysing one cell by hand (needed after `psh-batch.sh` only, never during a batch):

```
docs/evidence/x11-flash-03/tools/target/debug/xflash probe-analyze --probe-dir DIR/probe \
  --runs DIR/runs.jsonl --runs-csv DIR/runs.csv --out DIR
```

The debug build is slow at decoding PNGs: a 20-run cell takes about a minute, all cells together
about 10 minutes (it was run in the background on 2026-10-29 and finished).

## 7. Stop the Shell and finish

```
sh docs/evidence/x11-flash-03/tools/psh-stop.sh end
git status --short | head
git add docs/evidence/x11-flash-03 && git commit -m "data: X11-FLASH-03b matrix cells"
git push origin exp/x11-flash-03
echo "live: $(pgrep -x gnome-shell | head -1) $(awk '{print $22}' /proc/$LIVE_PID/stat)"
du -sh --exclude=target docs/evidence/x11-flash-03
```

Report: the table from section 6, the sentinel line, every STOP or "not measurable" line verbatim,
the evidence size, the live pid pair before and after.
