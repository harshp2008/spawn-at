---
title: "Troubleshooting and FAQ"
---

Each entry has a Symptom (what you see), a Cause and a Fix. Where the repository records no cause or
no fix, the entry says so instead of guessing. Nothing here was reproduced on a live session: the
messages and numbers are quoted from the source and from the pages linked in each entry.

## Troubleshooting

The entries run from the most likely to the least likely. There are no usage numbers in the
repository, so this is a judgement, made on three points: the entry happens in the first steps after
an install (before any daily use), it affects every user of the one tested setup (GNOME on Wayland),
or it needs an unusual setup (Sway, XWayland, a build that is not a release). The first four entries
are the install and first run, the next five are daily use, and the last four need an unusual setup
or an earlier failure.

### The extension was installed but is not active, or `extension is older than the CLI expects`

- **Symptom.** Either a warning that starts `The extension is loaded but reports protocol N; this
  spawn-at expects 3 or newer.` and ends `log out and back in to load the updated extension`, or an
  error such as `extension is older than the CLI expects (protocol 2, need 3): run spawn-at install
  and log out and back in`. The warning is printed only when the extension answered with an older
  protocol. An extension that does not answer at all gets the other entry below.
- **Cause.** On Wayland the Shell loads an extension at log-in. A newer extension written to disk is
  not loaded until you log out and back in, and the extension exposes only a protocol number, not a
  content hash. The installer decides whether a log-out is needed by comparing file times with the
  Shell's start time and reading `gnome-extensions info`.
- **Fix.** Log out and back in once after `install` or `update` on Wayland. The installer asks
  last, and skips the question when the deployed files are identical, the extension is ACTIVE, and
  the files are older than the running Shell. The installer reads the `State:` line of
  `gnome-extensions info spawn-at@harsh.local`; running that yourself is not verified as a user
  step.
- **More.** [Install](/spawn-at/install/#what-the-installer-asks).

### `Failed to communicate with SpawnAt GNOME extension via D-Bus`

- **Symptom.** The text continues with a reason and a fix, chosen from what spawn-at finds on disk
  and in `gnome-extensions info`: not installed (run `spawn-at install`); installed but disabled
  (run `gnome-extensions enable spawn-at@harsh.local`); OUT OF DATE, meaning the installed
  `metadata.json` does not list the running GNOME Shell version (upgrade spawn-at, run `spawn-at
  install`, log out and back in); or installed and listed but silent (check `gnome-extensions info
  spawn-at@harsh.local`; on Wayland a new extension loads at the next login).
- **Cause.** GNOME does not load an extension whose `shell-version` list lacks the running Shell
  major, and the extension's D-Bus object then never appears. The owner saw this on GNOME Shell 50.1
  with a release that listed 45 to 48. The choice between the causes is made from files and command
  output and is unit-tested; it has not been run on a live Shell.
- **Fix.** The message's own advice, then the log-out described in
  [the extension is not active entry](#the-extension-was-installed-but-is-not-active-or-extension-is-older-than-the-cli-expects)
  where the message asks for it.

### `spawn-at: command not found` after `install`

- **Symptom.** The shell says `spawn-at: command not found`.
- **Cause.** The binary's directory is not in `PATH`. `install` copies the binary to `~/.local/bin`
  (user scope) or `/usr/local/bin` (system scope), prints `'<dir>' is not currently in your $PATH.`
  and prints the line to add. It never edits a shell startup file.
- **Fix.** Put the printed line, a guarded `case ":$PATH:" in ... esac`, in `~/.profile`,
  `~/.bashrc` or your shell's file. For `~/.local/bin` on Debian and Ubuntu the printed advice also
  says `~/.profile` adds this directory once it exists, so logging out and back in (or running
  `. ~/.profile`) is usually enough. That sentence is the installer's, for those two distributions
  only; the repository records it for no other. Where the files go: [Files and paths](/spawn-at/files-and-paths/).

### `this desktop is not supported`

- **Symptom.** `[spawn-at] ERROR: Driver initialization error: this desktop is not supported
  (XDG_CURRENT_DESKTOP is ..., XDG_SESSION_TYPE is ...)`, exit status 1, also from `install` and
  `uninstall`.
- **Cause.** The session is not GNOME, not an X11 session, and `SPAWN_AT_BACKEND` is not `x11` or
  `sway`.
- **Fix.** The error itself offers `SPAWN_AT_BACKEND=x11` as a fallback that "may or may not work"
  (it sees XWayland windows only and has no cloak), and `SPAWN_AT_BACKEND=sway` for the experimental
  Sway backend. More: [Unsupported desktops](/spawn-at/environments/unsupported-desktops/).

### `spawn` printed a warning and exited 0, but no window was placed

- **Symptom.** `WARNING: ... did not appear within 2s`, exit status 0.
- **Cause.** The program started, but no window showed up within `spawn.window_timeout_ms` (default
  2000). `spawn` then warns and exits 0, on GNOME, Sway and X11 alike.
- **Fix.** For a heavy program (a browser or an Electron app starting cold), raise
  `spawn.window_timeout_ms`; [Configuration](/spawn-at/configuration/#on-a-slow-or-busy-machine)
  suggests 8000 as a value to try. With `--json` the result has `window_id: null` when no window
  appeared. More: [Known limits](/spawn-at/known-limits/#exit-status-and-results).

### Windows appear at the wrong size, or jump just after they appear

- **Symptom.** The window shows at one size or place and then changes.
- **Cause.** The extension waits up to `wait.cap_ms` for an app to answer a size request and shows
  the window anyway when the cap passes. An app already at its minimum size never answers, so it
  costs the full cap on every spawn.
- **Fix.** Raise `wait.cap_ms` ([Configuration](/spawn-at/configuration/#on-a-slow-or-busy-machine)
  says to try 400). To compare with the legacy rule: `spawn-at config set wait.strategy legacy`.

### The window ended up larger than the size I asked for

- **Symptom.** The window is bigger than `--size`, and `spawn` prints a line saying the size was
  raised or differs.
- **Cause.** Some apps refuse a requested size. They end up larger, but are positioned using their
  final size, so anchors still land correctly. The model is on [Concepts](/spawn-at/concepts/#windows-that-end-up-bigger-than-asked).
- **Fix.** None recorded; this is how those apps behave. See
  [Known limits](/spawn-at/known-limits/#geometry-what-reached-means).

### `focus`, `maximize`, `minimize`, `restore`, `transform` or `defocus` exited 0, but nothing changed

- **Symptom.** Exit status 0, and the window is as before.
- **Cause.** Each waits up to 2 seconds for the new state, drops the result of that wait and exits
  0. `--no-wait` skips the wait.
- **Fix.** None recorded: the repository records no way to learn from the exit status that the state
  did not change. Run `spawn-at query windows` and read the `STATE` and `GEOMETRY` of the row.
  More: [Known limits](/spawn-at/known-limits/#exit-status-and-results).

### `install` said the extension was deployed but not confirmed enabled

- **Symptom.** `Note: 'gnome-extensions enable' exited with code N. The extension files were
  deployed, but the extension is not confirmed enabled. If GNOME Shell has not loaded the new files
  yet, it may only be enabled after you log out and back in ...`, instead of `successfully
  installed and enabled`. Exit status 0.
- **Cause.** `gnome-extensions enable` exited non-zero. `install` stops with an error only if
  `gnome-extensions` cannot be started. Builds before this version printed the success line even
  then.
- **Fix.** Log out and back in (see
  [the extension is not active entry](#the-extension-was-installed-but-is-not-active-or-extension-is-older-than-the-cli-expects)),
  then read the state with
  `gnome-extensions info spawn-at@harsh.local`. No other fix is recorded. More:
  [Known limits](/spawn-at/known-limits/#exit-status-and-results).

### The window flashes at its default place (XWayland and X11 windows)

- **Symptom.** The window shows at its default place before it moves.
- **Cause.** XWayland, X11 and GNOME 42-44 windows use the legacy 30 ms rule. The project looked for
  a signal that says an X11 client has painted at its new size and found none that works for every
  toolkit. The measured flash rate of that rule on XWayland varied between logins: 1 of 10 in one
  session and 9 of 10 and 10 of 10 in others
  ([6.17](/spawn-at/experiments/6-17-flash-baseline/),
  [6.18](/spawn-at/experiments/6-18-ready-wait-live-validation/)), and the explanation of the
  difference is a hypothesis.
- **Fix.** None recorded. See [X11 and XWayland](/spawn-at/environments/x11-xwayland/) for what was
  tried.

### `spawn-at update` says "up to date (dev)"

- **Symptom.** `spawn-at is up to date (dev).`
- **Cause.** A binary built from a commit that is not on a release tag has no parseable version, so
  `update --check` finds no candidate. Without `--check` it refuses to upgrade unless you pass
  `--force`.
- **Fix.** `--force`, or install a release build. More:
  [Update](/spawn-at/update/#a-build-that-is-not-a-release).

### An unrelated window was placed after `spawn` failed

- **Symptom.** A window you opened by hand after a failed `spawn` got the position and size meant
  for the program that failed.
- **Cause.** `spawn` registers the placement with the extension before the program starts and never
  withdraws it on Linux, so a window the extension matches to that name before the registration
  expires is placed. The expiry times and the queue limit are on
  [Known limits](/spawn-at/known-limits/#after-a-failed-spawn).
- **Fix.** None recorded; the repository records only the expiry times.

### Sway: `--anchor cursor` puts the window in the corner

- **Symptom.** With `--anchor cursor` on Sway the window goes to the corner, and no message is
  printed.
- **Cause.** Sway has no pointer query, so `spawn` uses the position (0, 0). `transform` and `query
  pointer` return an error instead.
- **Fix.** None recorded. More: [Known limits](/spawn-at/known-limits/#sway).

## FAQ

### Which desktops and GNOME versions work?

Only GNOME Shell 46 on Wayland has been run on a real session. The full table, and what the code
accepts, is on [Requirements](/spawn-at/requirements/) and [Environments](/spawn-at/environments/).

### Does `uninstall` delete my config file?

No. It removes the binary, the extension directory and the login update entry. The file is not
removed by any command. See [Files and paths](/spawn-at/files-and-paths/).

### Where is the config file?

`spawn-at config path` prints it. The rule is on [Configuration](/spawn-at/configuration/#where-the-file-is).

### Why did my update channel or notice setting change after `update`?

By the code, it should not: on GNOME, `update` runs a child install that keeps the values the config
file already has. An earlier build overwrote them, and an update done by such a build may have left
that behind. Read from source, not run. Details, and what to check:
[Update](/spawn-at/update/#the-child-install-and-your-settings).

### Which version am I running?

`spawn-at -V` or `--version`. A build that is not on a release tag prints `DEV BUILD <crate
version>`, the last release and a commit id.

### Does `update` check what it downloads?

No checksum or signature is checked. Details and the search that was made are on
[Update](/spawn-at/update/#no-checksum-or-signature-check).

### Why do `--focus`, `--no-focus` and `--defocus` do nothing on `spawn`?

They are accepted on `spawn` and have no effect; [spawn](/spawn-at/commands/spawn/) has the details.

### Can I run it without GNOME?

On an X11 session, the X11 backend is used (experimental, untested with real applications). Sway has
an experimental backend chosen with `SPAWN_AT_BACKEND=sway`. A non-GNOME Wayland session is refused
unless you force a backend. See [Unsupported desktops](/spawn-at/environments/unsupported-desktops/).

## Not verified

- No entry was reproduced. The messages and numbers are quoted from the source and from the pages
  linked.
- The Debian and Ubuntu `~/.profile` statement is the installer's own text; it was not checked on
  those distributions.
- That `gnome-extensions info` is the right command for a user to check the state: the repository
  shows the installer reading it, not a user-facing instruction.
- The order of the entries is a judgement (see the top of Troubleshooting), not a measurement.

## Sources

- `src/platform/linux/gnome/driver.rs`: lines 41 and 199-203 and 241-247 (the D-Bus failure message and
  the code comment), 211-216 (the protocol mismatch warning)
- `src/commands/mod.rs`: line 888 (the `extension is older than the CLI expects` error)
- `src/platform/linux/gnome/logout.rs`: `decide`, lines 100-110 and 160-175 (file times,
  `gnome-extensions info`, the enable note); `src/session.rs`: lines 41-42
- `src/platform/installer.rs`: lines 96-125 and 210-216 (the `PATH` advice)
- `src/platform/linux/registry.rs`: lines 83-108 (the unsupported-desktop test)
- `src/update.rs`: lines 60-72 (`get_version_display`)
- `docs-site/src/content/docs/known-limits.md`, `configuration.md`, `environments/*`
- `docs/KNOWLEDGE.md` ("Dead ends"), `docs/TEST_LOG.md` (6.17, 6.18), `README.md` ("Support", "How
  well it works")
