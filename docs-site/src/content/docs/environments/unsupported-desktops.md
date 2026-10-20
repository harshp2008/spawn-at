---
title: "Unsupported desktops"
---

spawn-at supports GNOME. On a Wayland desktop that is not GNOME, such as KDE or Hyprland, or when
the session type is unknown, it refuses to run instead of silently switching to the X11 backend,
which on Wayland would only see XWayland windows.

## What you see

Run with `XDG_CURRENT_DESKTOP=KDE` and `XDG_SESSION_TYPE=wayland` (set on the child process only, in
a throwaway home directory), `spawn-at query layout` printed this and exited with status 1:

```text
[spawn-at] ERROR: Driver initialization error: this desktop is not supported (XDG_CURRENT_DESKTOP is 'KDE', XDG_SESSION_TYPE is 'wayland'). spawn-at supports GNOME; Sway is experimental, via SPAWN_AT_BACKEND=sway. To try the X11 fallback anyway, set SPAWN_AT_BACKEND=x11 (it may or may not work).
```

The same error comes from `install` and `uninstall`, which change nothing.

## What counts as unsupported

The check is made when the backend is chosen. A session is refused when **all** of these hold:

- `SPAWN_AT_BACKEND` is not `x11` or `sway`;
- `XDG_CURRENT_DESKTOP` does not contain `GNOME`;
- `XDG_SESSION_TYPE` is not `x11` (it is `wayland`, or unset).

Not refused: a GNOME session, a real X11 session on any desktop, and a forced `x11` or `sway`.

## Trying the X11 fallback anyway

```bash
SPAWN_AT_BACKEND=x11 spawn-at spawn --anchor center -- <your program>
```

It sees XWayland windows only, has no cloak, and the error says it "may or may not work". The
details are on [X11 and XWayland](/spawn-at/environments/x11-xwayland/). If your desktop is Sway, use the experimental
Sway backend instead: [Sway](/spawn-at/environments/sway/).

## The installer

`install.sh` asks the same question before it downloads anything: it prints that the desktop is not
supported yet and asks `Install anyway? [y/N]`. Without a terminal it stops and tells you to re-run
with `--allow-unsupported`. If you go on, spawn-at needs `SPAWN_AT_BACKEND=x11` to run on that
desktop. See [Install](/spawn-at/install/).

## What still works

`spawn-at update` and `spawn-at config` need no backend, so they run on any desktop. The login
update check (`update --check --headless`) works there too and exits 0 without printing the error.

## Other operating systems

On macOS and Windows spawn-at prints `spawn-at does not support this operating system yet` and exits
with status 1. The code compiles for them in CI; it has never been run there.

## Sources

- `CHANGELOG.md` ("[0.2.0-beta.5] Changed": unsupported desktops, `install.sh`; "[0.2.0-beta.3] Changed")
- `docs/DESIGN_PLATFORMS.md`, section 3, Leak 11
- `docs/KNOWLEDGE.md` ("Architecture map")
- `install.sh` (lines 91-130)
- `tests/unsupported_desktop_cli_test.rs`
- the error text was produced by running `spawn-at query layout` with those variables set
