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

| Flag or variable | Default | What it does | Source |
| :--- | :--- | :--- | :--- |
| `-v <tag>`, `--version <tag>` | latest stable release | Installs that release tag. The value must follow as the next word and must not start with `-`; otherwise the script prints `Flag --version / -v requires a release tag` and exits 1. The tag may contain only letters, digits, `.`, `_`, `+` and `-`, and must start with a letter or digit | `install.sh:57-70, 251-253` |
| `--version=<tag>` | | Same, in one word | `install.sh:71-74` |
| a pinned tag without the leading `v` | | If the download of `<tag>` fails and the tag does not start with `v`, the script tries `v<tag>` once | `install.sh:389` |
| `--prerelease` | off | Without `--version`, resolves the newest non-draft release including pre-releases (the first of the 10 newest releases the API returns), instead of `releases/latest` | `install.sh:75-78, 343` |
| `--allow-unsupported` | off | On a desktop that is not GNOME and not X11, installs without asking `Install anyway? [y/N]`. The script consumes it: `spawn-at` never sees it. If you go on, the installer runs with `SPAWN_AT_BACKEND=x11` set for that one process | `install.sh:79-83, 106-132, 433-436` |
| any other flag | | Passed on, in order, to `spawn-at install` | `install.sh:84-86, 441` |
| `--help`, `-h` | | **Not handled by the script.** The word is passed on like any other flag, so the script resolves a release, downloads and unpacks it, and `spawn-at install --help` prints its help. Read from source, not run | `install.sh:56-91` has no such case |
| `GITHUB_TOKEN` (environment) | unset | Sent as `Authorization: Bearer ...` on the GitHub API calls, to raise the rate limit | `install.sh:23, 264-266` |
| `SPAWN_AT_RESOLVE_ONLY=1` (environment) | unset | Prints the resolved tag and exits, with no download and no install. The unsupported-desktop question is not asked | `install.sh:24, 100-107, 355-358` |
| `SPAWN_AT_BACKEND` (environment) | unset | Read for the unsupported-desktop test: `x11` or `sway` (any letter case) counts as supported | `install.sh:100-107` |

How the release is found: with `--version`, the tag is used as given. Otherwise the script asks the
GitHub API (`releases/latest`, or `releases?per_page=10` with `--prerelease`), retrying, and falls
back to `releases.atom` (with `--prerelease`) or the redirect of `releases/latest` (stable). The
package itself is always downloaded from the plain `releases/download` URL
(`install.sh:7-17, 262-352`).

The script stops at once with a message when the machine is not Linux x86_64
(`install.sh:43-50`).

## `spawn-at install`

| Flag | Default | What it does | Source |
| :--- | :--- | :--- | :--- |
| `--scope <user\|system>` | `user` | Where the binary goes: `user` is `~/.local/bin/spawn-at`, `system` is `/usr/local/bin/spawn-at` | `src/platform/mod.rs:109-120`, `src/platform/installer.rs:20-21` |
| `--skip-bin` | off | Does not install the binary. The login update entry then points at the binary that is running | `src/main.rs:136-141, 337-343` |
| `--headless` | off | Never asks. The flags decide. See [Headless defaults](#headless-defaults) | `src/main.rs:106, 282` |
| `-y`, `--yes` | off | Goes on after the notice printed for an untested platform without asking. The notice is still printed | `src/platform/mod.rs:121-124`, `src/main.rs:106-118` |
| `--dry-run` | off | Prints the notice (if any) and `Dry run: would install the binary (<scope> scope...) and the <driver> extension; nothing was changed.`, then stops | `src/main.rs:126-134` |
| `--clean-install` | off | If spawn-at is already installed, removes it first (binary, extension, login update entry) without asking. Ignored, with a message, when `--skip-bin` is given | `src/main.rs:136-186` |
| `--update-channel <stable\|beta\|all>` | asked in a terminal; `stable` when headless | The update channel written to the config file. `all` prints `'all' is deprecated and now means 'beta'.` and is written as `beta` | `src/platform/mod.rs:129-130`, `src/main.rs:283-322` |
| `--update-notify [true\|false]` | asked in a terminal (default Yes); `false` when headless | Whether terminal update notices are on. The flag with no value means `true` | `src/platform/mod.rs:131-133`, `src/main.rs:302-326` |
| `--gnome-auto-enable [true\|false]` | `true` | GNOME: enable the extension after deploying it. **Read only when the run is not interactive** (`--headless`, or stdout is not a terminal); in a terminal the question `PROMPT_ENABLE_EXTENSION` is asked instead | `src/platform/linux/gnome/args.rs:21-22`, `deploy.rs:269-279` |
| `--gnome-x11-reload [true\|false]` | `true` | GNOME on X11: reload the Shell in place (applications stay open) | `args.rs:25-26, 41-49` |
| `--gnome-wayland-logout [true\|false]` | `false` | GNOME on Wayland: log out to load the extension. This closes your applications | `args.rs:29-30, 41-49` |
| `--no-action` | off | Does not reload or log out. Same as `--gnome-x11-reload=false --gnome-wayland-logout=false`, and it wins over both | `args.rs:33-34, 41-49` |
| `--no-wait` | off | Global flag: do not wait for the compositor | `--help` |

The log-out and reload flags decide only when there is no terminal; in a terminal the question is
asked last, with the exceptions in `src/session.rs`, `decision` (for example
`--gnome-wayland-logout` answers it with yes without asking).

### Headless defaults

With `--headless`, or when standard output is not a terminal, `install` asks nothing. By the code
of `src/main.rs:283-336`:

- the channel written is `stable` unless `--update-channel` is given;
- `update.notify` is written as `false` unless `--update-notify` is given;
- `update.auto_check` is always written as `true`;
- these three overwrite the `[update]` values already in the config file. The same happens in a
  terminal after you answer the two questions or pass the two flags.

This is read from source, not run. It differs from the sentences in `install.md` and the README
that say your configuration is kept: the file is kept, but these three keys are rewritten.

## `spawn-at uninstall`

| Flag | Default | What it does | Source |
| :--- | :--- | :--- | :--- |
| `--scope <user\|system>` | `user` | Which binary to remove | `src/platform/mod.rs:144-148` |
| `--skip-bin` | off | Does not remove the binary | `src/main.rs:358-390` |
| `--headless` | off | Never asks. Without `--skip-bin` a headless run removes the binary of the chosen scope | `src/main.rs:360, 380-390` |
| `--gnome-delete-files [true\|false]` | `true` | GNOME: remove the extension directory after disabling the extension. **Read only when the run is not interactive**; in a terminal `Delete the extension files?` is asked (default Yes) | `args.rs:61-63`, `deploy.rs:375-386` |
| `--gnome-x11-reload [true\|false]` | `true` | GNOME on X11: reload the Shell in place | `args.rs:65-67, 81-91` |
| `--gnome-wayland-logout [true\|false]` | `false` | GNOME on Wayland: log out afterwards. Closes your applications | `args.rs:69-71, 81-91` |
| `--no-action` | off | Does not reload or log out; wins over both | `args.rs:73-75, 81-91` |
| `--no-wait` | off | Global flag | `--help` |

`uninstall` has no `--dry-run` and no `--yes`. Run bare in a terminal, it first asks whether to
remove the binary (user scope, system scope or keep it; `src/main.rs:359-378`). It removes the
login update entry whether or not the binary is removed (`src/main.rs:398`).

## Not verified

- No flag was run. `install.sh` was not executed, and `spawn-at install` and `uninstall` were not
  run because they act on the live session.
- `--scope system`: whether it needs elevated rights was not checked.
- The `--help` behaviour of `install.sh` and the headless rewrite of the `[update]` table are read
  from source only.

## Sources

- `install.sh` (line numbers in the tables)
- `src/platform/mod.rs:109-148` (`InstallScope`, `InstallArgs`, `UninstallArgs`)
- `src/platform/linux/gnome/args.rs`, `src/platform/linux/gnome/deploy.rs:269-279, 375-386`
- `src/main.rs:100-186, 281-398`, `src/session.rs` (`decision`)
- the output of `spawn-at install --help` and `spawn-at uninstall --help`
