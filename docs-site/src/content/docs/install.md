---
title: "Install"
---

## The supported path

```bash
curl -fsSL https://raw.githubusercontent.com/harshp2008/spawn-at/main/install.sh | bash
```

Without `--version`, this installs the latest stable release. `--version <tag>` installs that
release and `--prerelease` takes the newest release including pre-releases.

`install.sh` is fetched from the `main` branch of the repository. It:

1. Stops with an error unless the machine is Linux x86_64, because that is the only prebuilt binary.
   The error suggests building from source.
2. Checks the desktop before it downloads anything (see [Unsupported desktops](/spawn-at/environments/unsupported-desktops/)).
3. Looks up the release on the GitHub API, and falls back to GitHub's web endpoints if the API
   fails, then downloads `spawn-at-x86_64-unknown-linux-gnu.tar.gz` from the release.
4. Runs `spawn-at install`. Every flag that `install.sh` does not recognise is passed on to it.

Only GNOME Shell 46 on Wayland has been tested. See [GNOME on Wayland](/spawn-at/environments/gnome-wayland/).
The programs it needs (`curl`, `tar`, `gnome-extensions`) and the accepted desktops are on
[Requirements](/spawn-at/requirements/).

## What the installer asks

In a terminal, `spawn-at install` can ask these questions. The flags answer them in advance.

| Question | When | Flag |
| :--- | :--- | :--- |
| Install anyway? `[y/N]` | The desktop is not GNOME and the session is not X11 | `--allow-unsupported` (`install.sh` only) |
| Go on, after a notice | An untested GNOME version | `--yes` skips the question; the notice is still printed |
| Upgrade in place (the default) or clean install | spawn-at is already installed; it says where and which version | `--clean-install` removes it first without asking and keeps your config file. What happens to the `[update]` keys is on [Installer flags](/spawn-at/installer-flags/#headless-defaults) |
| Release channel and update notices | Update preferences | `--update-channel stable\|beta`, `--update-notify` |
| Log out now? | GNOME Wayland, last of all | `--gnome-wayland-logout`, `--no-action` |

On Wayland you log out and back in once so GNOME loads the extension. The installer asks last. It
skips the log-out question and advice when the files about to be deployed are identical to the
deployed ones, `gnome-extensions info` reports the extension ACTIVE, and the files are older than
the running Shell. On an X11 session the installer reloads the Shell in place and your applications
stay open (from the `--help` of `--gnome-x11-reload`; not run here).

:::caution
`--gnome-wayland-logout` on Wayland logs you out **without asking** and closes your open
applications. `--no-action` answers No to the question. In a run that is `--headless` or whose
stdout is not a terminal, the flags decide: the log-out is off unless you pass
`--gnome-wayland-logout`, whose default is `false` (`install --help`).
:::

Without a terminal, `--headless` makes it non-interactive:

```bash
curl -fsSL https://raw.githubusercontent.com/harshp2008/spawn-at/main/install.sh | bash -s -- --headless --scope user
```

## Check that it worked

What you should see, from the code (not run): the installer prints `Successfully copied binary to:
<path>` and, on GNOME, `Spawn-At GNOME extension successfully installed and enabled.` If
`gnome-extensions enable` exited non-zero it prints a note that the extension is "not confirmed
enabled" instead, and still exits 0.

To check it, open a new terminal and run `spawn-at --version`, then, once you have logged out and
back in on Wayland, `spawn-at query layout`. The second prints the monitor table shown in
[Quick start](/spawn-at/quick-start/#2-see-the-screen).

If it did not work:

- `command not found`: [the `PATH` entry](/spawn-at/troubleshooting/#spawn-at-command-not-found-after-install).
- A `Failed to communicate with SpawnAt GNOME extension via D-Bus` error, or a mismatch warning:
  [the extension is not active entry](/spawn-at/troubleshooting/#the-extension-was-installed-but-is-not-active-or-extension-is-older-than-the-cli-expects).
- The note about "not confirmed enabled":
  [that entry](/spawn-at/troubleshooting/#install-said-the-extension-was-deployed-but-not-confirmed-enabled).
- `this desktop is not supported`: [that entry](/spawn-at/troubleshooting/#this-desktop-is-not-supported).

## More options

```bash
# Pin a version
curl -fsSL https://raw.githubusercontent.com/harshp2008/spawn-at/main/install.sh | bash -s -- --version v0.2.1

# Remove an existing install first, without asking; your config file is kept
curl -fsSL https://raw.githubusercontent.com/harshp2008/spawn-at/main/install.sh | bash -s -- --clean-install

# From source
git clone https://github.com/harshp2008/spawn-at.git && cd spawn-at
cargo build --release && ./target/release/spawn-at install
```

`--version` takes a release tag. `--scope` is `user` (the default) or `system`. Every flag of
`install.sh`, `spawn-at install` and `spawn-at uninstall`, with its default, is on
[Installer flags](/spawn-at/installer-flags/). The raw `--help` is on
[the reference page](/spawn-at/reference/spawn-at-install/).

## Unsupported desktops

On a Wayland desktop that is not GNOME, `install.sh` prints a notice and asks
`Install anyway? [y/N]` before it downloads anything. Answering no, or giving no answer, installs
nothing and exits with status 1. Without a terminal it stops and tells you to re-run with
`--allow-unsupported`, which skips the question. If you go on, the installer runs with
`SPAWN_AT_BACKEND=x11` and reminds you that spawn-at needs that variable on this desktop.

Not changed by this: a GNOME session, a real X11 session on any desktop, and a forced
`SPAWN_AT_BACKEND=x11` or `sway` see no notice.

## Update

`spawn-at update` checks GitHub for a newer release and installs it. It takes `--check` (install nothing and save nothing to the config file),
`--channel stable|beta` (`beta` also offers pre-releases; `all` is a deprecated alias of `beta`),
`--force` (reinstall even if current) and `--headless`. The release asset it asks for is named after
the platform the binary was built for. What it downloads, replaces and asks, what it does not
check, and the login update-check entry are on the [Update](/spawn-at/update/) page.

## Uninstall

```bash
spawn-at uninstall
```

removes the binary, the extension directory and the login update-check entry, and nothing else.
Your config file is never deleted.

What you should see, from the code (not run): `Successfully removed binary: <path>` and
`Spawn-At extension successfully uninstalled.` If `gnome-extensions disable` fails, a warning is
printed instead of the second line and the exit status stays 0 ([Known
limits](/spawn-at/known-limits/#exit-status-and-results)). To check it, look for the extension
directory named on [Files and paths](/spawn-at/files-and-paths/#extension-directory): it should be
gone.

:::caution
Run in a terminal, bare `spawn-at uninstall` first asks what to do with the binary (user scope,
system scope, or keep it) and then really removes things. `uninstall` has no `--dry-run`; only
`install` has one. On GNOME Wayland, `--gnome-wayland-logout` logs you out and closes your
applications; its default is `false`. `--headless --skip-bin` removes only the extension and the
login entry and asks nothing. `--scope system` and `--skip-bin` exist; see
[Installer flags](/spawn-at/installer-flags/#spawn-at-uninstall) and
[the reference page](/spawn-at/reference/spawn-at-uninstall/).
:::

## What is not verified

- `spawn-at install` and `spawn-at uninstall` were not run while writing this guide. The text above
  comes from the README, the installer script, the changelog and `--help`.
- Everything else that has not been run, such as GNOME versions other than 46 and system-scope
  removal, is listed on [Known shortcomings](/spawn-at/shortcomings/#not-verified-anywhere).
- The installer on Windows and macOS: it supports neither.

## Sources

- `README.md` ("Install", "More install options")
- `install.sh` (lines 1-130: platform check, flags, unsupported-desktop question)
- `CHANGELOG.md` ("[0.2.0-beta.5] Changed", "[0.2.0-beta.3] Added/Changed", "[0.2.0-beta.4] Changed")
- `docs/CONFIG.md` (uninstall keeps the config file; update channel)
- `docs/KNOWLEDGE.md` ("Install and release", "Architecture map": login autostart entry)
- `docs/RELEASE_NOTES_0.2.0-beta.4.md` ("What was NOT verified")
- `src/session.rs`: `decision` ("otherwise: asked at the end"; `--gnome-wayland-logout` logs out
  without asking) and `install_outcome` (returns `NoLogoutNeeded`)
- `src/platform/linux/gnome/logout.rs`: `decide`
- `src/main.rs`: `interactive` (headless or stdout not a terminal), the uninstall prompt for the
  binary, and `--dry-run` (install only)
- `src/platform/installer.rs`: the `Successfully copied binary` and `Successfully removed binary`
  lines; `src/platform/linux/gnome/deploy.rs`: the `successfully installed and enabled` and
  `successfully uninstalled` lines
- the output of `spawn-at install --help`, `spawn-at uninstall --help` and `spawn-at update --help`
