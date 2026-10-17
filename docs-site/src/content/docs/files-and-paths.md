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
a warning and the line to add (`src/platform/installer.rs:210-216`).

## Binary

`install` copies the running executable to `<bin dir>/spawn-at`, creates the directory if it is
missing, and sets mode 0755. If source and destination are the same file it skips the copy. If the
copy is refused with "permission denied" it prints a notice and retries through
`sudo install -D -m 755 <src> <dest>`, with the terminal attached so `sudo` can prompt
(`src/platform/installer.rs:129-200`, `src/platform/escalate.rs:13-35`). `uninstall` removes the
file the same way, with `sudo rm -f` as the fallback (`escalate.rs`, `remove_elevated`). A missing
file is not an error.

The bin directories are `$HOME/.local/bin` for user scope and `/usr/local/bin` for system scope
(`src/platform/linux/host.rs:22-36`).

## Extension directory

The GNOME driver writes the embedded extension files into
`$HOME/.local/share/gnome-shell/extensions/spawn-at@harsh.local`. It reads `$HOME`
only, not `XDG_DATA_HOME` (`src/platform/linux/gnome/deploy.rs:200-216`).

- The directory name is the extension's identifier, the `uuid` field of both `metadata.json` files
  (`assets/gnome/legacy/metadata.json`, `assets/gnome/modern/metadata.json`) and `EXTENSION_UUID` in
  `deploy.rs:28`.
- The file set is the **legacy** one when the Shell's major version is below 45, otherwise the
  **modern** one, also when the version cannot be read.
- The modern set is the 11 files of `assets/gnome/modern`: `anchor.js`, `cloak.js`, `commit.js`,
  `dbus.js`, `extension.js`, `logger.js`, `metadata.json`, `pulse.js`, `ready_wait.js`,
  `validator.js` and `x11_observe.js`. The legacy set is the 9 files of `assets/gnome/legacy` (the
  same names without `ready_wait.js` and `x11_observe.js`).
- A file whose bytes already match is not rewritten, so its modification time still says when the
  running Shell could have loaded it (`deploy.rs:234-238, 317-322`).
- Files already in the directory that are not in the set (for example from the other set) are left
  in place.
- `uninstall` runs `gnome-extensions disable` (its result is ignored) and then removes the whole
  directory, unless `--gnome-delete-files=false`, or you answered no in a terminal
  (`deploy.rs:395-410`).

### `gnome-extensions enable` and `disable`

`install` runs `gnome-extensions enable spawn-at@harsh.local` after writing the files, unless
`--gnome-auto-enable=false` (or you answered no in a terminal). `uninstall` runs
`gnome-extensions disable spawn-at@harsh.local` (`src/platform/linux/gnome/deploy.rs:28, 329, 400`).
GNOME keeps the list of enabled extensions in the `org.gnome.shell enabled-extensions` setting of
the user, and the repository records that `spawn-at@harsh.local` was seen in that value
(`docs/TEST_LOG.md` line 364, from a test where it ended up in the real database). So these two commands are expected to change a setting of your account
outside the three directories above. That the commands write exactly that key was not checked
here: it is not verified.

## Config file

The location comes from `SPAWN_AT_CONFIG` if set, otherwise `$XDG_CONFIG_HOME/spawn-at/config.toml`
when `XDG_CONFIG_HOME` is an absolute path, otherwise `$HOME/.config/spawn-at/config.toml`
(`src/platform/linux/config_store.rs:4, 24-34`). `spawn-at config path` prints the result. The file
holds the settings of [Configuration](/spawn-at/configuration/) and the `[update]` table. `uninstall`
and `install --clean-install` do not delete it: the `--clean-install` help says "your configuration
and settings are kept", and the only non-test `remove_file` in `src/config.rs` removes the login
entry (`src/config.rs:265`; `src/main.rs` has none). The `[update]` table of the file is rewritten
by `install` (see [Installer flags](/spawn-at/installer-flags/#headless-defaults)).

## Login update entry

`install` writes a desktop entry, `spawn-at-update-check.desktop`, in the `autostart` directory of
the config location. Its content (`src/config.rs:251-258`) is:

```text
Exec=<absolute path of the binary> update --check --headless
Hidden=false
NoDisplay=true
X-GNOME-Autostart-enabled=true
Comment=Check for spawn-at updates silently on login
```

plus the header lines of a desktop entry. The path in `Exec=` must be absolute (a relative path is
refused) and is quoted when it contains spaces or special characters. `SPAWN_AT_CONFIG` does not
move this file. See [Update](/spawn-at/update/) for what the command does and for a consequence
of how `update` writes the entry.

## Temporary files

- `update` creates `<temp dir>/spawn-at-update-<tag>-<pid>` with mode 0700 (the temp dir is Rust's
  `std::env::temp_dir()`, which is `$TMPDIR` or `/tmp`; the `$TMPDIR` rule is standard library
  behaviour, not checked here), downloads and unpacks the release there, and deletes it after a
  successful install (`src/update.rs:599-610, 685`). The functions return early with an error on
  a failed download, unpack or copy, and nothing before line 685 deletes the directory, so after
  such a failure it is left behind (read from source, not run).
- Before replacing the binary it renames the old one to `<name>.old` and removes that file after a
  successful copy; on a failed copy it renames it back (`src/update.rs:664-676`).
- `install.sh` makes a directory with `mktemp -d` and removes it with an `EXIT` trap
  (`install.sh:132-133`).

## Not verified

- Nothing was installed, removed or updated, so no path was seen on disk.
- The elevation fallback was read, not run.
- That `gnome-extensions enable` and `disable` write `enabled-extensions`.
- Paths on a system where `XDG_DATA_HOME` is set to something other than the default: the GNOME
  driver ignores it by the code, but no system was tried.

## Sources

- `src/platform/installer.rs:129-262`, `src/platform/escalate.rs`
- `src/platform/linux/host.rs:22-36`, `src/platform/linux/config_store.rs:4-34`
- `src/platform/linux/gnome/deploy.rs` (extension directory, install and uninstall)
- `src/config.rs:184-261`, `src/update.rs:584-722`, `install.sh`
- `assets/gnome/modern`, `assets/gnome/legacy` (file lists)
- `docs/TEST_LOG.md` line 364
