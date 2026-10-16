# Environments

What works where. "Tested" here means a person ran it on one machine, not that it works for
everyone. The exact limits of each row are on its own page.

| Environment | Status |
| :--- | :--- |
| [GNOME 46, Wayland session](gnome-wayland.md) | **Tested**, on one machine with one monitor |
| GNOME 46, X11 session | Partly tested: the project's test window only, no real apps |
| XWayland windows on a GNOME Wayland session | Test window only. They use the older, shorter wait, so they flash more |
| GNOME 42, 43, 44 (the legacy extension) | **Untested**; never run on a real GNOME 42 to 44 |
| GNOME 45 and 47 or later | Untested. The extension's metadata lists 45 to 48, but only 46 has been run |
| [Sway](sway.md) | **Experimental**, opt-in with `SPAWN_AT_BACKEND=sway`. Run only against a private headless Sway 1.9 with the test window |
| [X11 and XWayland (fallback)](x11-xwayland.md) | Experimental driver, untested with real apps. No cloaking, so windows flash |
| [Other Wayland desktops](unsupported-desktops.md) (KDE, Hyprland, unknown) | Not supported: spawn-at exits with an error unless you opt in to the X11 fallback |
| macOS, Windows | Not implemented. The code compiles for them in CI and has never been run |
| Multi-monitor, fractional scaling | Untested on GNOME |

## How spawn-at picks a driver

On Linux, in this order:

1. `SPAWN_AT_BACKEND=x11` or `SPAWN_AT_BACKEND=sway` (any letter case, nothing trimmed) forces that
   driver. Forcing Sway without a usable `$SWAYSOCK` is an error; it does not fall back.
2. If `XDG_CURRENT_DESKTOP` contains `GNOME` and the D-Bus connection works, the GNOME driver. A
   GNOME session whose D-Bus connection fails falls back to the X11 driver.
3. A real X11 session on any desktop gets the X11 driver.
4. Anything else, a non-GNOME Wayland session or an unknown session type, is an error that says the
   desktop is not supported.

Sway is never chosen automatically.

## What has not been verified anywhere

- GNOME versions other than 46.
- Real Sway sessions, and Sway versions other than 1.9.
- X11 or XWayland with real applications.
- More than two outputs, and output scales other than 1 and 2 (on Sway).
- Windows and macOS: only `cargo check` was run.
- Removal at system scope (`--scope system`).

## Sources

- `README.md` ("Support", "Sway (experimental)")
- `docs/KNOWLEDGE.md` ("Architecture map")
- `docs/DESIGN_PLATFORMS.md`, section 3 row 11 (backend choice)
- `docs/DESIGN_SWAY.md`, sections 7 and 12
- `docs/TEST_PLAN.md`, section 7.3 ("Not verified")
- `docs/RELEASE_NOTES_0.2.0-beta.4.md` ("What was NOT verified")
