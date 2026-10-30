# Every run

One row per run that has a folder in `docs/evidence/`, plus the two pull requests that fixed the
startup id and added the texture gate. The evidence for `x11-flash-02` to `x11-flash-05` is not on
`main`; read it on the branch named in the row with `git show <branch>:docs/evidence/<run-name>/`.

| Run | Date | Question | Result | Branch | Website page |
| :-- | :-- | :-- | :-- | :-- | :-- |
| `stage1` | 2026-10-11 | Did the stage-1 tooling refactor keep behaviour? | 16 hashes, help and list output unchanged; test names gained module prefixes after the move. | `main` (`0124060`) | not on the site yet |
| `x11-flash-02` | 2026-10-29 | Why do XWayland and X11 windows flash, and what aborts GNOME Shell on close? | Two focused-window Shell crashes were reproduced in a private Shell; the first explanation (DING plus a future `_TIME`) was incomplete, and run 03b later measured that DING is not required (2 of 2 old-id aborts without it, 50 of 50 closes clean with the fixed id): the long `_TIME` value in the startup id is the cause, fixed in PR #94. | `exp/x11-flash-02` | not on the site yet |
| `x11-flash-03` | 2026-10-29 | Is a texture-size gate trustworthy for every toolkit? | Probe trustworthy; `_TIME0` closed 50 of 50 focused windows; the gate removes GTK stale frames, not Qt or Chrome blank frames. | `exp/x11-flash-03` | not on the site yet |
| `x11-flash-04` | 2026-10-30 | Does the texture gate remove the X11 flash without breaking placement? | GTK bad frames fell from 16, 17 and 11 of 20 to 0, 0 and 0; one editor run was still bad; placement unchanged. | `exp/x11-flash-03` (evidence); product on `fix/x11-texture-gate` | not on the site yet |
| `x11-flash-05` | 2026-10-30 | What does the texture gate leave unfixed? | Blank frames remain (5 of 100 editor runs); the calculator size is a map race; the sync counter is dead; native Wayland 0 bad of 130. | `exp/x11-flash-05-ds` | not on the site yet |
| [PR #94](https://github.com/harshp2008/spawn-at/pull/94) | 2026-10-29 | Does an oversized startup-id time crash the Shell when a focused X11 window closes? | With DING, 10 of 10 crash cells aborted; `_TIME0` gave 0 of 10 and matching still worked. | `fix/startup-id-time` | not on the site yet |
| [PR #95](https://github.com/harshp2008/spawn-at/pull/95) | 2026-10-29 | Does holding the reveal until the texture matches remove the flash? | GTK stale-size flash gone; blank frames and the 250 ms cap tail remain; placement unchanged. | `fix/x11-texture-gate` | not checked |

`stage1` holds before and after snapshots of the tooling refactor, not an experiment. Its README and
findings are the snapshot files themselves. `x11-flash-02` has no `README.md` or `findings.md`; start
at its `PLAN.md`, then `matrix-notes.md` and `ding-crash.md`.

The draft pages 6.38 to 6.41 are on `exp/x11-flash-02`. They are not on `main` and not published.
No website page was found for PR #95.
