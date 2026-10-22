---
title: "Architecture and contributing"
---

How the code is organized, how to add a backend, the rules for a change, and how to build this
documentation. For tests, see [Testing and the verify tool](/spawn-at/testing/),
[Running the verify TUI](/spawn-at/contributing/verify-tui/) and
[Writing a test spec](/spawn-at/contributing/test-specs/). For the pages of this guide, see
[Writing the docs](/spawn-at/contributing/writing-docs/).

## The crates

| Member | Role |
| :--- | :--- |
| `spawn-at` (the repository root) | The CLI and the installer. The only default member, so a bare `cargo build` or `cargo test` covers only this package |
| `crates/spawn-at-core` | Logic with no I/O: geometry, the `Driver` trait and batch types, the ready-wait state machine, settings, the wait override, the resize report |
| `crates/spawn-at-verify` | The developer-only verification harness and its `fixture` test window |
| `crates/xtask` | Repository tooling: `cargo xtask ...` (comment-only check, doc coverage, `burn-cpu`, and the documentation commands) |

Outside the crates: `assets/gnome/{modern,legacy}/` (the GNOME Shell extensions, embedded in the
binary), `tests/` (Node tests, golden vectors, recorded Sway fixtures), `verify/` (test specs),
`scripts/`, `install.sh`, `.github/workflows/`. The file-by-file map is `docs/CODE_LAYOUT.md`.

## The traits a backend implements

All of this lives in `src/platform/` and `spawn-at-core`.

| Trait | Role |
| :--- | :--- |
| `Driver` (core) | `arm(Batch)` before the program starts, and `disarm` and `disarm_entry`. The platform-neutral heart; the spawn pipeline never sees D-Bus or X11 |
| `CompositorBackend` | Extends `Driver` with setup and teardown, capability flags, queries (windows, monitors, work areas, pointer) and actions (transform, state, focus, close, `post_spawn`). Most methods default to "unsupported", so a new backend can start small |
| `Capabilities` (a struct), `WindowQuery`, `WindowControl`, `ReadySignal` (traits) | What a backend can do, and narrow views of the trait (`src/platform/capability.rs`) |
| `Provisioner` | Install and uninstall of whatever a backend needs on the machine (for GNOME, the extension) |
| `Host` | Per-OS services: binary directories, the autostart path, the executable test, detaching a child |
| `SessionActions`, `UpdateHooks` | Log out or reload the shell; the desktop-specific steps of `spawn-at update` |
| `SettingsStore` (core) | Where the config text lives; the Linux implementation is the only one |

The command layer talks only to `CompositorBackend`. The GNOME-specific module `gnome::mechanics` is
used from production code only inside the GNOME backend.

## Adding a backend

Sway is the worked example: its design and the facts it rests on are in
[Design: a Sway backend](/spawn-at/project/design-sway/).

1. Create a module under `src/platform/linux/`, a file or a folder like `gnome/` and `x11/`. Implement
   `Driver` (`arm` is the required method) and `CompositorBackend`. Most `CompositorBackend` methods
   default to "unsupported", so a new backend can start small.
2. Add a variant to `BackendId` in `src/platform/linux/registry.rs` and teach `candidates` when to
   return it. The functions there read three environment variables and do no I/O, so every desktop
   can be tested without one. `unsupported` in the same file decides which sessions are refused.
3. Add a branch for the new variant in `LinuxBackend::bootstrap()` in `src/platform/linux/mod.rs`,
   which builds the driver.
4. Keep the backend **reachable only through `SPAWN_AT_BACKEND=<name>`** until its live suite
   passes. Sway is never chosen from the desktop.
5. Test it in layers, as Sway does: unit tests for the payloads and the matching rules; driver tests
   against a scripted fake of the compositor's socket; tests of the built binary against a stand-in
   socket (`tests/sway_backend_cli_test.rs`); and a live script on a private session of the
   compositor, run by hand (`scripts/sway-live.sh`).
6. Teach the harness the new backend name (`crates/spawn-at-verify/src/backend.rs`), so that specs can
   list it under `[requires] backend`. See [Writing a test spec](/spawn-at/contributing/test-specs/).

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
  numbers, and the extension files under `assets/gnome`. The extension files changed in v0.2.2 (the
  Shell versions in `metadata.json`) and in v0.2.3 (protocol 7); the `CHANGELOG.md` entry of a
  release says whether they changed.
- The ready-wait policy and its Rust and JavaScript parity (checked by `tests/ready_wait_vectors`),
  the 30 ms legacy rule, and `wait.strategy` and `wait.cap_ms`.
- The placement arithmetic (`verify/golden/placement.json`), config keys and file location, CLI flags
  and `--help` text (the `--pos` and `--offset` help was reworded in v0.2.2), and exit codes.
- The installer flow: the log-out question last, `--clean-install`, the prompt width rules, and the
  behaviour of `install.sh`.

## How a change is made

These are the project's contributor rules. The safety rules for running tests are on
[Testing and the verify tool](/spawn-at/testing/#safety-rules-for-contributors).

- Start every change from an up-to-date `main`. One commit per step, with a conventional message
  (`feat:`, `fix:`, `docs:`, `refactor:`, `test:`, `ci:`, `chore:`). Update doc counts in the same
  commit that changes them.
- Run `cargo fmt --check`, clippy and the tests after every commit, not only at the end.
- Push your branch and open a draft pull request. **Do not merge, tag or push `main` yourself.** The
  order for a release is: pull request, merge, pull `main`, then tag.
- The version comes from the git tag at build time; a build that is not on a tag prints `DEV BUILD`.
  `install.sh` is fetched from `main`, so a merge ships changes to it at once. Changes to the binary
  ship only with the next tag.
- A pure move needs proof: the sorted test names per binary, the `--help` of every subcommand, and
  the hashes of goldens and fixtures must be identical, and `git diff -M --stat` must show moves
  only. Never mix moves and logic in one commit. See `REFACTOR_PROOFS.md`.
- A comment-only commit must pass `cargo xtask check-comment-only <rev>`.
  `scripts/check-comment-only-product.sh <rev>` runs it over the product code. Write comments from
  the function body, not the function name; if unsure, leave it out.
- Keep `CHANGELOG.md` current (new entries under Unreleased) and add facts that took more than a few
  steps to find to `docs/KNOWLEDGE.md`.
- A page of this guide follows the rules and the templates on
  [Writing the docs](/spawn-at/contributing/writing-docs/); `cargo xtask docs-site` must exit 0.

## Build and test

The commands that CI runs (`.github/workflows/ci.yml`: format, clippy, the Rust tests and the Node
tests), and what each one needs, are in the table on
[Testing and the verify tool](/spawn-at/testing/#which-command-for-what). CI also compile-checks the
whole workspace for Windows and for both macOS targets; no test runs there.

The release workflow (`.github/workflows/release.yml`, on `v*` tags) builds the release binary with
`cargo build --release --locked`, which produces `target/release/spawn-at`. `ci.yml` does not run
that build.

Before the tests and the build, the workflow runs `cargo xtask check-release-version <tag>`: the
`version` of the four `Cargo.toml` files and the four matching `Cargo.lock` entries must all equal the
tag's version, or the run fails and names each value that differs. If `CHANGELOG.md` has a non-empty
`## [<version>]` section, `cargo xtask release-notes` writes it out and it becomes the release body;
otherwise the release keeps GitHub's generated notes. `workflow_dispatch` starts a dry run that
does all of that (with an optional `tag` input to check against) and never publishes: the one step
that publishes runs only for a push of a `v*` tag. Written from the file and the unit tests in
`crates/xtask/src/release.rs`; the workflow itself has not run on GitHub.

## Build this documentation

The site is built with [Starlight](https://starlight.astro.build/) (Astro). You need Node.js and
npm; the major version CI uses is in `docs-site/.nvmrc` (24), and `docs-site/package.json` pins
the exact versions of `astro` and `@astrojs/starlight`. `npm ci` downloads the packages from the npm
registry the first time (installed with `--ignore-scripts`, so no package runs an install script).

```bash
cargo xtask docs-site           # builds the binary, the CLI reference, the site and rustdoc, then checks them
```

The finished site is in `target/docs-site/site/`; open `index.html` there. The pages are the
markdown files in `docs-site/src/content/docs/`, and the order of the menu is the `sidebar` in
`docs-site/astro.config.mjs`. Links between pages are written as `/spawn-at/<page>/`, because the
site is served under `/spawn-at/`. The pieces:

| Command | What it does |
| :--- | :--- |
| `cargo xtask docs-cli-reference --bin target/debug/spawn-at` | Writes the CLI reference pages from `--help` into `docs-site/src/content/docs/reference/`. Fails if the binary is missing, a command has no page, or the sidebar does not list that folder |
| `cargo xtask docs-mirror` | Writes the "Project documents" pages from the files in `docs/` |
| `cargo xtask docs-check-src` | Checks the markdown and the config: front matter, a duplicate `#` title, empty pages, pages missing from the sidebar, `.md` links, environment pages, `Sources` lists, image references, `smartypants: false`, forbidden text. It also prints readability warnings, which never fail it |
| `npm ci` and `npm run build` in `docs-site/` | Install the pinned packages and build the site into `docs-site/dist/` (`docs-site` does both; it reuses `node_modules` when it is newer than the lock file) |
| `cargo xtask docs-check-html <dir>` | Checks a built site for broken links, missing ids, a flag such as `--id` turned into an en dash, and third-party requests |

## Not verified

- The steps for adding a backend were read from the source and from how Sway is wired in. They were
  not run as a procedure to build a new backend.

## Sources

- `CONTRIBUTING.md` and the repository's contributor rules
- `src/platform/linux/registry.rs` and `src/platform/linux/mod.rs` (`BackendId`, `candidates`,
  `unsupported`, `LinuxBackend::bootstrap`)
- `docs/CODE_LAYOUT.md` ("Workspace")
- `docs/DESIGN_PLATFORMS.md`, sections 2, 4 and 7
- `docs/REFACTOR_PROOFS.md`
- `docs/KNOWLEDGE.md` ("Architecture map", "Install and release")
- `.github/workflows/ci.yml`, `.github/workflows/release.yml` (the release build)
- `crates/xtask/src/` (the documentation commands)
