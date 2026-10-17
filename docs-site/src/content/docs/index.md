---
title: "Introduction"
---

spawn-at is a command-line tool that starts an application, or moves one that is already open,
and puts its window at an exact position and size.

```bash
spawn-at spawn --anchor top-right --size 700 450 -m 24 gnome-terminal
```

That opens `gnome-terminal` at the top-right of the work area, 24 pixels in from the edges, already
sized 700 by 450. On GNOME with Wayland, the window is kept invisible until it has been placed and
the application has had time to draw at the new size, so you do not see it appear in the wrong
place and jump.

## Why it exists

Every desktop places windows its own way. On X11 any program can move any window, but only after
the window is visible, so it flashes. On Wayland an application cannot place itself, and there is no
standard way for another program to do it: each compositor has its own mechanism. spawn-at is one
command and one placement model, with a separate driver for each desktop underneath.

## What it is not

- **Not a guarantee of no flicker.** The GNOME driver removes most flashes in the measurements
  that were made, not all of them. See [Findings](/spawn-at/findings/) for the numbers and their limits.
- **Not tested widely.** Only GNOME Shell 46 on Wayland has been tested in earnest, on one machine.
  Everything else is untested or experimental. See [Environments](/spawn-at/environments/) and
  [Known shortcomings](/spawn-at/shortcomings/).
- **Not a cross-desktop tool yet.** KDE, Hyprland, macOS and Windows are not implemented. On macOS
  and Windows the program prints `spawn-at does not support this operating system yet` and exits
  with status 1.
- **Not a single binary on GNOME.** On GNOME Wayland, only the compositor can place windows, so
  spawn-at ships a small GNOME Shell extension and talks to it over D-Bus. `spawn-at install`
  deploys the extension.

## Status

spawn-at is in beta. The latest release is `v0.2.0-beta.6`, a prerelease. Prebuilt binaries are published for Linux x86_64 only. Windows and
macOS builds are compile-checked in CI and have never been run.

A binary that is not built from a release tag prints `DEV BUILD` for `spawn-at -V`.

## How to read this book

| If you want to | Read |
| :--- | :--- |
| Install it | [Install](/spawn-at/install/), then [Quick start](/spawn-at/quick-start/) |
| Understand anchors, margins and the waiting rules | [Concepts](/spawn-at/concepts/) |
| Look up a command | [Commands](/spawn-at/commands/) and the [CLI reference](/spawn-at/reference/) |
| Change a setting | [Configuration](/spawn-at/configuration/) |
| Know whether your desktop works | [Environments](/spawn-at/environments/) |
| Know what is broken or unmeasured | [Known shortcomings](/spawn-at/shortcomings/) and [Findings](/spawn-at/findings/) |
| Work on spawn-at | [Testing and the verify tool](/spawn-at/testing/), [Architecture and contributing](/spawn-at/contributing/), [API documentation](/spawn-at/rustdoc/) |

Every page ends with a list of the files in the repository it was written from. Where this book
says something is "not verified", it means no test or run in the repository covers it.

## Sources

- `README.md` (intro, "Support", "How well it works", "Roadmap")
- `CHANGELOG.md` ("[0.2.0-beta.6]", "[0.2.0-beta.5]")
- `docs/RELEASE_NOTES_0.2.0-beta.6.md`
- `Cargo.toml` (package version), `install.sh` (prebuilt target), `CLAUDE.md` ("Versions and releases")
- `docs/DESIGN_PLATFORMS.md`, section 5 (macOS and Windows are not implemented)
