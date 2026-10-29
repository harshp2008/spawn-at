---
title: "Known shortcomings"
---

What is broken, missing or unmeasured in spawn-at, taken from the project's own audits. Nothing here
is a plan. How each command behaves (exit codes, waits, how windows are matched) is on
[Known limits](/spawn-at/known-limits/).

## Behaviour that may surprise you

### spawn ignores the focus flags

`spawn --focus`, `--no-focus` and `--defocus` are **accepted and do nothing**. A test pins this. What the
code does is on [spawn](/spawn-at/commands/spawn/).

### Some GNOME commands see only the active workspace

On GNOME, `focus`, `defocus`, `maximize`, `minimize`, `restore` and `close` find windows on the
active workspace only. For a window on another workspace the extension answers "false". The command
then fails with an error that says the compositor failed.

### A window that cannot answer a resize costs the full cap

The ready-wait holds a new window hidden until the app answers the resize request. See
[Concepts](/spawn-at/concepts/#cloak-and-waiting-for-the-app). An app already at its minimum size, or
a hung app, never answers. The window then **stays hidden until the cap**, `wait.cap_ms`, which is 250 ms
by default. The README reports about 230 ms extra in one test.

### Under load the window is hidden longer

Median time the window stayed hidden under load, in two measurements:

| Measurement | Old rule | Ready-wait |
| :-- | :-- | :-- |
| First | 45.3 ms | 66.8 ms |
| Second | 48.9 ms | 56.2 ms |

Idle, there was no difference. In the second measurement, 1 of 40 idle runs reached the 250 ms cap.
The cause is unknown.

### Sway can flash once

Sway cannot hold a new window invisible, so a window can show before it is placed. This was not
measured.

### Closing a focused GTK4 window that runs on XWayland could abort the Shell

Status: **fixed** after 0.2.4 (not yet in a release). Every release up to 0.2.4 has the bug.

Until now `spawn` gave the program a startup id of the form `spawn-at-<pid>-<ms>_TIME<ms>`, where
`<ms>` is the time in milliseconds since 1970 (13 digits). GDK, the GTK display layer, reads the
number after `_TIME` as an X11 server timestamp and cuts it to 32 bits. On Wayland that value is
about 16 days ahead of Mutter's clock. A GTK4 program on XWayland then asked to be activated with that
time, Mutter kept it as the time of the last focus change, and when the window was closed while it had
focus, Mutter refused to hand focus to the next window. GNOME Shell then stopped with `meta_window_unmanage:
assertion failed: (window->display->focus_window != window)` and took every window of the session
with it.

The id is now `spawn-at-<pid>-<ms>_TIME0`. It is still unique and still the value the window is
matched by. The part after `_TIME` is 0, which X11 reads as "no timestamp", so a program cannot take
it for a time in the future.

Who was affected, as far as it was measured:

- gnome-text-editor started with `GDK_BACKEND=x11` on Ubuntu 24.04 (GNOME Shell 46.0, Mutter 46.2),
  in a private headless Shell with Desktop Icons NG (`ding@rastersoft.com`, installed by default)
  loaded: the Shell stopped on 10 of 10 closes with the old id and on 0 of 10 with the new one.
- Without Desktop Icons NG the Shell did not stop in 30 runs (one or two native Wayland windows open,
  the editor focused or not when it was closed). In those runs the editor never asked for activation
  with the id's time, so the condition for the bug was not reached. That the bug needs Desktop Icons NG
  is therefore not shown.
- The investigation before this fix also tried gnome-calculator, eog, gnome-terminal, Chrome and Qt
  Assistant with the old id and Desktop Icons NG (5 runs each); the Shell did not stop. Those runs
  were not repeated for this change. Whether another toolkit would stop it is unknown.

Not established: other GNOME or Mutter versions (Mutter's development branch still has the assertion),
other programs, whether the two Shell stops seen on a live session were this cause (they are
consistent with it), and whether Mutter will change. The bug was never reproduced on a live session
after the fix, and no live session was used to test the fix.

The exit status of the state commands and the cursor options on Sway are covered on
[Known limits](/spawn-at/known-limits/).

## Not verified anywhere

| Area | Status |
| :-- | :-- |
| GNOME 46 on Wayland | The only setup tested in earnest: one machine, one monitor. |
| GNOME 42 to 44 | The legacy extension was never run on a real GNOME 42 to 44. It **may not work at all**. |
| GNOME 50 (50.1, Ubuntu 26.04), Wayland | Partly tested, with failures. One `release-gate` run on a clean build: of 36 required cases 16 passed, 9 failed (every "request below the minimum size" case came out 300x300 instead of 300x200), 4 cloak cases were skipped (their spec limits them to Shell 42-48) and 7 await a human verdict. The extension logged `reportedMinSize=(0x300)` for the test window, which may explain the extra height (not investigated). |
| GNOME 45, 47, 48, 49 and 51 and later | The extension's metadata lists 45 to 50, but only 46 has been run in earnest. |
| GNOME layouts and apps | Multi-monitor, fractional scaling, Qt and Electron apps: untested. |
| X11 and XWayland | Test window only, no real apps. No test touches an X server. |
| Sway | A private headless Sway 1.9 only. No real session, no other version, no i3. |
| Sway layouts | More than two outputs and output scales other than 1 and 2: untried. |
| Windows and macOS | Not implemented. The code is only compile-checked, never run. |
| KDE and Hyprland | Not implemented. |
| `spawn-at uninstall --scope system` | Not run. |
| `spawn-at update` off Linux x86_64 | Only Linux x86_64 builds are published, so no other release file was ever picked. |

More on each environment: [GNOME on Wayland](/spawn-at/environments/gnome-wayland/),
[Sway](/spawn-at/environments/sway/), [X11 and XWayland](/spawn-at/environments/x11-xwayland/).

## Limits of the flash measurement

- **No pixels are captured.** Nobody has measured what was on screen.
- A clean result is not a guarantee. It means no late paint or move was found in the runs made.
- "Flash" is computed from the Wayland protocol: the window was revealed before the app's first
  commit at its final size. That is the optimistic bound. It says nothing about what a frame showed.
- A flash with a lead under one frame (16.7 ms at 60 Hz) is labelled "possible". The code itself calls
  its probability a hypothesis. On X11, 19 of 20 recorded runs were "possible", so this is the normal
  state there, not a noise floor.
- Only GNOME `spawn` is measured for flash. GNOME `transform` is informational and not yet run on a
  real session. The other commands are not measured. Sway and X11 cannot be, because it needs the
  GNOME extension's clock.
- The extension's log has entries for `spawn` and `transform`, only some for `focus` and `minimize`,
  and none for the rest, so a flash or ordering check works for the first two on GNOME and only
  partly for the others. What it logs for each command is in the
  [runner setup](/spawn-at/project/design-runner-setup/) document.
- All numbers come from one GNOME 46 Wayland machine with the project's test window.

The gaps in the test suite itself are on [Testing and the verify tool](/spawn-at/testing/#limits-of-the-test-suite).

## Sources

- `docs/TEST_COVERAGE_AUDIT.md`, sections 3, 4 and 7 (gaps 1 to 18 and their updates)
- `README.md` ("How well it works", "Support", the `--focus` note)
- `docs/KNOWLEDGE.md` ("Testing and fixtures", "Gotchas", "Open questions")
- `docs/DESIGN_RUNNER_SETUP.md`, section 5.5 (what the extension logs per command)
- `assets/gnome/modern/extension.js` (the workspace lookup, the log calls)
- `src/commands/spawn.rs` (the test that pins the ignored focus flags) and `src/update.rs` (the
  release file name is built from the build target; only Linux x86_64 is published)
- `docs/TEST_PLAN.md` (the live checks that are still open)
- `docs/TEST_LOG.md`, section 6.23 (the measured costs)
