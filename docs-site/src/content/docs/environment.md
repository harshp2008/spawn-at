---
title: "Environment and exit codes"
---

The environment variables spawn-at reads, the exit statuses it returns, and the keys of the
`[update]` table of the config file. Each row names the line it was read from. The only command run
for this page was `spawn-at --no-such-flag`, which gave exit status 2 (below).

## Environment variables

| Variable | Effect | Source |
| :--- | :--- | :--- |
| `SPAWN_AT_BACKEND` | `x11` forces the X11 driver and `sway` the Sway driver; read in any letter case. See [Environments](/spawn-at/environments/) | `src/platform/linux/registry.rs:52-58` |
| `SPAWN_AT_CONFIG` | Path of the config file. A path that does not exist means "no config". It does not move the login update entry | `src/platform/linux/config_store.rs:24-34` |
| `SPAWN_AT_DEBUG` | `1` or `true` prints extra lines on stderr from the X11 driver and from the update code. Any other value, or none, prints nothing. It is read on every call | `src/update.rs:13-19`, `src/platform/linux/x11/atoms.rs:5-16` |
| `SWAYSOCK` | The Sway IPC socket. Required when the Sway driver is chosen: if it is unset, empty or not a Sway socket the command fails and does not fall back to another backend | `src/platform/linux/sway/mod.rs:60-66`, `sway/client.rs:29-41` |
| `XDG_CURRENT_DESKTOP` | Chooses the driver (a value containing `GNOME`, case-insensitive) and decides the unsupported-desktop refusal. `update` also checks it for GNOME to deploy the extension and offer the log-out question | `registry.rs:52-58`, `src/platform/linux/gnome/deploy.rs:456-460` |
| `XDG_SESSION_TYPE` | Read for the unsupported-desktop test: an X11 session counts as supported on any desktop | `registry.rs:56` |
| `XDG_SESSION_ID` | First source for the session id passed to `loginctl show-session` to find out whether the session is Wayland or X11 | `src/platform/linux/gnome/detect.rs:52-60` |
| `WAYLAND_DISPLAY` | Fallback for "is this a Wayland session": any value counts as yes when `loginctl` did not say `wayland` or `x11` | `detect.rs:104-136` |
| `XDG_CONFIG_HOME` | Used when it is an absolute path: the config file and the login entry go under it. Otherwise `$HOME/.config` | `config_store.rs:24-34`, `src/config.rs:184-203` |
| `XDG_DATA_DIRS` | Directories searched for `.desktop` files to find the application id of the program you spawn. Default `/usr/local/share:/usr/share` | `src/platform/linux/xdg.rs:92-118` |
| `HOME` | Binary directory (`~/.local/bin`), extension directory, application directories, config fallback. An unset `HOME` makes the user-scope install fail | `src/platform/linux/host.rs:19-33`, `deploy.rs:200-216` |
| `PATH` | `install` checks whether the binary's directory is in it and prints advice if not; it also looks for other installed copies | `src/platform/installer.rs:52-70`, `src/main.rs:145-152` |
| `NO_COLOR` | Any non-empty value turns colour off | `src/diagnostics.rs:30` |
| `TERM` | `dumb` turns colour off; a value containing `256color` selects 256-colour output | `diagnostics.rs:33, 39` |
| `COLORTERM` | `truecolor` or `24bit` selects 24-bit colour | `diagnostics.rs:36` |
| `LC_ALL`, `LC_CTYPE`, `LANG` | The first one that is set is read. If it is `C` or `POSIX` (any case) the update notice box is drawn without UTF-8 box characters | `src/update.rs:338-345, 521` |
| `COLUMNS` | A positive whole number is used as the terminal width for the update notice box; otherwise the width of stderr, then stdout, from the terminal | `src/update.rs:348-372, 521` |

Colour is also off whenever the stream is not a terminal (`diagnostics.rs:27-29`).

Two more variables belong to `install.sh` only: `GITHUB_TOKEN` and `SPAWN_AT_RESOLVE_ONLY`; see
[Installer flags](/spawn-at/installer-flags/). `build.rs` sets `SPAWN_AT_VERSION`,
`SPAWN_AT_COMMIT_ID`, `SPAWN_AT_RAW_COMMIT_ID`, `SPAWN_AT_LAST_RELEASE`, `SPAWN_AT_TARGET`,
`SPAWN_AT_DIRTY` and `SPAWN_AT_DIFF_HASH` at build time. They are compile-time values read with
`env!` or `option_env!`, not run-time settings.

## Exit codes

| Status | When | Source |
| :--- | :--- | :--- |
| 0 | The command succeeded. Also: `spawn` when the program started but no window appeared in time; the state commands when the state was not reached; `update` when the lookup of releases failed (it prints `Error checking updates: ...` and returns normally) | [Known limits](/spawn-at/known-limits/), `src/update.rs:786-788` |
| 1 | A command failed after the arguments were accepted: the driver could not start (including an unsupported desktop), a command returned an error, `install` or `uninstall` failed, or you answered no to "Install anyway?". Messages are printed as `[spawn-at] ERROR: ...` | `src/main.rs:67, 76, 98, 123, 210, 229, 267, 278, 373-454` (each a `std::process::exit(1)`) |
| 2 | The arguments were refused: a validation error of spawn-at itself (`cli.validate()`), or a parse error of the command line | `src/main.rs:56-59` |

For status 2 the check run for this page was:

```text
$ ./target/debug/spawn-at --no-such-flag; echo "exit=$?"
error: unexpected argument '--no-such-flag' found

Usage: spawn-at [OPTIONS] <COMMAND>

For more information, try '--help'.
exit=2
```

That is a parse error printed by the command-line parser. The validation errors of spawn-at itself
(`src/main.rs:56-59`, printed as `[spawn-at] ERROR: ...`) were not run; they exit 2 by the source.
`install.sh` returns the status of `spawn-at install`, and exits 1 on its own errors and when you
decline the unsupported-desktop question (`install.sh`). A run killed by a signal, or a Rust panic,
is not covered by this table: not verified.

## The `[update]` table

The update checker keeps its own table in the config file (`src/config.rs:12-44`). The settings of
[Configuration](/spawn-at/configuration/) (`wait.*`, `spawn.*`) are in other tables. These keys are
not set through `spawn-at config`.

| Key | Type | Default when absent | Meaning |
| :--- | :--- | :--- | :--- |
| `auto_check` | boolean | `true` | Allows the in-process background check (the 24 hour rule). It has no effect on the login entry, and only matters when `notify` is true. See [Update](/spawn-at/update/#notices-and-the-background-check) |
| `channel` | string | `"stable"` | `"stable"` or `"beta"`. `all` is read as `beta`; any other value is read as `stable` |
| `notify` | boolean | `true` | Whether the terminal update notice is shown |
| `last_check_time` | whole number | `0` | Unix time in seconds of the last check, set by the checks and by a successful upgrade |
| `cached_stable_version` | string, optional | absent | Newest stable release tag the last check found |
| `cached_beta_version` | string, optional | absent | Newest pre-release tag the last check found |
| `latest_cached_version` | string, optional | absent | Kept so a file written by 0.1 still reads. It is used only when the other two are absent (`src/config.rs:78-100`) |

`spawn-at install` writes `auto_check`, `channel` and `notify` every time it runs. It does not
keep your earlier values for the three: `auto_check` becomes `true`, `channel` becomes the one you
give or answer (`stable` without a terminal and without the flag), and `notify` becomes the one
you give or answer (`false` without a terminal and without the flag) (`src/main.rs:282-335`; read
from source, not run). `spawn-at update --check` also saves the cache keys and the check time
(`src/update.rs:788-799`). `config reset` leaves the `[update]` table alone, so nothing in
spawn-at clears the cache keys.

## Not verified

- Only the parse-error exit status was run. Every other status in the table is read from
  `std::process::exit` calls and from the `Result` handling in `src/main.rs`.
- Which validation errors reach `src/main.rs:56-59` was not enumerated.
- The terminal width and colour rows were read, not tried in a terminal.

## Sources

- `src/main.rs:56-152, 282-335`
- `src/platform/linux/registry.rs:52-58`, `config_store.rs`, `host.rs`, `xdg.rs`, `sway/mod.rs`,
  `gnome/detect.rs`, `gnome/deploy.rs`
- `src/diagnostics.rs:25-40`, `src/update.rs`, `src/config.rs`, `build.rs`
- the command shown above, run on the binary built from this tree
