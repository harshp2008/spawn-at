# Second GNOME Shell crash, 2026-10-29 13:06 (X11-FLASH-02e2, step 1)

Tags: MEASURED (read from the journal or a file named here), INFERRED (follows from measured
facts), HYPOTHESIS (possible, untested), SECONDHAND (from the web, not checked against this
machine). Times are local (+05:30). Nothing was launched to write this. `/var/crash` was listed
(names, sizes, times) and nothing in it was opened.

## 1. What happened

MEASURED. The batch `b_editor_idle` (`--app gnome-text-editor`, 20 reps per size, 10 native
Wayland controls, seed 21) started at 13:06:47 (`progress.jsonl`, `start`, 50 runs planned). Its
first run, `0:gnome-text-editor:x11:small` (X11-forced), was the only one that started.

MEASURED. Order of events (journal, trimmed; `win` = Shell window id 1187485931):

```
13:06:51.171 CLOAK / SET_SIZE_REQUEST 640x420 / COMMIT_RESOLVED reason=AT_TARGET / POSITION_SET (1256,636)
13:06:51.178 UNCLOAK, UNCLOAK_4D_REVEAL, ANCHOR_ARMED      (extension finished; window revealed)
13:06:51.147 gnome-shell: Buggy client sent a _NET_ACTIVE_WINDOW message with a timestamp of 0 for 0x1000004
13:06:51.679 X11_OBS_END reason=reveal+500ms damaged=15 first_frame=1 thawed=29
13:06:53.708 gnome-shell: Tried to ping window 0x1000004 with a bad serial! Not allowed.
13:06:53.708 ANCHOR_RELEASED reason=EXTERNAL_CLOSE       (the driver's graceful close, 2.5 s after the reveal)
13:06:53.714 gnome-shell: Failed to focus window W6 while avoiding 0x1000004
13:06:53.714 libmutter:ERROR:../src/core/window.c:1518:meta_window_unmanage: assertion failed: (window->display->focus_window != window)
13:06:53.714 GNOME Shell crashed with signal 6
```

- Same assertion as crash 1 (MEASURED: identical text and source line). Differences: the fallback
  focus target was `W6` (crash 1: `W1043`), the X window id `0x1000004` (crash 1: `0x1200004`).
- Shell pid before 2688147, after 2717694 (the login screen's Shell; the user session came back
  as 2718897 at about 13:15). Same boot (`boot_id` unchanged in `progress.jsonl`).
- The Shell logged no other warning of these kinds in the 5 minutes before, except `bad serial`
  lines from the earlier xterm batch (see 5).

## 2. Which step of the run (b)

MEASURED. `runs.jsonl` has one record: run 0, `win_pid` 2717278, `closed: true`,
`focused_at_close: true`, `killed: ""`. The progress file has `start`, `run` (status ok, 13:06:54),
then two `crash` events (13:06:58 and 13:07:01) written by the shell watch. So run 0 finished its
spawn, settle and close; the Shell died during the close; the driver saw "window gone" because the
session was torn down, wrote the run, and the watch caught the changed Shell pid before run 1 (run
1 never started). The in-flight step was the close, 6 ms after the extension logged
`EXTERNAL_CLOSE`.

MEASURED. `focused_at_close` was true although the window was spawned with `--no-focus`: the app,
or Mutter, gave it focus after the map. So `--no-focus` does not keep this window unfocused.

## 3. Was the extension mid-operation (c)

MEASURED. No. The batch steps had ended at 13:06:51.178 (`ANCHOR_ARMED`); the observer mark
`X11_OBS_END` followed at 13:06:51.679; the next line is the close at 13:06:53.708. No cloak was
active, no SetSize, no move_frame. The only extension activity at the crash was the anchor
release logged for the external close. INFERRED: the extension did not cause the abort, though it
is loaded in the Shell.

## 4. Crash 1 against crash 2 (d)

| | Crash 1 (12:32:34) | Crash 2 (13:06:53) |
| :-- | :-- | :-- |
| App in flight | gnome-text-editor, X11-forced (run 2 of the probe batch, after two Chrome runs) | gnome-text-editor, X11-forced (run 0 of `b_editor_idle`) |
| Mode, window kind | X11-forced, XWayland | X11-forced, XWayland |
| Step | graceful close (`spawn-at close --id`) | graceful close |
| Focus of the closing window | not recorded by that driver version; the window was spawned focused | `focused_at_close: true` (MEASURED) |
| Assertion | `meta_window_unmanage ... focus_window != window` | identical |
| Preceding warnings | `Buggy client ... timestamp of 0` (0x1200004), `bad serial`, `Failed to focus window W1043` | same three kinds (0x1000004, `W6`) |
| Time since batch start | about 21 s | about 6.5 s |
| Time since boot | Oct 20 03:12 plus 9 days | same boot |
| Observer attached | yes | yes |

MEASURED: the probe batch records of crash 1 are kept in `crash-2026-10-29/runs.jsonl`.

## 5. Warning counts per 10 minutes (e)

MEASURED (journal, Shell lines only, 11:30 to 13:30; buckets with no hits are omitted):

| Bucket | geometry | size accounting | stack_position | inf scale | Buggy _NET_ACTIVE_WINDOW | bad serial | Failed to focus |
| :-- | --: | --: | --: | --: | --: | --: | --: |
| 12:10 | 1 | 10 | 6 | 2 | 0 | 0 | 0 |
| 12:20 | 0 | 0 | 0 | 0 | 0 | 2 | 0 |
| 12:30 | 0 | 2 | 8 | 0 | 1 | 3 | 1 |
| 12:40 | 0 | 2 | 2 | 0 | 0 | 0 | 0 |
| 12:50 (xterm idle) | 0 | 0 | 0 | 0 | 0 | 42 | 0 |
| 13:00 (xterm loaded, crash 2) | 0 | 0 | 0 | 0 | 1 | 41 | 1 |
| 13:10 (after the restart) | 0 | 2 | 5 | 0 | 0 | 0 | 0 |

- The geometry, size-accounting, stack_position and `inf` lines were present at 12:10, before any
  xflash run and before the first live test of this work at about 12:24 (MEASURED).
- They did not rise during the two xterm batches (12:50 and 13:00 rows are zero) (MEASURED).
- `bad serial` appears about once per closed window in both xterm batches (80 windows, 83 lines)
  (MEASURED); it is not specific to the editor.
- `Buggy ... timestamp of 0` appears only with the editor: once in each crash, never in the 80
  xterm runs (MEASURED). `Failed to focus` appears only at the two crashes (MEASURED).
- The 13:10 bucket's lines are from the session that started at about 13:15 (new Shell 2718897).

## 6. /var/crash metadata (f)

MEASURED (`ls -la`): `_usr_bin_gnome-shell.1000.crash` 102915230 bytes, 12:32 (crash 1). No file
for the 13:06 crash was present when listed (whoopsie may not have written one yet, or it is
rate-limited; not known). Older entries: Chrome (Oct 23), blueman-applet (Oct 26), gnome-software
(Oct 29 01:50), antigravity-ide (12:32).

## 7. Upstream (g)

SECONDHAND (web search, not verified on this machine; no fix version confirmed).
- The assertion is a known class: Mutter aborts in `meta_window_unmanage` when the window being
  destroyed is still `focus_window` because focusing another window failed or never happened. Found
  threads: mutter issues 45, 462, 660, 1029, 1066, merge requests 1643 ("core: Make sure
  focus_default_window() worked"), 2704, 3376, and Launchpad bug 1870822.
- The warning "Buggy client sent a _NET_ACTIVE_WINDOW message with a timestamp of 0" has a GTK issue
  (gitlab.gnome.org/GNOME/gtk issue 6046, "[X11] Buggy client sent a _NET_ACTIVE_WINDOW message",
  reported on mutter 43.1 at app launch) and SDL/Eclipse reports; the cause named there is the
  client sending a zero timestamp.
- Not found: a report that ties the two together on Mutter 46.2, or a release that fixes it.
- Local, MEASURED: the Ubuntu changelog of `46.2-1ubuntu0.24.04.18` (`/usr/share/doc/...`) has no
  entry about `meta_window_unmanage` or `_NET_ACTIVE_WINDOW`; its focus-related lines are older
  patches.

## 8. Observer audit (h)

MEASURED (source `tools/src/obs.rs`, not run). Requests made:
- Root: `ChangeWindowAttributes` event mask `SUBSTRUCTURE_NOTIFY` only (`:155-157`); `QueryTree`
  (`:164`).
- Every window seen (at start, on `CreateNotify`, on `ReparentNotify`, including Mutter's own
  `mutter-x11-frames` and guard windows, depth 1): `ChangeWindowAttributes` with `STRUCTURE_NOTIFY |
  PROPERTY_CHANGE | SUBSTRUCTURE_NOTIFY` (`:241-243`), `DamageCreate` RawRectangles (`:245`),
  `GetWindowAttributes`, `GetGeometry` (`:250-251`), `QueryTree` (`:272`), `GetProperty` (`:298`,
  about 9 reads per `props` record), `GetAtomName` (`:224`), `InternAtom` (`:107`).
- Per damage event: `DamageSubtract` on its own damage object (`:499`).
- Never: `SetInputFocus`, `SendEvent`/`ClientMessage`, `ConfigureWindow`, `MapWindow`,
  `ChangeProperty`, `SetSelectionOwner`, `GrabServer` or `GrabKeyboard`/`GrabButton`. A grep of
  `obs.rs` for these names finds none; `run.rs` has no X11 code (grep `x11rb`: 0). So it cannot
  send `_NET_ACTIVE_WINDOW` or a focus request, and it takes no server grab.
- Races: `GetProperty`/`GetGeometry`/`QueryTree` on a window destroyed a moment later return an X
  error that the code ignores (`let _ =` and `.ok()`); `DamageCreate` on a window that is destroyed
  later is dropped by the server with the window. INFERRED: harmless to Mutter, but each event
  selection on Mutter's own windows is extra traffic Mutter does not expect from a client.
- Volume (MEASURED, xobs files): xterm idle batch 21082 lines for 40 runs (about 527 per run, 2232
  `CreateNotify`, 8664 `DamageNotify`, 120 `MapNotify`); the editor batch 431 lines for its single
  run (49 `CreateNotify`, 80 `DamageNotify`, 1 `MapNotify`). The observer ran identically in both
  and 80 xterm runs under it did not crash. INFERRED: volume does not distinguish the failing
  case.

## 9. X11-forcing audit (i)

MEASURED (source `tools/src/run.rs` app table and `src/platform/linux/gnome/driver.rs`):

| App | How it is on X11 | Natively |
| :-- | :-- | :-- |
| xterm | nothing (X11 only) | X11 |
| gnome-text-editor | `GDK_BACKEND=x11`, `GSETTINGS_BACKEND=memory` | Wayland (GTK4) |
| eog | same | Wayland (GTK3) |
| Chrome | `--ozone-platform=x11`, `--class=xflash-chrome`, fresh `--user-data-dir` | Wayland/X11 |
| Firefox | `MOZ_ENABLE_WAYLAND=0`, `GDK_BACKEND=x11`, fresh profile | Wayland |

- The arm step sets `XDG_ACTIVATION_TOKEN` and `DESKTOP_STARTUP_ID` to the entry key for every
  program it launches (`driver.rs:317-318`). The key is `spawn-at-<pid>-<ms since epoch>_TIME<same
  ms>` (`src/commands/spawn.rs:283`). MEASURED: the `_TIME` value is a 13-digit millisecond count
  (about 1.79e12), which does not fit the 32-bit X timestamp.
- HYPOTHESIS (new, testable): a GTK program on X11 reads `DESKTOP_STARTUP_ID`, takes the number
  after `_TIME` as its startup timestamp, and cannot represent it, so it presents its window
  with timestamp 0; Mutter logs "Buggy client ... timestamp of 0" and mishandles focus on that
  window. xterm does not read the variable, which fits: the warning never appears in the 80 xterm
  runs and appears in both editor crashes. Not tested; it is a spawn-at behaviour that every
  GTK-on-X11 spawn would share, and a native Wayland GTK app uses `XDG_ACTIVATION_TOKEN`, not the
  startup id.
- Extra windows: the observer file shows the editor's X windows (49 `CreateNotify` in the one run,
  mostly Mutter's and the app's helper windows); nothing else is known about GTK4 popover or shadow
  windows.

## 10. Verdicts (j)

| H | Verdict | Evidence | Cheapest splitting experiment (not run) |
| :-- | :-- | :-- | :-- |
| H1 gnome-text-editor specifically | Supported, not exclusive | both crashes were the editor; 80 xterm closes (focused or not) never crashed | eog X11-forced in the same way: crash means H2, no crash means H1 |
| H2 GTK app forced to X11 | Supported | the only apps seen with the timestamp-0 warning; both crashes are GTK4-on-X11; untested for eog and for GTK on native Wayland | editor on native Wayland with the same close (crash 0 times expected) vs forced X11 |
| H3 the observer | Contradicted as the sole cause; not decided as a co-factor | 80 xterm runs under it were clean; both crashes had it attached | editor X11-forced with and without the observer |
| H4 our extension's legacy path | Contradicted as the trigger | the extension was idle at both aborts (section 3); not excluded as a co-factor (it cloaks, resizes, moves the window before) | editor X11-forced launched plainly, no spawn-at |
| H5 Shell already degraded | Not decided | the odd warnings appeared at 12:10 with no xflash activity and again in the Shell that started at 12:33 (12:40 and 13:10 rows), so something other than our batches produces them; yet 80 xterm closes in that Shell were clean | run the editor close in a fresh private Shell |
| H6 closing a focused XWayland window | Supported as the proximate trigger | both aborts 6 ms after the close of a focused window; `Failed to focus window W` immediately before | close the focused editor window with its own quit key vs `spawn-at close`; and an xterm focused close (done 80 times, clean) |

Notes: H1, H2 and H6 are the same event seen from three sides: a focused GTK4 X11 window whose
unmanage triggers a failed focus hand-over. The new startup-id hypothesis (section 9) would explain
why only the editor shows the timestamp-0 message.
