# CLI reference

The pages in this section are the output of `--help`, one page per command, unchanged. They are
written by `cargo xtask docs-cli-reference` from the `spawn-at` binary at the time the site is
built, and they are not stored in the repository, so they match the binary the site was built
from and cannot drift from it. If the binary is missing, the build stops with an error.

Pages exist for the root command and for every command that a `Commands:` list shows, including
nested ones (`query layout`, `config set`). The `help` entry that clap adds to each list is not a
command and has no page. The build also fails if `--help` lists a command that has no page or no
entry in the book's table of contents.

Things that `--help` does not show, found by running the binary:

- `spawn-at -V` (and `--version`) prints the version, but neither is listed in `spawn-at --help`.
  There is no `version` command: `spawn-at version` is rejected as an unrecognized subcommand.
- `restore` also answers to `unmaximize`, `unmaximise`, `unminimize` and `unminimise`. They are
  not listed in `--help`; `spawn-at unmaximize --help` prints the `restore` help.
- `spawn` and `config` accept hidden wait flags that the help does not list. They are not
  documented here.

For what each command is for, see [Commands](../commands/index.md).

## Sources

- The output of `spawn-at --help` and `spawn-at <command> --help` of the binary built from this
  commit (`crates/xtask/src/docs_cli.rs`).
- `docs/TEST_COVERAGE_AUDIT.md`, section 2 (aliases of `restore`, hidden wait flags), checked
  against the binary.
