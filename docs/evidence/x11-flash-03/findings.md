# X11-FLASH-03a and 03b findings

Written 2026-10-29. Branch `exp/x11-flash-03`. Every number below comes from a file in `data/<cell>/`
(per-run rows in `probe-runs.csv`, summaries in `probe-summary.md`). "Measured" means a probe run
in the private headless Shell; "reasoned" means not run.

## Setup, in one table

| Item | Value |
| :-- | :-- |
| Shell | private headless GNOME Shell 46, one at a time, DING not loaded, extensions `spawn-at@harsh.local` (copy of `assets/gnome/modern`) and `xflash-probe@local`; Overview closed by the probe, so new windows take focus (a real desktop does too) |
| CLI "old id" | built from this tree (`src/` identical to main 7c053b9), launch key `..._TIME<epoch ms>` |
| CLI "fixed id" | PR #94 head `703dfb3` (still open), launch key `..._TIME0`; no `assets/` change, so the extension is identical. Not merged or cherry-picked into this branch |
| Used for | fixed id: every flash measurement of 03b. Old id: only the Task A crash cell, and the 03a controls/validation of xterm and eog (neither depends on the id) |
| Probe truth | per painted frame (`Clutter.Stage::after-paint`): actor and ancestors visible with paint opacity > 0, frame rect, buffer rect, surface texture size; and a PNG of the frame rect from `Shell.Screenshot` (capture mode). Passive mode logs the same without PNGs |
| Bad frame | a visible frame before the first good one. Texture reading: good = frame and texture have the final size. Pixel reading: also not blank (mostly transparent or black) |
| Display caveat | headless Shell, no vblank. Painted frames are not 60 Hz. Durations are only comparable between cells of one mode (see the perturbation check) |

## Verdict

1. The probe is trustworthy: a no-wait reveal gives bad frames in 20 of 20 xterm and eog runs;
   native Wayland gives 0 in 80 of 80 runs (editor, calculator, eog, Qt, Chrome).
2. **DING is not required for the focus-at-close abort.** Without DING, the old-id CLI aborted the
   private Shell on the first close of a focused forced-X11 gnome-text-editor in 2 of 2 attempts; the
   fixed-id CLI closed 50 of 50 focused windows (10 + 20 + 20 editor runs) with 0 aborts.
3. The flash on XWayland is real and varies by toolkit from none to every run. Capturing frames
   does not change **whether** a bad frame happens, but it inflates **how long** it lasts for fast
   apps (eog: median 11.6 ms with capture against 1.9 ms passive). Durations from capture mode
   must not be quoted as visibility.
4. For eog the driver is the window size, through the time the client needs to commit the
   final-size buffer: 0% bad runs at 600 and 800 px wide, 40% at 1000, 56% at 1100, 80% at 1400,
   100% at 1700.
5. Events seen from the X side (R, D, L) do not predict the bad frame. A texture-size gate on the
   compositor side does, except for Qt and Chrome, whose bad frames are black frames at the final
   size, visible only in pixels.

## Task A: bounded reproduction of crash 2 (editor, forced X11, DING not loaded)

| Run set | CLI | Runs | Closed gracefully | `focused_at_close` | Shell aborts | Startup id |
| :-- | :-- | --: | --: | :-- | --: | :-- |
| crash 2 (03a, `val-gnome-text-editor`) | old id | 1 | 1 | true | 1 (first close) | `_TIME1793...` |
| `A-old-editor` | old id | 1 | 1 | true | 1 (first close) | `_TIME1793291347721` |
| `A-fix-editor` | fixed | 10 | 10 | true in 10 | **0** | `_TIME0` |
| `val-fix-editor` (Task B) | fixed | 20 | 20 | true in 20 | **0** | `_TIME0` |
| `pas-gnome-text-editor` (passive) | fixed | 20 | 20 | true in 20 | **0** | `_TIME0` |

Both aborts: `libmutter:ERROR:../src/core/window.c:1518:meta_window_unmanage: assertion failed:
(window->display->focus_window != window)`, preceded by `Buggy client sent a _NET_ACTIVE_WINDOW
message with a timestamp of 0`. See `crash-2-focus-assertion.md` and `crash-3-reproduction.md`.
The old-id binary aborts on the first run, so the "up to 5 runs" were 1 per Shell (2 Shells in
total, counting crash 2). Plain statement: **without DING the old-id binary crashes and the fixed
one does not.** DING was one way to reach the abort; focus at close plus the far-future startup
time is sufficient.

Reconciliation with FIX-01b cell 1d (no DING, editor focused at close, 0 of 10 aborts): the cell
record is not in this repository (the fix branch's `docs/KNOWLEDGE.md` says only that without
DING the app "never sent the activation" in 30 runs with native windows). What I can say from the
runs here: the abort needs the window to be focused at close with the old id; the Shell in these
runs was out of the Overview, so the window took focus by itself (`focused_at_close` true in 52 of
52 editor runs; the 2 old-id runs aborted, the 50 fixed-id ones did not). In the 02i-b runs the Shell stayed in the Overview and `focused_at_close` was false in
50 of 50 editor runs and 0 aborted. If cell 1d focused the window with `spawn-at focus`, which
gives Mutter a current timestamp, that would overwrite the future focus time and the close would
survive. That reading is NOT tested (running it needs a focus assertion cell, which the
maintainer permitted only for Task A's cell).

## Task B: finishing Task 2

### Controls (03a, recap)

| Cell | Runs | Runs with a bad frame |
| :-- | --: | --: |
| positive, no-wait patch, xterm | 10 | 10 |
| positive, no-wait patch, eog | 10 | 10 |
| negative, native Wayland: editor, calculator, eog, Qt, Chrome (T1) | 20 each = 100 | 0 |

### Repeatability: eog 1100x760, the same cell in 8 batches of 10

| Batch | Mode | Runs with a bad frame (texture) | Late damage D>0 | Late resize R>0 |
| :-- | :-- | --: | --: | --: |
| `val-eog` (old id, 03a) | capture | 4 | 0 | 8 |
| `rep-eog-large-1` | capture | 7 | 6 | 9 |
| `rep-eog-large-2` | capture | 8 | 5 | 9 |
| `rep-eog-large-3` | capture | 6 | 4 | 7 |
| `pert-eog-cap1` | capture | 3 | 2 | 7 |
| `pert-eog-cap2` | capture | 6 | 6 | 9 |
| `pert-eog-pas1` | passive | 6 | 0 | 8 |
| `pert-eog-pas2` | passive | 3 | 5 | 6 |

Spread over the 7 batches of the fixed-id session: 3 to 8 of 10, mean 5.6, pooled 39 of 70 (56%).
The first batch (`val-eog`, a different Shell) is 4 of 10, which makes 43 of 80 with it. Counting events alone (R) does not
separate them: R>0 is 6 to 9 of 10 in every batch. D shifted between sessions (0 of 10 in
`val-eog`, 2 to 6 of 10 later) while the bad rate did not, so D and L from the 02 data are not
comparable with these.

### Probe perturbation: capture against passive, eog and xterm large, 20 runs each

| App | Mode | Runs with a bad frame | R>0 | D>0 | L median ms | Visible frames per run (median) | Window from first frame to final-size commit (median / max ms) |
| :-- | :-- | --: | --: | --: | --: | --: | :-- |
| eog | capture | 9 | 16 | 8 | 14.8 | 28 | 11.6 / 15.1 |
| eog | passive | 9 | 14 | 5 | 15.3 | 93 | 1.9 / 6.2 |
| xterm | capture | 0 | 0 | 0 | -0.4 | 6 | - |
| xterm | passive | 1 | 0 | 0 | -0.4 | 5 | one run, 25.5 ms |

Capture does not change the bad-frame rate (9 and 9 of 20; 0 and 1 of 20) nor R, D, L within the
spread seen in the repeatability table. It does stretch the frame cadence (median gap 5.0 ms
against 2.2 ms), and with it the measured bad window of a fast app: the screenshot blocks the main
loop, so the commit is dispatched later. For slow apps the effect is small: editor windows from
first frame to commit are 52 to 77 ms passive (9 of 20 runs) and 62 to 91 ms with capture (11 of
20); calculator 42 to 192 ms passive (20 of 20) and 37 to 220 ms with capture (40 of 40).
**Consequence:** earlier `p60` and `bad_ms` figures for eog (13 ms, 75%) are capture artefacts. In
passive mode a bad eog frame lasts about 2 ms (max 6), so on a 60 Hz display about 11% (max 37%)
of the bad runs would show it, roughly 5% of eog large spawns.

### Editor validation (forced X11, fixed id, 10 per size)

| Size | Runs with a bad frame | Window from first frame to commit | Notes |
| :-- | --: | :-- | :-- |
| small 640x420 | 6 of 10 | 62 to 91 ms (capture) | the first texture equals the previous window's final texture in 11 of 11 bad runs |
| large 1100x760 | 5 of 10 | 69 to 91 ms | R is 1 of 20 runs, D 0 of 20 |

The editor opens with the previous run's window size (the texture at the first visible frame equals
the previous run's final texture in 18 of 19 runs, and in all 11 bad ones); it is good when the
requested size equals that size, and bad otherwise, with a 60 to 90 ms gap while it relays out. R
fires in 1 of 20 runs and D never, so a gate built on X-side events would not see it. The 10 runs of
`A-fix-editor` agree (previous size in 8 of 9, 4 of its 5 bad runs).

## Task C: the original Tasks 3 to 5

### 3a. Does a lead under one frame produce a bad frame? What separates R-fired runs?

Yes. Runs with 0 < L <= 16.7 ms: eog 32 of 79 with a bad frame, calculator 4 of 4, editor 2 of 3,
xterm 10 of 10 (positive control); Chrome and Swing 0 of 4 each. Bad windows of 8.6 to 27 ms
(capture mode) and 0.4 to 6 ms (passive) existed, both under 16.7 ms. L itself is a poor
predictor: of 111 runs with L <= 0 (nothing painted after the reveal), 54 had a bad frame by
pixels (Qt, Chrome, editor, calculator).

eog, 140 runs with sizes from 600 to 1700 px: R>0 in 88, of which 58 bad and 30 good; R=0 in 52, of
which 7 bad. What separates the bad runs from the good ones is a race between two times measured
from the reveal: the time of the first painted frame and the time the final-size texture arrives.

| | first painted frame | final-size texture | ConfigureNotify (R_first) |
| :-- | --: | --: | --: |
| 25 bad runs of 50 (1100 and 600) | 2.9 ms | 15.4 ms | 2.65 ms mean over the 58 bad R>0 runs |
| 25 good runs | 7.8 ms | 7.2 ms | 1.14 ms mean over the 30 good R>0 runs |

A run is bad exactly when the first frame precedes the texture. R_first is 1.5 ms later in bad
runs, but the ranges overlap (0.17 to 1.81 ms good, 1.15 to 9.81 ms bad).

Size as the driver (eog, same session, all fixed id):

| Window width x height | Runs | Bad | Mean commit of the final-size texture |
| :-- | --: | --: | --: |
| 600x420 | 10 | 0 | 13.1 ms* |
| 800x560 | 10 | 0 | 7.3 ms |
| 1000x700 | 10 | 4 | 11.8 ms |
| 1100x760 | 70 | 39 | 11.9 ms |
| 1400x900 | 10 | 8 | 20.0 ms |
| 1700x1000 | 10 | 10 | 31.6 ms |

\* in that cell the first frame was also late (14.1 ms), so no run was bad; in the 03a session the
600x420 commit took 5.4 ms.

Larger windows take longer to commit, and the rate of bad runs rises with it. Size is a driver,
through client latency. The phase of the first frame (set by the frame clock) is the other half and
differs between cells, so a rate is not a property of a size alone.

Implications for the gate rules (R1 and R2 are in `tools/src/analyze.rs`; R3 and R4 are not
defined anywhere I could read, so I did not evaluate them). Evaluated offline against the probe on
the 120 eog runs that have both X-side data and probe data (61 bad):

| Rule | Fires before the commit (misfire) | Of the 61 bad runs left bad | Mean extra reveal delay |
| :-- | --: | --: | --: |
| R1: first damage after the resize, plus a frame | 25 | 20 | 17.2 ms |
| R2: end of the first damage burst, plus a frame | 3 | 1 | 35.8 ms |
| Texture gate: reveal once the texture size equals the buffer rect size | 0 by construction | 0 | 13.9 ms |

The texture gate (compositor side, no X traffic): at the first visible frame, texture size equal to
buffer-rect size occurred in 261 windows of all X11 cells, none of which was bad by the texture
reading; unequal in 205, of which 181 were bad (the 24 others: Chrome 20, editor 2, nautilus 1,
system-monitor 1, where the buffer rect is not the final texture size, so the gate would hold them
for nothing). It does not help where the texture is already final and the pixels are
not: Qt (16 of 20 runs black at final size) and Chrome (19 of 20), see Task 4. It also does not
help GTK4 apps that change their own size after the reveal (calculator).

### 3b. The A=0 oddity: late resize after reveal, zero ANCHOR_CORRECTION

Cause: the **frame rect is already final at the reveal**. The extension's `move_resize_frame` at
SetSize makes Mutter set the frame size at once, so `Meta.Window::size-changed` fires once, before
the reveal. What R counts afterwards is the X client window and its texture catching up inside the
already-sized frame. The anchor listener (`_armAnchor`, `size-changed`) has nothing to act on.
Evidence from the probe (`size_changed_after` and `pos_changed_after` in `probe-runs.csv`):

| App | Windows | `size-changed` after reveal | `position-changed` after reveal |
| :-- | --: | --: | --: |
| eog | 170 | 0 | 0 |
| gnome-text-editor | 30 | 0 | 0 |
| xterm | 70 | 0 | 0 |
| qt-assistant | 40 | 0 | 0 |
| google-chrome | 40 | 0 | 0 |
| java-swing | 20 | 0 | 0 |
| gnome-calculator | 40 | **3** | **3** |

Of the three candidates: not a same-size ConfigureNotify (R counts real size changes of the X
window), not "the listener does not fire for X11" (it fired in the 3 calculator runs where the
frame did change, each with an `ANCHOR_CORRECTION`, and the window ended at the right place), and
not "a resize that does not move the anchor" (the anchor is bottom-right, any frame size change
moves it). It is the fourth: **no frame change, so no event**.

Wrong final position because of it: no case found in the eog, xterm, editor, Qt, Chrome and Swing
data. Over 1128 X11 runs (719 from 02, 409 here) the final position differs from the bottom-right
anchor for the final size in 6: 5 in the 02 java-swing loaded-small batch (13 px too high, all with
A=0, cause not established; none of the 20 Swing runs here, where there are also 0
`position-changed` events after the reveal) and 1 gnome-system-monitor smoke run (requested
1000x700, final 700x500, A=1, position kept for 1000x700). So the listener's blind spot is a client
that moves or reshapes itself without a `size-changed`; the evidence for Swing's cause is
indirect and unverified.

### 3c. Why the R1 "settled" rule misfired on xterm

R1 fires at the first damage after the first size change that follows the resize request, plus
one frame. xterm answers a resize with a long burst of small DamageNotify events (at least 32 events
in run 4 of the 02 loaded batch, 60 or more in run 29; the dump prints the first 60), so the first damage is the start of the
repaint, not the end. Reproduced from the raw 02 data (`xflash analyze --dump 4`): run 4 resized to
898x511 at -41.9 ms, damage from -43.4 ms, the burst ended at +17.4 ms; R1 fired about 17 ms after
the first damage, still inside the burst. Counts in the 02 loaded data: R1 misfired in 4 of 5 "needed"
runs (large) and 1 of 2 (small). The idle xterm was the easy case only because nothing was late
(0 needed, 0 misfires); the rule is wrong whenever the answer to the resize is a burst longer
than a frame. R2 (end of the burst) does not have that flaw and misfired 3 of 120 times on eog.

### 3d. gnome-calculator ending at 360x492 instead of the request

40 runs, forced X11, fixed id, capture mode, plus 20 passive runs:

| Request | Runs | Final 900x700 / 500x504 | Final 360x492 | Other |
| :-- | --: | --: | --: | :-- |
| 900x700 (capture) | 20 | 14 | **6 (30%)** | |
| 500x400 (capture) | 20 | 13 at 500x504 | **7 (35%)** | |
| mixed (passive) | 20 | 7 at 900x700, 6 at 500x504 | 5 (25%) | 2 at 500x492 |

It is the **application resetting its own default size (360x492), a race, not a minimum**: 360x492
is smaller than 900x700, a minimum could only enlarge. In the 3 capture runs where the shrink
came after the reveal, the probe shows the frame rect going 900x700 to 360x492 about 23 ms after the
reveal and the first real frame of the app only about 100 ms after; the anchor listener moved
the window correctly. In the other runs the shrink landed before the reveal and was absorbed by
the late-resize check. A real minimum also shows: a 400 px high request ends at 492 or 504 px
(`size_raised` true), which is the app's minimum content height (492) plus 12 px. The old-build
rate was about 1 in 20 in a different Shell state; with the Overview closed it is 5 to 7 in 20, so
the rate depends on the session state, and I cannot say whether the fixed id changed it (the old-id
CLI was not run on the calculator, because a focus abort there would breach the stop rule).
gnome-system-monitor (GTK4 libadwaita) did the same in its one large smoke run: 1000x700 requested,
700x500 final.

## Task 4: per-toolkit candidate table (no implementation)

M = measured in this branch, R = reasoned only. The "old state" in 02i-b data means the Shell was in the Overview.
`n` is runs; "bad" means runs with a bad frame by pixels unless noted.

| Toolkit (app) | Bad-frame rate | What the client does after the configure | T1 native Wayland via env | T2 launch-time geometry | T3 client-settled gate | T4 `_NET_WM_SYNC_REQUEST` |
| :-- | :-- | :-- | :-- | :-- | :-- | :-- |
| Xt (xterm) | **M** 0 of 40 (capture 0/20, main-CLI 0/20) and passive 1 of 20 (one 25 ms old-size frame); positive control 10 of 10 | own resizes 10x17 then 484x316 at map, then the requested size, repaint is a burst of 30 to 60+ damage events that ends before the reveal when idle | not possible, X11 only (**M**, no Wayland mode) | **M** `-geometry` placed 10 of 10 in 02i-b (old state; cells, not px) | **M** R1 misfires on the burst (3c); R2 fine (offline, eog only: **R** for xterm) | **R** not supported by Xt clients |
| GTK3 (eog) | **M** 43 of 80 at 1100x760; 0 to 100% by size (0/20 at 600 and 800, 10/10 at 1700) | maps at its default 592x502 texture; ConfigureNotify +1 to 3 ms after reveal; final-size buffer 3 to 32 ms after reveal, longer for bigger windows; `size-changed` never fires after reveal | **M** native 0 of 20 bad, 20 of 20 final size; GDK_BACKEND=wayland | no option (**M**, 02i-b) | **M** offline: R1 leaves 20 of 61 bad, R2 1 of 61 (35.8 ms extra); texture gate 0 left (13.9 ms) | **R** GTK3 supports it; not tried |
| GTK3 (mousepad, evince) | **M** n=2 each: large bad in 1 of 1, small 0 of 1 | same pattern as eog (late texture on the large size) | **R** | **R** no option | **R** as eog | **R** |
| GTK4 forced X11 (editor) | **M** 16 of 30 (11 of 20 + 5 of 10); passive 9 of 20 | opens at the previous window's size; relayout takes 60 to 90 ms; R 1 of 30, D 0 of 30 | **M** native 0 of 20 bad | none | **M** blind: R and D almost never fire; the texture gate was closed in all 25 bad runs (and in 2 good ones, **M**) | **R** GTK4 supports it |
| GTK4 forced X11 (calculator, system-monitor, baobab, nautilus) | **M** calculator 40 of 40 (42 to 220 ms); passive 20 of 20; sysmon, baobab, nautilus n=2 each: 0, 2, 1 bad | GTK4 reset to its own default size 25 to 35% of runs (calculator 360x492, sysmon 700x500), +23 ms after reveal in some | **M** calculator native 0 of 20 bad | none | **M** a texture gate does not cover the size reset after the reveal | **R** |
| Qt 5.15 (assistant, xcb) | **M** 16 of 20 by pixels, 4 of 20 by texture | maps at the final size, first buffer is black (a flash invisible to the size), black for 12 to 36 ms in the runs where it ended (medians 19 and 12 ms); R 1 of 20, D 19 of 20 | **M** native 0 of 20 bad; final size 19 of 20 (one 700x500) | **M** no `-geometry` option in assistant (02i-b) | **R** damage after reveal is seen (D) so a damage-end rule would work; texture gate useless | **R** xcb supports it |
| Java Swing | **M** 0 of 20 (R 7, D 14 of 20 are false alarms) | resizes within the frame without a bad paint | none (no Wayland toolkit used) (**R**) | **R** no option | **M** R and D would delay 14 of 20 runs for no gain | **R** unknown |
| Chromium (Chrome, ozone x11) | **M** 19 of 20 by pixels (black at the final size, 150 ms to more than 300 ms, some censored); 0 by texture | maps at the final size with an unpainted black buffer until the page renders | **M** native 0 of 20, 20 of 20 final size | **M** `--window-size`/`--window-position` 10 of 10 in 02i-b | **R** damage-based; texture gate useless (the texture is final) | **R** supported |
| Electron (VS Code, x11) | **M** n=1: 0 bad; window cannot be closed gracefully (not_closed) | maps at final size | **R** `--ozone-platform=wayland`; not measured | **R** `--window-size` | **R** | **R** |
| OpenGL terminal (kitty) | **M** n=2: 2 bad (19 ms each), by pixels | not examined | **R** `-o linux_display_server=wayland` | **R** | **R** | **R** |
| Athena (xclock, xcalc) | **M** n=2 each: xclock 1, xcalc 0 | not examined | not possible (X11 only) | **R** | **R** | **R** |
| Gecko (Firefox) | not measurable: `no_window` in both smoke runs (snap) | - | **R** `MOZ_ENABLE_WAYLAND=1` | **R** | **R** | **R** |

LD_PRELOAD shim last: **R** only; not tried here. Best-measured reading: T1 removes every
flash in the five toolkits that have a native mode (0 of 100 runs); where a native mode is not
available, a compositor-side texture gate (reveal when the texture size matches) would remove every
texture-visible bad frame in GTK3 and GTK4, but it needs a second signal for Qt and Chromium (blank
frames at the final size).

## Measured vs reasoned

Measured: the controls, repeatability, perturbation, editor, calculator, size sweep, per-toolkit
bad rates, the 3b event counts, the 3c reproduction from the raw 02 data, the offline rule and gate
evaluations on the probe data, the Task A reproduction.

Reasoned only: why the first frame comes at 2 to 8 ms (frame-clock phase), the cause of crash 2
beyond the matching signature (the far-future startup time), cell 1d's outcome, every T4 cell
marked R, the 60 Hz visibility derived from windows, the cause of the 5 Swing position mismatches.

## Not run

Real GNOME Wayland (live), real displays, X11 sessions, other GNOME versions; loaded (CPU-busy)
conditions; the old-id CLI on anything but the editor; a focused close with an old id and
`spawn-at focus`; `focus-new-windows=strict`; native Wayland for mousepad, evince, kitty,
Electron, Firefox; RUNBOOK-03b's full matrix; R3 and R4 (undefined).
