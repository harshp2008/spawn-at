# Every run

One row per run that has a folder in `docs/evidence/`, plus the two pull requests that fixed the
startup id and added the texture gate. The evidence for `x11-flash-02` to `x11-flash-05` is not on
`main`; read it on the branch named in the row with `git show <branch>:docs/evidence/<run-name>/`.

| Run | Date | Question | Result | Branch | Website page |
| :-- | :-- | :-- | :-- | :-- | :-- |
| `stage1` | 2026-10-11 | Did the stage-1 tooling refactor keep behaviour? | 16 hashes unchanged; help output differs only in build time; list output unchanged; test names not checked. | `main` (`0124060`) | not on the site yet |
| `x11-flash-02` | 2026-10-29 | Why do XWayland and X11 windows flash, and what aborts GNOME Shell on close? | Two focused-window Shell crashes were reproduced in a private Shell; the first explanation (DING plus a future `_TIME`) was incomplete, and run 03b later measured that DING is not required (2 of 2 old-id aborts without it, 50 of 50 closes clean with the fixed id): the long `_TIME` value in the startup id is the cause, fixed in PR #94. | docs/evidence/x11-flash-02/ on main; full data on exp/x11-flash-02 | not on the site yet |
| `x11-flash-03` | 2026-10-29 | Is a texture-size gate trustworthy for every toolkit? | Probe checked against controls: 20 of 20 positive-control runs bad and 0 of 80 native Wayland runs bad. Toolkit rates varied: eog 39 of 70 at 1100 px, editor 6 of 10 at 640x420, and black frames at the final size in Qt 16 of 20 and Chrome 19 of 20. `_TIME0` closed 50 of 50 focused windows with no aborts. | docs/evidence/x11-flash-03/ on main; full data on exp/x11-flash-03 | not on the site yet |
| `x11-flash-04` | 2026-10-30 | Does the texture gate remove the X11 flash without breaking placement? | Old-size frames fell for eog 600x420 0 to 0 of 20, 1100x760 16 to 0 and 1700x1000 17 to 0; editor 640x420 11 to 0 and 1100x760 10 to 1; calculator 500x400 18 to 0 and 900x700 20 to 0. The gate does not see blank frames at the right size, and Chrome matched it in 20 of 20 runs. Placement was unchanged. | docs/evidence/x11-flash-04/ on main; full data on exp/x11-flash-03 | not on the site yet |
| `x11-flash-05` | 2026-10-30 | What does the texture gate leave unfixed? | Blank frames at the right size remain (5 of 100 editor runs), and the calculator's wrong final size was seen in the test sandbox, which had no saved window size; with a saved size 0 of 30 runs were wrong; the live rate was not measured. Native Wayland had 0 bad of 130. The sync counter does not move for spawn-at's programmatic resize on Mutter 46 (05 findings, Task C), though 6.33 showed the extended counter does move in 100 of 100 runs. | docs/evidence/x11-flash-05/ on main; full data on exp/x11-flash-05-ds | not on the site yet |
| [PR #94](https://github.com/harshp2008/spawn-at/pull/94) | 2026-10-29 | Does an oversized startup-id time crash the Shell when a focused X11 window closes? | With DING loaded, 10 of 10 crash cells aborted; `_TIME0` gave 0 of 10 and matching still worked. Run 03b later showed DING is not required. | `fix/startup-id-time` | not on the site yet |
| [PR #95](https://github.com/harshp2008/spawn-at/pull/95) | 2026-10-29 | Does holding the reveal until the texture matches remove the flash? | GTK stale-size flash gone; blank frames and the 250 ms cap tail remain; placement unchanged. | `fix/x11-texture-gate` | not on the site yet |

`stage1` holds before and after snapshots of the tooling refactor, not an experiment. Its README and
findings are the snapshot files themselves. `x11-flash-02` has no `README.md` or `findings.md`; start
at its `PLAN.md`, then `matrix-notes.md` and `ding-crash.md`.

The draft pages 6.38 to 6.41 are on `exp/x11-flash-02`. They are not on `main` and not published.
No website page was found for PR #95.

## Corrections

- **2026-10-31.** The `x11-flash-02` row said "DING plus a future `_TIME` from the startup id is the crash mechanism". That was wrong: DING is not required; the long `_TIME` value is the cause, fixed in PR #94. Evidence: `docs/evidence/x11-flash-03/findings.md` (03b) on `exp/x11-flash-03`.
- **2026-10-31.** The `x11-flash-03`, `x11-flash-04`, `x11-flash-05`, PR #94 and `stage1` rows were reworded to match what each run measured: the gate result moved from 03 to 04, the 04 counts now name each app and size, the 05 sync-counter claim was narrowed, PR #94 now says DING was loaded, and the unverified stage1 test-name claim was removed.
- **2026-10-31.** The `x11-flash-05` row said the calculator's wrong final size was "a map race, sandbox only". That was too strong: it was seen in the test sandbox, which had no saved window size; with a saved size 0 of 30 runs were wrong, and the live rate was not measured. Evidence: `docs/evidence/x11-flash-05/findings.md` (Task B) on `exp/x11-flash-05-ds`.
- **2026-10-31.** Evidence for runs `x11-flash-02` to `x11-flash-05` was copied onto `main` unchanged: the notes, summaries, README/PLAN/checkpoint files and small data files now sit under `docs/evidence/<run>/`. Images, compressed captures, raw data over 100 KB and tool code stayed on the branch named in each row; each run folder's `FILES.md` lists every file that stayed and the commit to fetch it from.

