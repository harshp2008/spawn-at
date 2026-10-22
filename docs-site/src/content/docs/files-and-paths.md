---
title: "Files and paths"
---

Every file that spawn-at writes or removes on Linux, where it goes and which command touches it.
The paths are read from the source. No `install`, `uninstall` or `update` was run while writing
this page, because they act on the live session and the real home directory.

## Overview

| What | Path | Written by | Removed by |
| :--- | :--- | :--- | :--- |
| Binary, user scope (the default) | `~/.local/bin/spawn-at` | `install`, `update` (replaces in place) | `uninstall`, `install --clean-install` |
| Binary, system scope | `/usr/local/bin/spawn-at` | `install --scope system` | `uninstall --scope system` |
| GNOME Shell extension | `~/.local/share/gnome-shell/extensions/spawn-at@harsh.local/` | `install`, and the new binary's `install` during `update` | `uninstall` (unless `--gnome-delete-files=false`), `install --clean-install` |
| Config file | `$XDG_CONFIG_HOME/spawn-at/config.toml`, else `~/.config/spawn-at/config.toml` | `install`, `update`, `config set/unset/reset` | never |
| Login update entry | `$XDG_CONFIG_HOME/autostart/spawn-at-update-check.desktop`, else `~/.config/autostart/spawn-at-update-check.desktop` | `install` | `uninstall`, `install --clean-install` |
| Update work directory | `<temp dir>/spawn-at-update-<tag>-<pid>` | `update` | `update`, after a successful install |
| Update backup | the running binary's path with the extension `.old` | `update` | `update`, right after the copy |
| Installer work directory | `mktemp -d` | `install.sh` | `install.sh`, on exit |

`install` never edits a shell startup file. When the binary's directory is not in `PATH` it prints
a warning and the line to add.

To check what an install wrote, list the paths in this table, for example
`ls ~/.local/share/gnome-shell/extensions/spawn-at@harsh.local`. By the code that lists the 11 files
of the modern set (or the 9 of the legacy set) named below. If a path is missing or `spawn-at` is
not found, see [Troubleshooting](/spawn-at/troubleshooting/#spawn-at-command-not-found-after-install).

## Binary

`install` copies the running executable to `<bin dir>/spawn-at`, creates the directory if it is
missing, and sets mode 0755. If source and destination are the same file it skips the copy. If the
copy is refused with "permission denied" it prints a notice and retries through
`sudo install -D -m 755 <src> <dest>`, with the terminal attached so `sudo` can prompt. `uninstall`
removes the file the same way, with `sudo rm -f` as the fallback. A missing file is not an error.

The bin directories are `$HOME/.local/bin` for user scope and `/usr/local/bin` for system scope.

## Extension directory

The GNOME backend writes the embedded extension files into
`$HOME/.local/share/gnome-shell/extensions/spawn-at@harsh.local`. It reads `$HOME` only, not
`XDG_DATA_HOME`.

- The directory name is the extension's identifier, the `uuid` field of both `metadata.json` files
  (the constant `EXTENSION_UUID` holds the same string).
- The file set is the **legacy** one when the Shell's major version is below 45, otherwise the
  **modern** one, also when the version cannot be read.
- The modern set is the 11 files of `assets/gnome/modern`: `anchor.js`, `cloak.js`, `commit.js`,
  `dbus.js`, `extension.js`, `logger.js`, `metadata.json`, `pulse.js`, `ready_wait.js`,
  `validator.js` and `x11_observe.js`. The legacy set is the 9 files of `assets/gnome/legacy` (the
  same names without `ready_wait.js` and `x11_observe.js`).
- A file whose bytes already match is not rewritten, so its modification time still says when the
  running Shell could have loaded it.
- Files already in the directory that are not in the set (for example from the other set) are left
  in place.
- `uninstall` runs `gnome-extensions disable` (its result is ignored) and then removes the whole
  directory, unless `--gnome-delete-files=false`, or you answered no in a terminal.

### `gnome-extensions enable` and `disable`

`install` runs `gnome-extensions enable spawn-at@harsh.local` after writing the files, unless
`--gnome-auto-enable=false` (or you answered no in a terminal). `uninstall` runs
`gnome-extensions disable spawn-at@harsh.local`. GNOME keeps the list of enabled extensions in the
`org.gnome.shell enabled-extensions` setting of the user, and the repository records that
`spawn-at@harsh.local` was seen in that value (from a test where it ended up in the real database).
So these two commands are expected to change a setting of your account outside the three
directories above. That the commands write exactly that key was not checked here: it is not
verified.

## Config file

The location comes from `SPAWN_AT_CONFIG` if set, otherwise `$XDG_CONFIG_HOME/spawn-at/config.toml`
when `XDG_CONFIG_HOME` is an absolute path, otherwise `$HOME/.config/spawn-at/config.toml`.
`spawn-at config path` prints the result. The file holds the settings of
[Configuration](/spawn-at/configuration/) and the `[update]` table. `uninstall` and
`install --clean-install` do not delete it: the `--clean-install` help says "your configuration and
settings are kept", and the only non-test file removal in the config code deletes the login entry.
`install` writes the `[update]` table as described on
[Installer flags](/spawn-at/installer-flags/#headless-defaults).

## Login update entry

`install` writes a desktop entry, `spawn-at-update-check.desktop`, in the `autostart` directory of
the config location. Its content is:

```text
Exec=<absolute path of the binary> update --check --headless
Hidden=false
NoDisplay=true
X-GNOME-Autostart-enabled=true
Comment=Check for spawn-at updates silently on login
```

plus the header lines of a desktop entry. The path in `Exec=` must be absolute (a relative path is
refused) and is quoted when it contains spaces or special characters. `SPAWN_AT_CONFIG` does not
move this file. Which binary path `update` leaves in it, and what happens in the one case where it
is the temp directory, is on
[Update](/spawn-at/update/#the-child-install-and-your-settings).

## Temporary files

- `update` creates `<temp dir>/spawn-at-update-<tag>-<pid>` with mode 0700 (the temp dir is Rust's
  `std::env::temp_dir()`, which is `$TMPDIR` or `/tmp`; the `$TMPDIR` rule is standard library
  behaviour, not checked here), downloads and unpacks the release there, and deletes it after a
  successful install. The functions return early with an error on a failed download, unpack or
  copy, and nothing before the final deletion removes the directory, so after such a failure it is
  left behind (read from source, not run).
- Before replacing the binary it renames the old one to `<name>.old` and removes that file after a
  successful copy; on a failed copy it renames it back.
- `install.sh` makes a directory with `mktemp -d` and removes it with an `EXIT` trap.

## Not verified

- Nothing was installed, removed or updated, so no path was seen on disk.
- The elevation fallback was read, not run.
- That `gnome-extensions enable` and `disable` write `enabled-extensions`.
- Paths on a system where `XDG_DATA_HOME` is set to something other than the default: the GNOME
  backend ignores it by the code, but no system was tried.

## Sources

- `src/platform/installer.rs`: lines 129-262 (copy, elevation, removal) and 210-216 (the `PATH`
  warning); `src/platform/escalate.rs`: lines 13-35 and `remove_elevated`
- `src/platform/linux/host.rs`: lines 22-36 (bin directories);
  `src/platform/linux/config_store.rs`: lines 4-34 (config location)
- `src/platform/linux/gnome/deploy.rs`: line 29 (`EXTENSION_UUID`), lines 303-316 (`$HOME` only),
  425-427 (unchanged files are not rewritten), 435 and 514 (`gnome-extensions`),
  525 (`uninstall` removes the directory)
- `src/config.rs`: lines 184-261 (config path and autostart entry; the entry's content is at 251-258),
  265 (the only non-test `remove_file` in that file; `src/main.rs` has none)
- `src/update.rs`: lines 606-751 (the work directory at 621-632, the `.old` backup at 694-706, the
  deletion at 715)
- `install.sh`: lines 132-133 (`mktemp -d` and the `EXIT` trap)
- `assets/gnome/modern`, `assets/gnome/legacy` (file lists and `metadata.json`)
- `docs/TEST_LOG.md`, line 364 (`enabled-extensions` seen in a test)
