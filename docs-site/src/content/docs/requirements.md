---
title: "Requirements"
---

What the machine needs, read from the scripts, the source and the extension metadata. This page says
what the code requires, not what has been tested; for that see [Environments](/spawn-at/environments/)
and the Support table in the README. Nothing was installed or run to write it.

## Check your machine

Two commands show what the installer will see. Both are standard programs, and the installer reads
the same things (the first is `gnome-shell --version`, the second is the two variables of the
desktop test below):

```bash
gnome-shell --version
echo "$XDG_CURRENT_DESKTOP $XDG_SESSION_TYPE"
```

What you want: a Shell major version in the table below, and a desktop name containing `GNOME` (or
`x11` as the session type). Which output counts as tested and which is refused is in the two tables
below; the output of these commands was not captured for this page. If the desktop is refused, see
[the unsupported-desktop entry](/spawn-at/troubleshooting/#this-desktop-is-not-supported).

## Platform

| Need | Detail |
| :--- | :--- |
| Linux on x86_64 for the prebuilt package | `install.sh` stops with an error on any other OS or architecture and suggests building from source |
| Linux for the binary at all | Any other OS prints `spawn-at does not support this operating system yet` and exits 1 |
| To build from source: a Rust toolchain | `Cargo.toml` declares edition 2021 and no minimum Rust version (`grep -n "rust-version" Cargo.toml` prints nothing) |

## Programs the scripts and the binary run

These are looked up in `PATH` when they are needed. None is checked ahead of time.

| Program | Used by | What happens if it is missing |
| :--- | :--- | :--- |
| `bash` | `install.sh` (`#!/usr/bin/env bash`) | the script does not run |
| `curl` | `install.sh` (every download) and `spawn-at update` (release lookup and download) | the step fails; `update` prints `Failed to execute curl` or `Download failed` |
| `tar` | `install.sh` and `spawn-at update` (unpacking) | the step fails |
| `jq` | `install.sh`, optional | the script falls back to `grep` and `sed` |
| `gnome-extensions` | `install` (enable) and `uninstall` (disable) on GNOME; also read by the log-out check | `install` stops with `Failed to execute 'gnome-extensions enable'`; `uninstall` ignores a missing program |
| `gnome-shell` | `install`, to read the Shell's major version (`gnome-shell --version`) | the version is unknown: the notice says so and the modern extension files are used |
| `loginctl`, `id` | the Wayland-or-X11 test | falls back to `WAYLAND_DISPLAY` being set |
| `gnome-session-quit` | the log-out after `install`, `uninstall` or `update` on Wayland, only if you agree or pass `--gnome-wayland-logout` | a failure to start it is ignored; you get no message |
| `xdotool` | the Shell reload after `install` on an X11 session (it types `Alt+F2`, `r`, `Return`) | prints `xdotool is not installed...` and does not reload |
| `sudo` | only as a fallback when copying or removing the binary is refused with "permission denied" (for example `--scope system`) | the step fails with `Failed to invoke sudo` |

## GNOME Shell versions

The extension has two file sets, chosen by the Shell's major version: the legacy set when it is
below 45, the modern set otherwise, **and also when the version cannot be read**. Each set has a
`metadata.json` that tells GNOME which Shell versions it may load on.

| GNOME Shell | File set | `shell-version` in its metadata | Notice at install | Status in the README |
| :--- | :--- | :--- | :--- | :--- |
| 42, 43, 44 | legacy (9 files) | `42`, `43`, `44` | "legacy extension ... UNTESTED" | untested |
| 45 | modern (11 files) | `45` to `50` | "is UNTESTED" | untested |
| 46 on Wayland | modern | listed | none | tested |
| 46 on X11 | modern | listed | "only PARTLY tested" | partly tested (test window only) |
| 47, 48, 49 | modern | listed | "is UNTESTED" | untested |
| 50 on Wayland | modern | listed | "only PARTLY tested (one run on 50.1: ...)" with the counts | partly tested, with failures (9 of 36 required cases failed on 50.1) |
| 50 on X11, 51 and later, or unreadable | modern | 50 is listed; 51 and later are **not listed** (whether GNOME loads them is not verified) | "is UNTESTED" or "could not be read" | untested |

`--yes`, `--headless`, or no terminal make the install go on after printing the notice.

## Which desktops are accepted

`spawn-at` refuses to run a command (including `install` and `uninstall`) on a session that is none
of these:

1. `SPAWN_AT_BACKEND` set to `x11` or `sway` (any letter case);
2. `XDG_CURRENT_DESKTOP` containing `GNOME`;
3. `XDG_SESSION_TYPE` equal to `x11`, on any desktop.

The refusal is the message `this desktop is not supported (XDG_CURRENT_DESKTOP is ..., XDG_SESSION_TYPE is
...)`, exit status 1. See [Unsupported desktops](/spawn-at/environments/unsupported-desktops/).
`update`, `config` and `--version` do not need a backend and still work.

| Session | Backend the code picks |
| :--- | :--- |
| `SPAWN_AT_BACKEND=x11` | X11 only |
| `SPAWN_AT_BACKEND=sway` | Sway only (experimental); needs `$SWAYSOCK`. See [Sway](/spawn-at/environments/sway/) |
| `XDG_CURRENT_DESKTOP` contains `GNOME` (Wayland or X11) | the GNOME backend, which talks to the extension over the session D-Bus; if it cannot connect, X11 |
| any other X11 session | X11 |
| any other Wayland session, or no session type | refused |

On GNOME Wayland the extension must be loaded by the Shell, which on Wayland happens at the next
log-in (see [Install](/spawn-at/install/)).

## Not verified

- No missing-program case was run; the "what happens" column is read from the source.
- That GNOME 49 or later loads an extension whose metadata does not list it.
- The output of the two commands under "Check your machine" was not captured.
- GNOME versions other than 46, X11 sessions with real applications, and any non-x86_64 machine are
  on [Known shortcomings](/spawn-at/shortcomings/#not-verified-anywhere).

## Sources

- `install.sh`: lines 43-50 (platform check), 1 (`#!/usr/bin/env bash`), 233-241 (`jq` fallback),
  409 (`tar`)
- `Cargo.toml`: line 4 (edition)
- `src/platform/mod.rs`: lines 433-447 (the operating-system refusal)
- `src/platform/linux/registry.rs`: lines 5-20 and 83-120 (the choice rules and the refusal)
- `src/platform/install_notice.rs`: lines 19-37 (the notice texts)
- `src/platform/linux/gnome/deploy.rs`: lines 251-256 (file set by major version), 329 and 400
  (`gnome-extensions`), 134-155 (`gnome-session-quit`), 164-196 (`xdotool`)
- `src/platform/linux/gnome/detect.rs`: line 19 (`gnome-shell --version`), lines 52-136
  (`loginctl`, `id`, `WAYLAND_DISPLAY`); `logout.rs`: line 168
- `src/platform/escalate.rs` (`sudo`); `src/update.rs`: lines 176, 202, 616 (`curl`), 630 (`tar`)
- `assets/gnome/legacy/metadata.json`, `assets/gnome/modern/metadata.json`
- `README.md` ("Support")
