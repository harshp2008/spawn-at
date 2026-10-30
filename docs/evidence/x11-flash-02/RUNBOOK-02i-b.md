# RUNBOOK-02i-b: exact commands (checked against the scripts before any run)

Written 2026-10-29 before Step 1 of X11-FLASH-02i-b. Branch `exp/x11-flash-02`, head `1805ff7`.
All runs are in a private headless GNOME Shell started by `tools/psh-start.sh`, used only through
`tools/psh` and stopped by `tools/psh-stop.sh`. Outside paths touched: `docs/evidence/x11-flash-02/`
and `scratch/xflash02/` only.

Checked against the scripts (read 2026-10-29):

- `psh-start.sh` non-H7 path writes `enabled-extensions=['spawn-at@harsh.local']` into the private
  keyfile, copies `assets/gnome/modern/` into `$XDG_DATA_HOME/gnome-shell/extensions/spawn-at@harsh.local`,
  and never reads the live dconf. `H7` is unset in this environment, so the H7 block is skipped.
- `psh` exits 97 without a sandbox and 98 unless the private Wayland socket, X display, bus, runtime
  dir and recorded gnome-shell pid are all the private ones.
- `psh-batch.sh` passes `--shell-pid-file "$SB/shell.pid" --shell-log "$SB/log/shell.log" --state
  "$SB/state.json"` to `xflash run` and runs it in `systemd-run --user --scope -p MemoryMax=3G
  -p MemorySwapMax=0`.
- `psh-stop.sh` signals only the pids in `shell.pid`, `sysd.pid`, `dbus.pid` and processes carrying
  `PSH_SB=<sandbox>`, then removes `/tmp/xflash-priv.*` after a `findmnt` check.
- `xflash analyze --batch DIR` (repeatable) reads `runs.jsonl`, `ext.log`, `xobs*.jsonl`,
  `calib.jsonl`, writes `runs.csv` and `summary.md` next to the batch.
- `run.rs` finds `spawn-at` on `PATH` (`Command::new("spawn-at")`, or `$SPAWN_AT` for `close`), so
  every batch below is started with `PATH="$PWD/target/debug:$PATH"`. The repo binary
  (`target/debug/spawn-at`, sha256 `60b8731...`, DEV BUILD 0.2.4) is the code under test.
- `run.rs` reads the Shell under test from `--shell-pid-file` and checks its pid/start time; a change
  makes the batch write a `crash` event and stop.

## 0. Pre-flight (every batch)

```
cd "$(git rev-parse --show-toplevel)"
p=$(pgrep -x gnome-shell | head -1); s=$(awk '{print $22}' "/proc/$p/stat"); echo "LIVE $p $s"
```

Record `$p` and `$s` in the progress file before the batch and compare after it. Stop on any change.

## 1. Start the private Shell

```
mkdir -p scratch/xflash02/matrix docs/evidence/x11-flash-02/data/matrix
docs/evidence/x11-flash-02/tools/psh-start.sh | tee scratch/xflash02/matrix/start.log
```

`psh-start.sh` prints the sandbox path twice; the last line is `$SB`. Then prove the extension set:

```
SB=$(cat scratch/xflash02/priv/CURRENT)
ls "$SB/data/gnome-shell/extensions/"                        # expect only spawn-at@harsh.local
docs/evidence/x11-flash-02/tools/psh gnome-extensions list --active
docs/evidence/x11-flash-02/tools/psh sh -c 'gsettings get org.gnome.shell enabled-extensions'
docs/evidence/x11-flash-02/tools/psh gdbus call --session --dest org.gnome.Shell \
  --object-path /org/gnome/Shell/Extensions \
  --method org.gnome.Shell.Extensions.ListExtensions
```

Expected: the one enabled line is `spawn-at@harsh.local`; the `ListExtensions` reply contains that
one uuid with state 1 (ENABLED). If anything else is loaded: `psh-stop.sh`, stop, report.

Wait for the extension to answer:

```
i=0; while [ $i -lt 40 ]; do
  q=$(docs/evidence/x11-flash-02/tools/psh "$PWD/target/debug/spawn-at" query windows --json 2>/dev/null)
  case "$q" in \[*) break;; esac; sleep 0.5; i=$((i+1)); done
printf '%s\n' "$q"
sleep 2
```

## 2. Smoke test (3 runs)

```
nohup env PATH="$PWD/target/debug:$PATH" docs/evidence/x11-flash-02/tools/psh-batch.sh \
  --app xterm --cond idle --reps 2 --seed 5 --max-runs 3 \
  --out docs/evidence/x11-flash-02/data/matrix/smoke \
  > scratch/xflash02/matrix/smoke.log 2>&1 &
echo $! > scratch/xflash02/matrix/smoke.pid
```

Poll with `tail -3 scratch/xflash02/matrix/smoke.log` and `git`-free short commands; a 3-run xterm
batch takes about 30 s. Then check `data/matrix/smoke/{runs.jsonl,end.json}` and the live pid.

## 3. One batch of one app (the general form)

Idle (20 runs per size, 10 native Wayland controls):

```
nohup env PATH="$PWD/target/debug:$PATH" docs/evidence/x11-flash-02/tools/psh-batch.sh \
  --app APP --cond idle --reps 20 --ctl-reps 10 --seed SEED \
  --out docs/evidence/x11-flash-02/data/matrix/APP/idle \
  > scratch/xflash02/matrix/APP-idle.log 2>&1 &
echo $! > scratch/xflash02/matrix/APP-idle.pid
```

Loaded (20 runs per size; the tool burns 22 CPU threads, 30 s warm-up):

```
nohup env PATH="$PWD/target/debug:$PATH" docs/evidence/x11-flash-02/tools/psh-batch.sh \
  --app APP --cond loaded --reps 20 --seed SEED \
  --out docs/evidence/x11-flash-02/data/matrix/APP/loaded \
  > scratch/xflash02/matrix/APP-loaded.log 2>&1 &
echo $! > scratch/xflash02/matrix/APP-loaded.pid
```

Poll with `tail -3 LOG`, `wc -l OUT/progress.jsonl`, `kill -0 $(cat PID)`. Stop the batch only if
its recorded pid is gone and `progress.jsonl`/`end.json` show it finished. Check the live pid after.

Apps and seeds:

| app | idle seed | loaded seed | notes |
| :-- | --: | --: | :-- |
| xterm (calibration) | 11 | 12 | same options as the live batches |
| gnome-text-editor | 31 | 32 | |
| eog | 33 | 34 | |
| gnome-calculator | 35 | 36 | |
| gnome-terminal | 37 | 38 | server app, `kill_ok: false` |
| google-chrome | 39 | 40 | forced X11 by `--ozone-platform=x11` |
| qt-assistant | 41 | 42 | |
| java-swing | 43 | 44 | no native Wayland switch |
| gnome-text-editor-nowindow | 45 | 46 | only if time allows |

## 4. Analyse a batch

```
docs/evidence/x11-flash-02/tools/target/debug/xflash analyze \
  --batch docs/evidence/x11-flash-02/data/matrix/APP/idle \
  --batch docs/evidence/x11-flash-02/data/matrix/APP/loaded
```

Prints the summary and writes `summary.md` and `runs.csv` into each batch directory. The xterm
calibration also analyses the live batches once more, from the scratch copies:

```
docs/evidence/x11-flash-02/tools/target/debug/xflash analyze \
  --batch scratch/xflash02/b_xterm_idle --batch scratch/xflash02/b_xterm_loaded
```

The committed live summary stays `data/xterm-live/{idle,loaded}/summary.md`; only the numbers printed
from the scratch copies go into `calibration.md`.

## 5. Stop the private Shell

```
docs/evidence/x11-flash-02/tools/psh-stop.sh LABEL
```

Logs are copied to `scratch/xflash02/priv/logs-LABEL-HHMMSS/`; the sandbox is removed. Confirm with
`ls scratch/xflash02/priv/CURRENT` (absent), `ls -d /tmp/xflash-priv.*` (absent), and the recorded
pids not alive. On a crash: copy the Shell stderr lines to
`data/matrix/crash-<n>.txt`, write a break event in `data/matrix/PROGRESS.md`, then start a NEW Shell.

## 6. T1 (native Wayland, spawn-at)

Same batch form, no forced-X11 runs and 20 control runs (10 per size):

```
nohup env PATH="$PWD/target/debug:$PATH" docs/evidence/x11-flash-02/tools/psh-batch.sh \
  --app gnome-text-editor --cond idle --reps 0 --ctl-reps 20 --seed 51 \
  --out docs/evidence/x11-flash-02/data/matrix/gnome-text-editor/t1-idle \
  > scratch/xflash02/matrix/t1-editor-idle.log 2>&1 &
```

Loaded uses `--cond loaded --reps 0 --ctl-reps 20 --seed 52`. Apps: gnome-text-editor (GTK4),
eog (GTK3), google-chrome (Chromium), qt-assistant (Qt 5.15). Swing has no native switch.

## 7. T2 (plain launch with the toolkit's own geometry option)

One run: start the observer, launch the app plainly, stop both, then repeat 10 times.

```
T2=scratch/xflash02/t2; mkdir -p "$T2"
nohup docs/evidence/x11-flash-02/tools/psh \
  docs/evidence/x11-flash-02/tools/target/debug/xflash observe --out "$T2/run01.xobs.jsonl" \
  > "$T2/run01.obs.log" 2>&1 &
obs=$!; sleep 1
timeout --signal=TERM --kill-after=2 6 docs/evidence/x11-flash-02/tools/psh \
  xterm -geometry 500x300+120+140 -T xflash-t2 > "$T2/run01.app.log" 2>&1
kill -TERM "$obs" 2>/dev/null; wait "$obs" 2>/dev/null
```

Plain launch = no `spawn-at spawn`; only the normal launch with the option. Apps/options:
xterm `-geometry 500x300+120+140`; google-chrome
`--user-data-dir=$T2/prof01 --no-first-run --no-default-browser-check --disable-sync --no-sandbox
--ozone-platform=x11 --class=xflash-chrome --window-size=800,600 --window-position=120,140 about:blank`;
qt-assistant `-geometry 800x600+120+140`. GTK apps have no such option; Electron (VS Code) is skipped
(no window in the earlier run). From each `xobs` file: the app window's CreateNotify/MapNotify/
ConfigureNotify geometry, whether it was created/mapped at the request, and every later
ConfigureNotify that changes its size (count and delay from the first map).

## 8. Pixel capture (private session only)

```
docs/evidence/x11-flash-02/tools/psh gdbus introspect --session \
  --dest org.gnome.Shell.Screencast --object-path /org/gnome/Shell/Screencast
```

Then record a 5 s, 60 fps capture of a fixture window:

```
mkdir -p scratch/xflash02/cap2
nohup docs/evidence/x11-flash-02/tools/psh gdbus call --session \
  --dest org.gnome.Shell.Screencast --object-path /org/gnome/Shell/Screencast \
  --method org.gnome.Shell.Screencast.Screencast \
  "$PWD/scratch/xflash02/cap2/cap.webm" "{'framerate': <uint32 60>}" \
  > scratch/xflash02/cap2/cast.log 2>&1 &
cast=$!
sleep 1
docs/evidence/x11-flash-02/tools/psh "$PWD/target/debug/fixture" --protocol x11 \
  --state-colors --width 640 --height 420 --title xflash-pixel --exit-after-ms 6000 \
  > scratch/xflash02/cap2/fixture.log 2>&1
kill -TERM "$cast" 2>/dev/null
```

Stop with `psh gdbus call ... org.gnome.Shell.Screencast.StopScreencast`, extract frames with
`ffmpeg -i cap.webm -vf fps=60 scratch/xflash02/cap2/frames/%04d.png`, and record every attempt and
error in `pixel-capture.md`. Three attempts, then stop.

## 9. Per-app wrap-up

1. Check the live pid/start unchange.
2. Analyze both batches; append the summary tables to `matrix-notes.md` under the app heading.
3. Copy/trim raw data into `data/matrix/APP/` (each app folder under 5 MB; keep `runs.jsonl`,
   `summary.md`, `runs.csv`, `progress.jsonl`, `ext.log`, `calib.jsonl`, `end.json`, `meta.json`;
   truncate `xobs*.jsonl` kept as `xobs.head.jsonl` and state what was left out in `README.txt`).
4. `git add docs/evidence/x11-flash-02` and commit `data: APP matrix (private Shell, no DING)`.
5. Check `.github/workflows/{ci,docs}.yml`: they run on pull requests and pushes to `main` only, so
   `git push origin exp/x11-flash-02` triggers no workflow. Push that branch only, never main.
6. Check `docs/evidence/x11-flash-02/STOP`; if present: stop the Shell cleanly, confirm nothing is
   left running, push, write the report, do not delete the STOP file.
