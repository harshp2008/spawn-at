---
title: "GNOME on Wayland"
---

**Status: supported. Tested on GNOME Shell 46 only**, Wayland, one monitor, one machine. GNOME
Shell 50.1 (Wayland) was run once for 0.2.4 and is partly tested, with failures: of the 36 required
`release-gate` cases 16 passed, 9 failed (windows requested below their minimum size came out too
tall), 4 were skipped and 7 await a human verdict.

This is the one environment where spawn-at hides a window until it is placed. Only the compositor
can place windows on Wayland, so spawn-at ships a GNOME Shell extension and talks to it over D-Bus
(`org.gnome.Shell.Extensions.SpawnAt`). `spawn-at install` deploys it. After installing, log out and
back in once so GNOME loads the extension.

What the extension does for a `spawn` is described in [Concepts](/spawn-at/concepts/): it is told the
target first, hides the new window, resizes it, waits for the app to answer, moves it, reveals it,
and keeps correcting late resizes for a short time.

## What was tested

- **GNOME Shell 46, Wayland.** The ready-wait, the cloak and placement were measured on one machine
  with the project's own test window ([Findings](/spawn-at/findings/)). The 0.2.0-beta.4 release notes state that
  the maintainer re-verified GNOME Wayland behaviour live on GNOME 46, Wayland, one monitor
  (`docs/RELEASE_NOTES_0.2.0-beta.4.md`, first paragraph). No log of that run is in the repository.
- **Real applications**, a few runs each, Wayland only: `gnome-text-editor` and `nautilus` (GTK4),
  `gnome-terminal`, `zenity` and `firefox` (snap), which the README believes are GTK3 (the toolkit
  version was not checked). Under the old rule Firefox flashed in 6 of 6 runs. Chrome was never
  matched.
- **Placement arithmetic**, by unit tests and golden vectors shared with the extension's JavaScript
  tests.

## What was not

- **GNOME 42, 43 and 44.** A separate legacy extension exists. It was never run on a real GNOME 42 to
  44 and may not work at all.
- **GNOME 45 and 47 or later.** The extension's metadata lists 45 to 50; only 46 has been run in earnest (50.1 on Ubuntu 26.04 was tried once by hand and placed a window correctly). On an
  untested version the installer prints a notice first and asks before it installs (`--yes` skips the
  question).
- **GNOME on X11.** Partly tested with the test window. On X11 the installer reloads the Shell in
  place and apps stay open.
- **XWayland windows** on a Wayland session: the test window only. They use the older rule (a 30 ms
  quiet period) and can flash.
- **Multi-monitor and fractional scaling.** Untested.
- **Qt and Electron apps.** Untested.
- **The attended GNOME test specs.** The catalog specs for existing windows (`transform`, `focus`,
  `maximize`, ...) exist and have not been run on GNOME.
- **A real flash measurement by pixels.** Nobody has measured pixels on screen.

## Facts that are easy to trip on

- The extension exposes only a protocol number over D-Bus, not a content hash, so spawn-at decides
  whether a log-out is needed from files and process facts. It skips the question when the deployed
  extension is byte-identical, active, and older than the running Shell.
- Mutter keeps about 75 pixels of a window on an active monitor; see
  [Concepts](/spawn-at/concepts/).
- The extension and the Rust side must agree on a few numbers (protocol versions, ready-wait rules).
  The ready-wait exists once in Rust and once in JavaScript, tied together by shared test vectors.

## Sources

- `README.md` ("How it works", "Support")
- `docs/DESIGN_PLATFORMS.md`, sections 4, 7 and 8
- `docs/KNOWLEDGE.md` ("Architecture map", "Install and release", "GNOME and Wayland behavior")
- `docs/RELEASE_NOTES_0.2.0-beta.4.md`
- `docs/legacy-gnome.md`
- `docs/TEST_PLAN.md`, section 5
- `CHANGELOG.md` ("[0.2.0-beta.3] Added")
