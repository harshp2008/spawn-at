# X11-FIX-02: the texture gate

Written 2026-10-30. Everything is `measured` unless it says `reasoned`. Product code: branch
`fix/x11-texture-gate`, commit `01bb587` (`assets/gnome/modern/commit.js` `waitForTexture`,
`extension.js` `_stepWaitForCommit`), 15 new node tests in `tests/ready_wait_wiring_test.js`.

## Verdict

The gate does what 03b predicted for GTK, at a price in hidden time, and it has no effect on the other
class of flash.

1. **GTK flashes by texture size are gone in the measured cells.** Windows with a bad frame, capture
   mode, 20 runs per size: eog 1100 px 16 to 0 of 20, 1700 px 17 to 0, 600 px 0 to 0; gnome-text-editor
   640x420 11 to 0, 1100x760 10 to 1; gnome-calculator 500x400 18 to 0, 900x700 20 to 0. The two
   eog sentinels (start and end of each session) went from 8 and 9 of 10 to 0 and 0 of 10.
   The one editor run that is still bad (and one more that never got a final-size buffer in either
   variant) are explained below. All 460 gated GTK-and-regression runs of the main `aft-` cells reached
   the reveal by `texture-matched` except 2 (both editor, `cap`).
2. **No placement regression.** The frame sat at the bottom-right anchor for its final size in every one
   of 520 + 520 runs, and the final size equalled the CLI's own report in every run. The final size
   equals the request as often as before except gnome-calculator at 900x700 (15 to 15 of 20 capture, 16
   to 14 of 20 passive), where the app resets itself to 360x492: 20 of 80 runs before, 21 of 80 after.
   The gate neither causes nor prevents that reset.
3. **Native Wayland is untouched.** 0 bad frames in 60 and 60 runs, `READY_WAIT_RESOLVED` medians
   within 1 ms (7.9/5.9/37.5/8.8/12.4/7.6 ms before, 7.2/5.3/37.0/9.1/11.4/7.7 ms after, per app and
   size), and the extension's full trace (log lines, D-Bus signals, final frame and elapsed time) of
   10 native scenarios through `_runBatch` is byte-identical on `main` and on the branch (same
   sha256, `native-trace/`).
4. **The price is hidden time, paid by apps that are slow to commit.** Median cloak to reveal (passive):
   eog +6, +15, +22 ms (600, 1100, 1700 px), editor +31 ms (640x420) and unchanged median at 1100x760
   (p90 11.6 to 88.2 ms), calculator +75 and +50 ms (max 115 ms). xterm +0, Swing, Qt and Chrome 0 to
   +9 ms.
5. **It does not touch a frame of the right size with nothing painted in it.** Qt (16 of 20 runs
   before, 5 after), Chrome (18 of 20 before and after), calculator large (4 of 20 after), editor
   large (1 of 20 after). That needs another signal.
6. **The premise about Chrome was wrong for the extension.** 03b said a texture gate "never matches
   Chrome", so the cap would always release it. In 20 of 20 gated Chrome runs the gate matched after a
   median of 9 to 13 ms (max 58). The cap path was therefore shown with the editor (2 natural cap hits)
   and with a forced 5 ms cap on eog.

If you read "success" as GTK 0 bad runs: it is met except one editor run of 20 (explained), so I would call
it met with that stated. I am not opening the PR as a ready PR only because the evidence is one
private Shell per side and no live session; it is a draft as you asked.

## Setup

| Item | Value |
| :-- | :-- |
| Shell | private headless GNOME Shell 46.0, one at a time (before: 00:26 to 01:41, after: 01:42 to 02:56 on 2026-10-30), `spawn-at@harsh.local` + `xflash-probe@local` only, DING not loaded, Overview closed, new windows take focus |
| CLI | built from `main` `0984e11` (PR #94 merged), `--no-focus --anchor bottom-right --size W H -m 24` (the 03b harness) |
| Extension before | `assets/gnome/modern` of `main` (`ext-trees/before-sha256.txt`; identical to `exp/x11-flash-03`'s copy) |
| Extension after | `01bb587`, same `assets/gnome/modern` as the tree loaded (measured before the commit was made; `diff -r` empty) (`ext-trees/after-sha256.txt`; only `commit.js` and `extension.js` differ; `gate.patch`) |
| Cells | capture: eog 600x420 + 1100x760 (20 each), eog 1700x1000 (20), editor (forced X11, 20 per size), calculator (forced X11, 20 per size), xterm, Java Swing, Qt Assistant, Chrome (10 per size), two eog sentinels (10 each); native: editor, calculator, eog (20 each, 10 per size); passive: the same X11 cells without the sentinels. 520 runs before, 520 + 70 (off switch and cap demo) after, all `status=ok` |
| Bad frame | capture mode, texture reading (a visible frame before the first good one, good = texture is the frame's final size) and pixel reading (also blank frames). Durations come from passive cells only (03b: capture stretches them) |
| Reveal delay | `cloak_ms`-style: time from the `BATCH_STEP inst=Cloak` line to `UNCLOAK_4D_REVEAL` in the extension log, per window (`ext.log.gz`) |
| Placement | `final = request` (`runs.csv`), `final = reported` (the window after the run equals what `spawn-at` printed), `anchored` (final x+w = 1896 and y+h = 1056, which is the 1920x1080 work area minus the 24 px margin) |
| Never final | the last texture the probe saw is not the buffer rect of the reveal (the probe's `bad_tex` is 0 for a window that never gets a good frame; this column finds them) |

Session drift: before and after ran in different Shells, so a rate could differ for reasons unrelated
to the gate (03b saw 3 to 8 of 10 between batches). Two things bound that: the sentinels bracket each
session (before 8 and 9 of 10, after 0 and 0), and the **same-Shell off switch** (`aft-off-*`: the gated
extension with `[wait] strategy = "legacy"`, eog 1100 px 14 of 20 bad, editor 9 of 20) against the gate
on in that Shell (eog 1100 px 0 of 20, editor 1 of 20).


### Bad runs, capture mode

| group | n | bad (texture) before → after | bad (pixels) before → after | final texture never reached buffer, before → after |
| :-- | :-- | :-- | :-- | :-- |
| eog x11 1100x760 | 10 | 8 → 0 | 8 → 0 | 0 → 0 |
| eog x11 1100x760 | 20 | 16 → 0 | 16 → 0 | 0 → 0 |
| eog x11 600x420 | 20 | 0 → 0 | 0 → 0 | 0 → 0 |
| eog x11 1700x1000 | 20 | 17 → 0 | 17 → 0 | 0 → 0 |
| gnome-text-editor x11 1100x760 | 20 | 10 → 1 | 10 → 2 | 1 → 1 |
| gnome-text-editor x11 640x420 | 20 | 11 → 0 | 11 → 0 | 0 → 0 |
| gnome-calculator x11 500x400 | 20 | 18 → 0 | 18 → 0 | 0 → 0 |
| gnome-calculator x11 900x700 | 20 | 20 → 0 | 20 → 4 | 1 → 0 |
| xterm x11 400x260 | 10 | 0 → 0 | 0 → 0 | 0 → 0 |
| xterm x11 900x560 | 10 | 0 → 0 | 0 → 0 | 0 → 0 |
| java-swing x11 600x400 | 10 | 0 → 0 | 0 → 0 | 0 → 0 |
| java-swing x11 900x650 | 10 | 1 → 0 | 1 → 0 | 0 → 0 |
| qt-assistant x11 1000x700 | 10 | 6 → 0 | 10 → 1 | 0 → 0 |
| qt-assistant x11 700x500 | 10 | 0 → 0 | 6 → 4 | 0 → 0 |
| google-chrome x11 1200x800 | 10 | 0 → 0 | 8 → 8 | 0 → 0 |
| google-chrome x11 700x480 | 10 | 0 → 0 | 10 → 10 | 0 → 0 |
| eog x11 1100x760 | 10 | 9 → 0 | 9 → 0 | 0 → 0 |
| gnome-text-editor wayland 1100x760 | 10 | 0 → 0 | 0 → 0 | 0 → 0 |
| gnome-text-editor wayland 640x420 | 10 | 0 → 0 | 0 → 0 | 0 → 0 |
| gnome-calculator wayland 500x400 | 10 | 0 → 0 | 0 → 0 | 0 → 0 |
| gnome-calculator wayland 900x700 | 10 | 0 → 0 | 0 → 0 | 0 → 0 |
| eog wayland 1100x760 | 10 | 0 → 0 | 0 → 0 | 0 → 0 |
| eog wayland 600x420 | 10 | 0 → 0 | 0 → 0 | 0 → 0 |

### Placement, capture and passive cells pooled by cell

| group | n | final = request | final = what the CLI reported | frame at the bottom-right anchor for its final size | ended 360x492 |
| :-- | :-- | :-- | :-- | :-- | :-- |
| eog x11 1100x760 | 20 | 20 → 20 | 20 → 20 | 20 → 20 | 0 → 0 |
| eog x11 600x420 | 20 | 20 → 20 | 20 → 20 | 20 → 20 | 0 → 0 |
| eog x11 1700x1000 | 20 | 20 → 20 | 20 → 20 | 20 → 20 | 0 → 0 |
| gnome-text-editor x11 1100x760 | 20 | 20 → 20 | 20 → 20 | 20 → 20 | 0 → 0 |
| gnome-text-editor x11 640x420 | 20 | 20 → 20 | 20 → 20 | 20 → 20 | 0 → 0 |
| gnome-calculator x11 500x400 | 20 | 0 → 0 | 20 → 20 | 20 → 20 | 4 → 6 |
| gnome-calculator x11 900x700 | 20 | 15 → 15 | 20 → 20 | 20 → 20 | 5 → 5 |
| gnome-calculator x11 500x400 | 20 | 0 → 0 | 20 → 20 | 20 → 20 | 7 → 4 |
| gnome-calculator x11 900x700 | 20 | 16 → 14 | 20 → 20 | 20 → 20 | 4 → 6 |
| xterm x11 400x260 | 10 | 0 → 0 | 10 → 10 | 10 → 10 | 0 → 0 |
| xterm x11 900x560 | 10 | 0 → 0 | 10 → 10 | 10 → 10 | 0 → 0 |
| java-swing x11 600x400 | 10 | 10 → 10 | 10 → 10 | 10 → 10 | 0 → 0 |
| java-swing x11 900x650 | 10 | 9 → 10 | 10 → 10 | 10 → 10 | 0 → 0 |
| qt-assistant x11 1000x700 | 10 | 10 → 10 | 10 → 10 | 10 → 10 | 0 → 0 |
| qt-assistant x11 700x500 | 10 | 10 → 10 | 10 → 10 | 10 → 10 | 0 → 0 |
| google-chrome x11 1200x800 | 10 | 10 → 10 | 10 → 10 | 10 → 10 | 0 → 0 |
| google-chrome x11 700x480 | 10 | 10 → 10 | 10 → 10 | 10 → 10 | 0 → 0 |

### Reveal delay and bad runs, passive cells

| group | n | bad (texture) | cloak→reveal median ms | p90 | max | gate held median / max ms (after) | gate reasons (after) |
| :-- | :-- | :-- | :-- | :-- | :-- | :-- | :-- |
| eog x11 1100x760 | 20 | 15 → 0 | 2.6 → 17.5 | 5.7 → 22.0 | 9.7 → 22.6 | 13.3 / 16.7 | texture-matched:20 |
| eog x11 600x420 | 20 | 1 → 0 | 2.4 → 8.5 | 6.0 → 12.4 | 9.8 → 13.3 | 6.3 / 10.8 | texture-matched:20 |
| eog x11 1700x1000 | 20 | 17 → 0 | 3.0 → 24.6 | 16.0 → 26.6 | 17.8 → 29.7 | 19.2 / 24.4 | texture-matched:20 |
| gnome-text-editor x11 1100x760 | 20 | 9 → 0 | 5.0 → 4.2 | 11.6 → 88.2 | 13.2 → 92.2 | 0.2 / 87.8 | texture-matched:20 |
| gnome-text-editor x11 640x420 | 20 | 10 → 0 | 4.9 → 35.6 | 7.4 → 76.4 | 8.9 → 86.0 | 31.4 / 82.0 | texture-matched:20 |
| gnome-calculator x11 500x400 | 20 | 18 → 0 | 36.2 → 86.5 | 37.0 → 95.3 | 48.8 → 114.7 | 47.5 / 93.6 | texture-matched:20 |
| gnome-calculator x11 900x700 | 20 | 20 → 0 | 7.3 → 82.0 | 10.7 → 93.5 | 19.9 → 101.3 | 78.3 / 97.5 | texture-matched:20 |
| xterm x11 400x260 | 10 | 0 → 0 | 32.4 → 32.3 | 38.8 → 41.5 | 42.5 → 43.0 | 0.1 / 0.1 | texture-matched:10 |
| xterm x11 900x560 | 10 | 0 → 0 | 32.3 → 32.7 | 33.8 → 35.3 | 40.8 → 41.9 | 0.0 / 0.1 | texture-matched:10 |
| java-swing x11 600x400 | 10 | 0 → 0 | 4.1 → 2.8 | 5.5 → 4.3 | 5.7 → 4.8 | 0.0 / 0.1 | texture-matched:10 |
| java-swing x11 900x650 | 10 | 2 → 0 | 3.3 → 12.6 | 7.5 → 14.5 | 8.5 → 14.6 | 9.1 / 10.3 | texture-matched:10 |
| qt-assistant x11 1000x700 | 10 | 4 → 0 | 3.7 → 11.6 | 9.9 → 15.5 | 10.5 → 27.6 | 9.1 / 18.4 | texture-matched:10 |
| qt-assistant x11 700x500 | 10 | 1 → 0 | 3.2 → 7.1 | 4.5 → 8.4 | 4.7 → 9.0 | 4.2 / 5.1 | texture-matched:10 |
| google-chrome x11 1200x800 | 10 | 0 → 0 | 11.5 → 14.8 | 18.5 → 25.6 | 19.7 → 28.7 | 10.5 / 24.9 | texture-matched:10 |
| google-chrome x11 700x480 | 10 | 0 → 0 | 5.5 → 14.0 | 17.7 → 20.7 | 24.7 → 23.8 | 9.4 / 15.9 | texture-matched:10 |

### Native Wayland control

| group | n | bad px | final=request | ready-wait elapsed median / max ms | cloak→reveal median / max ms |
| :-- | :-- | :-- | :-- | :-- | :-- |
| gnome-text-editor wayland 1100x760 | 10 | 0 → 0 | 10 → 10 | 7.9 / 11.1 → 7.2 / 11.3 | 9.5 / 12.7 → 8.7 / 12.9 |
| gnome-text-editor wayland 640x420 | 10 | 0 → 0 | 10 → 10 | 5.9 / 8.5 → 5.3 / 8.6 | 7.6 / 10.1 → 6.9 / 10.9 |
| gnome-calculator wayland 500x400 | 10 | 0 → 0 | 0 → 0 | 37.5 / 38.9 → 37.0 / 40.1 | 39.3 / 40.9 → 38.8 / 42.1 |
| gnome-calculator wayland 900x700 | 10 | 0 → 0 | 10 → 10 | 8.8 / 12.7 → 9.1 / 10.8 | 10.3 / 14.6 → 10.7 / 12.7 |
| eog wayland 1100x760 | 10 | 0 → 0 | 10 → 10 | 12.4 / 15.4 → 11.4 / 15.2 | 13.9 / 17.0 → 12.8 / 17.1 |
| eog wayland 600x420 | 10 | 0 → 0 | 10 → 10 | 7.6 / 11.1 → 7.7 / 10.4 | 8.8 / 12.6 → 8.9 / 12.5 |

### Same-Shell checks (AFTER extension only)

| cell | group | n | bad texture | bad px | gate held med / max ms | reasons |
| :-- | :-- | --: | --: | --: | :-- | :-- |
| aft-off-eog-1100 | eog x11 1100x760 | 20 | 14 | 14 | - / - | - |
| aft-off-editor | gnome-text-editor x11 640x420 | 10 | 4 | 4 | - / - | - |
| aft-off-editor | gnome-text-editor x11 1100x760 | 10 | 5 | 5 | - / - | - |
| aft-capdemo-eog-1100 | eog x11 1100x760 | 20 | 0 | 0 | 9.4 / 12.3 | cap:13 texture-matched:7 |
| aft-capdemo-eog-1700 | eog x11 1700x1000 | 10 | 7 | 7 | 6.0 / 12.4 | cap:10 |

## The cap path

| Case | What happened |
| :-- | :-- |
| Natural, editor 1100x760, capture cell (2 of 20; passive 0 of 20) | `X11_GATE_RESOLVED reason=cap waited_ms=250.9 texture=(822x642) buffer=(1222x882)`. In one the texture arrived 68 ms after the reveal (318 ms after the gate started, `bad_tex` 1); in the other the window never got a final-size buffer in 2.5 s (`samples/editor-1100-never-final-texture-first-visible-frame.png`). An ungated run did the same once in 20 (`bef-cap-editor`, never-final 1) |
| Forced, `[wait] cap_ms = 5`, eog | `reason=cap waited_ms=7.3 / 10.8 / 7.7 cap=5 texture=(592x502) buffer=(1152x812)`: the timer fires 2 to 6 ms late. 1100 px: cap 13 of 20, 0 bad (the texture arrived during the frame wait that follows); 1700 px: cap 10 of 10, bad 7 of 10 (ungated: 17 of 20) |
| What the user sees at a cap hit | what an ungated window shows, 250 ms later: a default-size window in the corner of the final frame (`samples/capdemo-cap5-eog1700-first-visible-frame.png`: a 540x450 viewer inside a 1700x1000 frame, desktop behind it) |
| Chrome | not a cap case: matched in 20 of 20 after a median of 9 to 13 ms (max 58) |

## Placement and the calculator self-reset

Final size = request (before to after, passive and capture cells separately): calculator 900x700 capture
15 to 15 of 20, passive 16 to 14 of 20; 500x400 is raised to the app's minimum height (492 or 504) in 20 of
20 on both sides, so it is 0 of 20 by definition. All other cells unchanged (eog, editor, Swing, Qt, Chrome
20 of 20 or 10 of 10; xterm 0 by definition, it rounds to character cells). Position: anchored 100% on both
sides. Self-reset to 360x492, calculator: capture 5 + 4 before, 5 + 6 after; passive 4 + 7 before, 6 + 4
after: 20 of 80 against 21 of 80. Not fixed here.

## Measured against reasoned

Measured: every number in the tables, the gate's reasons and wait times from the extension log, the
off-switch and cap controls, the native traces, the screenshots.

Reasoned only: that polling rather than a signal is enough on other Shell versions (it worked here: the
texture updates while the actor is cloaked, 458 matches); why Qt dropped from 16 to 5 blank-frame runs
(the gate holds Qt 4 to 13 ms, which may let its first black buffer pass before the reveal; not
shown); that a real Xorg session behaves like XWayland (the gate reads only the actor texture and the
buffer rect); that a loaded machine keeps the 250 ms cap sufficient (the editor's 318 ms case suggests
a tail); how long a human can see 2 to 6 ms of cap overshoot or any of the added delays.

## Not run

A live GNOME Wayland session, a real Xorg session, GNOME versions other than 46.0, a loaded
(CPU-busy) machine, `focus-new-windows=strict`, real displays and 60 Hz vblank (the headless Shell has
none, so the frame counts are not display frames), other apps than the eleven above, the legacy
extension (GNOME 42 to 44, unchanged), macOS, Windows. `scripts/smoke-live.sh` was not run (owner only).

## Decisions for the maintainer

1. **Cap: 250 ms (from the data) or longer.** 250 covered every match but one in 460 gated windows
   (max 233 ms with capture, 195 ms passive; the editor's 318 ms case missed). A longer cap (400) would
   catch that one and costs only a window that never matches, which here was one window in 460.
2. **Off switch.** I reused `wait.strategy = legacy` (also `--wait-strategy legacy`) and `wait.cap_ms`,
   so there is no new setting, no protocol change and no Rust change. The catch: `legacy` also sends
   native Wayland windows down the old rule, so one cannot keep the ready-wait and drop only the gate. A
   separate `wait.x11_gate = on|off` key is a settings, help, docs and golden change in Rust (about 150
   lines); say if you want it.
3. **Blank frames (Qt, Chrome, calculator large).** A size gate cannot see them. Candidates for the next
   task: native-Wayland launch for the toolkits that have it (0 of 100 bad in 03b), or a damage signal
   after the first full paint.
4. **Ready or draft.** Draft, per your CLAUDE.md. I would be comfortable moving it to ready after one
   look in your own session with `spawn-at` logging on (`X11_GATE_RESOLVED` lines), because nothing
   here ran on a live Shell.

## Repo gates on the product branch (2026-10-30, `777867c`)

`cargo fmt --check` clean. `cargo clippy --workspace --all-targets --locked --features
spawn-at-verify/tui,spawn-at-verify/fixture -- -D warnings` clean. `cargo test --locked -p spawn-at -p
spawn-at-core -p xtask`: spawn-at 529 (488 unit + 10 + 7 + 1 + 19 + 3 + 1), spawn-at-core 64 (60 + 2 +
2), xtask 141 (139 + 2), 0 failed. `cargo test --locked -p spawn-at-verify --features tui,fixture`: 639
passed, 5 ignored (4 in `fixture_test.rs`, 1 printed under `tui_test.rs`), 0 failed. Workspace Rust total 1373 passed, 5 ignored. `node --test tests/*.js`: 276
pass (261 on `main` + 15 new), 0 fail. `cargo xtask docs-site`: 1131 HTML files, no broken internal
link. Not run: `scripts/smoke-live.sh`, anything on a live session.
