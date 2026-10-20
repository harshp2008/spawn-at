# Contributing to spawn-at

spawn-at is a Rust CLI plus a GNOME Shell extension that places windows without flicker on GNOME
Wayland. This file is the short entry point. The full contributor guide is the "Developing" section
of the documentation site: <https://harshp2008.github.io/spawn-at/contributing/>.

## Where to start

| To do this | Read |
| :-- | :-- |
| Find where something lives in the code | [`docs/CODE_LAYOUT.md`](docs/CODE_LAYOUT.md) |
| Run the tests, and know which are safe on your machine | [Testing and the verify tool](https://harshp2008.github.io/spawn-at/testing/) |
| Run the interactive test UI | [Running the verify TUI](https://harshp2008.github.io/spawn-at/contributing/verify-tui/) |
| Add or change a test spec | [Writing a test spec](https://harshp2008.github.io/spawn-at/contributing/test-specs/) |
| Add a backend | [Architecture and contributing](https://harshp2008.github.io/spawn-at/contributing/), "Adding a backend" |
| Change a page of the guide | [Writing the docs](https://harshp2008.github.io/spawn-at/contributing/writing-docs/) |

## The workspace

| Member | Role |
| :-- | :-- |
| `spawn-at` (the repository root) | The CLI and the installer. The only default member, so a bare `cargo build` or `cargo test` covers only this package |
| `crates/spawn-at-core` | Logic with no I/O: geometry, the `Driver` trait and batch types, the ready-wait state machine, settings |
| `crates/spawn-at-verify` | The developer-only test harness and its `fixture` test window |
| `crates/xtask` | Repository tooling, run as `cargo xtask ...` |

Outside the crates: `assets/gnome/{modern,legacy}/` holds the GNOME Shell extensions, which are
embedded in the binary. `tests/` holds the Node tests, golden vectors and recorded Sway fixtures.
`verify/` holds the test specs.

## Design principles

1. Geometry knows no window manager. All coordinate math, cursor offsets, monitor boundaries and
   margin clamping live in `spawn_at_core::geometry`. A driver receives the final pixel position and
   size.
2. Escalation comes only after a refusal. The CLI and the installer run with the user's rights. When
   a file operation is refused with `PermissionDenied`, the installer retries it through `sudo`
   (`src/platform/escalate.rs`) and says why. It never starts with `sudo`.
3. Compositor code stays in drivers. D-Bus, Unix sockets and X11 calls live inside the drivers under
   `src/platform/linux/` (`gnome/`, `x11/`, `sway/`). The command layer sees only the
   `CompositorBackend` trait.

The GNOME driver talks to the extension over the session D-Bus interface
`org.gnome.Shell.Extensions.SpawnAt`. The extension hides the new window (the cloak), places it, and
reveals it. [Concepts](https://harshp2008.github.io/spawn-at/concepts/#cloak-and-waiting-for-the-app)
explains the cloak and the wait; the code is in `assets/gnome/modern/` (`extension.js`, `cloak.js`,
`commit.js`, `ready_wait.js`).

## Commands

```bash
cargo check
cargo build                      # debug binary: target/debug/spawn-at
cargo build --release --locked   # release binary: target/release/spawn-at
cargo fmt --check
cargo clippy --workspace --all-targets --locked --features spawn-at-verify/tui,spawn-at-verify/fixture -- -D warnings
```

The test commands, and which of them need a desktop, are in
[Testing and the verify tool](https://harshp2008.github.io/spawn-at/testing/#which-command-for-what).
Plain `cargo test` covers only the `spawn-at` package, so the other crates are named there.

Two scripts print what they would do without starting anything, when given `--dry-run`:

```bash
scripts/sway-headless.sh start --dry-run   # a private headless Sway, never your desktop session
scripts/sway-live.sh --dry-run             # the Sway live cases, run in that private Sway
```

## Debugging the GNOME extension

```bash
# Messages of the spawn-at extension in the GNOME Shell log
journalctl -f -o cat /usr/bin/gnome-shell | grep spawn-at

# The D-Bus interface of the extension
gdbus introspect --session --dest org.gnome.Shell --object-path /org/gnome/Shell/Extensions/SpawnAt

# A read-only call: the monitors and work areas the extension reports
gdbus call --session --dest org.gnome.Shell --object-path /org/gnome/Shell/Extensions/SpawnAt \
  --method org.gnome.Shell.Extensions.SpawnAt.GetLayout
```

`ArmSpawn` takes a target id and a string of instruction JSON, not coordinates. Build that string
with `spawn-at` rather than by hand.

## Rules for a change

- Start from an up-to-date `main`. One commit per step, with a conventional message (`feat:`,
  `fix:`, `docs:`, `refactor:`, `test:`, `ci:`, `chore:`).
- Run `cargo fmt --check`, clippy and the tests after every commit.
- Open a draft pull request. Do not merge, tag or push `main` yourself.
- **Never run `spawn-at install` or `spawn-at uninstall` for real while testing**: they act on the
  live GNOME session.

The full list is on the [Architecture and contributing](https://harshp2008.github.io/spawn-at/contributing/#how-a-change-is-made)
page.
