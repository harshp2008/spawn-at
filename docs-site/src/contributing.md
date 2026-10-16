# Architecture and contributing

## The crates

| Member | Role |
| :--- | :--- |
| `spawn-at` (the repository root) | The CLI and the installer. The only default member, so a bare `cargo build` or `cargo test` covers only this package |
| `crates/spawn-at-core` | Logic with no I/O: geometry, the driver trait and batch types, the ready-wait state machine, settings, the wait override, the resize report |
| `crates/spawn-at-verify` | The developer-only verification harness and its `fixture` test window |
| `crates/xtask` | Repository tooling: `cargo xtask ...` (comment-only check, doc coverage, `burn-cpu`, and the documentation commands) |

Outside the crates: `assets/gnome/{modern,legacy}/` (the GNOME Shell extensions, embedded in the
binary), `tests/` (Node tests, golden vectors, recorded Sway fixtures), `verify/` (test specs),
`scripts/`, `install.sh`, `.github/workflows/`. The file-by-file map is `docs/CODE_LAYOUT.md`.

## The traits a backend implements

All of this lives in `src/platform/` and `spawn-at-core`.

| Trait | Role |
| :--- | :--- |
| `Driver` (core) | `arm(Batch)` before the program starts, and `disarm`. The platform-neutral heart; the spawn pipeline never sees D-Bus or X11 |
| `CompositorBackend` | Extends `Driver` with setup and teardown, capability flags, queries (windows, monitors, work areas, pointer) and actions (transform, state, focus, close, `post_spawn`). Most methods default to "unsupported", so a new backend can start small |
| `Capabilities`, `WindowQuery`, `WindowControl`, `ReadySignal` | What a backend can do, and narrow views of the trait |
| `Provisioner` | Install and uninstall of whatever a backend needs on the machine (for GNOME, the extension) |
| `Host` | Per-OS services: binary directories, the autostart path, the executable test, detaching a child |
| `SessionActions`, `UpdateHooks` | Log out or reload the shell; the desktop-specific steps of `spawn-at update` |
| `SettingsStore` (core) | Where the config text lives; the Linux implementation is the only one |

The command layer talks only to `CompositorBackend`. The GNOME-specific module `gnome::mechanics` is
used from production code only inside the GNOME driver. To add a backend, create a module under
`src/platform/linux/` and register it in `LinuxBackend::bootstrap()` (the choice rules are in
`linux/registry.rs`). A new backend should be reachable only through `SPAWN_AT_BACKEND=<name>` until
its live suite passes; Sway is the worked example.

## The design documents

Each of these is a document in the repository, with the evidence for its claims:

- [DESIGN_PLATFORMS.md](https://github.com/harshp2008/spawn-at/blob/main/docs/DESIGN_PLATFORMS.md):
  the inventory, the capability model, feasibility per target and the migration plan.
- [DESIGN_SWAY.md](https://github.com/harshp2008/spawn-at/blob/main/docs/DESIGN_SWAY.md): the Sway
  backend, its verified facts and what was built.
- [DESIGN_READY_WAIT.md](https://github.com/harshp2008/spawn-at/blob/main/docs/DESIGN_READY_WAIT.md):
  the ready-wait, and the X11 experiments.
- [DESIGN_RUNNER_SETUP.md](https://github.com/harshp2008/spawn-at/blob/main/docs/DESIGN_RUNNER_SETUP.md):
  the tests of existing windows.
- [CODE_LAYOUT.md](https://github.com/harshp2008/spawn-at/blob/main/docs/CODE_LAYOUT.md),
  [CONFIG.md](https://github.com/harshp2008/spawn-at/blob/main/docs/CONFIG.md),
  [TEST_PLAN.md](https://github.com/harshp2008/spawn-at/blob/main/docs/TEST_PLAN.md),
  [TEST_LOG.md](https://github.com/harshp2008/spawn-at/blob/main/docs/TEST_LOG.md),
  [TEST_COVERAGE_AUDIT.md](https://github.com/harshp2008/spawn-at/blob/main/docs/TEST_COVERAGE_AUDIT.md),
  [REFACTOR_PROOFS.md](https://github.com/harshp2008/spawn-at/blob/main/docs/REFACTOR_PROOFS.md),
  [KNOWLEDGE.md](https://github.com/harshp2008/spawn-at/blob/main/docs/KNOWLEDGE.md) (short facts,
  each with its source).

## What must not change

The GNOME Wayland contract, from `DESIGN_PLATFORMS.md` section 7:

- The D-Bus interface `org.gnome.Shell.Extensions.SpawnAt`, the instruction JSON, the protocol
  numbers, and the extension files under `assets/gnome`, which every release compares byte for byte.
- The ready-wait policy and its Rust and JavaScript parity (checked by `tests/ready_wait_vectors`),
  the 30 ms legacy rule, and `wait.strategy` and `wait.cap_ms`.
- The placement arithmetic (`verify/golden/placement.json`), config keys and file location, CLI flags
  and `--help` text, and exit codes.
- The installer flow: the log-out question last, `--clean-install`, the prompt width rules, and the
  behaviour of `install.sh`.

## How a change is made

These are the project's rules, written down in `CLAUDE.md` in the repository.

- Start every task from an up-to-date `main`. One commit per step, with a conventional message
  (`feat:`, `fix:`, `docs:`, `refactor:`, `test:`, `ci:`, `chore:`). Update doc counts in the same
  commit that changes them.
- Run `cargo fmt --check`, clippy and the tests after every commit, not only at the end.
- **Pull requests:** push the branch you created, open a draft pull request, and never merge, tag or
  push `main` yourself. The order for a release is: pull request, merge, pull `main`, then tag. The
  version comes from the git tag at build time; a build that is not on a tag prints `DEV BUILD`.
  `install.sh` is fetched from `main`, so a merge ships changes to it at once; changes to the binary
  ship only with the next tag.
- **A pure move needs proof**: the sorted test names per binary, the `--help` of every subcommand,
  and the hashes of goldens and fixtures must be identical, and `git diff -M --stat` must show moves
  only. Never mix moves and logic in one commit. See `REFACTOR_PROOFS.md`.
- **A comment-only commit must pass `cargo xtask check-comment-only <rev>`.**
  `scripts/check-comment-only-product.sh <rev>` runs it over the product code. Write comments from
  the function body, not the function name; if unsure, leave it out.
- Keep `CHANGELOG.md` current (new entries under Unreleased) and add facts that took more than a few
  tool calls to find to `docs/KNOWLEDGE.md`.

## Build and test

What CI runs (`.github/workflows/ci.yml`):

```bash
cargo fmt --check
cargo clippy --workspace --all-targets --locked --features spawn-at-verify/tui,spawn-at-verify/fixture -- -D warnings
cargo test --locked -p spawn-at -p spawn-at-core -p xtask
cargo test --locked -p spawn-at-verify --features tui,fixture
node --test tests/*.js
cargo build --release --locked       # target/release/spawn-at
```

CI also compile-checks the workspace for Windows and both macOS targets. Plain `cargo test` at the
root covers only the `spawn-at` package.

## Build this documentation

You need `mdbook` 0.5.4 on your `PATH` (a release archive from the mdBook repository; the checked
download is in `.github/workflows/docs.yml`).

```bash
cargo xtask docs-site           # builds the binary, the CLI reference, the book and rustdoc, then checks them
```

The finished site is in `target/docs-site/site/`; open `index.html` there. The pieces:

| Command | What it does |
| :--- | :--- |
| `cargo xtask docs-cli-reference --bin target/debug/spawn-at` | Writes the CLI reference pages from `--help`. Fails if the binary is missing or a command has no page |
| `cargo xtask docs-check-src` | Checks the book's markdown: empty pages, environment pages, `Sources` lists, forbidden text |
| `mdbook build docs-site` | Builds the book into `target/docs-site/book/` |
| `cargo xtask docs-check-html <dir>` | Checks a built site for broken links, missing ids and third-party requests |

## Sources

- `CONTRIBUTING.md` and `CLAUDE.md`
- `docs/CODE_LAYOUT.md` ("Workspace")
- `docs/DESIGN_PLATFORMS.md`, sections 2, 4 and 7
- `docs/REFACTOR_PROOFS.md`
- `docs/KNOWLEDGE.md` ("Architecture map", "Install and release")
- `.github/workflows/ci.yml`
- `crates/xtask/src/` (the documentation commands)
