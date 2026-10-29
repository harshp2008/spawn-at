# UNREVIEWED raw measurements, private headless Shell, DING not loaded

X11-FLASH-02i-b Step 4. Every batch ran in the one private headless GNOME Shell of this run
(enabled extensions `['spawn-at@harsh.local']`, DING installed but disabled). Live Shell pid
2718897 / start time 81373971 did not change before or after any batch. Numbers are printed by
`xflash analyze` or counted directly from the run records; no interpretation.

## T1: native Wayland with spawn-at

Method: `xflash run --reps 0 --ctl-reps 20` (so 10 native Wayland runs per size, no forced-X11
runs in the batch), idle and loaded, one representative app per toolkit; the native launch and
its environment come from the `run.rs` table (`env_wl`). Output directories
`data/matrix/<app>/t1-{idle,loaded}`.

Limitation: the analyser converts the extension's `mono_ms` origin only from `X11_OBS` lines of an
X11 window. A native-only batch has none, so `A` and `cloak` are not printable for these batches
("A>0" reads 0 for that reason, not as a measurement). The tables therefore give the native
run outcomes (`n`, window appeared, final size) and the native control numbers from the Step 3 idle
batches, where X11 runs supplied the origin.

### Forced X11 numbers from Step 3 (same apps)

| app | toolkit | cond | size | n | V | R | D | A | L med / max ms |
| :-- | :-- | :-- | :-- | --: | --: | --: | --: | --: | :-- |
| gnome-text-editor | GTK4 | idle | large | 20 | 0 | 3 | 2 | 0 | -1.6 / 48.3 |
| gnome-text-editor | GTK4 | idle | small | 20 | 0 | 2 | 1 | 0 | -1.0 / 50.1 |
| gnome-text-editor | GTK4 | loaded | large | 20 | 0 | 4 | 3 | 0 | -3.0 / 26.5 |
| gnome-text-editor | GTK4 | loaded | small | 20 | 0 | 0 | 0 | 0 | -10.4 / -2.8 |
| eog | GTK3 | idle | large | 20 | 0 | 19 | 0 | 0 | 10.4 / 16.2 |
| eog | GTK3 | idle | small | 20 | 0 | 20 | 0 | 0 | 5.3 / 8.8 |
| eog | GTK3 | loaded | large | 19 | 0 | 10 | 12 | 0 | 28.1 / 66.9 |
| eog | GTK3 | loaded | small | 20 | 0 | 0 | 12 | 0 | 22.3 / 48.4 |
| google-chrome | Chromium | idle | large | 20 | 0 | 9 | 1 | 0 | -1.8 / 23.6 |
| google-chrome | Chromium | idle | small | 20 | 0 | 14 | 3 | 0 | 0.8 / 52.4 |
| google-chrome | Chromium | loaded | large | 20 | 0 | 15 | 13 | 0 | 20.5 / 49.5 |
| google-chrome | Chromium | loaded | small | 20 | 0 | 4 | 0 | 0 | -0.6 / 14.6 |
| qt-assistant | Qt 5.15 | idle | large | 20 | 0 | 10 | 20 | 0 | 43.9 / 52.7 |
| qt-assistant | Qt 5.15 | idle | small | 20 | 0 | 1 | 14 | 0 | 23.6 / 37.1 |
| qt-assistant | Qt 5.15 | loaded | large | 20 | 0 | 5 | 6 | 0 | 21.5 / 67.3 |
| qt-assistant | Qt 5.15 | loaded | small | 20 | 0 | 2 | 10 | 0 | 12.5 / 49.8 |

### T1 native Wayland runs (10 per size)

| app | toolkit | cond | size | n | window appeared | status ok | final == requested | ready-wait outcomes |
| :-- | :-- | :-- | :-- | --: | --: | --: | --: | :-- |
| gnome-text-editor | GTK4 | idle | large | 10 | 10 | 10 | 10 | 10x reached:at-target |
| gnome-text-editor | GTK4 | idle | small | 10 | 10 | 10 | 10 | 10x reached:at-target |
| gnome-text-editor | GTK4 | loaded | large | 10 | 10 | 10 | 10 | 10x reached:at-target |
| gnome-text-editor | GTK4 | loaded | small | 10 | 10 | 10 | 10 | 10x reached:at-target |
| eog | GTK3 | idle | large | 10 | 10 | 10 | 10 | 10x reached:at-target |
| eog | GTK3 | idle | small | 10 | 10 | 10 | 10 | 10x reached:at-target |
| eog | GTK3 | loaded | large | 10 | 10 | 10 | 10 | 1x timed-out:cap; 9x reached:at-target |
| eog | GTK3 | loaded | small | 10 | 10 | 10 | 10 | 1x timed-out:cap; 9x reached:at-target |
| google-chrome | Chromium | idle | large | 10 | 10 | 10 | 10 | 10x reached:at-target |
| google-chrome | Chromium | idle | small | 10 | 10 | 10 | 10 | 10x reached:at-target |
| google-chrome | Chromium | loaded | large | 10 | 10 | 10 | 10 | 10x reached:at-target |
| google-chrome | Chromium | loaded | small | 10 | 10 | 10 | 10 | 1x timed-out:cap; 9x reached:at-target |
| qt-assistant | Qt 5.15 | idle | large | 10 | 10 | 10 | 8 | 2x timed-out:cap; 8x reached:at-target |
| qt-assistant | Qt 5.15 | idle | small | 10 | 10 | 10 | 10 | 10x reached:at-target |
| qt-assistant | Qt 5.15 | loaded | large | 10 | 10 | 10 | 8 | 2x timed-out:cap; 8x reached:at-target |
| qt-assistant | Qt 5.15 | loaded | small | 10 | 10 | 10 | 10 | 10x reached:at-target |

The two Qt large cases ended at 700x500 instead of 1000x700 (`final` in `runs.jsonl`), which is why
`final == requested` is 8 of 10.

### Native Wayland controls in the Step 3 idle batches (5 per size, origin available)

| app | toolkit | size | n | A>0 | cloak med / max ms | final == requested |
| :-- | :-- | :-- | --: | --: | :-- | --: |
| gnome-text-editor | GTK4 | large | 5 | 0 | 12.0 / 14.7 | 5 |
| gnome-text-editor | GTK4 | small | 5 | 0 | 11.1 / 11.7 | 5 |
| eog | GTK3 | large | 5 | 0 | 16.4 / 19.2 | 5 |
| eog | GTK3 | small | 5 | 0 | 11.2 / 14.1 | 5 |
| google-chrome | Chromium | large | 5 | 0 | 32.2 / 34.4 | 5 |
| google-chrome | Chromium | small | 5 | 0 | 31.5 / 34.1 | 5 |
| qt-assistant | Qt 5.15 | large | 5 | 0 | 252.7 / 253.6 | 2 |
| qt-assistant | Qt 5.15 | small | 5 | 0 | 12.4 / 14.1 | 5 |

Not measured by T1: xterm (no native Wayland), java-swing (no native Wayland),
gnome-terminal (native controls were server windows, `final == requested` 0 of 10),
gnome-calculator (not a T1 representative; its native controls are in `matrix-notes.md`).
VS Code (Electron) was not run.

## T2: plain launch with the toolkit's own geometry option

Method: no `spawn-at spawn`; `tools/psh APP <geometry option>` with the observer running for the
whole launch (`xflash observe`), 10 runs per app, idle, 6 s per run. The geometry read is the last
X11 ConfigureNotify before the WM reparent (or the CreateNotify geometry when the client sends no
ConfigureNotify), i.e. the position/size the client asked for on the root window. "Later size"
counts ConfigureNotify events after the map whose width or height differs from that initial
geometry. `-geometry` for xterm is in character cells (80x24), so no pixel size is requested.

| app | option | runs | at requested position | at requested size | later size changes | first change ms | final size |
| :-- | :-- | --: | --: | --: | --: | :-- | :-- |
| xterm | `-geometry 80x24+120+140` | 10 | 10 | - (cells) | 0 in 10 | - | 484x316 in 10 |
| google-chrome | `--window-size=800,600 --window-position=120,140` | 10 | 10 | 10 | 0 in 10 | - | 800x600 in 10 |
| qt-assistant | `-geometry 800x600+120+140` | 0 (skipped) | - | - | - | - | - |
| VS Code (Electron) | `--window-size` | 0 (skipped) | - | - | - | - | - |
| GTK apps | no geometry option | 0 (skipped) | - | - | - | - | - |

Skips and the exact error: Qt Assistant prints `Unknown option: -geometry` and exits 255 before
creating a window (the installed assistant's option list has no geometry option; no other Qt app
is installed). VS Code was skipped because the earlier run showed no window. The GTK apps in the
table (`gnome-text-editor`, `eog`, `gnome-calculator`) have no geometry command-line option;
`gnome-terminal --geometry` exists but that app's windows in this session belong to the Wayland
server (see `matrix-notes.md`).

Raw: `data/matrix/t2/xterm.tsv`, `data/matrix/t2/chrome.tsv` (10 rows each), one 100-line observer
sample per app, and `data/matrix/t2/README.txt`; the full observer files stayed in
`scratch/xflash02/t2/` (git-ignored).
