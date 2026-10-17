---
title: "Requirements"
---

What the machine needs, read from the scripts, the source and the extension metadata. This page says
what the code requires, not what has been tested; for that see [Environments](/spawn-at/environments/)
and the Support table in the README. Nothing was installed or run to write it.

## Platform

| Need | Evidence |
| :--- | :--- |
| Linux on x86_64 for the prebuilt package. `install.sh` stops with an error on any other OS or architecture and suggests building from source | `install.sh:43-50` |
| Linux for the binary at all. Any other OS prints `spawn-at does not support this operating system yet` and exits 1 | `src/platform/mod.rs:433-439` |
| To build from source: a Rust toolchain. `Cargo.toml` declares edition 2021 and no minimum Rust version (`grep -n "rust-version" Cargo.toml` prints nothing) | `Cargo.toml:4` |

## Programs the scripts and the binary run

These are looked up in `PATH` when they are needed. None is checked ahead of time.

| Program | Used by | What happens if it is missing | Source |
| :--- | :--- | :--- | :--- |
| `bash` | `install.sh` (`#!/usr/bin/env bash`) | the script does not run | `install.sh:1` |
| `curl` | `install.sh` (every download) and `spawn-at update` (release lookup and download) | the step fails; `update` prints `Failed to execute curl` or `Download failed` | `install.sh`, `src/update.rs:176, 202, 616` |
| `tar` | `install.sh` and `spawn-at update` (unpacking) | the step fails | `install.sh:409`, `src/update.rs:630` |
| `jq` | `install.sh`, optional | the script falls back to `grep` and `sed` | `install.sh:233-241` |
| `gnome-extensions` | `install` (enable) and `uninstall` (disable) on GNOME; also read by the log-out check | `install` stops with `Failed to execute 'gnome-extensions enable'`; `uninstall` ignores a missing program | `src/platform/linux/gnome/deploy.rs:329, 400`, `logout.rs:168` |
| `gnome-shell` | `install`, to read the Shell's major version (`gnome-shell --version`) | the version is unknown: the notice says so and the modern extension files are used | `src/platform/linux/gnome/detect.rs:19` |
| `loginctl`, `id` | the Wayland-or-X11 test | falls back to `WAYLAND_DISPLAY` being set | `detect.rs:52-136` |
| `gnome-session-quit` | the log-out after `install`, `uninstall` or `update` on Wayland, only if you agree or pass `--gnome-wayland-logout` | a failure to start it is ignored; you get no message | `deploy.rs:134-155` |
| `xdotool` | the Shell reload after `install` on an X11 session (it types `Alt+F2`, `r`, `Return`) | prints `xdotool is not installed...` and does not reload | `deploy.rs:164-196` |
| `sudo` | only as a fallback when copying or removing the binary is refused with "permission denied" (for example `--scope system`) | the step fails with `Failed to invoke sudo` | `src/platform/escalate.rs` |

## GNOME Shell versions

The extension has two file sets, chosen by the Shell's major version: the legacy set when it is
below 45, the modern set otherwise, **and also when the version cannot be read**
(`src/platform/linux/gnome/deploy.rs:251-256`). Each set has a `metadata.json` that tells GNOME
which Shell versions it may load on.

| GNOME Shell | File set | `shell-version` in its metadata | Notice at install | Status in the README |
| :--- | :--- | :--- | :--- | :--- |
| 42, 43, 44 | legacy (9 files) | `42`, `43`, `44` (`assets/gnome/legacy/metadata.json`) | "legacy extension ... UNTESTED" | untested |
| 45 | modern (11 files) | `45`, `46`, `47`, `48` (`assets/gnome/modern/metadata.json`) | "is UNTESTED" | untested |
| 46 on Wayland | modern | listed | none | tested |
| 46 on X11 | modern | listed | "only PARTLY tested" | partly tested (test window only) |
| 47, 48 | modern | listed | "is UNTESTED" | untested |
| 49 and later, or unreadable | modern | **not listed**; whether GNOME loads it is not verified | "is UNTESTED" or "could not be read" | untested |

The notice texts are in `src/platform/install_notice.rs:19-37`. `--yes`, `--headless`, or no terminal
make the install go on after printing it.

## Which desktops are accepted

`spawn-at` refuses to run a command (including `install` and `uninstall`) on a session that is none
of these (`src/platform/linux/registry.rs:14-20, 83-108`):

1. `SPAWN_AT_BACKEND` set to `x11` or `sway` (any letter case);
2. `XDG_CURRENT_DESKTOP` containing `GNOME`;
3. `XDG_SESSION_TYPE` equal to `x11`, on any desktop.

The refusal is the message `this desktop is not supported (XDG_CURRENT_DESKTOP is ..., XDG_SESSION_TYPE is
...)`, exit status 1. See [Unsupported desktops](/spawn-at/environments/unsupported-desktops/).
`update`, `config` and `--version` do not need a backend and still work.

| Session | Driver the code picks | Source |
| :--- | :--- | :--- |
| `SPAWN_AT_BACKEND=x11` | X11 only | `registry.rs:5-7, 110-113` |
| `SPAWN_AT_BACKEND=sway` | Sway only (experimental); needs `$SWAYSOCK` | `registry.rs:8-9, 113-116`, [Sway](/spawn-at/environments/sway/) |
| `XDG_CURRENT_DESKTOP` contains `GNOME` (Wayland or X11) | the GNOME driver, which talks to the extension over the session D-Bus; if it cannot connect, X11 | `registry.rs:10-11, 116-119` |
| any other X11 session | X11 | `registry.rs:12, 83-108, 120` |
| any other Wayland session, or no session type | refused | `registry.rs:83-108` |

On GNOME Wayland the extension must be loaded by the Shell, which on Wayland happens at the next
log-in (see [Install](/spawn-at/install/)).

## Not verified

- No missing-program case was run; the "what happens" column is read from the source.
- That GNOME 49 or later loads an extension whose metadata does not list it.
- GNOME versions other than 46, X11 sessions with real applications, and any non-x86_64 machine.

## Sources

- `install.sh`, `Cargo.toml`
- `src/platform/mod.rs:433-447`, `src/platform/linux/registry.rs`, `src/platform/install_notice.rs`
- `src/platform/linux/gnome/deploy.rs`, `detect.rs`, `logout.rs`, `src/platform/escalate.rs`, `src/update.rs`
- `assets/gnome/legacy/metadata.json`, `assets/gnome/modern/metadata.json`
- `README.md` ("Support")
