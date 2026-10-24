---
title: "Project documents"
sidebar:
  label: Overview
  order: 0
---

The pages in this section are files of the `docs/` folder of the repository: design notes, the test
plan, the code layout and house rules. They are written by `cargo xtask docs-mirror` when the site is
built and are not stored in the site's source, so each page is the file in `docs/` as it is in the
commit the site was built from. To change one, change the file in `docs/`; the page follows.

The headings of the files are their titles, links between the files point to the pages here, and
every page ends with a `## Sources` section that names its file. These pages are working documents,
not a reference: they keep the wording and the dates of the day they were written, so a statement in
one of them can be older than the code.

- [Code layout](/spawn-at/project/code-layout/): where each part of the code lives.
- [Behavior notes](/spawn-at/project/behavior-notes/): the coordinate model, reference areas and monitor defaults.
- [Settings](/spawn-at/project/config/): the `spawn-at config` command and its keys.
- [Legacy GNOME (42–44)](/spawn-at/project/legacy-gnome/): differences of the legacy extension and what the installer prints.
- [Design: more platforms](/spawn-at/project/design-platforms/): restructuring `spawn-at` for more platforms.
- [Design: a Sway backend](/spawn-at/project/design-sway/)
- [Design: waiting until the window is ready](/spawn-at/project/design-ready-wait/)
- [Design: tests of an existing window in the verify runner](/spawn-at/project/design-runner-setup/)
- [Test library and verification plan](/spawn-at/project/test-plan/)
- [Command and flicker test coverage audit](/spawn-at/project/test-coverage-audit/)
- [Proofs for behavior-preserving refactors](/spawn-at/project/refactor-proofs/)
- [Comment style](/spawn-at/project/comment-style/)
- [Knowledge base](/spawn-at/project/knowledge/)
- [Project history](/spawn-at/project/history/): what changed in the documents and the tests, and when.

One file in `docs/` is not mirrored, and links to it go to GitHub: the experiment log
([`TEST_LOG.md`](https://github.com/harshp2008/spawn-at/blob/main/docs/TEST_LOG.md)). The release
notes of each version are in their own menu group, [Release notes](/spawn-at/release-notes/).
The experiment records of sections 6.10 to 6.35 and P8 of the log have a page each under Experiments
in the menu.

## Sources

- `docs/` (the files named above) and `crates/xtask/src/docs_mirror.rs` (the list of mirrored and
  excluded files, and what the mirror changes in a file).
- `docs-site/astro.config.mjs` (the sidebar group "Project documents").
