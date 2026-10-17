---
title: "Install"
---

## The supported path

```bash
# v0.2.0 is in beta, so --prerelease is needed for now
curl -fsSL https://raw.githubusercontent.com/harshp2008/spawn-at/main/install.sh | bash -s -- --prerelease
```

`install.sh` is fetched from the `main` branch of the repository. It:

1. Stops with an error unless the machine is Linux x86_64, because that is the only prebuilt binary.
   The error suggests building from source.
2. Checks the desktop before it downloads anything (see [Unsupported desktops](/spawn-at/environments/unsupported-desktops/)).
3. Looks up the release on the GitHub API, and falls back to GitHub's web endpoints if the API
   fails, then downloads `spawn-at-x86_64-unknown-linux-gnu.tar.gz` from the release.
4. Runs `spawn-at install`. Every flag that `install.sh` does not recognise is passed on to it.

Only GNOME Shell 46 on Wayland has been tested. See [GNOME on Wayland](/spawn-at/environments/gnome-wayland/).

## What the installer asks

In a terminal, `spawn-at install` can ask these questions. The flags answer them in advance.

| Question | When | Flag |
| :--- | :--- | :--- |
| Install anyway? `[y/N]` | The desktop is not GNOME and the session is not X11 | `--allow-unsupported` (`install.sh` only) |
| Go on, after a notice | An untested GNOME version | `--yes` skips the question; the notice is still printed |
| Upgrade in place (the default) or clean install | spawn-at is already installed; it says where and which version | `--clean-install` removes it first without asking and keeps your configuration |
| Release channel and update notices | Update preferences | `--update-channel stable\|beta`, `--update-notify` |
| Log out now? | GNOME Wayland, last of all | `--gnome-wayland-logout`, `--no-action` |

On Wayland you log out and back in once so GNOME loads the extension. The installer asks last
(`src/session.rs`, `decision`: "otherwise: asked at the end"). It skips the log-out question and
advice when the files about to be deployed are identical to the deployed ones, `gnome-extensions
info` reports the extension ACTIVE, and the files are older than the running Shell
(`src/platform/linux/gnome/logout.rs`, `decide`; `src/session.rs`, `install_outcome` returns
`NoLogoutNeeded`). On an X11 session the installer reloads the Shell in place and your applications
stay open (`--help` of `--gnome-x11-reload`; not run here).

:::caution
`--gnome-wayland-logout` on Wayland logs you out **without asking** (`src/session.rs`,
`decision`) and closes your open applications. `--no-action` answers No to the question. In a
run that is `--headless` or whose stdout is not a terminal (`src/main.rs`, `interactive`) the flags decide: the log-out is off unless
you pass `--gnome-wayland-logout`, whose default is `false` (`install --help`).
:::

Without a terminal, `--headless` makes it non-interactive:

```bash
curl -fsSL https://raw.githubusercontent.com/harshp2008/spawn-at/main/install.sh | bash -s -- --headless --scope user
```

## More options

```bash
# Pin a version
curl -fsSL https://raw.githubusercontent.com/harshp2008/spawn-at/main/install.sh | bash -s -- --version v0.2.0-beta.5

# Remove an existing install first, without asking; your config and settings are kept
curl -fsSL https://raw.githubusercontent.com/harshp2008/spawn-at/main/install.sh | bash -s -- --clean-install

# From source
git clone https://github.com/harshp2008/spawn-at.git && cd spawn-at
cargo build --release && ./target/release/spawn-at install
```

`--version` takes a release tag. `--scope` is `user` (the default) or `system`. All flags of
`spawn-at install` are listed on [its reference page](/spawn-at/reference/spawn-at-install/).

## Unsupported desktops

:::note
**Release status (remove after the first release newer than v0.2.0-beta.5).** `install.sh` is
fetched from `main`, so the question below is asked whichever version it installs. The binary that
the v0.2.0-beta.4 release installs does not refuse a non-GNOME Wayland desktop: it switches to the
X11 driver without a message. The refusal is in the binary from v0.2.0-beta.5. The `install.sh`
inside the beta.4 tag has no such question and is byte-identical to beta.3's (`CHANGELOG.md`).
:::

On a Wayland desktop that is not GNOME, `install.sh` prints a notice and asks
`Install anyway? [y/N]` before it downloads anything. Answering no, or giving no answer, installs
nothing and exits with status 1. Without a terminal it stops and tells you to re-run with
`--allow-unsupported`, which skips the question. If you go on, the installer runs with
`SPAWN_AT_BACKEND=x11` and reminds you that spawn-at needs that variable on this desktop.

Not changed by this: a GNOME session, a real X11 session on any desktop, and a forced
`SPAWN_AT_BACKEND=x11` or `sway` see no notice.

## Update

`spawn-at update` checks GitHub for a newer release and installs it. It takes `--check` (look only),
`--channel stable|beta` (`beta` also offers pre-releases; `all` is a deprecated alias of `beta`),
`--force` (reinstall even if current) and `--headless`. The release asset it asks for is named after
the platform the binary was built for.

At login, an autostart entry runs `spawn-at update --check --headless`. It needs no backend, so it
also works on an unsupported desktop.

## Uninstall

```bash
spawn-at uninstall
```

removes the binary, the extension directory and the login update-check entry, and nothing else.
Your config file is never deleted.

:::caution
Run in a terminal, bare `spawn-at uninstall` first asks what to do with the binary (user scope,
system scope, or keep it; `src/main.rs`) and then really removes things. `uninstall` has no
`--dry-run` (`spawn-at uninstall --help` lists none; only `install` has one). On GNOME Wayland,
`--gnome-wayland-logout` logs you out and closes your applications; its default is `false`.
`--headless --skip-bin` removes only the extension and the login entry and asks nothing. `--scope system` and `--skip-bin` exist; see
[the reference page](/spawn-at/reference/spawn-at-uninstall/).
:::

## What is not verified

- `spawn-at install` and `spawn-at uninstall` were not run while writing this book. The text above
  comes from the README, the installer script, the changelog and `--help`.
- GNOME versions other than 46, and system-scope removal.
- `spawn-at update` choosing the release file on any platform other than Linux x86_64.
- The installer on Windows and macOS: it supports neither.

## Sources

- `README.md` ("Install", "More install options")
- `install.sh` (lines 1-130: platform check, flags, unsupported-desktop question)
- `CHANGELOG.md` ("[Unreleased] Changed", "[0.2.0-beta.3] Added/Changed", "[0.2.0-beta.4] Changed")
- `docs/CONFIG.md` (uninstall keeps the config file; update channel)
- `docs/KNOWLEDGE.md` ("Install and release", "Architecture map": login autostart entry)
- `docs/RELEASE_NOTES_0.2.0-beta.4.md` ("What was NOT verified")
- `src/session.rs` (`decision`, `install_outcome`), `src/platform/linux/gnome/logout.rs` (`decide`), `src/main.rs` (`interactive`, `--dry-run`)
- `git show v0.2.0-beta.4:src/platform/linux/registry.rs` and `git diff v0.2.0-beta.3 v0.2.0-beta.4 --quiet -- install.sh assets` (the release-status notes)
- the output of `spawn-at install --help` and `spawn-at update --help`
