---
title: "Configuration"
---

Nothing needs to be set. With no config file and no flags, spawn-at behaves as it did before the
settings existed.

## Settings

| Key | Default | Allowed | What it does |
| :--- | :--- | :--- | :--- |
| `wait.cap_ms` | not sent: the extension uses its own default, 250 ms (it was 150 ms in development builds) | whole milliseconds, 1 to 1500 | The longest the extension waits for an app to answer a size request before it shows the window anyway |
| `wait.strategy` | `auto` | `auto`, `legacy`, `ready-wait` | Which rule waits for the resized window. `auto` lets the extension choose; `legacy` is the 30 ms quiet rule; `ready-wait` is the newer wait on GNOME Wayland |
| `spawn.window_timeout_ms` | 2000 | whole milliseconds, 200 to 60000 | How long `spawn` waits for the new window to appear before it warns that it did not (applies to the extension's claim and again to the fallback poll) |

## The `config` command

```sh
spawn-at config                        # every setting, its value, and where the value comes from
spawn-at config set wait.cap_ms 400
spawn-at config get wait.cap_ms
spawn-at config unset wait.cap_ms      # back to the default
spawn-at config path                   # the file
spawn-at config reset                  # remove all settings above (update preferences and unknown keys stay)
```

Real output of the binary built from this repository, run with an empty temporary config
directory (the path of the file is replaced by a placeholder, and the lines after the second table
header are cut):

```text
$ spawn-at config
KEY                      VALUE  SOURCE                                     ALLOWED
wait.cap_ms              250    default (not sent: the extension decides)  whole milliseconds from 1 to 1500
wait.strategy            auto   default (not sent: the extension decides)  auto, legacy or ready-wait
spawn.window_timeout_ms  2000   default                                    whole milliseconds from 200 to 60000

$ spawn-at config set wait.cap_ms 5000
[spawn-at] ERROR: wait.cap_ms must be whole milliseconds from 1 to 1500 (got '5000')

$ spawn-at config set wait.cap_ms 400
wait.cap_ms = 400 (<path of config.toml>)
$ spawn-at config
KEY                      VALUE  SOURCE                                     ALLOWED
wait.cap_ms              400    file                                       whole milliseconds from 1 to 1500
...
```

The file it wrote contained:

```toml
[wait]
cap_ms = 400
```

### Check that a setting took effect

Run `spawn-at config` again. In the output above, the `wait.cap_ms` row shows `400` and its `SOURCE`
column shows `file`, where it showed `default` before. If the `SOURCE` column still says `default`,
check which file is read ([Where the file is](#where-the-file-is)) and the warnings under
[Rules](#rules). A value outside the allowed range is refused with the `ERROR` line shown above.
For windows that still misbehave after a change, see
[Troubleshooting](/spawn-at/troubleshooting/#windows-appear-at-the-wrong-size-or-jump-just-after-they-appear).

## Where the file is

`$XDG_CONFIG_HOME/spawn-at/config.toml` if `XDG_CONFIG_HOME` is an absolute path, else
`~/.config/spawn-at/config.toml`. Set `SPAWN_AT_CONFIG=<path>` to use a different file; a path that
does not exist means "no config". `SPAWN_AT_CONFIG` names the config file only; it does not move the
login autostart entry.

## Rules

- A value on the command line beats the file, and the file beats the default. `config` names the
  source of each value: `default`, `file` or `flag`.
- A setting that is not set sends nothing to the extension. An explicit `wait.cap_ms = 250` is sent
  because it was set.
- An unknown key in the file is a warning. A file that is not valid TOML is one warning with the line
  number. In both cases `spawn` carries on with the defaults. A bad value is a warning for that key
  only.
- `set`, `unset` and `reset` rewrite the file **without its comments**. Unknown keys and tables are
  kept. A malformed file is never overwritten by `set`. Checked with a throwaway file that had a
  comment on top, comments inside and after `[wait]` and `cap_ms`, a `[custom]` table and an
  `[update]` table: after `config set wait.strategy legacy` every comment was gone, `[custom]` and
  `[update]` were kept unchanged, and the tables came out sorted by name (`custom`, `update`,
  `wait`). `unset` and `reset` gave the same shape (`reset` left only `[custom]` and `[update]`).
  `set` on a file containing `[wait` exited 1 with "unclosed table … fix or remove the file before
  changing settings" and left it untouched.
- The update checker keeps its own `[update]` table in the same file and replaces only that table
  when it saves.
- `spawn-at uninstall` and `install --clean-install` never delete this file.

## Hidden flags for one run

`spawn` has two flags that `--help` does not list. The code comment calls them developer switches
for A/B runs in the same login. They exist for comparing wait rules, not for daily use; the
settings above are the supported way.

| Flag | Value | Effect for that one `spawn` |
| :--- | :--- | :--- |
| `--wait-strategy` | `auto`, `legacy` or `ready-wait` | The same choice as `wait.strategy`, for this spawn only |
| `--wait-cap-ms` | `1` to `1500` | The same choice as `wait.cap_ms`, for this spawn only |

Both are read as text so that a bad value does not fail the spawn: it is ignored with a warning.
The warning for a bad strategy, as recorded when it was tried, reads
`[spawn-at] WARNING: ignoring --wait-strategy '<value>' (expected auto, legacy or ready-wait)` and
the spawn goes on with the default. The exact warning for a bad cap was not recorded.

`spawn-at config`, `config show` and `config get` take the same two flags, also hidden, to show what
a spawn with them would use; the source column then says `flag`. Their values are not stored by
anything.

## On a slow or busy machine

- Windows that still appear at the wrong size or jump just after they appear: raise `wait.cap_ms`
  (try 400). A longer cap costs nothing when the app answers in time; it only delays the window for
  an app that never answers. An app already at its minimum size is such an app: it costs the full
  cap on every spawn.
- `WARNING: ... did not appear within 2s` for a heavy app (a browser or an Electron app starting
  cold): raise `spawn.window_timeout_ms` (try 8000).
- To compare with the behaviour before 0.2: `spawn-at config set wait.strategy legacy`.

## Environment variables

`SPAWN_AT_CONFIG` (the config file path, above), `SPAWN_AT_BACKEND`, `SPAWN_AT_DEBUG` and `NO_COLOR`
are the ones you are most likely to set. Every variable the code reads and the exit statuses are on
[Environment and exit codes](/spawn-at/environment/).

## Update preferences

`[update]` in the same file holds the release channel (`stable`, the default, or `beta`, which also
offers pre-releases), whether to check at login and whether to show notices. `all` is a deprecated
name for `beta` and is read as `beta`. These are not yet set through `spawn-at config`; use
`spawn-at update --channel` or `spawn-at install --update-channel`. The key names (`auto_check`,
`channel`, `notify` and the four cache keys), their defaults, and when `spawn-at install` writes
three of them are on [Environment and exit codes](/spawn-at/environment/#the-update-table).

## Not configurable yet

The ready-wait quiet and hard limits, the 15 second batch deadline, the 2 second waits of the state
commands and the X11 timeouts are fixed in code. A per-application override does not exist.

## Sources

- `README.md` ("Settings")
- `docs/CONFIG.md` ("First slice", "Design", "Later", update channel paragraph)
- `docs/KNOWLEDGE.md` ("Install and release": channel `all`)
- `CHANGELOG.md` ("[0.2.0-beta.1] Changed": "Default `wait.cap_ms` is 250 ms (it was 150 ms in development builds)")
- `src/cli/args.rs`: lines 259-268 (`--wait-strategy` and `--wait-cap-ms`: `hide = true`, read as text,
  a bad value ignored with a warning) and 709-716 (the same flags on `config`)
- `docs/TEST_LOG.md`, section 6.30 (the warning for a bad strategy)
- the output of `spawn-at config`, `config set`, `config get`, `config unset` and `config reset`, run in a temporary directory with `env -i`, including the file with comments described under Rules
