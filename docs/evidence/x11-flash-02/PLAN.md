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
- Gates for a push: G1 diff vs main touches only `docs/`, `docs-site/` and `docs/evidence/x11-flash-02/`; Cargo.toml and Cargo.lock are allowed ONLY under `docs/evidence/x11-flash-02/tools/`, and only while that project is standalone (empty `[workspace]`) and nothing picks it up (checked 2026-10-29: no workflow, xtask, test, script, dependabot config or installer list refers to it; root `cargo metadata` lists the same 4 packages as clean main; `target/` is gitignored);
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

## Update after X11-FLASH-02f (private headless Shell)

- A private headless Shell works (`tools/psh-start.sh`, `psh`, `psh-batch.sh`, `psh-stop.sh`, `bisect-cells.sh`,
  `xflash cell`, `sysd1stub`); see `bisect-2026-10-29.md` section 1. Start it, use it only through
  `psh`, stop it with `psh-stop.sh`.
- The bisect (69 runs, 4 rounds) did not reproduce the crash or the timestamp-0 warning, so the
  trigger is not in the app, spawn-at, the observer or the startup id alone. The live xterm closes
  were focused too (80 of 80), so a focused close is not sufficient.
- No batch of the measurement plan has run on the live session since the second crash. Remaining
  batches (editor, eog, others) can run in the private Shell with `psh-batch.sh`, which is safe for
  the desktop but whose timing differs from the live compositor (headless, no real input, a
  single virtual monitor). State that on every page that uses those numbers.

## Update after X11-FLASH-02g2

- Site pages 6.38 to 6.41 are written and committed. They are NOT published: publishing goes through a pull request from `exp/x11-flash-02` (the repo rules forbid pushing main). The branch is backed up on origin only (hash in the report of that prompt; see `git ls-remote --heads origin exp/x11-flash-02`). No workflow runs on a push of this branch (ci and docs run on pull requests and pushes to main; release on tags).
- Pages still owed (numbers 6.42 and up): tool inventory and environment; pixel-capture feasibility (negative); the X-side observer and clock correlation; the xterm measurement (idle and loaded, 80 runs, data under `data/xterm-live/`); the offline evaluation of the "client settled" rule; plus the remaining app batches if they are run (in the private Shell, stated on each page).
- The Known shortcomings entry on the crashes was softened: observed, established, not established, under investigation, and a precaution only.

## Update after X11-FLASH-02h (H7)

- Step 1 done: `live-inventory.md` (22 extensions enabled live, allowlisted settings, warning counts).
- Step 2 done, and it stopped the prompt: the live extension set in the private Shell reproduces the
  crash (same assertion). Bisect by halves names **Desktop Icons NG (ding@rastersoft.com)**: it alone,
  with the spawn-at launch path, crashes at run 1; all other extensions without it do not (5 runs);
  see `h7-replica.md`, raw data `data/h7/`. Plain launches (no spawn-at command) do not crash with
  DING but show the timestamp-0 warning. Removing both `DESKTOP_STARTUP_ID` and `XDG_ACTIVATION_TOKEN`
  removed the crash; a 32-bit-safe `_TIME` did not. The startup-id idea in its original form
  (oversized `_TIME`) is contradicted.
- Steps 3 to 6 (xterm calibration, the matrix, rule evaluation, private pixel capture) NOT run: the
  prompt says to stop at a reproduced crash and wait for the maintainer.
- Tools added: `H7=1` / `H7_SKIP` in `psh-start.sh`, `--unset`, `--setenv`, `--bg-window`, `--focus`
  cell options, `CELL`/`RUNS` in the scratch `h7try.sh` (not committed).
- H table: H1 editor: supported only together with DING; H2 any GTK app on X11: eog and xterm clean
  with DING, so not general; H3 observer: contradicted (cell 8 crashes with it, c5 without);
  H4 extension path: spawn-at launch path required (plain launches clean) but cause unknown;
  H5 degraded live Shell: not needed (a fresh private Shell crashes); H6 focused close: not
  sufficient; H7 another extension: **supported, DING named**.
- Site pages still owed (numbers 6.42 and up): live inventory and H7 replica (the key one); the
  earlier-listed inventory, pixel-capture failure, observer and clock correlation; the xterm
  measurement; the offline rule evaluation; per-app matrices once the batches run. The existing
  6.38 (hypothesis table), 6.40 (startup-id) and 6.41 (bisect) and the Known shortcomings entry
  need an update that points at DING; not done in this prompt (no site pages here).

## Update after X11-FLASH-02i (mechanism, exposure map, activation-env patch)

- **H7 is final**: DING is necessary and sufficient *together with an app that turns the startup
  time into its focus time*. The mechanism is measured, not just a hypothesis: spawn-at's
  `_TIME<epoch_ms>` suffix reaches GDK as an X11 timestamp, is truncated to 32 bits
  (`1793266852988 -> 2265490556`), lands in `_NET_WM_USER_TIME` and `_NET_ACTIVE_WINDOW`, and is
  16.7 days in the future on Mutter's monotonic clock. Mutter accepts it, and at the close refuses
  the focus hand-over to DING's desktop window as "too old"; the fallback `unset_input_focus` is
  refused by the same check; `meta_window_unmanage` asserts. Full detail: `ding-crash.md`.
- The launch path is **not** needed: a plain launch with the real variables crashes (the earlier
  `h7` plain cells 17/18/19 used `_TIME3`, a past time, so they did not test it; cell 7 changed
  only one of the two variables). Size, anchor, `--no-wait`, the armed registration: irrelevant.
  With a harmless value (or no `_TIME`) the same cells are clean.
- **Exposure map** (DING loaded): `gnome-text-editor` X11 crashes (with and without
  `--new-window`); eog, gnome-calculator, gnome-terminal, Chrome, Qt, Java Swing are clean on X11;
  everything tested is clean on native Wayland. nautilus (D-Bus service window), VS Code (no
  window), xterm (pty refused to the agent) and Firefox (snap refuses the private runtime dir) are
  not measured here.
- **Candidate fix** (patch only, not applied): drop the `_TIME<ms>` suffix, keep the unique id.
  `proposed-fix-activation-env.patch`; tested in a throwaway worktree inside `psh`: the crashing
  cell 5 of 5 clean, matching still works (all claimed), GTK3 eog clean, 521 crate tests pass.
  Upstream is also at fault (no future-time sanity check; the `unset_input_focus` fallback cannot
  work); draft in `upstream-report-draft.md`, bug report in `product-bug-activation-env.md`.
- Migration note: `src/`, `crates/` and the existing tests are unchanged in the tree; only
  `docs/evidence/x11-flash-02/tools/` (the standalone measurement tool) gained the cell options and
  the Qt/Swing/editor app definitions, and `docs/evidence/x11-flash-02/` gained the new files.
- Site pages still owed (numbers 6.42 and up), revised list:
  1. **the DING crash mechanism and the factor table** (`ding-crash.md`) — the H7 follow-up page;
  2. **the exposure map** (which apps crash, which do not);
  3. **the activation-env bug, the patch and its verification** (`product-bug-activation-env.md`,
     `upstream-report-draft.md`);
  4. the live inventory and the H7 replica (already owed from 02h);
  5. the per-app matrices from 02i-b if they exist (private Shell, no DING; do not duplicate);
  6. the earlier-listed pages: tool inventory, pixel-capture failure, observer and clock
     correlation, xterm measurement, offline rule evaluation.
  Rewrites: **6.38** (hypothesis table: H2/H6/H7 and the startup-id lead corrected), **6.40**
  (startup-id audit: the `_TIME` value is the trigger, with the truncation arithmetic), **6.41**
  (bisect: why 69 runs without DING were clean, and that the plain cells were confounded), and the
  **Known shortcomings** entry (the crash is a confirmed, mechanism-known bug with a workaround and
  a candidate fix). None written in this prompt (no site pages here).
- Gate note: the branch now also touches only `docs/`; the tool project under
  `docs/evidence/x11-flash-02/tools/` is still standalone (empty `[workspace]`, not picked up).

