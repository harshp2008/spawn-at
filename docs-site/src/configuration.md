# Configuration

Nothing needs to be set. With no config file and no flags, spawn-at behaves as it did before the
settings existed.

## Settings

| Key | Default | Allowed | What it does |
| :--- | :--- | :--- | :--- |
| `wait.cap_ms` | not sent: the extension uses its own default, 250 ms (it was 150 ms in development builds, `CHANGELOG.md`) | whole milliseconds, 1 to 1500 | The longest the extension waits for an app to answer a size request before it shows the window anyway |
| `wait.strategy` | `auto` | `auto`, `legacy`, `ready-wait` | Which rule waits for the resized window. `auto` lets the extension choose; `legacy` is the original 30 ms quiet rule; `ready-wait` is the newer wait on GNOME Wayland |
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

## On a slow or busy machine

- Windows that still appear at the wrong size or jump just after they appear: raise `wait.cap_ms`
  (try 400). A longer cap costs nothing when the app answers in time; it only delays the window for
  an app that never answers. An app already at its minimum size is such an app: it costs the full
  cap on every spawn.
- `WARNING: ... did not appear within 2s` for a heavy app (a browser or an Electron app starting
  cold): raise `spawn.window_timeout_ms` (try 8000).
- To compare with the behaviour before 0.2: `spawn-at config set wait.strategy legacy`.

## Environment variables

| Variable | Effect |
| :--- | :--- |
| `SPAWN_AT_BACKEND` | `x11` forces the X11 driver, `sway` the experimental Sway driver. See [Environments](environments/index.md) |
| `SPAWN_AT_CONFIG` | Path of the config file |
| `SPAWN_AT_DEBUG` | `1` or `true` turns on debug output |
| `NO_COLOR` | Plain output |

## Update preferences

`[update]` in the same file holds the release channel (`stable`, the default, or `beta`, which also
offers pre-releases), whether to check at login and whether to show notices. `all` is a deprecated
name for `beta` and is read as `beta`. These are not yet set through `spawn-at config`; use
`spawn-at update --channel` or `spawn-at install --update-channel`.

## Not configurable yet

The ready-wait quiet and hard limits, the 15 second batch deadline, the 2 second waits of the state
commands and the X11 timeouts are fixed in code. A per-application override does not exist.

## Sources

- `README.md` ("Settings")
- `docs/CONFIG.md` ("First slice", "Design", "Later", update channel paragraph)
- `docs/KNOWLEDGE.md` ("Install and release": channel `all`)
- `CHANGELOG.md` ("[0.2.0-beta.1] Changed": "Default `wait.cap_ms` is 250 ms (it was 150 ms in development builds)")
- the output of `spawn-at config`, `config set`, `config get`, `config unset` and `config reset`, run in a temporary directory with `env -i`, including the file with comments described under Rules
