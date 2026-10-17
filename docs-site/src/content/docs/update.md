---
title: "Update"
---

`spawn-at update` checks GitHub for a newer release of spawn-at and, if you agree, replaces the
running binary with it. It does not need a window backend, so it works on any desktop. Everything
below is read from `src/update.rs` unless it says otherwise; no update was run while writing this
page.

## Commands

```bash
spawn-at update                    # check, then ask what to do (in a terminal)
spawn-at update --check            # look for a newer release and print the result
spawn-at update --channel beta     # use another channel for this run
spawn-at update --force            # offer a reinstall even when already current
spawn-at update --headless         # never ask; install if a newer release exists
```

| Flag | What it does |
| :--- | :--- |
| `--check` | Looks for a candidate and prints `An update is available: <current> → <tag>` or `spawn-at is up to date (<current>).` It installs nothing. It does write to the config file, see [Check also writes the cache](#check-also-writes-the-cache) |
| `--channel <stable\|beta>` | Overrides the configured channel for this run only. `all` is accepted, prints `'all' is deprecated and now means 'beta'.`, and is read as `beta` |
| `--force` | Installs the newest release of the channel even if it is not newer than the running one |
| `--headless` | Does not show the menu. Without `--check` it installs when a newer release exists (or with `--force`), and otherwise prints `Already up to date. Use --force to reinstall.` |

`--headless --check` together is a different, silent mode: it prints nothing, stores the newest
stable and the newest pre-release tag it finds in the config file, and returns. A failed lookup is
ignored there. The login entry and the background check use this mode (see below).

## What a check does

1. It calls `curl -fsSL -H "User-Agent: spawn-at"` on `https://api.github.com/repos/harshp2008/spawn-at/releases/latest`
   (channel `stable`) or `.../releases` (channel `beta`). For `stable`, a failure of the `latest`
   call falls back to the full `.../releases` list.
2. It ignores drafts and tags that are not valid semantic versions. On `stable` it also ignores
   pre-releases.
3. It picks the highest version that is greater than the running one. If none is greater, the
   answer is "up to date".

If the lookup fails, `update` prints `Error checking updates: ...` through the error renderer and
returns without an error status (`src/update.rs:786`, then `return Ok(())`). The process therefore
exits 0 on a failed lookup. This is read from source, not run.

### A build that is not a release

A binary built from a commit that is not on a release tag has no parseable version (`spawn-at -V`
prints `DEV BUILD`). For such a build:

- `update --check` finds no candidate and prints `spawn-at is up to date (dev).`
- `update` without `--check` prints `Running build does not have a parseable release version (dev
  build).` and `spawn-at update refuses to upgrade without an eligible candidate. Use --force to
  override.` and stops (`src/update.rs:833-834`).
- `--force` skips that refusal.

Both are read from source, not run.

## The menu

In a terminal, without `--headless`, `update` shows four choices (`src/update.rs:716-727`):

| Item | Does |
| :--- | :--- |
| `Upgrade to <tag> now [Recommended]` (or `Reinstall <tag> (Force download)` when there is nothing newer and `--force` was given) | Downloads and installs |
| `Change update channel (Currently: <channel>)` | Asks `stable` or `beta`, **saves** the choice to the config file, and checks again |
| `Re-check GitHub for updates` | Checks again |
| `Cancel` | Prints `Update cancelled.` |

When standard output is not a terminal, or with `--headless`, there is no menu.

## What an install does

`perform_upgrade` (`src/update.rs:584-722`):

1. Finds the release asset named `spawn-at-<target triple>.tar.gz`. For the one released target that
   is `spawn-at-x86_64-unknown-linux-gnu.tar.gz`. A release without that asset ends with
   `Release <tag> is missing pre-built binary asset '<name>'`.
2. Creates `<temp dir>/spawn-at-update-<tag>-<pid>` with mode 0700 and downloads the asset into it
   with `curl -# -L`.
3. Unpacks it with `tar -xzf ... --no-same-owner`, and refuses to go on unless `spawn-at` in it is
   a regular file.
4. On GNOME (the desktop name in `XDG_CURRENT_DESKTOP` contains GNOME), runs the **new** binary as
   `install --skip-bin --headless`, to put the extension files from the new version in place. It
   waits for that child to exit (`.status()`, `src/platform/linux/gnome/deploy.rs:472-474`) and
   sets no working directory and no environment for it, so it inherits both. Only a failure to start
   the child is reported; its exit status is not looked at
   (`src/platform/linux/gnome/deploy.rs:463-484`, called at `src/update.rs:656`).
5. Renames the running binary to the same name with the extension `.old`, copies the new one over
   the old path, and deletes the `.old` file. If the copy fails it renames the backup back and
   returns the error. The mode is then set to 0755. The path replaced is the one the running
   binary was started from (`current_exe`), which is not necessarily one of the directories the
   installer uses.
6. Deletes the temp directory (`src/update.rs:685`, after the child of step 4 has exited) and
   stores the installed tag in the config file as a cached version.
7. On GNOME and in a terminal, records the end-of-run question; on Wayland that is the log-out
   question, on X11 the Shell reload (`src/session.rs`, `update_outcome`). The log-out happens only
   if you answer yes.

The message `Successfully upgraded spawn-at to <tag>!` is printed before step 7.

### No checksum or signature check

The code does not verify what it downloads. `grep -n -i "sha256\|checksum\|signature\|gpg\|minisign"
src/update.rs install.sh` printed nothing. The only checks are that the download and the unpacking
exit 0 and that the unpacked `spawn-at` is a regular file. The download is over HTTPS from the
release asset URL that the GitHub API returns.

### Code finding: the child install rewrites settings

Step 4 runs `install --skip-bin --headless`. By the code of `install` (`src/main.rs:283-347`) a
headless install sets `auto_check`, `channel` and `notify` in the `[update]` table unconditionally
(`src/main.rs:330-332`): `auto_check = true`, the channel from `--update-channel` or `stable`, and
`notify` from `--update-notify` or `false`. The update passes no flags, so the values it writes are
`true`, `stable` and `false`. The install then writes the login entry from the path of the binary
that is running: with `--skip-bin` that is `current_exe` (`src/main.rs:341-343`), and in this call
it is the binary in the temp directory. The `Exec=` line is built from that path and nothing else
(`src/config.rs:252-258`). This follows from reading the code, and none of it was run:

- an update on GNOME resets `auto_check` to `true`, a configured `beta` channel to `stable` and
  `notify = true` to `false`. The parent reloads the file after the child exits and keeps those
  values (`src/update.rs:688`);
- the `Exec=` line of the login entry is written with the temp directory path
  (`<temp dir>/spawn-at-update-<tag>-<pid>/spawn-at`). Step 6 then deletes that directory.

Nothing rewrites the login entry afterwards. The writer is called only at `src/main.rs:347`, and
`update` and `update --check` never touch the entry. So after an update on GNOME the entry points
at a path that no longer exists, until a later `spawn-at install` without `--skip-bin` writes the
installed binary's path. What GNOME does at login with a missing `Exec=` target was not
verified. On a desktop where `XDG_CURRENT_DESKTOP` does not contain `GNOME` the child is not run
(`src/platform/linux/gnome/deploy.rs:455-460, 470`), so none of this happens.

## Notices and the background check

At the end of most commands (not `update`, not `config`, not `query layout --json` or `query
windows --json`), `render_update_notice_if_available` runs (`src/main.rs:461`,
`src/update.rs:531-575`). It does nothing unless **both** standard output and standard error are
terminals and `update.notify` is true. When it runs it:

1. starts a detached `spawn-at update --check --headless` if `update.auto_check` is true and the
   last check is more than 86400 seconds (24 hours) old (`src/update.rs:283-300`);
2. reads the cached tag for the channel, and prints a boxed notice when that tag is greater than
   the running version. A build with no parseable version prints nothing.

Because step 1 sits after the `notify` test, `auto_check` has an effect only when `notify` is true.
This is read from source, not run.

### The login entry

`spawn-at install` writes an autostart entry that runs `<binary> update --check --headless` when
you log in (`src/config.rs:184-261`; the path is on [Files and paths](/spawn-at/files-and-paths/)).
That command does not read `auto_check`, so setting `auto_check = false` does not stop the login
check. `uninstall` removes the entry. After an `update` on GNOME its `Exec=` path is the deleted
temp path described in [Code finding](#code-finding-the-child-install-rewrites-settings).

## Channels and notices

`stable` (the default) offers official releases. `beta` also offers pre-releases. Any other value
in the config file is read as `stable`. They are set with `spawn-at install --update-channel`,
`spawn-at update --channel` (this run only), or the menu.

## Version 0.1 to 0.2

When the running version is 0.1.x and the chosen release is 0.2 or later, `update --check` and the
install path print `Note: spawn-at v0.2 CLI is not backward-compatible with v0.1 command syntax.`
(`src/update.rs:131-133, 805, 851`), and the notice box adds a line about it. The code does not say
what changed. See the changelog for that.

## Check also writes the cache

`update --check` is not read-only: when it finds a candidate it stores that tag and the check time
in the `[update]` table of the config file and saves it (`src/update.rs:788-799`). The silent
`--headless --check` mode does the same (`src/update.rs:744-775`).

## Not verified

- No `update` run: the download, the replacement, the extension step, the log-out question and the
  code findings above were read from source only.
- Behaviour on a platform other than Linux x86_64.
- What GNOME does at login when the `Exec=` target of the autostart entry does not exist.
- What happens when the replaced path is not writable (system scope). The code ignores the result
  of the first rename (`let _ =`), but this was not run.

## Sources

- `src/update.rs` (lines cited above)
- `src/platform/update_hooks.rs`, `src/platform/linux/gnome/deploy.rs:463-484`
- `src/session.rs` (`update_outcome`), `src/main.rs:283-347, 461`
- `src/config.rs:18-44, 184-261`, `src/prompts.rs:54-77`
- the output of `spawn-at update --help`
- `grep -n -i "sha256\|checksum\|signature\|gpg\|minisign" src/update.rs install.sh` (no output)
