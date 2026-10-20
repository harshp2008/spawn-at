---
title: "Environment and exit codes"
---

The environment variables spawn-at reads, the exit statuses it returns, and the keys of the
`[update]` table of the config file. The files each row was read from are in Sources. The only
command run for this page was `spawn-at --no-such-flag`, which gave exit status 2 (below).

## Environment variables

| Variable | Effect |
| :--- | :--- |
| `SPAWN_AT_BACKEND` | `x11` forces the X11 backend and `sway` the Sway backend; read in any letter case. See [Environments](/spawn-at/environments/) |
| `SPAWN_AT_CONFIG` | Path of the config file. A path that does not exist means "no config". It does not move the login update entry |
| `SPAWN_AT_DEBUG` | `1` or `true` prints extra lines on stderr from the X11 backend and from the update code. Any other value, or none, prints nothing. It is read on every call |
| `SWAYSOCK` | The Sway IPC socket. Required when the Sway backend is chosen: if it is unset, empty or not a Sway socket the command fails and does not fall back to another backend |
| `XDG_CURRENT_DESKTOP` | Chooses the backend (a value containing `GNOME`, case-insensitive) and decides the unsupported-desktop refusal. `update` also checks it for GNOME to deploy the extension and offer the log-out question |
| `XDG_SESSION_TYPE` | Read for the unsupported-desktop test: an X11 session counts as supported on any desktop |
| `XDG_SESSION_ID` | First source for the session id passed to `loginctl show-session` to find out whether the session is Wayland or X11 |
| `WAYLAND_DISPLAY` | Fallback for "is this a Wayland session": any value counts as yes when `loginctl` did not say `wayland` or `x11` |
| `XDG_CONFIG_HOME` | Used when it is an absolute path: the config file and the login entry go under it. Otherwise `$HOME/.config` |
| `XDG_DATA_DIRS` | Directories searched for `.desktop` files to find the application id of the program you spawn. Default `/usr/local/share:/usr/share` |
| `HOME` | Binary directory (`~/.local/bin`), extension directory, application directories, config fallback. An unset `HOME` makes the user-scope install fail |
| `PATH` | `install` checks whether the binary's directory is in it and prints advice if not; it also looks for other installed copies |
| `NO_COLOR` | Any non-empty value turns colour off |
| `TERM` | `dumb` turns colour off; a value containing `256color` selects 256-colour output |
| `COLORTERM` | `truecolor` or `24bit` selects 24-bit colour |
| `LC_ALL`, `LC_CTYPE`, `LANG` | The first one that is set is read. If it is `C` or `POSIX` (any case) the update notice box is drawn without UTF-8 box characters |
| `COLUMNS` | A positive whole number is used as the terminal width for the update notice box; otherwise the width of stderr, then stdout, from the terminal |

Colour is also off whenever the stream is not a terminal.

To see the effect of the three that choose the config file (`SPAWN_AT_CONFIG`, `XDG_CONFIG_HOME`,
`HOME`), run `spawn-at config path`: by the code it prints the file those variables lead to (read
from source, not run). If a variable does not seem to do anything, see
[Troubleshooting](/spawn-at/troubleshooting/).

Two more variables belong to `install.sh` only: `GITHUB_TOKEN` and `SPAWN_AT_RESOLVE_ONLY`; see
[Installer flags](/spawn-at/installer-flags/). `build.rs` sets `SPAWN_AT_VERSION`,
`SPAWN_AT_COMMIT_ID`, `SPAWN_AT_RAW_COMMIT_ID`, `SPAWN_AT_LAST_RELEASE`, `SPAWN_AT_TARGET`,
`SPAWN_AT_DIRTY` and `SPAWN_AT_DIFF_HASH` at build time. They are compile-time values read with
`env!` or `option_env!`, not run-time settings.

## Exit codes

| Status | When |
| :--- | :--- |
| 0 | The command succeeded. Also: `spawn` when the program started but no window appeared in time; the state commands when the state was not reached; `update` when the lookup of releases failed (it prints `Error checking updates: ...` and returns normally). See [Known limits](/spawn-at/known-limits/#exit-status-and-results) |
| 1 | A command failed after the arguments were accepted: the backend could not start (including an unsupported desktop), a command returned an error, `install` or `uninstall` failed, or you answered no to "Install anyway?". Messages are printed as `[spawn-at] ERROR: ...` |
| 2 | The arguments were refused: a validation error of spawn-at itself (`cli.validate()`), or a parse error of the command line |

To check a status yourself, run the command and then `echo "exit=$?"`, as in the example below.

For status 2 the check run for this page was:

```text
$ ./target/debug/spawn-at --no-such-flag; echo "exit=$?"
error: unexpected argument '--no-such-flag' found

Usage: spawn-at [OPTIONS] <COMMAND>

For more information, try '--help'.
exit=2
```

That is a parse error printed by the command-line parser. The validation errors of spawn-at itself
(printed as `[spawn-at] ERROR: ...`) were not run; they exit 2 by the source.
`install.sh` returns the status of `spawn-at install`, and exits 1 on its own errors and when you
decline the unsupported-desktop question (`install.sh`). A run killed by a signal, or a Rust panic,
is not covered by this table: not verified.

## The `[update]` table

The update checker keeps its own table in the config file. The settings of
[Configuration](/spawn-at/configuration/) (`wait.*`, `spawn.*`) are in other tables. These keys are
not set through `spawn-at config`.

| Key | Type | Default when absent | Meaning |
| :--- | :--- | :--- | :--- |
| `auto_check` | boolean | `true` | Allows the in-process background check (the 24 hour rule) and the silent `update --check --headless` that the login entry runs: with `false` that command returns before it asks GitHub. The in-process check only happens when `notify` is true. See [Update](/spawn-at/update/#notices-and-the-background-check) |
| `channel` | string | `"stable"` | `"stable"` or `"beta"`. `all` is read as `beta`; any other value is read as `stable` |
| `notify` | boolean | `true` | Whether the terminal update notice is shown |
| `last_check_time` | whole number | `0` | Unix time in seconds of the last check, set by the checks and by a successful upgrade |
| `cached_stable_version` | string, optional | absent | Newest stable release tag the last check found |
| `cached_beta_version` | string, optional | absent | Newest pre-release tag the last check found |
| `latest_cached_version` | string, optional | absent | Kept so a file written by 0.1 still reads. It is used only when the other two are absent |

When `spawn-at install` writes these keys, and what it writes without a terminal, is decided key by
key and is explained once, on [Installer flags](/spawn-at/installer-flags/#headless-defaults); the
`install --skip-bin --headless` that `update` runs on GNOME is on
[Update](/spawn-at/update/#the-child-install-and-your-settings). What the commands save to the cache
keys is on [Update](/spawn-at/update/#what---check-saves). `config reset` leaves the `[update]`
table alone, so nothing in spawn-at clears the cache keys.

## Not verified

- Only the parse-error exit status was run. Every other status in the table is read from
  `std::process::exit` calls and from the `Result` handling in `main`.
- Which validation errors reach the exit-2 branch of `main` was not enumerated.
- The terminal width and colour rows were read, not tried in a terminal.

## Sources

- `src/main.rs`: lines 56-59 (exit 2: `cli.validate()` and the parse error), 67, 76, 98, 123, 210, 229,
  267, 278 and 373-454 (each a `std::process::exit(1)`), 145-152 (`PATH`), 56-152 and 282-335
- `src/platform/linux/registry.rs`: lines 52-58 and 56 (`SPAWN_AT_BACKEND`, `XDG_CURRENT_DESKTOP`,
  `XDG_SESSION_TYPE`)
- `src/platform/linux/config_store.rs`: lines 24-34 (`SPAWN_AT_CONFIG`, `XDG_CONFIG_HOME`);
  `src/config.rs`: lines 184-203 (`XDG_CONFIG_HOME`), 12-44 (the `[update]` table), 78-100
  (`latest_cached_version`)
- `src/update.rs`: lines 13-19 (`SPAWN_AT_DEBUG`), 338-372 and 521 (`LC_*`, `COLUMNS`), 786-788 (exit 0
  on a failed lookup)
- `src/platform/linux/x11/atoms.rs`: lines 5-16 (`SPAWN_AT_DEBUG`); `src/platform/linux/sway/mod.rs`:
  lines 60-66 and `sway/client.rs`: lines 29-41 (`SWAYSOCK`)
- `src/platform/linux/gnome/deploy.rs`: lines 200-216 (`HOME`) and 456-460 (`XDG_CURRENT_DESKTOP`
  in `update`); `gnome/detect.rs`: lines 52-60 (`XDG_SESSION_ID`) and 104-136 (`WAYLAND_DISPLAY`)
- `src/platform/linux/xdg.rs`: lines 92-118 (`XDG_DATA_DIRS`); `host.rs`: lines 19-33 (`HOME`);
  `src/platform/installer.rs`: lines 52-70 (`PATH`)
- `src/diagnostics.rs`: lines 25-40 (`NO_COLOR`, `TERM`, `COLORTERM`, streams that are not terminals)
- `build.rs` (the compile-time `SPAWN_AT_*` values)
- the command shown above, run on the binary built from this tree
