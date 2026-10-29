# Bug: the GNOME Shell aborts when a focused XWayland GTK4 window launched by `spawn-at` closes

Status: **confirmed in a private headless Shell, mechanism measured, candidate fix written, not
applied**. Tags: MEASURED (this work, raw data under `docs/evidence/x11-flash-02/data/ding/`),
INFERRED, HYPOTHESIS. Full analysis: `ding-crash.md`.

## Severity

**Release-blocking for the affected configuration**; not a data-loss bug. A default Ubuntu 24.04
desktop has Desktop Icons NG (`ding@rastersoft.com`) enabled. With it, closing a focused GTK4 window
that `spawn-at` started **on XWayland** (or any X11 client that reads a `_TIME` from
`DESKTOP_STARTUP_ID`/`XDG_ACTIVATION_TOKEN` and requests activation with it) aborts `gnome-shell`:

```
libmutter:ERROR:../src/core/window.c:1518:meta_window_unmanage: assertion failed: (window->display->focus_window != window)
Bail out! ...
```

The session loses every window of the Shell (the Shell process dies; the session usually restarts).
Frequency in the private Shell: 11 crashes in 11 attempts with that configuration; 0 clean runs. In
the maintainer's live session it happened twice, both with `gnome-text-editor` forced to X11 and
closed while focused.

## Affected

- spawn-at 0.2.4 (and every version since `ccc84dd`, 2026-10-08, which added the `_TIME<ms>`
  suffix), any backend that sets `DESKTOP_STARTUP_ID`/`XDG_ACTIVATION_TOKEN`: the GNOME driver and
  the X11 driver.
- GNOME Shell 46.0 / mutter 46.2 (Ubuntu 24.04, `46.2-1ubuntu0.24.04.18`). Mutter's current `main`
  still has the assertion and the same timestamp check, so it is not fixed upstream.
- Desktop Icons NG 47.0.9 (`46+really47.0.9-1ubuntu5`, Ubuntu's default). DING is necessary: it
  supplies the focus candidate (its desktop window) and, in a fresh Shell, the focused window that
  makes a new window focused. Without it the tested configuration did not crash.
- The app must be an X11/XWayland client whose toolkit turns the `_TIME` suffix into
  `_NET_WM_USER_TIME` **and** requests activation with it. Measured: `gnome-text-editor` (GTK4)
  crashes; `gnome-calculator` (GTK4) does not (it never sends the activation with that time);
  eog/gnome-terminal (GTK3), Chrome, Qt 5, Java Swing do not.

## Reproduction (private headless Shell only; never the live session)

Everything below runs in the private Shell started by `tools/psh-start.sh`; the live session is
untouched. `tools/ding-cell.sh` does all of it for one cell (fresh DING-only Shell, 5 runs, stop at
the first crash, live Shell pid checked before and after).

```
# 1. a fresh DING-only private headless Shell (own bus, Wayland socket, Xwayland, keyfile)
H7=1 H7_SKIP="<every live extension except ding@rastersoft.com>" tools/psh-start.sh
SB=$(cat scratch/xflash02/priv/CURRENT)

# 2. one run of the crashing configuration (the tool starts the Shell for you)
tools/ding-cell.sh repro docs/evidence/x11-flash-02/data/ding/repro --app gnome-text-editor --runs 1

# 3. what it does, by hand, inside the same Shell
tools/psh spawn-at spawn --no-focus --anchor bottom-right --size 640 420 -m 24 \
  --json -- env GDK_BACKEND=x11 gnome-text-editor --new-window
# wait until the window is focused (`spawn-at query windows --json`), then:
tools/psh spawn-at close --id <window id>
```

The Shell aborts within a second of the close. The same run with the two variables unset
(`spawn-at spawn -- env -u DESKTOP_STARTUP_ID -u XDG_ACTIVATION_TOKEN <app>`) is clean, and so is
the same app on native Wayland.

## Expected / actual

Expected: the window closes, the Shell stays up, focus moves to the next window.
Actual: Mutter cannot hand focus over (`Failed to focus window W0 while avoiding 0x400004`), the
`meta_window_unmanage` assertion fires, and the Shell aborts.

## Root cause (measured)

1. spawn-at builds the startup id `spawn-at-<pid>-<epoch_ms>_TIME<epoch_ms>` (13-digit epoch
   milliseconds) and sets it as both variables for the child.
2. GDK reads the part after `_TIME` as an X11 server timestamp, truncates it to 32 bits
   (`1793266852988 mod 2^32 = 2265490556`), writes it into the window's `_NET_WM_USER_TIME` and,
   when the app requests activation, sends `_NET_ACTIVE_WINDOW` with it.
3. Mutter's Wayland clock is the monotonic clock (milliseconds since boot, about 820,688,193 at
   the time of the run). The truncated value is 16.7 days **in the future** in that clock. Mutter
   has no sanity check against a future user time (its own FIXME in
   `meta_display_timestamp_too_old` says there should be one) and stores it as `last_user_time`
   and `last_focus_time`.
4. On the close, Mutter tries to focus the next window with the current time (about 820,690,000).
   `meta_display_timestamp_too_old` sees a time before both stored future times and refuses the
   focus change. `try_to_set_focus_and_check` warns; the fallback
   `meta_display_unset_input_focus` is refused by the same check; the assertion aborts the Shell.

The last step is an unfixed mutter bug: the fallback added by mutter `e4d8e4b9f7d` (2020, issue
#862) cannot work when the reason for the failure is the timestamp check.

## Workaround (until a fix ships)

- Launch X11 apps with the variables unset; in the private Shell this still matched the window (5 of
  5 claimed): `spawn-at spawn -- env -u DESKTOP_STARTUP_ID -u XDG_ACTIVATION_TOKEN <app>`. This is
  the same effect the candidate patch has.
- Avoid starting forced-X11 GTK4 apps through `spawn-at` on a DING desktop.
- Disabling DING removes the crash and the desktop icons; a lose-lose change, so last resort.

## Candidate fix

`proposed-fix-activation-env.patch`: drop the `_TIME<epoch_ms>` suffix from the startup id (keep the
unique `spawn-at-<pid>-<ms>`). Matching is unchanged (the id is still unique and still the value of
both variables), GTK then activates with CurrentTime, and the Shell survives: 5 of 5 clean runs of
the crashing cell with the patched binary, plus 15 matching/regression runs and the crate's 521
tests. A second fix is needed in mutter (reject a future user time; never refuse to unset focus);
the draft is in `upstream-report-draft.md`.

## Evidence

`ding-crash.md` (mechanism, factor table, exposure map, mitigation ranking, what is not
established), `data/ding/focus/` (the crash and the clean debug runs with the full focus log),
`data/ding/factor/`, `data/ding/matrix-x11/`, `data/ding/matrix-wl/`, `data/ding/mitigation/`.
