---
title: "6.39 A private headless Shell for the batches"
---

A separate GNOME Shell, with its own bus, Wayland socket and X server, was built so that a Shell crash cannot take down the desktop. It works, with one stand-in service. It is **headless**: software rendering, one virtual monitor and no real input, so it does not behave like the live session.

| Field | Value |
| :-- | :-- |
| **ID** | X11-FLASH-02f step 1. Follows [6.38](/spawn-at/experiments/6-38-shell-crash-live-xwayland-editor/). |
| **Date** | 2026-10-29, about 13:30 to 14:00 (+05:30). |
| **Build** | GNOME Shell 46.0, Mutter 46.2, spawn-at 0.2.4; the extension copy is `diff -r` identical to the installed one. Tree: branch `exp/x11-flash-02`. |
| **Verdict** | Works for xterm and for the bisect ([6.41](/spawn-at/experiments/6-41-bisect-private-shell/)). The live Shell was untouched (pid 2718897, start time 81373971, before and after). |
| **Raw results** | Smoke test records in the repository (Evidence below). |

## At a glance

| Question | Answer |
| :-- | :-- |
| Started how | `gnome-shell --headless --wayland --wayland-display=xflash-wl --virtual-monitor 1920x1080`, own D-Bus, in a memory-limited scope |
| Extension loaded | yes, protocol 7 over the private bus (measured) |
| Isolated from the live session | windows only in the private Shell; layout differs (work area 1920x1048 against 1920x1040) |
| Smoke test | xterm, 3 runs, all ok |
| Crash handling | killed only the recorded private pid: the wrapper refused to run, a new Shell started |
| Representative of the live session | **no**: headless, no gnome-session, portals, input devices or history |

## What was tested

**Question**

Can the measurement batches run where a Shell crash is harmless?

**Setup and environment**

Everything lives in one sandbox under `/tmp`: runtime directory, config, data, cache and a keyfile gsettings backend (so no dconf service can be reached), plus a copy of the extension enabled only in the sandbox keyfile. One wrapper, `psh`, runs every command and refuses unless the private socket, X display, bus and Shell pid are the private ones and none is the live one. The sandbox is removed with a mount-point check, and helper processes the Shell starts (ibus, gvfs, evolution, at-spi) are ended by a tag in their environment, never by name.

## Result

**Failed attempts, and the fixes.**

1. The Shell did not start: `systemd-run` was given the private bus and could not create its scope. It now gets the live bus, and the Shell sources the private environment inside the scope.
2. Every X client hung. The private X server announced two displays; Mutter logs "Using public X11 display :2, (using :3 for managed services)" and the newest socket (:3) is the Shell's own. The wrapper now reads the log line.
3. X clients still hung: the Shell asks systemd to start the X11 services target before it opens its X display, and the private bus has no systemd, so Mutter reported "Failed to initialize X11 display". A small stand-in for `org.freedesktop.systemd1` on the private bus fixed it. It starts nothing.

**What worked.** The extension loaded; `spawn-at query layout` answered with the private monitor; the observer attached to the private X display; three xterm runs completed with the window closing; the run tool read the private Shell's pid and log instead of `pgrep` and the journal.

**What could leak.** Socket numbers in `/tmp/.X11-unix`; one transient scope in the user's systemd manager per Shell; CPU and memory; a crash file written by apport if the private Shell crashes (none was written). The private session writes nothing to the live dconf, config, cache or enabled-extensions.

## Not verified

- That timings or focus behaviour match the live compositor. They do not in at least one way: a new window is not given focus unless it is asked for.
- Any other Shell version, a hardware GPU, a second monitor, fractional scaling.
- That the stand-in service is enough for everything the Shell does with systemd.

## Sources

- [Isolation design and bisect plan](https://github.com/harshp2008/spawn-at/blob/main/docs/evidence/x11-flash-02/isolation-feasibility.md)
- [Results and deviations](https://github.com/harshp2008/spawn-at/blob/main/docs/evidence/x11-flash-02/bisect-2026-10-29.md) (section 1)
- [Smoke test records](https://github.com/harshp2008/spawn-at/blob/main/docs/evidence/x11-flash-02/data/smoke/)
- [The tools](https://github.com/harshp2008/spawn-at/blob/main/docs/evidence/x11-flash-02/tools/) (`psh-start.sh`, `psh`, `psh-stop.sh`, `psh-batch.sh`, `src/bin/sysd1stub.rs`)
- The links point into the repository on GitHub and resolve once the branch is merged.
