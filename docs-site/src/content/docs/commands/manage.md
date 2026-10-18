---
title: "install, uninstall, update"
---

These three manage spawn-at itself, not windows. `update` needs no backend: it returns before one
is chosen, so it also works on an unsupported desktop. `install` and `uninstall` do not (see below).

| Command | What it does |
| :--- | :--- |
| `install` | Copies the binary to your `PATH` and, on GNOME, deploys and enables the extension |
| `uninstall` | Removes the binary, the extension directory and the login update-check entry. Your config file stays |
| `update` | Checks GitHub for a newer release; `--check` installs nothing and saves nothing to the config file, `--force` reinstalls |

The questions `install` asks and the flags that answer them in advance are on the
[Install](/spawn-at/install/) page. What `update` does step by step is on the
[Update](/spawn-at/update/) page. The full option lists are in the reference:
[`install`](/spawn-at/reference/spawn-at-install/), [`uninstall`](/spawn-at/reference/spawn-at-uninstall/),
[`update`](/spawn-at/reference/spawn-at-update/).

Two options are worth knowing before you run them:

- `install --dry-run` prints the notice and what would be installed, then stops without changing
  anything.
- On a desktop that is not supported, `install` and `uninstall`
  print the same error as every other command and change nothing. Run `install.sh` with
  `--allow-unsupported` instead (see
  [Unsupported desktops](/spawn-at/environments/unsupported-desktops/)).
- **`uninstall` run bare, in a terminal, asks what to do with the binary** (user scope, system scope
  or keep it), and it has no `--dry-run`. On GNOME Wayland, `--gnome-wayland-logout` logs you out and
  closes your applications; it defaults to `false`. Use `--skip-bin --headless` to leave the binary
  alone and not be asked.

On GNOME, the closing line reports the result of `gnome-extensions`, but a failure is a warning and
does not change the exit status:

- `install` prints `Spawn-At GNOME extension successfully installed and enabled.` only when `gnome-extensions
  enable` exits 0. When it exits non-zero, `install` prints a warning instead (`'gnome-extensions enable'
  exited with code N. The extension files were deployed, but the extension is not confirmed
  enabled ...`) and still exits 0. It stops with an error only when `gnome-extensions` cannot be
  started.
- `uninstall` prints `Spawn-At extension successfully uninstalled.` only when `gnome-extensions
  disable` exits 0. When it cannot be started or exits non-zero, `uninstall` still deletes the files
  if asked to, prints a warning that the extension may still be enabled and whether its files were
  removed, and exits 0. An error is raised only if deleting the extension directory fails.

Read from the code (`src/platform/linux/gnome/deploy.rs`), not run, because running them would
change the live session.

For a backend that needs no helper (X11, Sway), the backend's part of `install` prints `No
installation required for NAME.` and the backend's part of `uninstall` prints `No uninstallation
required for NAME.`, where NAME is `X11` or `Sway` (the driver's `name()`; `src/platform/mod.rs`, the default `install`
and `uninstall`). Only GNOME has an extension to deploy.

## Sources

- `CHANGELOG.md` ("[0.2.0-beta.5] Changed", "[0.2.0-beta.3] Added")
- `docs/DESIGN_PLATFORMS.md`, section 6 step 5
- `docs/TEST_COVERAGE_AUDIT.md`, section 2 (`install`, `uninstall`, `update`)
- `docs/KNOWLEDGE.md` ("Architecture map")
- the output of `spawn-at install --help`
