# Known shortcomings

An honest list of what is broken, missing or unmeasured, taken from the repository's own audits.
Nothing here is a plan; it describes the state at version 0.2.0-beta.4 plus the unreleased changes
on `main`.

## Behaviour that does not match what you might expect

- **`spawn --focus`, `--no-focus` and `--defocus` are accepted and ignored.** `spawn` always asks for
  exclusive focus. `--help` still describes the flags, and the README's own `btop` example uses
  `--no-focus`. A test pins the current behaviour; it is not a contract. On `transform`, `maximize`
  and `restore` the flags do work.
- **State commands exit 0 even if nothing changed.** `focus`, `defocus`, `maximize`, `minimize`,
  `restore` and `transform` wait up to 2 seconds for the new state, drop the result of that wait, and
  exit 0. Reporting the timeout would change an exit code, so it was pinned, not fixed.
- **GNOME `focus`, `defocus`, the state commands and `close` find windows on the active workspace
  only.** For a window elsewhere the extension answers "false" and the CLI says the compositor
  failed.
- **A window that cannot answer a resize costs the full cap.** An app already at its minimum size, or
  a hung app, makes the ready-wait run to `wait.cap_ms` (about +230 ms in one test).
- **Under load the window is hidden longer.** The median was 45.3 ms with the old rule against 66.8
  ms with the ready-wait in one measurement, and 48.9 against 56.2 ms in another. In the second
  measurement 1 of 40 idle runs hit the 250 ms cap; the cause is unknown.
- **Sway can flash once**, because Sway cannot hold a new window invisible. Not measured.
- **`--anchor cursor` and `--monitor cursor` on Sway use (0, 0).**

## Gaps per environment

- **GNOME 42 to 44, 45, 47 and later, X11 sessions, XWayland, multi-monitor, fractional scaling, Qt
  and Electron apps:** untested. See [GNOME on Wayland](environments/gnome-wayland.md).
- **Sway:** a private headless 1.9 only; `defocus`, `maximize`, `minimize`, `restore`,
  `query pointer` unsupported; no i3. See [Sway](environments/sway.md).
- **X11 driver:** no test touches an X server, and the tests named "supported on X11" pass on a
  connection error. See [X11 and XWayland](environments/x11-xwayland.md).
- **Windows, macOS:** not implemented; `cargo check` only. The macOS and Windows branches of the
  privilege-escalation code have never been run.
- **KDE, Hyprland:** not implemented.

## Not verified anywhere

- GNOME versions other than 46.
- Real Sway sessions, and Sway other than 1.9.
- XWayland and real X11 applications.
- More than two outputs; scales other than 1 and 2.
- Windows and macOS runs (only `cargo check`).
- Removal at system scope (`spawn-at uninstall --scope system`).
- Whether `spawn-at update` picks the right release file on platforms other than Linux x86_64.

## What the extension's log shows, and does not

Test tooling reads the extension's timing log. A trace inventory of the extension
(`docs/DESIGN_RUNNER_SETUP.md` section 5.5; the call sites were re-opened in
`assets/gnome/modern/extension.js` for this book, for example `RAISE` and `ACTIVATE` at lines 1469
and 1472, `MINIMIZE` at 1756, `UNMINIMIZE` at 1763 and 1771, `WINDOW_CREATED` at 486 and `ACTOR_MAP`
at 531; the `legacy/` extension has them at other lines) found:

- `transform` logs a full timeline with the window id.
- `focus` logs `RAISE` and `ACTIVATE`; `minimize` and `unminimize` log `MINIMIZE` and `UNMINIMIZE`.
- `maximize`, `unmaximize`, `defocus`, `close` and the queries log **nothing** for their target (an
  `ANCHOR_RELEASED` line appears if an anchor was armed).
- `WINDOW_CREATED` and `ACTOR_MAP` are logged for **every** window on the desktop, so anything that
  reads the log must filter by window id.

So a flicker or ordering check can be made for `spawn` and `transform` on GNOME, only partly for
`focus` and `minimize`, and not at all for the rest.

## Limits of the flash measurement

- **No pixels are captured.** Nobody has measured what was on screen.
- **A clean result is not a guarantee.** It means no late paint or move was found in the runs that
  were made.
- **"Flash" is computed from the wire.** It means the window was revealed before the app's first
  commit at its final size, read from the Wayland protocol. That is the optimistic bound: it says
  nothing about what a compositor frame actually showed.
- **"Possible" flashes have a known noise level.** A flash with a lead under one frame (16.7 ms at 60
  Hz) is labelled "possible", with a probability the code itself calls a hypothesis. On X11, 19 of 20
  recorded runs were "possible", so it is not a floor of noise but the normal state there.
- **Only GNOME `spawn` is measured for flash**, and, informationally and not yet run on a real
  session, GNOME `transform`. Focus, defocus, maximize, minimize, restore and close are not.
  Sway and X11 cannot be measured this way: it needs the GNOME extension's clock.
- **One machine.** The numbers come from one GNOME 46 Wayland machine with the project's test window.

## The test suite itself

- The verify catalog's original 45 specs only run `spawn`. 22 more entries exist for existing
  windows; they passed 15 times on a private headless Sway and have **not been run on GNOME**.
- Five tests stay ignored: four in `fixture_test` and one Sway focus-chain test. They need a live
  display or a private Sway.
- The harness drives GNOME-only tools (`journalctl`, `gdbus`) for its logs and display probes.

## Sources

- `docs/TEST_COVERAGE_AUDIT.md`, sections 3, 4 and 7 (gaps 1 to 18 and their updates)
- `README.md` ("How well it works", "Support", `--focus` note)
- `docs/KNOWLEDGE.md` ("Testing and fixtures", "Gotchas", "Open questions")
- `docs/DESIGN_RUNNER_SETUP.md`, section 5.5 (log inventory), as summarised in `verify/README.md`
- `docs/RELEASE_NOTES_0.2.0-beta.4.md` ("What was NOT verified")
- `docs/TEST_PLAN.md`, sections 7.3 and 7.5
- `docs/TEST_LOG.md`, section 6.23 (the measured costs)
