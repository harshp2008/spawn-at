---
title: "Installer flags"
---

Two programs take install flags. `install.sh` is the script you pipe to `bash`; it parses four
flags of its own and passes everything else to `spawn-at install`. `spawn-at install` and
`spawn-at uninstall` are the binary's commands. This page lists every flag of all three, one row
each. The generated pages [`install`](/spawn-at/reference/spawn-at-install/) and
[`uninstall`](/spawn-at/reference/spawn-at-uninstall/) show the raw `--help`. Nothing here was
run: the rows are read from `install.sh`, the clap definitions and `--help`.

## `install.sh`

```bash
curl -fsSL https://raw.githubusercontent.com/harshp2008/spawn-at/main/install.sh | bash -s -- <flags>
```

| Flag | Default | What it does |
| :--- | :--- | :--- |
| `-v <tag>`, `--version <tag>` | latest stable release | Installs that release tag. The value must follow as the next word and must not start with `-`; otherwise the script prints `Flag --version / -v requires a release tag` and exits 1. The tag may contain only letters, digits, `.`, `_`, `+` and `-`, and must start with a letter or digit |
| `--version=<tag>` | | Same, in one word |
| `--prerelease` | off | Without `--version`, resolves the newest non-draft release including pre-releases (the first of the 10 newest releases the API returns), instead of `releases/latest` |
| `--allow-unsupported` | off | On a desktop that is not GNOME and not X11, installs without asking `Install anyway? [y/N]`. The script consumes it: `spawn-at` never sees it. If you go on, the installer runs with `SPAWN_AT_BACKEND=x11` set for that one process |
| any other flag | | Passed on, in order, to `spawn-at install` |
| `--help`, `-h` | | Prints the script's own flags (`--version`/`-v`, `--prerelease`, `--allow-unsupported`, `--help`), says that every other flag is passed on to `spawn-at install`, names `GITHUB_TOKEN` and `SPAWN_AT_RESOLVE_ONLY`, and exits 0. It is looked for before the platform check, the desktop check, the temp directory and every network call, so nothing is resolved or downloaded. It is not passed on: `spawn-at install --help` shows the binary's flags. Read from source and `bash -n` only, not run |

If the download of a pinned `<tag>` fails and the tag does not start with `v`, the script tries
`v<tag>` once.

### Environment variables of `install.sh`

| Variable | Default | What it does |
| :--- | :--- | :--- |
| `GITHUB_TOKEN` | unset | Sent as `Authorization: Bearer ...` on the GitHub API calls, to raise the rate limit |
| `SPAWN_AT_RESOLVE_ONLY=1` | unset | Prints the resolved tag and exits, with no download and no install. The unsupported-desktop question is not asked |
| `SPAWN_AT_BACKEND` | unset | Read for the unsupported-desktop test: `x11` or `sway` (any letter case) counts as supported |

Every variable the binary reads is on [Environment and exit codes](/spawn-at/environment/).

How the release is found: with `--version`, the tag is used as given. Otherwise the script asks the
GitHub API (`releases/latest`, or `releases?per_page=10` with `--prerelease`), retrying, and falls
back to `releases.atom` (with `--prerelease`) or the redirect of `releases/latest` (stable). The
package itself is always downloaded from the plain `releases/download` URL.

The script stops at once with a message when the machine is not Linux x86_64.

## `spawn-at install`

| Flag | Default | What it does |
| :--- | :--- | :--- |
| `--scope <user\|system>` | `user` | Where the binary goes: `user` is `~/.local/bin/spawn-at`, `system` is `/usr/local/bin/spawn-at` |
| `--skip-bin` | off | Does not install the binary. The login update entry then points at the installed copy rather than the running binary; the rule is on [Update](/spawn-at/update/#the-child-install-and-your-settings), which also says why `update` uses this flag |
| `--headless` | off | Never asks. The flags decide. See [Headless defaults](#headless-defaults) |
| `-y`, `--yes` | off | Goes on after the notice printed for an untested platform without asking. The notice is still printed |
| `--dry-run` | off | Prints the notice (if any) and `Dry run: would install the binary (<scope> scope...) and the <driver> extension; nothing was changed.`, then stops. The text is quoted as the binary prints it |
| `--clean-install` | off | If spawn-at is already installed, removes it first (binary, extension, login update entry) without asking. Ignored, with a message, when `--skip-bin` is given |
| `--update-channel <stable\|beta\|all>` | asked in a terminal; `stable` when headless | The update channel written to the config file. `all` prints `'all' is deprecated and now means 'beta'.` and is written as `beta` |
| `--update-notify [true\|false]` | asked in a terminal (default Yes); `false` when headless | Whether terminal update notices are on. The flag with no value means `true` |
| `--gnome-auto-enable [true\|false]` | `true` | GNOME: enable the extension after deploying it. **Read only when the run is not interactive** (`--headless`, or stdout is not a terminal); in a terminal the question `PROMPT_ENABLE_EXTENSION` is asked instead |
| `--gnome-x11-reload [true\|false]` | `true` | GNOME on X11: reload the Shell in place (applications stay open) |
| `--gnome-wayland-logout [true\|false]` | `false` | GNOME on Wayland: log out to load the extension. This closes your applications |
| `--no-action` | off | Does not reload or log out. Same as `--gnome-x11-reload=false --gnome-wayland-logout=false`, and it wins over both |
| `--no-wait` | off | Global flag: do not wait for the compositor |

The log-out and reload flags decide only when there is no terminal; in a terminal the question is
asked last, with exceptions (for example `--gnome-wayland-logout` answers it with yes without
asking).

To preview an install without changing anything, use `--dry-run`: by the code it prints the notice
for your platform, if there is one, and the `Dry run: ...` line, then stops. Not run here, because
`install` acts on the live session. If a flag does not do what this table says, see
[Troubleshooting](/spawn-at/troubleshooting/).

### Headless defaults

With `--headless`, or when standard output is not a terminal, `install` asks nothing. By the code,
each of the three `[update]` keys is decided on its own:

- `update.channel`: set from `--update-channel` if given; else kept if the config file has the key;
  else `stable`;
- `update.notify`: set from `--update-notify` if given; else kept if the file has the key; else
  `false`;
- `update.auto_check`: kept if the file has the key; else `true` (there is no flag for it).

"The file has the key" is read from the file as it is on disk, so a key set to its default counts
as present. A file that is missing, is not valid TOML, or has no `[update]` table has none of the
keys, so the three defaults are written, as before. The `install --skip-bin --headless` that
`update` runs on GNOME passes no flags, so it keeps your values (see
[Update](/spawn-at/update/#the-child-install-and-your-settings)). In a terminal without
`--headless`, `install` asks for the channel and `notify` (unless the flags are given) and writes
both, together with `auto_check = true`, as before.

This is read from source and unit-tested, not run on a live install. The file is kept; the three
keys are rewritten only as described here.

## `spawn-at uninstall`

| Flag | Default | What it does |
| :--- | :--- | :--- |
| `--scope <user\|system>` | `user` | Which binary to remove |
| `--skip-bin` | off | Does not remove the binary |
| `--headless` | off | Never asks. Without `--skip-bin` a headless run removes the binary of the chosen scope |
| `--gnome-delete-files [true\|false]` | `true` | GNOME: remove the extension directory after disabling the extension. **Read only when the run is not interactive**; in a terminal `Delete the extension files?` is asked (default Yes) |
| `--gnome-x11-reload [true\|false]` | `true` | GNOME on X11: reload the Shell in place |
| `--gnome-wayland-logout [true\|false]` | `false` | GNOME on Wayland: log out afterwards. Closes your applications |
| `--no-action` | off | Does not reload or log out; wins over both |
| `--no-wait` | off | Global flag |

`uninstall` has no `--dry-run` and no `--yes`. Run bare in a terminal, it first asks whether to
remove the binary (user scope, system scope or keep it). It removes the login update entry whether
or not the binary is removed.

## Not verified

- No flag was run. `install.sh` was not executed, and `spawn-at install` and `uninstall` were not
  run because they act on the live session.
- `--scope system`: whether it needs elevated rights was not checked.
- The `--help` of `install.sh` was checked with `bash -n` only (`shellcheck` is not installed
  here); the script was not run. The headless `[update]` rules are read from source and
  unit-tested only.

## Sources

- `install.sh`: lines 97-110 and 291-293 (`--version`), 111-114 (`--version=`), 429 (retry with
  `v<tag>`), 115-118 and 383 (`--prerelease`), 119-123 and 146-172 and 473-476
  (`--allow-unsupported`), 124-126 and 481 (flags passed on), 44-78 (`--help`), 24 and 304-306
  (`GITHUB_TOKEN`), 25 and 140-147 and 395-398 (`SPAWN_AT_RESOLVE_ONLY`, `SPAWN_AT_BACKEND`), 7-17
  and 262-352 (how the release is found), 43-50 (platform check)
- `src/platform/mod.rs`: lines 109-148 (`InstallScope`, `InstallArgs`, `UninstallArgs`);
  `src/platform/installer.rs`: lines 20-21 (bin directories) and 61
- `src/platform/linux/gnome/args.rs`: lines 21-34 and 41-49 (install flags), 61-91 (uninstall flags);
  `src/platform/linux/gnome/deploy.rs`: lines 333-349 (the doc comment of `install`) and 435-455 (the
  `enable` call and its note)
- `src/main.rs`: lines 100-191 (install flags, `--dry-run` at 131-139, `--clean-install` at
  141-191), 291-357 (headless `[update]` keys), 362-370 (`--skip-bin`), 384-429 (uninstall: the
  prompt for the binary at 385-409, the login entry removal at 429); `UpdateConfig::apply_headless_install`
  in `src/config.rs`
- `src/session.rs`: `decision` (when the log-out question is asked and when a flag answers it)
- the output of `spawn-at install --help` and `spawn-at uninstall --help`
