# Changelog

All notable changes to this project are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and versions follow
[Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- On GNOME Wayland, `spawn-at install` no longer asks about (or warns about) a log-out when none is needed: the extension about to be deployed is byte-identical to the deployed one (same file set), `gnome-extensions info` reports it ACTIVE, and every deployed file is older than the running gnome-shell process. It prints a green "No log-out needed" line instead. If any of these facts cannot be read, the question and warning stay. Files that are already identical are no longer rewritten, so their modification time keeps meaning "installed before this Shell started". X11 is unchanged (takes effect in the next release).
- `spawn-at install` detects an existing spawn-at binary (user location, `/usr/local/bin`, or on PATH; never the running binary) and tells you where it is and its version (`<existing> -V`, 2 s timeout, failure tolerated) next to the version being installed. In a terminal it asks: keep and upgrade in place (default), or clean install; for the same version the default changes nothing, for a newer installed version it warns about the downgrade and the default changes nothing. Without a terminal, or when the prompt cannot be shown, nothing is uninstalled and the install behaves as before.
- `spawn-at install --clean-install` removes the existing install first (binaries, the extension, the login update-check entry; the same removal as `spawn-at uninstall`) and then installs, without asking, with or without a terminal. Your configuration file and settings are never deleted. With no existing install it prints that there is nothing to remove. `install.sh` passes the flag on (`... | bash -s -- --clean-install`) (takes effect in the next release).

### Changed

- `spawn-at install` and `spawn-at uninstall` output (and the update manager heading): one blank line before and after each `=== ... ===` section, no double blank lines, none at the start or end (takes effect in the next release). The section headings are plain text when stdout is not a terminal or `NO_COLOR` is set.
- The Wayland line "If the extension is not immediately active, log out and back in to complete loading." is printed in bold yellow, only on a terminal with `NO_COLOR` unset; the text is unchanged and it stays plain when piped (takes effect in the next release).

### Fixed

- `install.sh` shows curl's progress bar for the tarball download when stderr is a terminal, announces each retry of a stalled or failed download ("attempt 1 of 4 timed out or stalled, retrying in 2s..."), and prints the elapsed time when a download took longer than 5 seconds. The bar is not drawn for a failed HTTP status (a pinned version that does not exist no longer shows 100% before the error). Without a terminal (logs, CI, redirects) there is no bar and no retry text.
- `install.sh` errors share one layout on stderr (blank line, red headline, indented causes, "What you can do:" with indented steps, blank line), including the unsupported-platform, `--version` and invalid-tag errors, which now also say what to do; the phases (resolving, downloading, extracting, launching) are separated by single blank lines. Wording and exit codes are otherwise unchanged.

## [0.2.0-beta.2] - 2026-10-14

Pre-release. Placement behavior, the ready-wait policy and the extension are the same as in
0.2.0-beta.1: no file under `assets/gnome/` changed. See
[docs/RELEASE_NOTES_0.2.0-beta.2.md](docs/RELEASE_NOTES_0.2.0-beta.2.md) for upgrade notes and
what is not verified.

### Changed

- **The login update-check autostart entry now honours `XDG_CONFIG_HOME`**, the same way the
  config file does: with an absolute `XDG_CONFIG_HOME` it is written to
  `$XDG_CONFIG_HOME/autostart/spawn-at-update-check.desktop`; with it unset the path is
  unchanged (`~/.config/autostart/...`). Before, it was always under `~/.config`, so a desktop
  session using `XDG_CONFIG_HOME` did not see it. An entry an earlier version wrote under
  `~/.config/autostart` is not moved: re-run `spawn-at install` to write the new one.
- The README no longer calls the X11 driver "finished": the intro now says what the rest of the
  README already said (X11 and XWayland keep the old 30 ms rule, can flash, and were tried with
  the test window only), and the long X11/XWayland paragraph is a short summary that links to
  [docs/TEST_PLAN.md](docs/TEST_PLAN.md) and [docs/DESIGN_READY_WAIT.md](docs/DESIGN_READY_WAIT.md).

### Fixed

- **`install.sh` resolves releases reliably.** "Could not resolve latest release via GitHub API"
  appeared intermittently because every API call hid its error and had no retry. Now: API calls
  use `--connect-timeout 10 --max-time 60 --retry 3 --retry-delay 2` and send
  `Authorization: Bearer $GITHUB_TOKEN` when it is set; if the API fails, `--prerelease` reads the
  first entry of `releases.atom` and the stable channel follows the redirect of `releases/latest`;
  the tarball always comes from `releases/download/<tag>/<asset>`, never the API; a pinned tag
  without a leading `v` is retried with one; tags are validated before they reach a URL; the
  tarball is extracted in a temporary directory that is removed afterwards. Failures print the
  cause (HTTP status, rate limit, timeout, no network) and what to do. The two `python3 -c`
  fallbacks were removed (`jq` is still used when present, otherwise `grep` and `sed`).
  `SPAWN_AT_RESOLVE_ONLY=1` prints the resolved tag and exits. All flags are unchanged. This
  landed on `main` after the beta.1 tag, so it is part of a tagged release for the first time here.
- `install.sh` sends the unsupported-platform error and the "package does not contain
  `spawn-at`" error to stderr.
- The release workflow now fails if a file it packages (`README.md`, `LICENSE`) is missing,
  instead of silently publishing a tarball without it, and checks that the tarball lists
  `./spawn-at`. It had not run on a real tag when this was written.

### Added

- **CI** (`.github/workflows/ci.yml`) on pull requests and pushes to `main`: `cargo fmt --check`,
  clippy with `-D warnings` on the whole workspace, the tests of `spawn-at`, `spawn-at-core`,
  `xtask` and `spawn-at-verify` (`--features tui,fixture`), and the Node tests. Linux x86_64
  only; the 4 tests that need a live display stay ignored. It had not run on GitHub when this
  was written.
- The `hostile` category (9 specs) is part of the `full` and `release-gate` verification suites.
  All 9 are `required = false`, so the gate's required set is unchanged.

### Internal (no user-visible change)

- Plain `cargo build` / `cargo test` at the repository root are explicitly limited to the
  `spawn-at` package (`default-members = ["."]`); this was already their behavior. Name
  `spawn-at-verify` explicitly (`-p spawn-at-verify --features tui,fixture`).
- `scripts/smoke-live.sh` no longer fails on its build step (`no bin target named
  spawn-at-verify`).
- Removed the unused `tokio` dependency of `spawn-at-verify`, the unused `get_active_driver`,
  `EMBEDDED_EXTENSION_FILES` and the re-exports `apply_focus_policy`, `run_unminimize`, `Atoms`
  and `Entry`, the never-constructed `ExpectedState::{Mapped, Position, Size}`, the never-read
  `GitHubRelease::name` field, and ten unreferenced `pub` helpers of `spawn-at-verify`.
  Test-only helpers are now `#[cfg(test)]` and the `spawn-at` modules are private, so the compiler
  reports dead code; six remaining dead-code warnings carry an explained `#[allow(dead_code)]`.
  `spawn-at --help` and every subcommand's help are byte-identical to before.
- Comments the code contradicted were corrected (`src/main.rs`, `lifecycle.rs`, the verify trace
  parser, one node test, `verify/hostile/README.md`).

### Documentation

- Broken paths fixed in `CONTRIBUTING.md`; its "adding a driver" guide now points at the real
  traits. `docs/CODE_LAYOUT.md` now describes the current tree (the never-built reorganization
  plan was removed), `AI_DEV/` was removed, and `CLAUDE.md` lists the real build and test commands.
- `docs/TEST_PLAN.md`: the 23 catalog rows without a spec file are marked "Not written", and the
  dated log (sections 6.10 to 6.35) moved to `docs/TEST_LOG.md` with a stub at each old heading.
  Docs now say that cited `scratch/` and `verify-results/` files are not in the repository.

## [0.2.0-beta.1] - 2026-10-14

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
- **Placement model.** `--area workarea|screen|unbounded`, `--offset/-o` (an alias of `--pos/-p`),
  `--pivot`, `--clamp`, per-side margins and `--monitor`. `--area unbounded` uses global desktop
  coordinates and rejects margins, clamp, `--monitor` and every anchor except `cursor`. `query windows`
  reports `GEOMETRY` in global screen coordinates; `query layout --json` has `schema_version` 1
  (monitors, work areas, insets). See the README "Coordinate Model".
- **New commands and flags:** `close` (graceful close of a window), the `--id` selector (numeric
  window id from `query windows`), `spawn --json` (one JSON object on stdout; diagnostics stay on
  stderr), and `install --yes` / `--dry-run`.
- **D-Bus protocol 6 (modern extension, GNOME 45 to 48) and 5 (legacy extension, GNOME 42 to 44).**
  New over the old extension: the `ProtocolVersion` property, `DisarmSpawn`, `GetLayout`,
  `CloseWindow`, and the `SpawnClaimed` and `ResizeOutcome` signals (`ResizeOutcome` is protocol 6
  only). The older methods are still there. The CLI warns when the loaded extension reports a protocol
  older than 3.
- **A legacy extension tree for GNOME 42 to 44** (`assets/gnome/legacy`), installed on those versions;
  the modern tree is installed on 45 and newer. The legacy tree has never been run on a real GNOME 42
  to 44.
- **`XDG_CONFIG_HOME` is honoured** for the config file (`$XDG_CONFIG_HOME/spawn-at/config.toml`, else
  `~/.config/spawn-at/config.toml`); before it was always `~/.config/spawn-at/config.toml`.
- **Installer notice on untested GNOME versions.** On a GNOME version other than 46 the installer
  prints what is and is not tested and asks for confirmation (`--yes` skips the question; headless
  runs print the notice and continue).

### Changed

- **Default `wait.cap_ms` is 250 ms** (it was 150 ms in development builds). The extension's own
  default applies when nothing is set; an explicit value is sent with the spawn.
- Under load the window is hidden longer with the ready-wait. The median was 66.8 ms against 45.3 ms
  for the old rule in one measurement and 56.2 against 48.9 ms in a second, so it varies between
  runs (95th percentile 105 against 53 ms, then 116 against 71 ms). Idle there is no difference
  (about 20 ms). In the second measurement 1 of 40 idle runs with the new rule hit the 250 ms cap
  (the app answered late); the cause is unknown.
- An app that is already at its minimum size, or that never answers, makes the ready-wait run
  for the full cap on every spawn (about +230 ms in our test with the cap at 250 ms).
- The four crates are versioned 0.2.0-beta.1. The version the binary reports and the update
  check use comes from the git tag at build time, not from `Cargo.toml`.

- **Update check.** Versions are compared as semver, and `beta` is the pre-release channel (`all` is
  a deprecated alias, see Fixed). A build that is not at an exact release tag now reports itself as
  `dev` and the update check says it is up to date; before, it was compared as the last tag.
- `spawn gnome-terminal` without `--class` now looks for a window of class `org.gnome.Terminal`
  (it was `gnome-terminal-server`). An explicit `--class` is unaffected.

### Breaking changes and upgrade notes

Checked against a build of the previous code (`pre-cleanup-beta`, `875aead`) for argument parsing,
`--help`, config loading and the update check only; no real application was launched and the
Shell was not connected to.

- **`-a` no longer means `--anchor`.** The old build accepted `spawn -a center ...`; the new one
  rejects it (`unknown option '-a'` for `spawn`, `unexpected argument '-a'` for `transform`, exit 2).
  Use `--anchor`. The short flag collided with options of the programs being spawned.
- **A word starting with `-` before the command is a usage error.** `spawn-at spawn -x true` used to
  try to run a program called `-x` (exit 1, "Failed to spawn command '-x'"); it now stops with
  `unknown option '-x'; to run a command that starts with '-', put it after --` (exit 2).
  `spawn-at spawn -- -x true` is still passed through.
- **`transform` with only a target** (`--class`, `--id`, ...) and nothing to change now stops with
  `nothing to change: give --size, -p/-o, --anchor, or --pivot with --size` (exit 2). The old build
  went on to the compositor; what it did to a real window was not tested.
- **`update --channel` takes `stable` or `beta`.** `all` still works as a deprecated alias of `beta`
  and prints one line. A config file that says `channel = "all"` is read as `beta`.
- **An old config file still loads.** A file with an `[apps]` table (v0.1 settings) is accepted;
  `spawn-at config` ignores the `apps` key with a warning, and the `[update]` settings are kept.
- **Re-run `spawn-at install` after upgrading** to rewrite the autostart entry (it now holds the
  absolute binary path) and the extension files. On Wayland, log out and back in so GNOME loads the
  new extension; until then the CLI prints a protocol mismatch warning.
- **Development builds.** The old build, when not at an exact tag, offered "an update" from its last
  tag to `v0.1.0` (checked: `pre-cleanup-beta -> v0.1.0`); the new build says `dev` is up to date.
  Release builds (built at the tag) report the tag as their version.
- **Install locations are unchanged:** the binary in `~/.local/bin` (or `/usr/local/bin` for system
  scope), the extension in `~/.local/share/gnome-shell/extensions/spawn-at@harsh.local`, the
  autostart entry `~/.config/autostart/spawn-at-update-check.desktop`.

### Fixed

- **Update channel name.** The update channels are `stable` and `beta`. `all` (the beta channel's name in 0.2.0-alpha) is a deprecated alias that is read as `beta` in the config file, in `update --channel` and in `install --update-channel`; it prints one line saying so. `install` and its prompt write only `stable` or `beta`. Before this fix, `install` could write `all` while the update check treated it as `stable`, so pre-releases were never offered.
- **Login autostart entry.** `~/.config/autostart/spawn-at-update-check.desktop` had
  `Exec=spawn-at`, which is not found at login because `~/.local/bin` is not on `PATH` there. The
  installer now writes the absolute path of the binary it installed, quoted as the Desktop Entry
  specification requires. Run `spawn-at install` again to rewrite an existing file.
- `cargo clippy --workspace --all-targets -- -D warnings` passes (mechanical fixes, no behaviour change).

### Developer notes (not user-facing)

- The extension can log the paint signals of XWayland/X11 windows (`X11_OBS_*` lines, including the
  surface texture size) while logging is switched on. It changes no timing, order or outcome and was
  used to study the X11 question described under Known limitations.
- The test harness reads the client's commit time from the Wayland protocol for its flash check, and
  the test fixture has hostile-app flags and a test-only `--state-colors` option (solid magenta at the
  initial size, green at any other).

### Known limitations

- **XWayland and X11 windows still use the old 30 ms rule and can flash.** We looked for a signal that says an X11 client has painted at its new size and found none that works for every toolkit. Tried and not usable, all measured on the test window only (TEST_PLAN 6.23 to 6.29): the first `damaged` event, `first-frame`, the actor size, the buffer rect, the surface texture size, holding the cloak and waiting for a second `damaged`, resizing before the map, and a window ping (no reply API). A pixel probe could tell a blank buffer from a painted one, but it crashed GNOME Shell in testing and was removed. Then the X sync counter (6.31 to 6.33), tested with our own GTK4 client: a GTK4-style client's extended counter steps a few milliseconds after the client's final paint in 100 of 100 runs, so it is usable as an experimental signal for GTK4-style clients only. Chromium and Electron use the basic counter, which never moved, so it likely gives nothing for them. In an earlier survey 6 of 9 measured windows advertised some sync counter and about 3 of 9 (the GTK4 ones) would benefit. Sending the client a `_NET_WM_SYNC_REQUEST` ourselves (6.34) was negative at the timing tried, and the extension that causes the resize cannot send X messages. Last, an `LD_PRELOAD` shim that rewrites the geometry of an app's top-level windows (6.35) made the test GTK4 client's windows map at the target size in 80 of 80 runs, and under load it flashed 0 of 40 against 36 of 40 without it. That is one toolkit (GTK4 through Xlib) on one machine: it does not cover xcb-based clients, it did not exempt override-redirect windows, popups were not tested, the reveal came later, 10 of 40 idle runs had a 30 to 44 ms stall whose cause is unverified, and injecting a library into other programs has security and compatibility costs. No screen-capture measurement of what is visible has been made (the Mutter screencast stream could not be consumed; the portal needs a user prompt). **None of this ships in 0.2.0-beta.1:** XWayland and X11 windows keep the old 30 ms rule and can still flash. See [docs/DESIGN_READY_WAIT.md](docs/DESIGN_READY_WAIT.md), sections 12 and 15.
- On GNOME 46 the extension cannot read an app's reported minimum size (it reads 0x0), so it learns
  a minimum only by waiting.
- Untested: real X11 and XWayland applications, GNOME 42 to 45 and 47 to 48, Qt and Electron apps,
  multi-monitor and fractional scaling, and any screen-capture check of what is visible.

[Unreleased]: https://github.com/harshp2008/spawn-at/compare/v0.2.0-beta.2...HEAD
[0.2.0-beta.2]: https://github.com/harshp2008/spawn-at/compare/v0.2.0-beta.1...v0.2.0-beta.2
[0.2.0-beta.1]: https://github.com/harshp2008/spawn-at/releases/tag/v0.2.0-beta.1
