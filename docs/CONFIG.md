# Settings (`spawn-at config`)

Labels: CONFIRMED = read in the code or run; HYPOTHESIS = not tested.

## Inventory before this change (CONFIRMED, read from the source)

There was one config mechanism: `src/config.rs` read and wrote `~/.config/spawn-at/config.toml`
for the update checker (`[update]`: `auto_check`, `channel`, `notify`, cached versions). It had no
XDG lookup and no `spawn-at config` command; `save()` rewrote the whole file from its own struct,
so any other table in the file would have been lost on the next update check (fixed here).

| Tunable | Name / where | Default | User-settable before |
| :--- | :--- | :--- | :--- |
| Wait for the new window (claim signal, then the poll after it) | two literals in `commands/spawn.rs` | 2000 ms each | no |
| Whole-batch deadline handed to the driver | `Batch.deadline` in `commands/spawn.rs` | 15000 ms | no |
| Ready-wait cap | `Policy.cap_ms` (core), `DEFAULT_POLICY` (extension) | 250 ms | hidden `--wait-cap-ms` (1 to 1500) |
| Wait rule | `WaitStrategy` | `auto` | hidden `--wait-strategy` |
| Ready-wait quiet / hard limit | `Policy.quiet_ms` / `hard_ms`; the extension's own copies | 30 ms / 1500 ms | no (hard limit is also the upper bound of the cap) |
| Legacy idle timeout carried in `WaitForCommit` | `timeout_ms` in `mechanics.rs` | 300 ms | no |
| `close` verification | `commands/lifecycle.rs` | 1500 ms, polled every 50 ms | no |
| focus / defocus / maximize / minimize / restore state wait | `commands/focus.rs`, `lifecycle.rs` | 2000 ms | no |
| transform settle | `commands/transform.rs` | 2000 ms, then 300 ms | no |
| X11 interception | `platform/linux/x11/interception.rs` | 5000 ms (find), 3000 ms (map) | no |
| Retries | none found (HYPOTHESIS: a grep for retry/retries finds nothing in `src/`) | - | - |
| Debug output | env `SPAWN_AT_DEBUG` (`1` or `true`) | off | env only |
| Backend choice | env `SPAWN_AT_BACKEND=x11` | auto | env only |
| Colour | `NO_COLOR`, `TERM`, `COLORTERM` | by terminal | env only |
| Extension timing log | `SetLogging` D-Bus call (in memory, per Shell session) | off | harness / developer |
| Update preferences | `[update]` in the same file | stable, check on, notify on | `spawn-at update --channel`, `spawn-at install --update-channel`, install prompt |

## First slice (this change)

| Key | Default | Range | Sent to the extension |
| :--- | :--- | :--- | :--- |
| `wait.cap_ms` | none set (the extension's default applies) | 1 to 1500 | in the existing `cap_ms` field of `WaitForCommit` when set by file or flag |
| `wait.strategy` | `auto` | `auto`, `legacy`, `ready-wait` | in the existing `strategy` field when set and not `auto` |
| `spawn.window_timeout_ms` | 2000 | 200 to 60000 | no (CLI-side) |

Defaults do not change behaviour (CONFIRMED by test): with no file and no flags the arming
instruction is byte-for-byte the one sent before (`{"WaitForCommit":{"timeout_ms":300}}`, no
`strategy`, no `cap_ms`); `strategy = "auto"` in the file is the same as nothing. An explicit
`wait.cap_ms = 250` is sent (`"cap_ms":250`) because it was set.

## Design

- **Pure core** (`spawn_at_core::settings`): keys, ranges, defaults, the TOML reading with its
  warnings, the edit of a file's text (`set`/`unset`/`reset`), precedence (flag > file > default)
  and the `config` actions themselves, run against a `SettingsStore` trait. No I/O, no
  environment, no terminal.
- **Platform layer** (`src/platform/linux/config_store.rs`): the location
  (`SPAWN_AT_CONFIG`, else an absolute `$XDG_CONFIG_HOME/spawn-at/config.toml`, else
  `~/.config/spawn-at/config.toml`) and the I/O: read, and an atomic write (temporary file in the
  same directory, flushed, renamed). Another platform brings its own store.
- **Spawn never breaks because of the config**: a missing file is the defaults; an unreadable or
  malformed file is the defaults and one warning (with the line number for a parse error); a bad
  value or an unknown key is a warning for that key only.
- **Shared file**: the update checker now replaces only its `[update]` table when it saves, and
  both use the same location.
- **Autostart entry follows the same directory**: the login update-check entry is
  `$XDG_CONFIG_HOME/autostart/spawn-at-update-check.desktop` for an absolute `XDG_CONFIG_HOME`,
  else `~/.config/autostart/spawn-at-update-check.desktop` (unchanged when the variable is unset).
  Before this fix it was always under `~/.config`, so with `XDG_CONFIG_HOME` set the desktop
  session did not find it. `SPAWN_AT_CONFIG` names the config file only and does not move it.
  An entry written by an earlier version under `~/.config/autostart` is not moved or removed;
  re-run `spawn-at install` to write the new one.
- **Uninstall and clean install never delete the config file.** `spawn-at uninstall` removes the binary, the extension directory and the login update-check entry, and nothing else; `spawn-at install --clean-install` runs the same removal first and then installs, so your settings survive it. (`install` itself still writes `[update] channel` and `notify` as before.)
- **Comments are not kept** when `set`, `unset` or `reset` rewrites the file (the file is parsed
  and written again); unknown keys and tables are. A malformed file is never overwritten by `set`.
- **Isolation**: `spawn-at-verify` sets `SPAWN_AT_CONFIG` to a path that does not exist on every
  `spawn-at` action it runs (`no_config.rs`), so a user's `wait.cap_ms` cannot change a measurement.

## Later (needs a protocol or extension change, or more design)

- Ready-wait `quiet_ms` and `hard_ms`, and the legacy `timeout_ms` (the extension reads them from
  its own constants; a `policy` object in `WaitForCommit` exists in the design but is not sent).
- The 15 s batch deadline, the 2 s state waits of focus/close/transform, X11 timeouts (CLI-side,
  easy, but each needs its own range and tests).
- Logging (`SPAWN_AT_DEBUG` as a key), colour, backend choice.
- Update preferences through `spawn-at config` (they live in the same file already).
- A per-application override (for one slow app) and a `config edit` command.

## Live check (CONFIRMED, run once, installed extension untouched)

A config file with `[wait] cap_ms = 7, strategy = "ready-wait"` in a temporary `XDG_CONFIG_HOME`:
- `spawn-at-verify run --glob placement/bottom-right-larger`: the extension log has no
  `READY_WAIT_OVERRIDE` line and `READY_WAIT_START ... cap=150` (the installed extension's own
  default), run recorded `wait_cap_ms` 150: the config did not reach the run. (A first attempt
  used a `spawn-at-verify` binary built before the isolation and showed `cap=7`; rebuilt, the
  leak was gone. The unit test could not catch a stale binary, the live run did.)
- A direct `spawn-at spawn ... -- target/debug/fixture` with the same file: `--json` reports
  `resize.outcome` `timed-out`, `reason` `cap`, `waited_ms` 7: the file reached the extension
  through the existing fields.
- Not run: the installed extension was not reinstalled; `spawn.window_timeout_ms` was not
  exercised live (covered by the default being the old literal; HYPOTHESIS that a larger value
  behaves as read: the same `Duration` feeds both waits).

**Update channel.** `[update] channel` is `stable` (official releases only, the default) or `beta` (also offers pre-releases). `all` was the name of the beta channel in 0.2.0-alpha; it is a deprecated alias that is read as `beta` everywhere, so a config file that says `channel = "all"` keeps offering pre-releases. `install --update-channel all` and `update --channel all` still work and print one line saying `all` now means `beta`; `install` writes only `stable` or `beta`. `install --update-channel` rejects any other value; `update --channel` and a config file with an unknown value fall back to `stable`.
