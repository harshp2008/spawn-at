# Changelog

All notable changes to this project are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and versions follow
[Semantic Versioning](https://semver.org/).

## [Unreleased]

## [0.2.0-beta.1] - unreleased

Pre-release. Tested on one machine only: GNOME 46, Wayland, one monitor, with the project's own
test window. See [docs/RELEASE_NOTES_0.2.0-beta.1.md](docs/RELEASE_NOTES_0.2.0-beta.1.md) for the
caveats and [docs/TEST_PLAN.md](docs/TEST_PLAN.md) (sections 6.22 to 6.24) for the measurements.

### Added

- **Ready-wait (GNOME 46, Wayland).** The extension keeps a resized window hidden until the app
  has answered the size request or a cap runs out, instead of revealing it after a 30 ms quiet
  period. Hard limit 1500 ms. In our measurements under heavy CPU load (22 busy threads) the
  old rule flashed in 26 of 40 runs and the ready-wait in 0 of 40, by Wayland wire time; idle,
  neither flashed in 20 runs. Wire time is an optimistic bound and no screen capture was done.
- **`spawn-at config`** with the commands `show`, `get`, `set`, `unset`, `reset` and `path`, and
  these settings (see [docs/CONFIG.md](docs/CONFIG.md)):
  - `wait.cap_ms` (1 to 1500): the longest the extension waits for an app to answer a size request.
  - `wait.strategy` (`auto`, `legacy`, `ready-wait`): which rule waits; `legacy` is the pre-0.2 rule.
  - `spawn.window_timeout_ms` (200 to 60000, default 2000): how long `spawn` waits for the window to appear.

  A command-line flag beats the file, and the file beats the default. `SPAWN_AT_CONFIG=<path>`
  names a different file.
- **Installer notice on untested GNOME versions.** On a GNOME version other than 46 the installer
  prints what is and is not tested and asks for confirmation (`--yes` skips the question; headless
  runs print the notice and continue).
- **Observe-only X11/XWayland logging** in the extension, active only while logging is switched
  on. It logs the paint signals of X11 windows and changes no timing, order or outcome.
- Test harness: a flash check that reads the client's commit time from the Wayland protocol,
  hostile-app fixture flags (slow, refusing, grid-snapping, flapping, stalled, hung) and nine
  catalog cells that use them.

### Changed

- **Default `wait.cap_ms` is 250 ms** (it was 150 ms in development builds). The extension's own
  default applies when nothing is set; an explicit value is sent with the spawn.
- Under load the window is hidden longer with the ready-wait: median 67 ms against 45 ms for
  the old rule, 95th percentile 105 ms against 53 ms. Idle there is no difference (19.5 and 19.6 ms).
- An app that is already at its minimum size, or that never answers, makes the ready-wait run
  for the full cap on every spawn (about +230 ms in our test with the cap at 250 ms).
- The four crates are versioned 0.2.0-beta.1. The version the binary reports and the update
  check use comes from the git tag at build time, not from `Cargo.toml`.

### Fixed

- **Login autostart entry.** `~/.config/autostart/spawn-at-update-check.desktop` had
  `Exec=spawn-at`, which is not found at login because `~/.local/bin` is not on `PATH` there. The
  installer now writes the absolute path of the binary it installed, quoted as the Desktop Entry
  specification requires. Run `spawn-at install` again to rewrite an existing file.
- `cargo clippy --workspace --all-targets -- -D warnings` passes (mechanical fixes, no behaviour change).

### Known limitations

- **XWayland and X11 windows still use the old 30 ms rule and can flash.** We looked for a signal
  that says an X11 client has painted at its new size and found none that works. Tried and not
  usable, all measured on the test window only: the first `damaged` event, `first-frame`, the actor
  size, the buffer rect, the surface texture size, holding the cloak and waiting for a second
  `damaged`, and resizing before the map. They fire at the shell's resize or at the map, not at the
  client's paint (for example the texture size reached the target before the client painted in 59 of
  80 fixture runs). A window ping has no reply API (`Meta.Window.check_alive` has no reply signal).
  A pixel probe could tell a blank buffer from the painted one on the test window, but it crashed
  GNOME Shell in testing and was removed. Still untried: the X sync counter (client-side, partial
  toolkit support) and a screen-capture measurement rig (the Mutter screencast stream could not be
  consumed; the portal needs a user prompt). See [docs/DESIGN_READY_WAIT.md](docs/DESIGN_READY_WAIT.md),
  section 12.
- On GNOME 46 the extension cannot read an app's reported minimum size (it reads 0x0), so it learns
  a minimum only by waiting.
- Untested: real X11 and XWayland applications, GNOME 42 to 45 and 47 to 48, Qt and Electron apps,
  multi-monitor and fractional scaling, and any screen-capture check of what is visible.

[Unreleased]: https://github.com/harshp2008/spawn-at/compare/v0.2.0-beta.1...HEAD
[0.2.0-beta.1]: https://github.com/harshp2008/spawn-at/releases/tag/v0.2.0-beta.1
