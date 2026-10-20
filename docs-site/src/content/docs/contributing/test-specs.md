---
title: "Writing a test spec"
---

A test of the verify harness is one TOML file, called a spec, under `verify/<category>/`. This page
shows how to add or change one, how to check it, and what else moves with it. The field list is in
`verify/README.md`; the way tests are run is on [Testing and the verify tool](/spawn-at/testing/).

## The shortest way

Copy a spec that is close to what you want and change it. This is `verify/placement/top.toml`:

```toml
id = "placement/top"
category = "placement"
title = "Window placed at top edge"
description = "Spawns a test window anchored to the top edge of the work area with a 16px boundary margin."
kind = "auto"
required = true
tags = ["visual", "needs-fixture"]

[requires]
session = ["any"]
backend = ["any"]
gnome = "any"
monitors_min = 1
scale = "any"
apps = []

[subject]
app = "fixture"
profile = "plain"
args = ["--title", "Verify-Placement-Top", "--app-id", "verify.placement.top", "--expect", "Should appear at top edge"]

[action]
command = "spawn"
repeatable = false
args = ["--class", "verify.placement.top", "--anchor", "top", "--size", "400", "300", "--margin", "16"]

[expect]
exit_code = 0
stderr_absent = ["[spawn-at] ERROR:", "[spawn-at] WARNING:"]

[expect.rect]
anchor = "top"
size = [400, 300]
margin = 16
tolerance_px = 1

[human]
watch = "Did the window appear once at the top edge, without flickering or jumping?"
```

The file starts the fixture (the repository's own test window) through `spawn-at spawn`, then checks
the exit code, the text on stderr and the rectangle of the window.

## Fields

| Field | Meaning |
| :-- | :-- |
| `id` | The path of the file under `verify/`, without `.toml` |
| `category` | One of `basics`, `placement`, `transforms`, `focus`, `animation-and-cloak`, `queries`, `robustness`, `platform`, `hostile` |
| `kind` | `auto` (automatic assertions), `human` (a person looks) or `both` |
| `required` | When true, a pass counts toward qualifying an environment as supported |
| `tags` | Free labels such as `visual`, `slow`, `needs-fixture`, `moves-focus`. Platform tags are derived from `[requires]` and must not be repeated |
| `[requires]` | The environment the spec needs. An unmet need ends the test as Skipped with the reason, never as a pass |
| `[subject]` | The app to start: `app = "fixture"` or `"system:<command>"`, with its arguments |
| `[action]` | The `spawn-at` subcommand to run and its arguments |
| `[expect]` | `exit_code`, `stderr_absent`, `stderr_present`, `diagnostics`, `never_visible_off_target`, and `[expect.rect]` |
| `[human]` | `watch`: the one visual thing the person must look for |
| `[regression]` | `commit` and `note`, for a test that guards a past bug |

The id, the category and the margin are checked when the spec loads:

- The id must equal the path. A spec at `verify/placement/top.toml` has the id `placement/top`.
- The category is the `category` field, not the first part of the id. The spec
  `cloak/delayed-paint` has `category = "animation-and-cloak"`.
- `[expect.rect]` must give `margin` (or all four per-side margins). The oracle assumes 0 when none
  is given, while `spawn-at spawn` uses 16, so an implicit margin would be judged wrong.

`stderr_absent` and `stderr_present` match literal substrings, case-sensitive. A pattern that starts
with `re:` is a regular expression.

A `[matrix]` block makes several tests from one file. Each combination gets the id
`<base>/<combination>`. For example `delay = [0, 50, 150, 500]` in `cloak/delayed-paint` gives
`cloak/delayed-paint/delay-0` and three more. A `{delay}` in a string is replaced.

## Specs that act on an existing window

A spec with `schema_version = 2` can start its own fixture windows with `[[setup]]`, run more actions
with `[[then]]`, and assert with `[[check]]`. It names a window only as `--id ${name}`, where `name`
is a setup it declared. The loader refuses a literal id, `-c`, `-t`, `--pid` and `--focused`, so a
spec cannot name a window the runner did not create.

A spec with a `[[setup]]` must carry the tag `moves-focus`, because a new window takes focus. The
`quick` suite leaves that tag out. The full example and the list of rules are in
`verify/README.md` (section 2, "Version 2 specs"), and the reasons are in
[Design: tests of an existing window](/spawn-at/project/design-runner-setup/).

## Check and run a spec

1. Create the file under `verify/<category>/`.
2. Check that it loads: `target/debug/spawn-at-verify check`. The error names the file and the rule.
3. Check that it is selected: `target/debug/spawn-at-verify list --glob 'placement/top*'`.
4. See what would run without running it: `target/debug/spawn-at-verify run --glob 'placement/top' --dry-run`.
5. Run it as described on [Testing](/spawn-at/testing/): `scripts/verify-sway.sh -- --glob '<id>'` on a
   private headless Sway, or `run` or the [TUI](/spawn-at/contributing/verify-tui/) on an idle GNOME
   desktop.

If the oracle and `spawn-at` disagree, the failure stands. **Do not change the oracle to fit the
code.**

## What else moves with a new spec

- The files in `verify/suites/` select specs by category and tag (a tag with `!` in front excludes
  it). A test checks that every category a suite names has specs, and that every spec is selected by
  some suite. A spec in a new category or with a new tag can fail that test until a suite names it.
- `crates/spawn-at-verify/tests/catalog_golden_test.rs` pins the 45 original specs. New specs may
  be added; none of the 45 may change.
- The catalog in `docs/TEST_PLAN.md` section 2 has a row per planned spec. Update the row, and the
  counts that change, in the same commit.

## Not verified

- The steps above were read from the loader, the CLI help and `verify/README.md`; the commands were
  not run for this page.

## Sources

- `verify/README.md` (section 2, "Test Definition Schema", and the version 2 rules)
- `crates/spawn-at-verify/src/loader/registry.rs` (id, margin and category checks)
- `crates/spawn-at-verify/src/schema/matrix.rs` (`VALID_CATEGORIES`)
- `crates/spawn-at-verify/src/schema/types.rs` (the version 2 keys)
- `verify/placement/top.toml`, `verify/cloak/delayed-paint.toml`, `verify/suites/quick.toml`
- `docs/KNOWLEDGE.md` ("The verify catalog")
- the output of `spawn-at-verify check --help`, `list --help` and `run --help`
