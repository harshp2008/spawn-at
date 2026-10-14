# spawn-at 0.2.0-beta.1: release notes (draft, not published)

This is a beta. It has been run on one machine: GNOME 46, Wayland, one monitor. Read "What is untested" before you rely on it.

## What is new

- **A wait for the app on GNOME Wayland (the "ready-wait").** Before 0.2 the extension revealed a window after a 30 ms quiet period, which reveals too early when an app is slow to redraw after a resize. On GNOME 46 Wayland the extension now keeps the window hidden until the app has answered the size request, or until a cap runs out. The cap defaults to **250 ms** (it was 150 ms in development builds). The hard limit is 1500 ms. XWayland and X11 windows, and the legacy GNOME 42-44 extension, still use the old rule.
- **`spawn-at config`.** Show and change settings without editing files:
  - `wait.cap_ms` (1 to 1500): how long the extension waits for an app to answer a size request.
  - `wait.strategy` (`auto`, `legacy`, `ready-wait`): which rule waits. `legacy` is the pre-0.2 behaviour.
  - `spawn.window_timeout_ms` (200 to 60000, default 2000): how long `spawn` waits for the window to appear.
  - Commands: `show`, `get`, `set`, `unset`, `reset`, `path`. A value on the command line beats the file, and the file beats the default. See [CONFIG.md](CONFIG.md).
- **A notice on untested GNOME versions.** The installer prints what is and is not tested and asks for confirmation before it installs on a GNOME version other than 46 (`--yes` skips the question; headless runs print the notice and continue).
- **Autostart fix.** The login update-check entry used a bare `spawn-at`, which is not found at login because `~/.local/bin` is not on `PATH` there. The installer now writes the absolute path of the binary it installed. Re-run `spawn-at install` to rewrite the file.
- **Developer-only note (not a user feature):** the test harness has a flash check that reads the client's commit time from the Wayland protocol, hostile-app fixture flags (slow, refusing, grid-snapping, flapping, stalled and hung apps) and nine catalog cells that use them; and the extension can log the paint signals of XWayland/X11 windows while logging is switched on (it changes no timing or outcome; it was used to study the X11 question below).

## What is measured, and the caveats

All numbers are from the project's own test window, not from real apps, on **one machine** (GNOME 46, Wayland, one monitor, 60 Hz, 22 CPUs). Methods and raw counts: [TEST_PLAN.md](TEST_PLAN.md), sections 6.22 to 6.24.

| Measurement (native Wayland, test window) | Old rule (legacy) | Ready-wait (new default) |
| :-- | :-: | :-: |
| Flashes, idle (20 runs each) | 0 | 0 |
| Flashes, under heavy CPU load (22 busy threads, 40 runs each) | 26 | **0** |
| Window hidden for, under load (median, ms) | 45 | 67 |
| Window hidden for, idle (median, ms) | 19.6 | 19.5 |

- **Wire time is the optimistic bound.** A "flash" here means the window was revealed before the app's first commit at its final size, read from the Wayland protocol dump. A commit is not a frame on the screen, so this can only undercount what you would see.
- **No screen capture was done.** Nobody measured pixels. "Far fewer flashes in our measurements" is the claim; "no flashes" is not.
- **It costs time under load:** about 21 ms more hidden time at the median, 105 ms against 53 ms at the 95th percentile.
- **An app that is already at its minimum size, or one that never answers, costs the full cap** on every spawn (about +230 ms in our test, with the cap at 250 ms).
- **XWayland/X11: investigated, no reliable signal.** We looked for a way to tell when an X11 client has painted at its new size, using the test window only. Tried and not usable: the first `damaged` event, `first-frame`, the actor size, the buffer rect, the surface texture size, holding the cloak and waiting for a second `damaged`, and resizing before the map. They fire at the shell's resize or at the map, not at the client's paint (for example the texture size reached the target before the client painted in 59 of 80 runs). A window ping has no reply API. A pixel probe could tell a blank buffer from the painted one on the test window, but it crashed GNOME Shell in testing and was removed. XWayland and X11 therefore keep the old rule and can still flash. Still untried: the X sync counter (client-side, partial toolkit support) and a screen-capture measurement rig (the Mutter screencast stream could not be consumed; the portal needs a user prompt). See [DESIGN_READY_WAIT.md](DESIGN_READY_WAIT.md), section 12.
- **Known gap:** on GNOME 46 the extension cannot read an app's reported minimum size (it reads 0x0), so it learns a minimum only by waiting.

## What is untested

- Real X11 and XWayland applications (only the fixture was used), and native X11 sessions beyond the fixture.
- GNOME 42, 43, 44 (the legacy extension has never run on a real GNOME of those versions) and GNOME 45, 47, 48 (the metadata lists 45 to 48, but only 46 has been run).
- Qt and Electron apps. Real apps were tried only a few times each, on Wayland: gnome-text-editor and nautilus (GTK4), gnome-terminal, zenity and firefox (snap), which we believe are GTK3 but did not check. Chrome was never matched.
- Multi-monitor setups and fractional scaling.
- Release builds of the test fixture, and any screen-capture check of what is visible.
- Other desktops: Hyprland, Sway, KDE, macOS and Windows are not implemented.

## How to report a problem

Open an issue: <https://github.com/harshp2008/spawn-at/issues>. Please include:

1. Your GNOME Shell version and session type (`gnome-shell --version`, `echo $XDG_SESSION_TYPE`), and the output of `spawn-at config show`.
2. The exact `spawn-at spawn ...` command and the app, and whether it is a native Wayland, XWayland or X11 window.
3. What you saw (flash, wrong size, wrong position, window never appeared) and whether it happens every time or only under load.
4. Whether `spawn-at config set wait.strategy legacy` makes it better or worse. That one comparison tells us whether the new wait is involved.
5. If you can, the extension log of one failing spawn: switch logging on with
   `gdbus call --session --dest org.gnome.Shell --object-path /org/gnome/Shell/Extensions/SpawnAt --method org.gnome.Shell.Extensions.SpawnAt.SetLogging true`,
   repeat the spawn, then run `journalctl --user --since "5 minutes ago" | grep spawn-at-time`, and switch logging off again with `false`. This is an in-memory flag; it is off after a log-out.

To go back to the pre-0.2 behaviour at any time: `spawn-at config set wait.strategy legacy`.
