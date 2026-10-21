---
title: "Update"
---

`spawn-at update` checks GitHub for a newer release of spawn-at and, if you agree, replaces the
running binary with it. It does not need a window backend, so it works on any desktop. The first
half of this page is what you do and what you will see; the mechanism starts at
[How it works](#how-it-works). Everything below is read from the source unless it says otherwise; no
update was run while writing this page.

## Run it

```bash
spawn-at update                    # check, then ask what to do (in a terminal)
spawn-at update --check            # look for a newer release and print the result
spawn-at update --channel beta     # use another channel for this run
spawn-at update --force            # offer a reinstall even when already current
spawn-at update --headless         # never ask; install if a newer release exists
```

| Flag | What it does |
| :--- | :--- |
| `--check` | Looks for a candidate and prints `An update is available: <current> → <tag>` or `spawn-at is up to date (<current>).` It installs nothing and saves nothing to the config file, see [What `--check` saves](#what---check-saves) |
| `--channel <stable\|beta>` | Overrides the configured channel for this run only. `all` is accepted, prints `'all' is deprecated and now means 'beta'.`, and is read as `beta` |
| `--force` | Installs the newest release of the channel even if it is not newer than the running one |
| `--headless` | Does not show the menu. Without `--check` it installs when a newer release exists (or with `--force`), and otherwise prints `Already up to date. Use --force to reinstall.` |

`--headless --check` together is a different, silent mode: it prints nothing, stores the newest
stable and the newest pre-release tag it finds in the config file, and returns. A failed lookup is
ignored there. With `update.auto_check = false` it returns at once, before the lookup, and stores
nothing. The login entry and the background check use this mode (see
[The login entry](#the-login-entry)).

## What you will see

| When | Output |
| :--- | :--- |
| `update --check`, a newer release exists | `An update is available: <current> → <tag>` |
| `update --check`, nothing newer | `spawn-at is up to date (<current>).` |
| `update --headless`, nothing newer, no `--force` | `Already up to date. Use --force to reinstall.` |
| after an install | `Successfully upgraded spawn-at to <tag>!` |
| the lookup of releases failed | `Error checking updates: ...`; the exit status is still 0 |
| the release has no file for this platform | `Release <tag> is missing pre-built binary asset '<name>'` |
| you choose Cancel in the menu | `Update cancelled.` |

### The menu

In a terminal, without `--headless`, `update` shows four choices:

| Item | Does |
| :--- | :--- |
| `Upgrade to <tag> now [Recommended]` (or `Reinstall <tag> (Force download)` when there is nothing newer and `--force` was given) | Downloads and installs |
| `Change update channel (Currently: <channel>)` | Asks `stable` or `beta`, **saves** the choice to the config file, and checks again |
| `Re-check GitHub for updates` | Checks again |
| `Cancel` | Prints `Update cancelled.` |

When standard output is not a terminal, or with `--headless`, there is no menu.

### A build that is not a release

A binary built from a commit that is not on a release tag has no parseable version (`spawn-at -V`
prints `DEV BUILD`). For such a build:

- `update --check` finds no candidate and prints `spawn-at is up to date (dev).`
- `update` without `--check` prints `Running build does not have a parseable release version (dev
  build).` and `spawn-at update refuses to upgrade without an eligible candidate. Use --force to
  override.` and stops.
- `--force` skips that refusal.

Both are read from source, not run.

## After an update

- **The end-of-run question.** On GNOME and in a terminal, `update` ends with a question: on
  Wayland the log-out question, on X11 the Shell reload. The log-out happens only if you answer yes.
  On Wayland the new extension files are loaded only after you log out and back in.
- **No terminal (the login entry).** `update --headless` without a terminal has nobody to ask.
  When it changes the deployed extension files on Wayland, it sends one desktop notification,
  `spawn-at updated its GNOME extension; log out and back in to finish`, and writes
  `$XDG_STATE_HOME/spawn-at/needs-logout` (`~/.local/state/spawn-at/needs-logout` when that
  variable is not set) with the protocol and a fingerprint of the deployed files. While that file
  exists and the loaded extension reports a lower protocol, `spawn` prints one hint line; once the
  loaded protocol has caught up, `spawn` removes the file. There is no notification and no file on
  X11, on a first install, when the files did not change, or for the same deployed files a second
  time. A change that keeps the protocol number (for example a new Shell version in
  `metadata.json`) is announced once but gives no `spawn` hint, because the extension exposes only a
  protocol number. Run end to end against a fake release in a temporary tree on a private session
  bus with a stub notification service, not on a real login.
- **Check the version.** Run `spawn-at --version`. By the code it prints `spawn-at <version>` for a
  release build, with the new tag (not run). A build that is not on a release tag prints `DEV BUILD`
  and cannot show this.
- **Your settings.** On GNOME your `[update]` values stay; see
  [The child install and your settings](#the-child-install-and-your-settings).
- **If it did not work.** `command not found`, a mismatch warning or a D-Bus error after an
  update: [the extension is not active entry](/spawn-at/troubleshooting/#the-extension-was-installed-but-is-not-active-or-extension-is-older-than-the-cli-expects).
  `up to date (dev)`: [that entry](/spawn-at/troubleshooting/#spawn-at-update-says-up-to-date-dev).
  A failed download or unpack leaves its work directory behind
  ([Files and paths](/spawn-at/files-and-paths/#temporary-files)).

## Notices and the background check

At the end of most commands (not `update`, not `config`, not `query layout --json` or `query
windows --json`), a check for a cached newer release runs. It does nothing unless **both** standard
output and standard error are terminals and `update.notify` is true. When it runs it:

1. starts a detached `spawn-at update --check --headless` if `update.auto_check` is true and the
   last check is more than 86400 seconds (24 hours) old;
2. reads the cached tag for the channel, and prints a boxed notice when that tag is greater than
   the running version. A build with no parseable version prints nothing.

Because step 1 sits after the `notify` test, `auto_check` has an effect only when `notify` is true.
This is read from source, not run.

### The login entry

`spawn-at install` writes an autostart entry that runs `<binary> update --check --headless` when
you log in (the path is on [Files and paths](/spawn-at/files-and-paths/#login-update-entry)).
With `update.auto_check = false` that command returns before it asks GitHub and stores nothing, so
the login check does nothing (the entry stays in place; `uninstall` removes it). After an `update`
on GNOME its `Exec=` path is the installed binary, see
[The child install and your settings](#the-child-install-and-your-settings).

## Channels and notices

`stable` (the default) offers official releases. `beta` also offers pre-releases. Any other value
in the config file is read as `stable`. They are set with `spawn-at install --update-channel`,
`spawn-at update --channel` (this run only), or the menu.

## Version 0.1 to 0.2

When the running version is 0.1.x and the chosen release is 0.2 or later, `update --check` and the
install path print `Note: spawn-at v0.2 CLI is not backward-compatible with v0.1 command syntax.`,
and the notice box adds a line about it. The code does not say what changed. See the changelog for
that.

## How it works

### What a check does

1. It calls `curl -fsSL -H "User-Agent: spawn-at"` on `https://api.github.com/repos/harshp2008/spawn-at/releases/latest`
   (channel `stable`) or `.../releases` (channel `beta`). For `stable`, a failure of the `latest`
   call falls back to the full `.../releases` list.
2. It ignores drafts and tags that are not valid semantic versions. On `stable` it also ignores
   pre-releases.
3. It picks the highest version that is greater than the running one. If none is greater, the
   answer is "up to date".

If the lookup fails, `update` prints `Error checking updates: ...` through the error renderer and
returns without an error status: the error is rendered, then `run_update` returns `Ok(())`. The
process therefore exits 0 on a failed lookup. This is read from source, not run.

### What an install does

`perform_upgrade` does these steps:

1. Finds the release asset named `spawn-at-<target triple>.tar.gz`. For the one released target that
   is `spawn-at-x86_64-unknown-linux-gnu.tar.gz`. A release without that asset ends with
   `Release <tag> is missing pre-built binary asset '<name>'`.
2. Creates `<temp dir>/spawn-at-update-<tag>-<pid>` with mode 0700 and downloads the asset into it
   with `curl -# -L`.
3. Unpacks it with `tar -xzf ... --no-same-owner`, and refuses to go on unless `spawn-at` in it is
   a regular file.
4. On GNOME (the desktop name in `XDG_CURRENT_DESKTOP` contains GNOME), runs the **new** binary as
   `install --skip-bin --headless`, to put the extension files from the new version in place. It
   waits for that child to exit and sets no working directory and no environment for it, so it
   inherits both. Only a failure to start the child is reported; its exit status is not looked at.
5. Renames the running binary to the same name with the extension `.old`, copies the new one over
   the old path, and deletes the `.old` file. If the copy fails it renames the backup back and
   returns the error. The mode is then set to 0755. The path replaced is the one the running
   binary was started from (`current_exe`), which is not necessarily one of the directories the
   installer uses.
6. Deletes the temp directory (after the child of step 4 has exited) and stores the installed tag
   in the config file as a cached version.
7. On GNOME and in a terminal, records the end-of-run question; on Wayland that is the log-out
   question, on X11 the Shell reload. The log-out happens only if you answer yes.

The message `Successfully upgraded spawn-at to <tag>!` is printed before step 7.

### No checksum or signature check

The code does not verify what it downloads. `grep -n -i "sha256\|checksum\|signature\|gpg\|minisign"
src/update.rs install.sh` printed nothing. The only checks are that the download and the unpacking
exit 0 and that the unpacked `spawn-at` is a regular file. The download is over HTTPS from the
release asset URL that the GitHub API returns.

### The child install and your settings

Step 4 runs `install --skip-bin --headless`. By the code, that install does two things that matter
after an update. Neither was run: this is read from the source (see Sources).

- **The `[update]` values stay.** `update` passes that install no flags, and an install without a
  terminal leaves a key the config file already has as it is, so an update on GNOME keeps your
  `auto_check`, `channel` and `notify`. A key the file lacks gets its headless default. The rule,
  key by key, is on [Installer flags](/spawn-at/installer-flags/#headless-defaults).
- **The login entry points at the installed binary.** With `--skip-bin` the entry's `Exec=` is the
  installed copy: the file at the bin directory of the requested scope (user, unless
  `--scope system`), or else the one at the other scope's. Only if neither exists is it the running
  binary (`current_exe`), as before. In the update's child that running binary is the one in the
  temp directory that step 6 deletes, so a machine that has an installed copy never gets that path
  in the entry.

What is left: when the installed copy is in neither bin directory (for example a binary that was
copied by hand elsewhere, and `update` was started from it), the entry gets the temp directory path
as before, and nothing rewrites it after the update. What GNOME does at login with a missing
`Exec=` target was not verified. On a desktop where `XDG_CURRENT_DESKTOP` does not contain `GNOME`
the child is not run, so none of this happens.

A build from before this change overwrote the three values and wrote the temp directory path. If
`update` was run with such a build, check `spawn-at config path` and the entry (see
[Files and paths](/spawn-at/files-and-paths/)).

### What `--check` saves

`update --check` in a terminal looks, prints and saves nothing: it does not touch the config file.
`update` without `--check` (and `update --headless`) stores the candidate it found, as the cached
stable or beta tag, plus the check time, in the `[update]` table before it goes on. The silent
`--headless --check` mode stores the newest stable and newest pre-release tag and the check time,
unless `auto_check` is false. The cache keys stay in the config file; they are not moved to a
separate cache file.

## Not verified

- No `update` run: the download, the replacement, the extension step, the log-out question, the
  child install and the `--check` behaviour above were read from source and unit-tested only.
- What GNOME does at login when the `Exec=` target of the autostart entry does not exist.
- What happens when the replaced path is not writable (system scope). The code ignores the result
  of the first rename (`let _ =`), but this was not run.
- Behaviour on a platform other than Linux x86_64 is on
  [Known shortcomings](/spawn-at/shortcomings/#not-verified-anywhere).
- The `spawn-at --version` line after an update is read from the code, not seen.

## Sources

- `src/update.rs`: `get_version_display` (lines 60-72); `run_update` (renders the lookup error and
  returns `Ok(())`; refuses a build with no parseable version); `update_menu_options` (the menu);
  `perform_upgrade` (lines 584-722: asset name, temp directory at 599-610, the child at 656, the
  `.old` rename at 664-676, the temp directory deletion at 685, the success message at 698);
  `render_update_notice_if_available` (lines 531-575) and the 24-hour rule (lines 283-300);
  `lookup_is_cached` and `headless_check` (what `--check` saves); the 0.1 to 0.2 note (lines 131-133,
  805, 851); the failed-lookup exit (lines 786-788)
- `src/main.rs`: line 461 (the notice at the end of `main`) and the `Commands::Install` arm
- `src/platform/update_hooks.rs`; `src/platform/linux/gnome/deploy.rs`: lines 463-484 (the child
  install: `.status()` at 472-474, `on_gnome`)
- `src/session.rs`: `update_outcome` (the end-of-run question)
- `src/config.rs`: `UpdateConfig`, `apply_headless_install`, the autostart entry (lines 184-261);
  `src/platform/installer.rs`: `login_entry_exe`; `src/prompts.rs`: lines 54-77
- the output of `spawn-at update --help`
- `grep -n -i "sha256\|checksum\|signature\|gpg\|minisign" src/update.rs install.sh` (no output)
- `grep -n -i "sha256\|checksum\|signature\|gpg\|minisign" src/update.rs install.sh` (no output)
