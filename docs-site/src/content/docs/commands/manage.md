---
title: "install, uninstall, update"
---

These three manage spawn-at itself, not windows. `update` needs no backend: it returns before one
is chosen, so it also works on an unsupported desktop. `install` and `uninstall` do not (see below).

| Command | What it does |
| :--- | :--- |
| `install` | Copies the binary to your `PATH` and, on GNOME, deploys and enables the extension |
| `uninstall` | Removes the binary, the extension directory and the login update-check entry. Your config file stays |
| `update` | Checks GitHub for a newer release; `--check` only looks, `--force` reinstalls |

The questions `install` asks and the flags that answer them in advance are on the
[Install](/spawn-at/install/) page. The full option lists are in the reference:
[`install`](/spawn-at/reference/spawn-at-install/), [`uninstall`](/spawn-at/reference/spawn-at-uninstall/),
[`update`](/spawn-at/reference/spawn-at-update/).

Two options are worth knowing before you run them:

- `install --dry-run` prints the notice and what would be installed, then stops without changing
  anything.
- On `main` (not in a release yet), on a desktop that is not supported, `install` and `uninstall`
  print the same error as every other command and change nothing. Run `install.sh` with
  `--allow-unsupported` instead (see
  [Unsupported desktops](/spawn-at/environments/unsupported-desktops/)). In v0.2.0-beta.4 and earlier the
  binary does not refuse.
- **`uninstall` run bare, in a terminal, asks what to do with the binary** (user scope, system scope
  or keep it), and it has no `--dry-run`. On GNOME Wayland, `--gnome-wayland-logout` logs you out and
  closes your applications; it defaults to `false`. Use `--skip-bin --headless` to leave the binary
  alone and not be asked.

On GNOME, the closing line does not report the result of `gnome-extensions`:

- `install` prints `Note: 'gnome-extensions enable' returned a non-zero exit code ...` when the
  enable command exits non-zero, and then still prints `Spawn-At GNOME extension successfully
  installed and enabled.` It stops with an error only when `gnome-extensions` cannot be started.
- `uninstall` runs `gnome-extensions disable` and ignores the outcome, including a missing
  `gnome-extensions`. `Spawn-At extension successfully uninstalled.` is printed whatever happened;
  an error is raised only if deleting the extension directory fails.

Read from the code (`src/platform/linux/gnome/deploy.rs`), not run, because running them would
change the live session.

For a backend that needs no helper (X11, Sway), the backend's part of `install` prints `No
installation required for NAME.` and the backend's part of `uninstall` prints `No uninstallation
required for NAME.`, where NAME is `X11` or `Sway` (the driver's `name()`; `src/platform/mod.rs`, the default `install`
and `uninstall`). Only GNOME has an extension to deploy.

## Sources

- `CHANGELOG.md` ("[Unreleased] Changed", "[0.2.0-beta.3] Added")
- `docs/DESIGN_PLATFORMS.md`, section 6 step 5
- `docs/TEST_COVERAGE_AUDIT.md`, section 2 (`install`, `uninstall`, `update`)
- `docs/KNOWLEDGE.md` ("Architecture map")
- the output of `spawn-at install --help`
