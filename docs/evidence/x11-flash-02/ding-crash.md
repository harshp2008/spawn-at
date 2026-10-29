# X11-FLASH-02i: the DING Shell-abort mechanism, the factor cells and the exposure map

Written 2026-10-29 on branch `exp/x11-flash-02`, after the H7 replica (`h7-replica.md`) named Desktop
Icons NG (DING, `ding@rastersoft.com`) and left the launch path and the other toolkits open. Tags:
**MEASURED** (a run in this work, raw data under `data/ding/`), **INFERRED** (from source or from a
measured difference), **HYPOTHESIS** (not tested), **SECONDHAND** (web or package metadata).
Everything here ran in a private headless GNOME Shell via `tools/psh`; the live session was only
read. Live Shell pid 2718897, start time 81373971, before and after every cell (MEASURED; no
`LIVE SHELL CHANGED` line in any runner log).

## 1. Key result

The crash is **not** about spawn-at's placement path, its wait, its cloak, or the window geometry.
It is the **`_TIME<ms>` suffix** that spawn-at puts in `DESKTOP_STARTUP_ID` and
`XDG_ACTIVATION_TOKEN`. GDK reads that suffix as an X11 server timestamp; the 13-digit
milliseconds-since-1970 value, truncated to 32 bits, is a time Mutter's monotonic clock reads as
16.7 days in the **future**. Mutter stores it as the display's user and focus time, and then refuses
the focus hand-over when the window closes as "too old", which trips the
`meta_window_unmanage` assertion and aborts the Shell. DING supplies the window Mutter tries to hand
focus to (its maximized desktop window), and — in a fresh headless Shell with no focused window — is
also what makes the new window focused at all. The same value with a past timestamp
(`_TIME123456789`), or with no `_TIME` suffix, does not crash (MEASURED, cells 8a to 8d).

## 2. Setup (MEASURED)

- `tools/ding-cell.sh LABEL OUTDIR --app APP ...`: starts a **fresh DING-only private headless
  Shell** for one cell (`H7=1` with every other live extension in `H7_SKIP`, so the enabled list is
  exactly `['spawn-at@harsh.local', 'ding@rastersoft.com']` — logged on every cell), waits until the
  extension answers on D-Bus, runs one `xflash cell`, copies the Shell log, stops the sandbox, and
  compares the live Shell pid/start time before and after.
- Binary: the repo's `target/debug/spawn-at` (sha256
  `60b873160dbf1c68b45fdb9bcf020ea42bc68b26296c7d21418cc2e336b6711a`, DEV BUILD 0.2.4), not the
  installed one, so the code under test is the branch. Recorded in each runner log.
- App: `gnome-text-editor` with `GDK_BACKEND=x11` unless stated; `spawn-at spawn --no-focus
  --anchor bottom-right --size 640 420 -m 24` unless the cell changes it; graceful
  `spawn-at close --id`.
- Every cell got its own Shell. A crash ends the cell (the cell tool stops at the first crash), the
  log lines are saved as `<label>.shell-crash.txt`, and the next cell starts a new sandbox. Eleven
  private Shells aborted in this work (all the same assertion); `data/ding/PROGRESS.md` lists each
  break.
- 5 runs per cell; a cell that crashes stops at its first crash (so a crashing cell contributes one
  run, as the prompt allows).

## 3. Mechanism (MEASURED log + INFERRED from the mutter 46.2 source)

### 3.1 What GTK does with the suffix

The startup id of the crashing run was `spawn-at-2891488-1793266852988_TIME1793266852988`
(MEASURED, `data/ding/focus/f1_debug_focus.shell.log`). GDK reads the part after `_TIME` as an X11
`Time` (32 bits, milliseconds since the X server started; GDK documents this for
`gdk_x11_display_set_startup_notification_id`, SECONDHAND). Truncated:

```
1793266852988 mod 2^32 = 2265490556
```

The Shell log then shows, with `MUTTER_DEBUG=focus,startup,ping` (MEASURED):

```
STARTUP: Window 0x400004 has _NET_WM_USER_TIME of 2265490556
STARTUP: Applying startup props to 0x400004 id "spawn-at-2891488-1793266852988_TIME1793266852988"
Window manager warning: Buggy client sent a _NET_ACTIVE_WINDOW message with a timestamp of 0 for 0x400004
FOCUS: _NET_ACTIVE_WINDOW message sent for 0x400004 at time 0 by client type 1.
STARTUP: Window 0x400004 _NET_WM_USER_TIME not updated to 820688193, because it is less than 2265490556
...
FOCUS: _NET_ACTIVE_WINDOW message sent for 0x400004 at time 2265490556 by client type 1.
FOCUS: Setting input focus to window 0x400004, input: 1 focusable: 1
```

So GTK set the window's `_NET_WM_USER_TIME` to the truncated value and sent a **second**
`_NET_ACTIVE_WINDOW`, 2 ms after the first, carrying it. Both are accepted.

### 3.2 Why Mutter accepts a time in the future

On a Wayland compositor, Mutter's current time is the monotonic clock:

```c
/* src/core/display.c, mutter 46.2 */
meta_display_get_current_time_roundtrip (MetaDisplay *display)
{
  if (meta_is_wayland_compositor ())
    /* Xwayland uses monotonic clock, so lets use it here as well */
    return (guint32) (g_get_monotonic_time () / 1000);
```

At the run, `/proc/uptime` was 820,688 s, i.e. the current time was 820,688,193 ms. The truncated
startup value 2,265,490,556 is **1,444,802,363 ms (16.7 days) ahead** of it, and the difference is
below 2^31, so the 32-bit "is before" comparison reads it as **after** now. Mutter's
`meta_window_activate_full` accepts it (it only ignores a time older than `last_user_time`), and
`meta_display_set_input_focus` stores it as `last_focus_time`. `meta_window_set_user_time` (called
from the `_NET_WM_USER_TIME` property) stores it as `display->last_user_time`. There is no sanity
check against the future; the source has the FIXME saying there should be (INFERRED from source,
MEASURED arithmetic).

### 3.3 Why the close aborts the Shell

The close (`spawn-at close` → `Meta.Window.delete`) destroys the X window. Mutter's DestroyNotify
path unmanages it with a fresh current time (820,690,xxx) and, because the window has focus, calls
`meta_workspace_focus_default_window (workspace, not_this_one=0x400004, now)`. The MRU candidate is
DING's desktop window W0. `meta_window_focus (W0, now)` → `meta_display_set_input_focus` →
`meta_display_timestamp_too_old`:

```c
else if (XSERVER_TIME_IS_BEFORE (*timestamp, display->last_focus_time))   /* 820690xxx < 2265490556 */
  {
    if (XSERVER_TIME_IS_BEFORE (*timestamp, display->last_user_time))     /* also < 2265490556 */
      return TRUE;                                                        /* ignore: "too old" */
```

The focus does not move. `try_to_set_focus_and_check` warns and returns FALSE; the fallback
`meta_display_unset_input_focus` goes through the same check and also does nothing; then
`meta_window_unmanage` reaches its assertion:

```
Window manager warning: Failed to focus window W0 while avoiding 0x400004
No MRU window to focus found; focusing no_focus_window.
libmutter:ERROR:../src/core/window.c:1518:meta_window_unmanage: assertion failed: (window->display->focus_window != window)
Bail out! ... (EE) failed to read Wayland events: Connection reset by peer
```

(MEASURED: the four lines, every crashing run; INFERRED: the bail inside
`meta_display_timestamp_too_old`, from the absence of any `* Focus --> W0` line after "Setting input
focus to window W0" and from the source.)

### 3.4 The other side: a past value does not crash

With both variables set to `xflash-harmless_TIME123456789` (MEASURED, `f4_debug_harmless_time123`):

```
STARTUP: Window 0x400004 has _NET_WM_USER_TIME of 123456789
FOCUS: _NET_ACTIVE_WINDOW message sent for 0x400004 at time 0 by client type 1.
STARTUP: Window 0x400004 has _NET_WM_USER_TIME of 821355178   (the current monotonic time wins)
FOCUS: _NET_ACTIVE_WINDOW message sent for 0x400004 at time 123456789 by client type 1.
FOCUS: last_user_time (821355178) is more recent; ignoring _NET_ACTIVE_WINDOW message.
...  FOCUS: * Focus --> W0        (at the close)
```

A past value is ignored by the activation guard and never becomes `last_focus_time`, so the close's
hand-over succeeds. That is why "a 32-bit-safe `_TIME` still crashed" in `h7-replica.md` was wrong:
that cell changed **only** `DESKTOP_STARTUP_ID` and left `XDG_ACTIVATION_TOKEN` at the real
13-digit spawn-at value, which GTK still read (MEASURED cell 8e/8f: either variable alone with the
real value crashes; the harmless value in either alone does not).

### 3.5 The app must ask for activation with that time

`gnome-calculator` (GTK4, X11) also gets `_NET_WM_USER_TIME = 2266420986` from the same startup id
(MEASURED `f5_debug_calc_x11`), but it sends **only** the timestamp-0 `_NET_ACTIVE_WINDOW`; there is
no second one with the future time, so `last_focus_time` stays the current monotonic time and the
close hands focus to W0 normally. `gnome-text-editor` sends the second one. So the app-side
condition is "the app requests activation with the startup-supplied time", not "GTK4" as such
(MEASURED for the two apps; HYPOTHESIS for the general rule).

### 3.6 Why DING is needed

`h7-replica.md` measured DING as necessary and sufficient with the spawn-at path. With the mechanism
above, DING does two things: its maximized desktop window (class `gjs`, app id
`com.rastersoft.ding`, `keepAtBottom`, `hide_from_window_list`) is the MRU focus candidate at the
close; and in a fresh headless Shell, with no other window focused, Mutter does not focus the new
window (MEASURED `f3`: plain launch, real variables, **no DING** → `focused=false`, clean, no
timestamp-0 warning). DING's own `destroy` handler activates the top-most window
(`emulateX11WindowType.js:285-292, 372-404`), which is how the desktop window keeps the focus in the
live session too. The crash needs a focused window at close; DING provides both the focus and the
target (MEASURED f3 vs f1/f2).

## 4. Factor cells (MEASURED, editor X11, fresh DING-only Shell each, 5 runs, stop at first crash)

| # | Cell | Configuration | Result |
| :-- | :-- | :-- | :-- |
| 1 | `g1_geom_none` | `spawn-at spawn`, no `--size`, no `--anchor`, both variables | **crash run 1** |
| 2 | `g2_geom_size` | `--size` only | **crash run 2** (run 1 had no window: setup miss) |
| 3 | `g3_geom_anchor` | `--anchor` only | **crash run 1** |
| 4 | `g4_geom_both` | `--anchor` + `--size` (the known crashing cell) | **crash run 1** |
| 5 | `g5b_no_wait` | `--no-wait` (window found by class) | **crash run 1** |
| 6 | `g6_plain_armed_novars` | plain launch, extension armed for another class, **no** variables | 5 clean, timestamp-0 5/5 |
| 7 | `g7_plain_armed_vars` | plain launch, same armed registration, **real** variables | **crash run 1** |
| 8a | `g8a_both_harmless_nosuffix` | both variables `xflash-harmless` (no `_TIME`) | 5 clean, timestamp-0 5/5 |
| 8b | `g8b_both_harmless_time123` | both `xflash-harmless_TIME123456789` (past 32-bit time) | 5 clean, timestamp-0 5/5 |
| 8c | `g8c_dsid_harmless_token_unset` | only `DESKTOP_STARTUP_ID`, harmless | 5 clean |
| 8d | `g8d_token_harmless_dsid_unset` | only `XDG_ACTIVATION_TOKEN`, harmless | 5 clean |
| 8e | `g8e_dsid_real_token_unset` | only `DESKTOP_STARTUP_ID`, real spawn-at value | **crash run 1** |
| 8f | `g8f_token_real_dsid_unset` | only `XDG_ACTIVATION_TOKEN`, real spawn-at value | **crash run 1** |
| ctrl | `f2_debug_plain_vars` | plain launch, real variables, no armed registration, debug log | **crash run 1** |
| ctrl | `f3_debug_plain_vars_noding` | plain launch, real variables, **no DING** | clean, `focused=false`, no warning |

What each outcome says about the path:

- Cells 1 to 4: the size and the anchor (the resize and the move) are **not needed**. The crash
  happens with a plain `spawn-at spawn` that only arms, claims, cloaks and uncloaks.
- Cell 5: the CLI's wait is **not needed** (the extension still places the window).
- Cells 6 and 7: the extension's armed registration and its cloak/uncloak of a new window are **not
  needed**; the same plain launch crashes once the variables are real. Together with `f2` (plain,
  no armed registration, real variables) that removes spawn-at's launch path entirely: only the
  **variables** matter (plus DING, focus, and the close).
- Cells 8a to 8f: the value matters, not the variable. Any one variable with the real 13-digit
  `_TIME` crashes; the same variable with a harmless value does not.
- Controls: without DING the window is not focused and nothing crashes; the timestamp-0 warning
  alone (cells 6, 8a, 8b, and the surviving calculator) does not crash.

## 5. Focus story with evidence (MEASURED `data/ding/focus/`)

`f1_debug_focus` (crash) and `f4_debug_harmless_time123` (clean) are the same cell with the same app
and launcher, differing only in the `_TIME` value. The sequence, with the Shell's `MUTTER_DEBUG=focus`
lines:

| Step | Crashing (f1) | Clean (f4) |
| :-- | :-- | :-- |
| 1. `_NET_WM_USER_TIME` at map | 2265490556 (future) | 123456789 (past) |
| 2. GTK's first `_NET_ACTIVE_WINDOW` (time 0) | accepted; "not updated to 820688193, because it is less than 2265490556" | accepted; current time 821355178 becomes the user time |
| 3. GTK's second `_NET_ACTIVE_WINDOW` | at time 2265490556; **accepted**, `last_focus_time` becomes 2265490556 | at time 123456789; **ignored**, "last_user_time (821355178) is more recent" |
| 4. Close: "Focusing default window since we're unmanaging 0x400004" | candidate **W0** | candidate **W0** |
| 5. "Setting input focus to window W0, input: 1 focusable: 1" | no `* Focus --> W0` follows | `FOCUS: * Focus --> W0` follows |
| 6. Result | "Failed to focus window W0 while avoiding 0x400004", fallback fails, assertion, Shell aborts | focus moves, next `_NET_ACTIVE_WINDOW` for W0, no crash |

Which window got the failed focus: **W0, DING's desktop window**. Evidence: the failed message names
W0; the only other window in the DING-only Shell is the app's X11 window 0x400004 (which is the
`not_this_one` in the same message); `spawn-at query windows --json` in the same Shell lists
`{"title":"Desktop Icons 1","class":"gjs","app_id":"com.rastersoft.ding","maximized":true}` as the
only window (MEASURED, runner log of `g1`/`g3`); and the DING-only Shell enables exactly
`['spawn-at@harsh.local', 'ding@rastersoft.com']`. The failing call is Mutter's Wayland `focus()`
class method, which prints nothing of its own, and no `* Focus --> W0` appears — consistent with a
refusal inside `meta_display_set_input_focus` before `meta_display_update_focus_window` (INFERRED
from source).

## 6. Exposure map (MEASURED, fresh DING-only Shell per app and protocol, 5 runs, stop at crash)

"X11" = the app forced onto XWayland (`GDK_BACKEND=x11`, `QT_QPA_PLATFORM=xcb`,
`--ozone-platform=x11`, or an X11-only app); "native" = the same app on Wayland. All launches via
`spawn-at spawn` with the real variables, `--no-focus`, close by `spawn-at close`.

| App | Toolkit | X11 | native Wayland | note |
| :-- | :-- | :-- | :-- | :-- |
| gnome-text-editor | GTK4 | **CRASH run 1** | clean 5 | also crashes without `--new-window` (crash 1) |
| eog | GTK3 | clean 5 (no timestamp-0) | clean 5 | |
| gnome-calculator | GTK4 | clean 5 (timestamp-0 5/5) | clean 5 | sends only the time-0 activation |
| nautilus | GTK4 | **not valid** | clean 5 | D-Bus activation: DING's own name watch starts a nautilus service; every window belongs to it and carries no startup variables (env null), so the cell tested nothing |
| gnome-terminal | GTK3 (server) | clean 5 (no timestamp-0) | clean 5 | |
| google-chrome | Chromium | clean 5 (no timestamp-0) | clean 5 | `--no-sandbox` needed in the private session; 5 runs with no window before that |
| code (VS Code) | Electron | **no window** | **no window** | the launcher exits 0 without mapping a window within 25 s, twice; not measured |
| qt-assistant | Qt 5.15 Widgets | clean 5 | clean 5 | |
| xterm | Xt (X11 only) | **cannot start** | n/a | `xterm: Error 32 ... get_pty: not enough ptys`; pty creation is refused to processes this agent starts (`script` and `python3 pty.openpty()` fail the same way). The 80 live xterm runs and the private bisect cell stand (xterm ignores the variables) |
| Java Swing (local client) | AWT/X11 | clean 5 | n/a | source quoted in §6.1 |
| firefox | Gecko (snap) | **cannot run** | **cannot run** | the snap refuses the private `XDG_RUNTIME_DIR` (`cannot create XDG_RUNTIME_DIR folder "/run/user/1000/snap.firefox"`); not measured |

X11 rows that had the variables and a focused window at close: editor (crash), eog, calculator,
gnome-terminal, chrome, qt-assistant, java Swing (clean). So on stock Ubuntu the exposed set is
narrower than "any forced-X11 app": it is the apps that request activation with the startup time.
gnome-text-editor, the app in both live crashes, is one.

### 6.1 The Java client

```java
import javax.swing.*;
public class SwingApp {
    public static void main(String[] args) {
        SwingUtilities.invokeLater(() -> {
            JFrame f = new JFrame("xflash-swing");
            f.add(new JLabel("xflash java swing client"));
            f.setSize(600, 400);
            f.setLocationByPlatform(true);
            f.setVisible(true);
        });
    }
}
```

`javac SwingApp.java` in `scratch/xflash02/swing/` (not committed: committed tooling is Rust).

## 7. Upstream and Ubuntu (SECONDHAND unless stated)

Ubuntu package versions (MEASURED, `dpkg -l`): `gnome-shell-extension-desktop-icons-ng
46+really47.0.9-1ubuntu5` (DING 47.0.9), `mutter`/`libmutter-14-0` `46.2-1ubuntu0.24.04.18`,
`gnome-shell` `46.0-0ubuntu6~24.04.16`.

| Report | State | What it says |
| :-- | :-- | :-- |
| mutter #45 `meta_window_unmanage: assertion failed (focus_window != window)` (2017, gnome-3-26) | closed 2018-02-26 | same assertion; closed without a code fix found; the descendant of the old report |
| mutter #862 "Mutter error/abort" | closed 2020-12-18 | fixed by commit `e4d8e4b9f7d` "core: Make sure focus_default_window() worked": it adds `try_to_set_focus_and_check` (the "Failed to focus window … while avoiding …" warning) and falls back to `meta_display_unset_input_focus` |
| Launchpad #2016308 (Ubuntu 23.04) same assertion at window.c:1533 | Fix Released (2023-05-19 upstream, 2023-06-13 lunar) | a different trigger; mutter 46.2 still has the assertion and still aborts (MEASURED) |
| Launchpad #1422253 (2015) | — | same assertion via `handle_other_xevent` |
| DING issues | — | no issue reports this Shell crash. Adjacent focus reports: #278 "Wrong window gets focus on Wayland-X11" (closed), #362 "XDG Activation Protocol: windows open in background if focus-new-windows is strict" (open), #235/#176/#174 focus stealing, #211 "Minimized windows get restored on closing another window". DING 47.0.13's notes say "made the desktop receive focus when no other windows are open" |
| GDK X11 startup-id docs | — | `gdk_x11_display_set_startup_notification_id`: the part after `_TIME` is taken as the X11 timestamp of the event |

The upstream fix in #862 does **not** cover this case: its fallback is
`meta_display_unset_input_focus`, which goes through the same
`meta_display_set_input_focus` → `meta_display_timestamp_too_old` and is refused for the same reason.
Mutter's current main still has the assertion (`src/core/window.c:1640`), the same
`meta_display_timestamp_too_old`, and the same unset path (MEASURED from the fetched sources), so
this trigger is **unfixed upstream** as of 46.2 and main. No DING workaround release was found.

## 8. Mitigation (ranked; candidate built and tested)

Where the two variables are set: `src/platform/linux/gnome/driver.rs:317-318` (GNOME driver) and
`src/platform/linux/x11/mod.rs:57-58` (X11 driver), both to `entry.key`, which is built once in
`src/commands/spawn.rs:276-283`. The extension matches on `window.get_startup_id()` as one of the
window's identifiers (`assets/gnome/modern/extension.js:385` in `_getIdentifiers`, then
`_matchesArmKey`, which also tries `wm_class`, the GTK application id and the sandboxed app id with a
substring rule). The CLI's own wait matches by pid/descendant and by the startup id in the
title/class.

| Rank | Option | What it fixes | What it breaks |
| :-- | :-- | :-- | :-- |
| 1 (chosen) | **Drop the `_TIME<epoch_ms>` suffix** from `entry.key`; keep the unique id (`spawn-at-<pid>-<ms>`) | Removes the X11 timestamp from both variables everywhere (X11 and Wayland, focus or no focus). Matching still works: the id is unchanged in kind, so `_NET_STARTUP_ID`, the app-id/class fallback and the CLI's wait all still match. The GTK3 parser fix of `ccc84dd` is not undone: the id is a plain, well-formed startup id | GTK then activates with `CurrentTime` (0): the "Buggy client … timestamp of 0" warning stays and the app loses the precise user-time of the launch event. Measured harmless here (cells 6, 8a, 8b and the surviving apps). The id no longer carries a time, so anything that expected one (none found in this repo) loses it |
| 2 | Set no `DESKTOP_STARTUP_ID`/`XDG_ACTIVATION_TOKEN` for X11-kind clients | Removes the trigger for X11 clients | X11 windows lose the startup-id identifier; matching falls back to wm class / GTK app id / pid. `h7` cell 6 (both variables unset) still claimed the editor through its GTK app id, but a D-Bus/daemon app whose window id differs from the requested class loses placement silently, while the CLI's pid-based wait still reports success. Does not address the same value on a native Wayland client (harmless today, untested for other toolkits) |
| 3 | Match by pid/class and drop the id when activation is not wanted (`--no-focus`) | Narrowest: removes the trigger for batch spawns that do not want focus | Leaves the crash for every default (focused) `spawn-at spawn`; drops the id for all `--no-focus` spawns, with the same matching regressions as option 2; and the app can be focused at close by the user anyway, so the trigger can return without the flag |

Same value on Wayland: native runs with the real variables were clean in every cell (7 apps), so
option 1's scope (all clients) is safe for those.

### 8.1 Candidate: patch + tests

`docs/evidence/x11-flash-02/proposed-fix-activation-env.patch` (not applied to the tree):

- `src/commands/spawn.rs`: `entry_key = format!("spawn-at-{}-{}", pid, ts_ms)` (no `_TIME`), with a
  comment that states why. Two lines of behaviour, no API change, no protocol change, no
  extension change.
- `src/commands/spawn.rs` test `test_the_launch_key_carries_no_x11_timestamp`: asserts the key
  matched by the mock backend is `spawn-at-<digits>-<digits>`, contains no `_TIME`, and that the
  two variable values built from it (both drivers copy the key verbatim) carry no timestamp.
- `verify/apps/gtk4-x11-close-focused.toml` (PROPOSAL): a release-gate-session live spec that
  spawns `gnome-text-editor` with `GDK_BACKEND=x11` through `spawn-at spawn`, closes it while
  focused, and asserts the window is gone and the Shell is the same process. It needs a real
  GNOME session with DING; it states the two harness extensions it needs (`command`/`env` in a
  setup step, a `shell_alive` check).

Built in a throwaway worktree (`/tmp/xflash-fix`, detached at `e7f6f19`, deleted afterwards) and
tested with the worktree's binary inside `psh` only:

| Cell | Binary | Result |
| :-- | :-- | :-- |
| `p1_patch_editor_x11` (the crashing cell) | patched | **5 clean**, all claimed, key `spawn-at-2983504-1793268949319` (no `_TIME`) in the child env |
| `p2_patch_eog_x11` (GTK3 regression, `ccc84dd`) | patched | 5 clean, all claimed; the only CRITICALs are DING's chronic `GFileInfo` ones, no GTK startup critical |
| `p3_patch_editor_wl` (native Wayland matching) | patched | 5 clean, all claimed |
| `p4_patch_calc_x11` | patched | 5 clean, all claimed, timestamp-0 5/5 |
| unit tests in the worktree | `cargo test --locked -p spawn-at` | 480 + 10 + 7 + 1 + 19 + 3 + 1 passed, 0 failed; the new test passes |

The patched cells still show the timestamp-0 warning (`ts0=true`): the app activates with
`CurrentTime` and Mutter resolves it itself, which the factor cells show is harmless.

### 8.2 User workaround now

- Unset both variables for the child; placement still matched in the private Shell for the editor
  (`h7` cell 6, 5 of 5 claimed): `spawn-at spawn -- env -u DESKTOP_STARTUP_ID -u
  XDG_ACTIVATION_TOKEN <app>`.
- Or avoid launching a forced-X11 GTK4 app this way until the fix ships.
- Or, on the desktop, disable DING (`gnome-extensions disable ding@rastersoft.com`), which removes
  both the focus target and the desktop on stock Ubuntu; that is a large change, so it is the last
  resort.

## 9. What is established and what is not

Established (MEASURED):

- DING + a forced-X11 GTK4 app that activates with the startup time + the real spawn-at variables +
  focus at close → the assertion, every time, in a fresh DING-only private Shell. 11 crashes, all
  the same assertion; 0 clean runs with that configuration.
- The trigger is the **value** in the variables (the 13-digit `_TIME`, truncated to 32 bits, read as
  a future time): the same cells with a harmless value (past or no `_TIME`) are clean 20 of 20.
- The launch path is not needed: plain launches with the real variables crash; size, anchor,
  `--no-wait` and the armed registration are irrelevant; without DING nothing is focused and
  nothing crashes.
- The focus target is DING's desktop window W0; the focus change is refused, the fallback is
  refused, and the assertion aborts the Shell.
- The candidate patch removes the crash in the private Shell (5 of 5) and keeps matching and the
  GTK3 behaviour (15 more runs).

Not established:

- Whether every toolkit that reads `_TIME` this way is affected (GTK4 measured; GTK3 not; Qt,
  Chromium, Electron, Java, xterm not, but for the last four the startup id is not read or the app
  was not testable here). Electron, Firefox, nautilus and xterm are **not measured** (see §6).
- The exact GPtruncation code path in GTK4 (the truncation is MEASURED by the arithmetic and the
  `_NET_WM_USER_TIME` value; the code was not read).
- Whether a live session with DING but with no other *Wayland* window behaves the same; the private
  Shell is the reproduction, the live crashes are consistent with it.
- Whether other GNOME versions (47 to 50) behave the same; mutter main still has the assertion and
  the timestamp check (source read), so the trigger is expected to exist, but no run was made.
- The crash needs a focused window at close; a user closing an unfocused window is not affected
  (the unmanage path is only entered for the focused window) — INFERRED from the source, not
  measured as a separate cell.

Correction to the earlier notes: the `h7` plain-launch cells 17, 18 and 19 were clean because they
used `DESKTOP_STARTUP_ID=spawn-at-1-2_TIME3` (see `bisect-cells.sh`), a **past** time, not the real
spawn-at value; and cell 7 ("fitid", `_TIME123456789`) changed only `DESKTOP_STARTUP_ID` and left
the token at the real value. Both therefore did not test what they were meant to. With the real
value, a plain launch crashes (MEASURED f2).

## 10. Raw data

`data/ding/`: `focus/` (the three debug runs, crash and clean), `factor/` (cells 1 to 8f and the
controls), `matrix-x11/`, `matrix-wl/`, `mitigation/` (patched cells). Each directory has
`cells.jsonl` (one record per run: env, focus at close, close note, warning flags, spawn JSON), the
runner log `ding-cells.log`, the Shell log per cell, and `<label>.shell-crash.txt` for the eleven
aborts. `data/ding/PROGRESS.md` lists the Shell breaks. The driver is `tools/ding-cell.sh`; the cell
tool is `tools/src/cell.rs` (`--geom`, `--no-wait`, `--arm-class`, `--startup-env`, `--find-class`,
`--setenv`).
