---
title: "Commands"
---

`spawn-at <command>`. Run `spawn-at --help` or `spawn-at <command> --help` for the options of one
command; the same text is in the [CLI reference](/spawn-at/reference/).

| Command | What it does |
| :--- | :--- |
| [`spawn <cmd...>`](/spawn-at/commands/spawn/) | Tell the compositor where the next window goes, launch the program, place its window |
| [`transform`](/spawn-at/commands/transform/) | Move or resize a window that is already open |
| [`focus`, `defocus`](/spawn-at/commands/window-state/) | Raise a window, or give focus away |
| [`maximize`, `minimize`, `restore`](/spawn-at/commands/window-state/) | Change window state |
| [`close`](/spawn-at/commands/window-state/) | Ask a window to close |
| [`query layout\|pointer\|windows`](/spawn-at/commands/query/) | Print monitors, the pointer position, or the open windows |
| [`install`, `uninstall`, `update`](/spawn-at/commands/manage/) | Manage the binary and the GNOME Shell extension, and releases |
| [`config`](/spawn-at/configuration/) | Show or change settings |

There is no `version` command. `spawn-at -V` and `spawn-at --version` print the version; a binary
not built from a release tag prints `DEV BUILD` and its commit.

## Choosing a window

Commands that act on a window already open take one selector:

| Option | Picks the window by |
| :--- | :--- |
| `--id <ID>` | The numeric id shown by `query windows` |
| `-c, --class <CLASS>` | Class or application id |
| `-t, --title <TITLE>` | A substring of the title |
| `--pid <PID>` | Process id |
| `--focused` | Whichever window has focus |

## Options every command takes

`--no-wait` exits without waiting for the compositor to apply the change.

## Which command works where

From the code and the audit of the test suite. "Unsupported" means the backend answers with a
"does not support this feature" error.

| Command | GNOME Wayland | X11 | Sway (experimental) |
| :--- | :--- | :--- | :--- |
| `spawn` | yes, cloaked | yes, no cloak | yes, can flash once |
| `transform` | yes | yes | yes |
| `query layout`, `query windows` | yes | yes | yes |
| `query pointer` | yes | yes | unsupported |
| `focus` | yes | yes | yes |
| `defocus` | yes | yes | unsupported |
| `maximize`, `minimize`, `restore` | yes | yes | unsupported |
| `close` | yes | yes | yes |
| `install`, `uninstall` | does the real work | "No installation required…" / "No uninstallation required…" | same as X11 |
| `update`, `config` | independent of the desktop | same | same |

Presence in the table is not the same as tested. Several of these cells have no behaviour test at
all; see [Known shortcomings](/spawn-at/shortcomings/).

## Aliases

`restore` also answers to `unmaximize`, `unmaximise`, `unminimize` and `unminimise`. They are not
listed in `--help`.

## Sources

- `README.md` ("CLI reference")
- `docs/TEST_COVERAGE_AUDIT.md`, section 2 (command inventory, backend support) and section 3.9
- the output of `spawn-at --help` and `spawn-at -V`; `spawn-at version` and `spawn-at unmaximize --help` were run
- `docs/DESIGN_SWAY.md`, section 12 (what Sway does not support)
