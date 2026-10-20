---
title: "Known shortcomings"
---

What is broken, missing or unmeasured in spawn-at, taken from the project's own audits. Nothing here
is a plan. How each command behaves (exit codes, waits, how windows are matched) is on
[Known limits](/spawn-at/known-limits/).

## Behaviour that may surprise you

### spawn ignores the focus flags

`spawn --focus`, `--no-focus` and `--defocus` are accepted and do nothing. A test pins this. What the
code does is on [spawn](/spawn-at/commands/spawn/).

### Some GNOME commands see only the active workspace

On GNOME, `focus`, `defocus`, `maximize`, `minimize`, `restore` and `close` find windows on the
active workspace only. For a window on another workspace the extension answers "false". The command
then fails with an error that says the compositor failed.

### A window that cannot answer a resize costs the full cap

The ready-wait holds a new window hidden until the app answers the resize request. See
[Concepts](/spawn-at/concepts/#cloak-and-waiting-for-the-app). An app already at its minimum size, or
a hung app, never answers. The window then stays hidden until the cap, `wait.cap_ms`, which is 250 ms
by default. The README reports about 230 ms extra in one test; the test log has no entry for that
figure.

### Under load the window is hidden longer

Median time the window stayed hidden under load, in two measurements:

| Measurement | Old rule | Ready-wait |
| :-- | :-- | :-- |
| First | 45.3 ms | 66.8 ms |
| Second | 48.9 ms | 56.2 ms |

Idle, there was no difference. In the second measurement, 1 of 40 idle runs reached the 250 ms cap.
The cause is unknown.

### Sway can flash once

Sway cannot hold a new window invisible, so a window can show before it is placed. This was not
measured.

The exit status of the state commands and the cursor options on Sway are covered on
[Known limits](/spawn-at/known-limits/).

## Not verified anywhere

| Area | Status |
| :-- | :-- |
| GNOME 46 on Wayland | The only setup tested in earnest: one machine, one monitor. |
| GNOME 42 to 44 | The legacy extension was never run on a real GNOME 42 to 44. It may not work at all. |
| GNOME 45, 47 and later | The extension's metadata lists 45 to 48, but only 46 has been run. |
| GNOME layouts and apps | Multi-monitor, fractional scaling, Qt and Electron apps: untested. |
| X11 and XWayland | Test window only, no real apps. No test touches an X server. |
| Sway | A private headless Sway 1.9 only. No real session, no other version, no i3. |
| Sway layouts | More than two outputs and output scales other than 1 and 2: untried. |
| Windows and macOS | Not implemented. The code is only compile-checked, never run. |
| KDE and Hyprland | Not implemented. |
| `spawn-at uninstall --scope system` | Not run. |
| `spawn-at update` off Linux x86_64 | Only Linux x86_64 builds are published, so no other release file was ever picked. |

More on each environment: [GNOME on Wayland](/spawn-at/environments/gnome-wayland/),
[Sway](/spawn-at/environments/sway/), [X11 and XWayland](/spawn-at/environments/x11-xwayland/).

## What the extension's log shows

Test tools read the extension's timing log. What each command writes there, from a trace inventory of
the extension:

| Command | Log entries |
| :-- | :-- |
| `spawn`, `transform` | A full timeline with the window id. |
| `focus` | `RAISE` and `ACTIVATE`. |
| `minimize`, `restore` | `MINIMIZE` or `UNMINIMIZE` (`restore` only if the window was minimized). |
| `maximize`, `close`, the queries | Nothing for the target. |
| `defocus` | Nothing for the target. The window that receives focus gets `RAISE` and `ACTIVATE`. |
| Any new window | `WINDOW_CREATED` and `ACTOR_MAP`, for every window on the desktop. |

A reader of the log must filter by window id. A flash or ordering check works for `spawn` and
`transform` on GNOME, only partly for `focus` and `minimize`, and not for the rest.

## Limits of the flash measurement

- No pixels are captured. Nobody has measured what was on screen.
- A clean result is not a guarantee. It means no late paint or move was found in the runs made.
- "Flash" is computed from the Wayland protocol: the window was revealed before the app's first
  commit at its final size. That is the optimistic bound. It says nothing about what a frame showed.
- A flash with a lead under one frame (16.7 ms at 60 Hz) is labelled "possible". The code itself calls
  its probability a hypothesis. On X11, 19 of 20 recorded runs were "possible", so this is the normal
  state there, not a noise floor.
- Only GNOME `spawn` is measured for flash. GNOME `transform` is informational and not yet run on a
  real session. The other commands are not measured. Sway and X11 cannot be, because it needs the
  GNOME extension's clock.
- All numbers come from one GNOME 46 Wayland machine with the project's test window.

## The test suite itself

- The catalog has 22 entries for existing windows. 15 passed on a private headless Sway, 7 were
  skipped, and none has run on GNOME. The original 45 entries only run `spawn`.
- Five tests stay ignored. Four in `fixture_test` need a live display, and one Sway focus-chain test
  needs a private Sway.
- Four tests named "supported on X11" pass on a connection error, so they do not test X11.
- The harness uses GNOME-only tools (`journalctl`, `gdbus`) for its logs and display probes.

## Sources

- `docs/TEST_COVERAGE_AUDIT.md`, sections 3, 4 and 7 (gaps 1 to 18 and their updates)
- `README.md` ("How well it works", "Support", the `--focus` note)
- `docs/KNOWLEDGE.md` ("Testing and fixtures", "Gotchas", "Open questions")
- `docs/DESIGN_RUNNER_SETUP.md`, section 5.5 (what the extension logs per command)
- `assets/gnome/modern/extension.js` (the workspace lookup, the log calls)
- `src/commands/spawn.rs` (the test that pins the ignored focus flags) and `src/update.rs` (the
  release file name is built from the build target; only Linux x86_64 is published)
- `docs/TEST_PLAN.md` (the live checks that are still open)
- `docs/TEST_LOG.md`, section 6.23 (the measured costs)
