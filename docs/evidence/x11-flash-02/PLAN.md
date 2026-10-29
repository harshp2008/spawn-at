# X11-FLASH-02: plan and state (resume file)

Written 2026-10-29 after the second Shell crash. A fresh Claude Code session can resume from this
file plus `git log --oneline main..exp/x11-flash-02`. Branch: `exp/x11-flash-02`, local only.

## Goal

Measure, on real applications, why windows spawned through the GNOME extension flash on XWayland
and X11 (leading hypothesis: the legacy wait resolves in about 0.2 ms because Mutter sets the frame
rect at once, so the reveal beats the client's repaint). Evaluate offline the rule "reveal when the
X side saw a size-matching ConfigureNotify followed by a DamageNotify, then wait one frame". Publish
every experiment on the docs site under "Findings and experiments" in the house style of the 6.x
pages, with evidence committed under `docs/evidence/x11-flash-02/`. Push only through the gates and
only after the maintainer confirms.

## The session is Wayland

The maintainer runs GNOME Shell 46.0, mutter 46.2, `XDG_SESSION_TYPE=wayland`. There is no X11
session. Every X11 window is an XWayland client; `xflash observe` is an X client on Xwayland.
xterm is natively X11; gnome-text-editor, eog, Chrome and Firefox are forced onto X11 (see
`tools/src/run.rs`). Forced GTK apps behave unusually (see the crash notes).

## Standing rules (condensed)

- No change to `src/`, `crates/`, `assets/`, `install.sh`, product Cargo files, `build.rs`, `.cargo`,
  existing tests. No reinstall or reload of the extension. No gjs or Shell eval, no in-Shell pixel
  probe, no Mutter ScreenCast through PipeWire. Never open anything under `/var/crash` (`ls -la` only).
- No push, no merge to main, no force, no history rewriting, no Co-Authored-By trailers or session
  links in commits. Never kill by name; only pids the tool recorded. Work in `scratch/` (gitignored).
- Tools are Rust, outside the workspace (`tools/` has its own `[workspace]` table and lockfile).
  No Python in committed files.
- **After any Shell crash a session must stop, record the journal lines in a new crash note, and
  wait for the maintainer.**
- **No batch runs on the maintainer's live session until an isolation or bisect result says it is
  safe** (`isolation-feasibility.md`). Both crashes happened on the live session while closing a
  focused gnome-text-editor window.
- Gates for a push: G1 diff vs main only under `docs/`, `docs-site/` (and sidebar/index config);
  G2 the full test suite equals the baseline (`scratch/xflash02/baseline/`); G3 docs site build and
  checks clean (`cargo xtask docs-site`); G4 installer embedded-file guards pass, no new installer
  file; G5 evidence under 20 MB and scrubbed (no tokens, keys, emails, absolute home paths); G6 read
  `.github/workflows/docs.yml` to confirm Pages deploys on a push to main, else stop. Then
  fast-forward main and push only after the maintainer's explicit confirmation (the project rules
  otherwise forbid pushing main).

## Task list and status

| # | Task | Status | Where |
| :-- | :-- | :-- | :-- |
| 0 | Tool inventory, real apps, proto check | partly done. Present: ffmpeg, gst-launch, pipewire, python-xlib, xdotool, xev, Rust with a cached x11rb, xterm, xclock, xcalc, gnome-text-editor, gnome-calculator, nautilus, gnome-terminal, eog, evince, google-chrome, firefox (snap), code, java (JRE). Absent: stress-ng, Qt apps. `proto=XWayland` confirmed for xterm and gnome-text-editor (X11-forced) in the extension log; Chrome needs class `xflash-chrome` | this file, `scratch/xflash02/` |
| 1 | Pixel ground truth | done, negative: Shell Screencast D-Bus service, 3 attempts (default, 30 fps, custom jpeg pipeline); each produced one frame and then `pipewire remote error: no more input formats`. Nothing else safe was found; finish without pixels | `scratch/xflash02/cap/` (not committed; page to write) |
| 2 | X-side observer | done and tested on xterm; capped (400 damage lines/s, 40 MB) | `tools/src/obs.rs` |
| 3 | Clock correlation | done: `mono_ms` in the extension's `X11_OBS_*` lines gives the origin directly; per-window spread 0.56 ms idle, 2.4 ms loaded; `SetLogging` brackets cross-check inside 4.3 ms (idle) and 18 ms (loaded) intervals | `tools/src/analyze.rs`; summary in `summary.md` of each batch |
| 4 | Measurement matrix | partly: xterm done (idle and loaded, 40 runs each). gnome-text-editor not measured (crashed on run 0). eog, Chrome, Firefox not started. No native-Wayland controls yet | batches below |
| 5 | Per-app diagnosis and offline "client settled" rules R1, R2 | partly: implemented in `analyze.rs`; xterm numbers computed (see below) | `scratch/xflash02/b_xterm_*/summary.md` |
| 6 | Docs pages on the site | not started. Next free numbers: 6.38 onward (6.36 and 6.37 exist in `docs/TEST_LOG.md` without a page; the site pages run to 6.35). Templates: `experiments/6-35-ld-preload-geometry-shim.md`, `6-29-pixel-probe-retry-shell-crash.md`, rules in `contributing/writing-docs.md` | `docs-site/src/content/docs/experiments/` |
| 7 | Gated push | not started; waits for the maintainer | |

Definitions (task 4). V: the client window was mapped on the X side before the extension's first
cloak (WINDOW_CREATED). R: a client ConfigureNotify that changes the size after UNCLOAK, with its
delay. D: DamageNotify events after UNCLOAK plus one frame (16.7 ms) that belong to the damage burst
following the last resize (gap at most 50 ms). A: ANCHOR_CORRECTION lines after UNCLOAK. L: time from
UNCLOAK to the end of that burst (and to the last size change). P: frames with the window visible at
the wrong size or position; not measurable without pixels. Rules: interleave sizes and modes in
random order within a batch (seeded); idle and loaded are separate batches in the same login; 20
runs per app and size and condition; 10 native Wayland control runs per app, idle, in the idle
batch; sizes above each app's real minimum. Run batches with `tools/run-batch.sh` (3 GB memory scope).

## Batches (directories in `scratch/xflash02/`, git-ignored)

| Dir | App | Cond | Runs ok | Note |
| :-- | :-- | :-- | --: | :-- |
| `b_xterm_idle` | xterm | idle | 40 of 40 | clean, Shell unchanged |
| `b_xterm_loaded` | xterm | loaded (22 burn threads, 30 s warm-up) | 40 of 40 | clean |
| `b_editor_idle` | gnome-text-editor | idle, +10 controls planned | 1 | Shell crashed during the close of run 0 |
| `p_xterm` | xterm | probe | 2 | smoke test |

xterm results (from `analyze`, `R1` = reveal at the first damage after the first size change plus
one frame): idle large/small: V 0, R 0, A 0, D 0 and 1 of 20, cloak median 47 ms; loaded large: R 4,
D 6 of 20; loaded small: R 6, D 6 of 20. Native Wayland controls: none yet.

## Crash files

- `crash-2026-10-29.md` (12:32:34, gnome-text-editor X11-forced, focused close) with the
  `crash-2026-10-29/` records.
- `crash-2026-10-29-second.md` (13:06:53, same app, same assertion, first run of the batch).
- `isolation-feasibility.md`: private headless Shell design and the bisect plan.

## Suspect list (status after crash 2)

| H | Suspect | Status |
| :-- | :-- | :-- |
| H1 | gnome-text-editor specifically | supported, not exclusive |
| H2 | any GTK app forced onto X11 | supported (new lead: spawn-at sets `DESKTOP_STARTUP_ID` with a 13-digit `_TIME` the X protocol cannot hold, which may produce the timestamp-0 warning; xterm ignores it) |
| H3 | the observer | contradicted as sole cause (80 clean xterm runs under it) |
| H4 | the extension's legacy path | contradicted as the trigger (idle at both aborts) |
| H5 | Shell already degraded | not decided |
| H6 | closing a focused XWayland window | supported as the proximate trigger |

## App order

xterm (done), gnome-text-editor, eog, then others from the list (gnome-calculator, nautilus,
gnome-terminal with the server caveat, Chrome), Firefox last, each in its own batch and idle before
loaded. Do not start gnome-text-editor or any forced GTK app on the live session before the bisect.

## Next steps

1. Maintainer decides on the isolated session (`isolation-feasibility.md`, cells 6, 1, 2, 3).
2. If safe, run the bisect, then the remaining batches inside the isolated session (tool changes
   listed in `isolation-feasibility.md` section 5).
3. Write the pages (task 6), run `cargo xtask docs-site`, check gates G1 to G6, report, wait.
