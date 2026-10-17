---
title: "Troubleshooting and FAQ"
---

Every entry here comes from something the repository already records: the source, another page of
this book, or `docs/TEST_LOG.md`. Each entry gives the symptom as recorded, the cause as recorded
and where it was recorded. Where the repository records no cause, or no remedy, the entry says so
instead of guessing. Nothing on this page was reproduced on a live session while writing it.

## Troubleshooting

### `spawn` printed a warning and exited 0, but no window was placed

- **Symptom.** `WARNING: ... did not appear within 2s`, exit status 0.
- **Cause as recorded.** The program started, but no window showed up within
  `spawn.window_timeout_ms` (default 2000). `spawn` then warns and exits 0, on GNOME, Sway and X11
  alike.
- **What is recorded.** For a heavy program (a browser or an Electron app starting cold), raise
  `spawn.window_timeout_ms`; the Configuration page suggests 8000 as a value to try. With
  `--json` the result then has `window_id: null`.
- **Sources.** [Known limits](/spawn-at/known-limits/#exit-status-and-results),
  [Configuration](/spawn-at/configuration/#on-a-slow-or-busy-machine).

### `focus`, `maximize`, `minimize`, `restore`, `transform` or `defocus` exited 0, but nothing changed

- **Cause as recorded.** Each waits up to 2 seconds for the new state, drops the result of that
  wait and exits 0. `--no-wait` skips the wait. The repository records no way to learn from the
  exit status that the state did not change.
- **Source.** [Known limits](/spawn-at/known-limits/#exit-status-and-results).

### `this desktop is not supported`

- **Symptom.** `[spawn-at] ERROR: Driver initialization error: this desktop is not supported
  (XDG_CURRENT_DESKTOP is ..., XDG_SESSION_TYPE is ...)`, exit status 1, also from `install` and
  `uninstall`.
- **Cause as recorded.** The session is not GNOME, not an X11 session, and `SPAWN_AT_BACKEND` is not
  `x11` or `sway`.
- **What is recorded.** The error itself offers `SPAWN_AT_BACKEND=x11` as a fallback that "may or may
  not work" (it sees XWayland windows only and has no cloak), and `SPAWN_AT_BACKEND=sway` for the
  experimental Sway driver.
- **Sources.** [Unsupported desktops](/spawn-at/environments/unsupported-desktops/),
  `src/platform/linux/registry.rs:83-108`.

### `Failed to communicate with SpawnAt GNOME extension via D-Bus`

- **Symptom.** The text continues `Reason: The extension does not appear to be running on the
  session bus.` and `Fix: Run 'spawn-at install' to install and activate the extension.`
- **Cause as recorded.** The message is returned for any failure of the call to the extension, so
  it says the extension is not running whatever the real cause was. The code comment adds that
  whether this call already fails when the extension is not loaded is not verified
  (`src/platform/linux/gnome/driver.rs:41, 199-203, 241-247`).
- **What is recorded.** The message's own advice (`spawn-at install`) and the log-out rule in the
  next entry. No other cause is recorded.

### The extension was installed but is not active, or `extension is older than the CLI expects`

- **Symptom.** Either `Extension protocol mismatch (detected older extension). Please log out and
  back in to load the updated extension.` as a warning, or an error such as `extension is older
  than the CLI expects (protocol 2, need 3): run spawn-at install and log out and back in`.
- **Cause as recorded.** On Wayland the Shell loads an extension at log-in. A newer extension
  written to disk is not loaded until you log out and back in, and the extension exposes only a
  protocol number, not a content hash. The installer decides whether a log-out is needed by
  comparing file times with the Shell's start time and reading `gnome-extensions info`.
- **What is recorded.** Log out and back in once after `install` or `update` on Wayland. The
  installer asks last, and skips the question when the deployed files are identical, the extension
  is ACTIVE, and the files are older than the running Shell.
- **Sources.** [Install](/spawn-at/install/), `src/platform/linux/gnome/driver.rs:211-216`,
  `src/commands/mod.rs:888`, `src/platform/linux/gnome/logout.rs`, `src/session.rs:41-42`.

### `install` said "successfully installed and enabled" but the extension is not enabled

- **Cause as recorded.** `install` prints that line even when `gnome-extensions enable` exited
  non-zero (it prints a `Note:` first). It stops with an error only if `gnome-extensions` cannot be
  started.
- **What is recorded.** The state can be read with `gnome-extensions info spawn-at@harsh.local`; the
  installer reads its `State:` line. No other remedy is recorded.
- **Sources.** [Known limits](/spawn-at/known-limits/#exit-status-and-results),
  `src/platform/linux/gnome/logout.rs:100-110, 160-175`.

### `spawn-at: command not found` after `install`

- **Cause as recorded.** The binary's directory is not in `PATH`. `install` copies the binary to
  `~/.local/bin` (user scope) or `/usr/local/bin` (system scope), prints `'<dir>' is not currently
  in your $PATH.` and prints the line to add. It never edits a shell startup file.
- **What is recorded.** The printed advice is a guarded `case ":$PATH:" in ... esac` line to put in
  `~/.profile`, `~/.bashrc` or your shell's file. For `~/.local/bin` on Debian and Ubuntu the
  advice also says `~/.profile` adds this directory once it exists, so logging out and back in (or
  running `. ~/.profile`) is usually enough. That sentence is the installer's, for those two
  distributions only; the repository records it for no other.
- **Sources.** `src/platform/installer.rs:96-125, 210-216`, [Files and paths](/spawn-at/files-and-paths/).

### An unrelated window was placed after `spawn` failed

- **Symptom.** A window you opened by hand after a failed `spawn` got the position and size meant
  for the program that failed.
- **Cause as recorded.** `spawn` registers the placement with the extension before the program
  starts. On Linux the disarm call is never sent, not even when the program fails to start. The
  registration ends by itself: after 15 seconds when it has a name, after 1.2 seconds when it has
  none (`--class '*'` or an empty name). A window the extension matches to the name in that time is
  placed. Registrations under one name queue up to 16.
- **What is recorded.** Nothing beyond the expiry times. No workaround is recorded.
- **Source.** [Known limits](/spawn-at/known-limits/#after-a-failed-spawn).

### Windows appear at the wrong size, or jump just after they appear

- **Cause as recorded.** The extension waits up to `wait.cap_ms` for an app to answer a size request
  and shows the window anyway when the cap passes. An app already at its minimum size never
  answers, so it costs the full cap on every spawn.
- **What is recorded.** Raise `wait.cap_ms` (the Configuration page says to try 400). To compare
  with the earlier rule: `spawn-at config set wait.strategy legacy`.
- **Sources.** [Configuration](/spawn-at/configuration/#on-a-slow-or-busy-machine), `README.md`
  ("How well it works").

### The window flashes at its default place (XWayland and X11 windows)

- **Cause as recorded.** XWayland, X11 and GNOME 42-44 windows use the older 30 ms rule. The
  project looked for a signal that says an X11 client has painted at its new size and found none
  that works for every toolkit. The measured flash rate of that rule on XWayland varied between
  logins: 1 of 10 in one session and 9 of 10 and 10 of 10 in others (`docs/TEST_LOG.md` 6.17 and
  6.18), and the log's explanation of the difference is a hypothesis.
- **What is recorded.** No fix is recorded. See [X11 and XWayland](/spawn-at/environments/x11-xwayland/)
  for what was tried.
- **Sources.** `docs/KNOWLEDGE.md` ("Dead ends"), `README.md` ("Support").

### The window ended up larger than the size I asked for

- **Cause as recorded.** Some apps refuse a requested size. They end up larger, but are positioned
  using their final size, so anchors still land correctly. The final report says when the size
  differs from the request.
- **Sources.** `README.md` ("Support"), [Known limits](/spawn-at/known-limits/#geometry-what-reached-means).

### Sway: `--anchor cursor` puts the window in the corner

- **Cause as recorded.** Sway has no pointer query, so `spawn` uses the position (0, 0) and prints
  no message. `transform` and `query pointer` return an error instead.
- **Source.** [Known limits](/spawn-at/known-limits/#sway).

### `spawn-at update` says "up to date (dev)"

- **Cause as recorded.** A binary built from a commit that is not on a release tag has no parseable
  version, so `update --check` finds no candidate. Without `--check` it refuses to upgrade unless
  you pass `--force`.
- **Source.** [Update](/spawn-at/update/#a-build-that-is-not-a-release).

## FAQ

### Which desktops and GNOME versions work?

Only GNOME Shell 46 on Wayland has been run on a real session. The full table, and what the code
accepts, is on [Requirements](/spawn-at/requirements/) and [Environments](/spawn-at/environments/).

### Does `uninstall` delete my config file?

No. It removes the binary, the extension directory and the login update entry. The file is not
removed by any command. See [Files and paths](/spawn-at/files-and-paths/).

### Where is the config file?

`spawn-at config path` prints it. The rule is on [Configuration](/spawn-at/configuration/#where-the-file-is).

### Which version am I running?

`spawn-at -V` or `--version`. A build that is not on a release tag prints `DEV BUILD`, the last
release and a commit id (`src/update.rs:60-72`).

### Does `update` check what it downloads?

No checksum or signature is checked. Details and the search that was made are on
[Update](/spawn-at/update/#no-checksum-or-signature-check).

### Why do `--focus`, `--no-focus` and `--defocus` do nothing on `spawn`?

They are accepted and never read for `spawn`, which always asks for exclusive focus
([spawn](/spawn-at/commands/spawn/)).

### Can I run it without GNOME?

On an X11 session, the X11 driver is used (experimental, untested with real applications). Sway has
an experimental driver chosen with `SPAWN_AT_BACKEND=sway`. A non-GNOME Wayland session is refused
unless you force a backend. See [Unsupported desktops](/spawn-at/environments/unsupported-desktops/).

## Not verified

- No entry was reproduced. The messages and numbers are quoted from the source and from the pages
  cited.
- The Debian and Ubuntu `~/.profile` statement is the installer's own text; it was not checked on
  those distributions.
- That `gnome-extensions info` is the right command for a user to check the state: the repository
  shows the installer reading it, not a user-facing instruction.

## Sources

- `src/platform/linux/gnome/driver.rs`, `logout.rs`, `src/commands/mod.rs`, `src/session.rs`
- `src/platform/installer.rs`, `src/platform/linux/registry.rs`, `src/update.rs`
- `docs-site/src/content/docs/known-limits.md`, `configuration.md`, `environments/*`
- `docs/KNOWLEDGE.md`, `docs/TEST_LOG.md` (6.17, 6.18), `README.md`
