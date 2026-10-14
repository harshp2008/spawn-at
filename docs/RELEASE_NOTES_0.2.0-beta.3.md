# spawn-at 0.2.0-beta.3: release notes

This is a beta, and an installer release. **Window placement, the ready-wait policy and the GNOME extension are exactly as in 0.2.0-beta.2** (`git diff v0.2.0-beta.2..HEAD -- assets crates` is empty). It has been run on one machine: GNOME 46, Wayland, one monitor. The known limits of beta.1 and beta.2 still apply: read [RELEASE_NOTES_0.2.0-beta.1.md](RELEASE_NOTES_0.2.0-beta.1.md), "What is measured, and the caveats" and "What is untested".

## What is new

- **`install.sh` shows what it is doing (already live, because `install.sh` is fetched from `main`).** In a terminal it shows curl's progress bar for the tarball, says when a download attempt timed out or stalled and is being retried ("attempt 1 of 4 ... retrying in 2s..."), and prints "Downloaded in Ns, slow connection?" after a slow download. A version that does not exist (HTTP 404) is reported with no progress bar. Errors have one layout: a blank line, the red headline, the causes, "What you can do:" with steps, a blank line. Without a terminal (logs, CI) there is no bar and no retry text.
- **`spawn-at install` detects an existing install.** It looks in `~/.local/bin`, `/usr/local/bin` and on PATH (never the binary that is running), shows where it is and which version (`<existing> -V`, 2 s limit), next to the version being installed. In a terminal it asks: upgrade in place (the default) or clean install. For the same version the default changes nothing; for a newer installed version it warns about the downgrade and the default changes nothing. Without a terminal nothing is uninstalled.
- **`spawn-at install --clean-install`.** Removes the existing install first (the binary, the extension, the login update-check entry; the same removal as `spawn-at uninstall`) and then installs, without asking. **Your configuration file and settings are never deleted** (`spawn-at uninstall` does not delete them either). With nothing installed it says so and does nothing. See "How to use `--clean-install`".
- **No needless log-out on GNOME Wayland.** The log-out question and warning are replaced by a green "No log-out needed" line only when the extension to install is byte-identical to the deployed one (same file set), `gnome-extensions info` says ACTIVE, and every deployed file is at least 2 s older than the running gnome-shell process. If any of these cannot be read, you are asked as before. Identical files are no longer rewritten, so their modification time stays meaningful.
- **The log-out question is the very last prompt.** It used to be asked in the middle of the GNOME section, and "Yes" logged you out before Update Preferences and the settings were saved. Now `install`, `install --clean-install`, `uninstall` and the upgrade offered by `update` only record the answer's inputs; the question is asked at the end, the advice is the last thing printed (the yellow warning is bold yellow only on a terminal with `NO_COLOR` unset), and the log-out (or X11 Shell reload) is the final action. A step that fails never reaches it. Headless runs ask nothing: `--gnome-wayland-logout` and `--no-action` decide.
- **Readable output.** One blank line before and after each `=== ... ===` section, no double blank lines; every prompt and item fits on one 70-column line (the explanation is printed above the question) and none ends with a colon, so a narrow terminal no longer prints a question twice and "Select update channel::" is gone.

## How to install

```bash
# v0.2.0 is in beta, so --prerelease is needed for now
curl -fsSL https://raw.githubusercontent.com/harshp2008/spawn-at/main/install.sh | bash -s -- --prerelease

# or pin this version
curl -fsSL https://raw.githubusercontent.com/harshp2008/spawn-at/main/install.sh | bash -s -- --version v0.2.0-beta.3
```

On Wayland the installer asks last whether to log out. Answer No and log out yourself later, or pass `--gnome-wayland-logout` to log out at the end without asking.

## How to use `--clean-install`

```bash
# From the installer script: remove an existing install first, without asking
curl -fsSL https://raw.githubusercontent.com/harshp2008/spawn-at/main/install.sh | bash -s -- --prerelease --clean-install

# From a binary you already have
spawn-at install --clean-install
```

`install.sh` passes flags it does not know on to `spawn-at install`, so every install flag works after `-s --`. A clean install replaces the extension files, so the log-out rule above applies afterwards on its own facts (a fresh extension directory means you are asked). Binaries found on PATH outside `~/.local/bin` and `/usr/local/bin` are reported and not removed.

## Upgrade notes

- No extension reinstall is needed for placement: no extension file changed since beta.2. Installing replaces nothing that is identical.
- No other command-line flag, config key, exit code or D-Bus member changed; `--clean-install` is new on `install`.
- `uninstall` and the clean-install removal never delete the config file. `install` still writes `[update] channel` and `notify` as before.

## Tests (this tree)

| Suite | Passed |
| :-- | --: |
| `spawn-at` | 273 |
| `spawn-at-core` | 64 |
| `xtask` | 10 |
| `spawn-at-verify` (`--features tui,fixture`) | 404, 4 ignored (they need a live display) |
| Rust total | 751, 4 ignored |
| Node (`node --test tests/*.js`) | 183 |

All file system work in the new tests is in temporary directories.

## What was NOT verified

- **A real log-out, a real X11 Shell reload, and `gnome-session-quit`.** The order (question last, advice last, log-out final) is tested with a recorder and source guards, not by logging out.
- **The "no log-out needed" decision against a real Shell.** The parsers (`gnome-extensions info`, `/proc/<pid>/stat`) and the file comparison are tested; the real Shell start time and a real second install were not tried. The extension's D-Bus interface exposes only a protocol number, no hash, so it is not used.
- **GNOME versions other than 46 on Wayland.** Not run on GNOME 42 to 45 or 47 to 50, and not on X11 (the X11 reload question follows the same rules but is untested).
- **System-scope removal** (`/usr/local/bin`, needs elevation) and the clean install end to end on a real machine.
- **The installer against the real network.** It was tested with a fake `curl` and a local HTTP server. How GitHub answers a `Range: bytes=0-0` request for a release asset was not checked; if it ignores the range, the preflight downloads the file once more.
- **Real terminal wrapping.** Prompt lengths are checked in tests; a narrow real terminal was not tried.
- **CI and the release workflow** on the beta.3 tag are untested until the tag is pushed.

## Known limits

- The existing-install check runs `<existing> -V` (2 s limit); a development build has no comparable version and is treated as unknown.
- The preflight in `install.sh` adds one small request and, on a dead route, up to about 30 s before the real download starts.
- The limits listed for beta.1 and beta.2 (one tested machine, minimum-size reads on GNOME 46, X11 and XWayland findings that are documentation only) are unchanged.

## How to report a problem

The same as before: open an issue at <https://github.com/harshp2008/spawn-at/issues> with your GNOME Shell version, session type, `spawn-at config show`, the exact command, and what the installer printed.
