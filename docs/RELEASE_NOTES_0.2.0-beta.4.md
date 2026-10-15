# spawn-at 0.2.0-beta.4: release notes

This is a beta. **On GNOME Wayland nothing visible changes.** Window placement, the ready-wait policy, the GNOME extension (`assets/gnome/`) and `install.sh` are byte-identical to 0.2.0-beta.3 (`git diff v0.2.0-beta.3..HEAD -- assets install.sh` is empty). GNOME Wayland behavior was re-verified live by the maintainer on GNOME 46, Wayland, one monitor, the only configuration ever tested. The known limits of beta.1 to beta.3 still apply: read [RELEASE_NOTES_0.2.0-beta.1.md](RELEASE_NOTES_0.2.0-beta.1.md), "What is measured, and the caveats" and "What is untested".

## What is new

- **Internal restructuring (no visible change).** The code that talks to the desktop is split into small parts, so other desktops and operating systems can be added later. Commands, flags, config keys, output and exit codes are the same. Windows and macOS are still not supported (running spawn-at there says so and exits); the crate is now checked in CI to compile for them.
- **`spawn-at update` asks for the download of its own platform.** The release file is chosen by the platform the program was built for instead of being fixed. On Linux x86-64 this is the same file as before.
- **Experimental Sway backend, off by default.** Set `SPAWN_AT_BACKEND=sway` inside a Sway session to place windows through Sway's IPC socket: `spawn` with anchors, margins, `--size` and `--monitor`, plus `query layout`, `query windows`, `transform`, `close` and `focus`. Nothing needs installing. It is **never chosen automatically**: a Sway session without the variable gets exactly what it got before (X11 through XWayland), and so does every other desktop. If the variable is set and `$SWAYSOCK` is missing or not a Sway socket, spawn-at stops with an error naming `$SWAYSOCK`; it does not fall back to X11. See the README section "Sway (experimental)" and [DESIGN_SWAY.md](DESIGN_SWAY.md).

## Sway: limitations

- **A window can show for one frame** at its tiled position before it is placed. Sway cannot hold a new window invisible, and the flash has not been measured.
- Not supported: `--anchor cursor` and `--monitor cursor`, maximize, minimize, defocus. No i3. `--monitor` counts outputs in the order Sway lists them.
- A window Sway will not make as small as asked is placed using its real size. If no window of the launched program appears within 5 seconds, spawn-at warns that it was started but not placed.
- Tested only in a private headless Sway 1.9 with the repo's own test window (62 live checks, plus unit tests). Not tested: a real Sway session, real applications, XWayland windows, other Sway versions, `--area screen`/`unbounded`, more than two outputs.

## How to install

```bash
# v0.2.0 is in beta, so --prerelease is needed for now
curl -fsSL https://raw.githubusercontent.com/harshp2008/spawn-at/main/install.sh | bash -s -- --prerelease

# or pin this version
curl -fsSL https://raw.githubusercontent.com/harshp2008/spawn-at/main/install.sh | bash -s -- --version v0.2.0-beta.4
```

## Upgrade notes

- No extension reinstall is needed: no file under `assets/gnome/` changed since beta.3.
- No command-line flag, config key, exit code or D-Bus member changed. The Sway backend adds one accepted value (`sway`) of the existing `SPAWN_AT_BACKEND` variable, which was ignored for that value before.

## Tests (this tree)

| Suite | Passed |
| :-- | --: |
| `spawn-at` | 380 |
| `spawn-at-core` | 64 |
| `xtask` | 10 |
| `spawn-at-verify` (`--features tui,fixture`) | 404, 4 ignored (they need a live display) |
| Rust total | 858, 4 ignored |
| Node (`node --test tests/*.js`) | 183 |

## What was NOT verified

- Everything listed under Sway above.
- GNOME versions other than 46 on Wayland, and X11 sessions with real applications.
- Windows and macOS: the crate compiles for them, nothing was run.
- `spawn-at update` choosing the asset on a platform other than Linux x86-64.
- CI and the release workflow on the beta.4 tag are untested until the tag is pushed.

## How to report a problem

Open an issue at <https://github.com/harshp2008/spawn-at/issues> with your GNOME Shell (or Sway) version, session type, `spawn-at config show`, the exact command, and what was printed.
