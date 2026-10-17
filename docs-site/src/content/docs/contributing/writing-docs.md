---
title: "Writing the docs"
---

How the pages of this book are written and checked. It covers the hand-written pages in
`docs-site/src/content/docs/`. Rust code comments have their own rules in `docs/COMMENT_STYLE.md`
and are not covered here. Everything below is taken from the checks in `crates/xtask/src/` and the
conventions already used by the pages; where the repository states no rule, the text says so.

## Purpose

The book tells a user or a contributor what spawn-at does, and tells them plainly what nobody has
verified. A page that sounds sure about something nobody ran does harm, so the rules below are about
evidence first and style second.

## The truth rule

- Every statement comes from a file in the repository or from a command that was run. The page names
  the file (with line numbers or a heading) in its `## Sources` list, or shows the command and its
  output.
- Anything else is written as "not verified", in the page's own `## Not verified` list or inline. The
  book's front page defines the word: it means no test or run in the repository covers it.
- Code that was read but not run is said to be read: "read from source, not run".
- Do not invent a symptom, a step, a number or a version. If the repository does not record a cause,
  write that it records none.
- If a page has to describe behavior that looks like a bug, it describes what the code does, says so,
  and the finding is reported to the owner; the page does not hide it and the code is not changed in a
  docs change.
- `CLAUDE.md` ("Working style") has the same rules for reports: never claim something is verified
  unless it was run, and always list what could not be run (a real GNOME Wayland session, X11, other
  GNOME versions, macOS, Windows).
- State no release status ("in beta", "from v0.x") in new text. Status notes that already exist, such as
  the box on the Install page that says when it can be removed, are left to the release that
  outdates them.

## Anatomy of a page

A hand-written page is a Markdown file under `docs-site/src/content/docs/`. `cargo xtask
docs-check-src` fails when:

- the file has no front matter (a `---` block) or no non-empty `title:`;
- the page repeats its title as a `# ` heading (Starlight prints the title as the heading, so the body
  starts with `##`);
- the body has fewer than 2 lines of text;
- the page is not named in the sidebar in `docs-site/astro.config.mjs` (the generated CLI reference
  is covered by an `autogenerate` entry instead);
- one of the four environment pages is missing;
- a hand-written page has no `## Sources` heading followed by a list;
- a link ends in `.md`;
- the config does not set `markdown.smartypants: false`;
- the text contains something that must not be published (below).

A page that adds new content therefore needs, in the same commit, its sidebar entry. Put the summary
first, then the body, then `## Not verified`, then `## Sources` last.

## Links

Link to other pages as `/spawn-at/<slug>/`, with the base written out and a trailing slash, for
example `[Update](/spawn-at/update/)`. Starlight does not rewrite links, so a relative link to a `.md`
file gives a 404 and is rejected by the check. A link to a heading uses its generated id:
`/spawn-at/update/#the-menu`. `cargo xtask docs-check-html` follows every relative link, script,
stylesheet and image in the built site and fails on a missing file or a `#fragment` the page does not
have. Links to other sites are counted and listed, not fetched.

The book makes no third-party request: no external fonts, scripts or analytics. The same check fails if
any file names a known font, CDN or analytics host.

## Text that must not be published

`crates/xtask/src/docs_check.rs` (`FORBIDDEN`, 14 entries) rejects these anywhere in a page, ignoring
case. The exact strings are in that constant; this page does not repeat them, because a page that
contained them would fail the check.

| Class | Why |
| :--- | :--- |
| AI attribution: a co-author trailer, a tool-credit line, a session link or id, the vendor's domain and the no-reply address, the robot emoji | the book carries no machine attribution |
| Two marker words for unfinished work, and one filler word for placeholder text (the markers match as whole words) | a published page must be finished |
| An absolute path under a user's home directory, in the Linux, macOS and Windows forms | it names a person's machine |
| The owner's first name, as a whole word | the book does not use it |

The repository owner's account name in a URL is allowed, because the match is whole-word only. The
one string that contains the owner's name on purpose is the extension identifier, which the check
allows only as that exact string (`ALLOWED`, with a test): not extended by letters or digits and not
in another letter case. For example the Files and paths page writes `spawn-at@harsh.local`. Anything
else that needs one of these words is rewritten.

## Flags and dashes

`markdown.smartypants` is off in `astro.config.mjs`, because smart punctuation would turn `--id` in
prose into an en dash and `...` into an ellipsis. `docs-check-html` fails if a dash glued to the front
of a word appears in the built text. Write flags and paths in backticks anyway, and write the long form
of a flag (`--margin-top`) the way `--help` prints it.

## Generated and hand-written pages

- The CLI reference (`reference/spawn-at*.md`) is generated from the built binary's `--help` at build time
  by `cargo xtask docs-cli-reference`. It is not committed (`.gitignore` lists it; only
  `reference/index.md` is tracked) and has no `Sources` list. To change it, change the `--help` text in
  the source; the reference follows. A wording problem in a generated page (for example a flag whose
  `--help` text differs from its verified behavior) is explained on a hand-written page that links to
  it.
- Everything else is hand-written and must follow the anatomy above.

## Building and checking

```bash
cargo xtask docs-site                       # everything, in CI order; site in target/docs-site/site/
cargo xtask docs-check-src                  # markdown, sidebar, Sources, forbidden text
cargo xtask docs-cli-reference --bin target/debug/spawn-at   # only the generated reference
cargo xtask docs-check-html target/docs-site/site --generated-dir api/   # links, ids, dashes, requests
```

`docs-site` needs Node and npm (the major version is in `docs-site/.nvmrc`) and runs `npm ci
--ignore-scripts` in `docs-site/`. Set `ASTRO_TELEMETRY_DISABLED=1` for any manual `astro` or `npm`
run there. The CI job is `.github/workflows/docs.yml`; it runs the same command on every pull request.
The checks above are listed in `docs/CODE_LAYOUT.md` ("Documentation site").

## Review checklist

- Every statement has a source in `## Sources` or a command shown with its output.
- What was not run is under `## Not verified`; code read but not run is marked as such.
- The page is in the sidebar, has front matter and a `title`, has no `# ` heading, and ends with
  `## Sources` and a list.
- Links are `/spawn-at/<slug>/`, none ends in `.md`, and the anchors exist.
- No forbidden text, no home path, no first name, no placeholder words.
- Numbers (test counts, line counts, versions) are from a run, with the date; counts that changed are
  updated in the same commit that changed them (`CLAUDE.md`, "Git and PRs").
- `cargo xtask docs-site` exits 0.

## Page template

Copy this to a new file, fill it in and add the slug to the sidebar:

````markdown
---
title: "Name of the page"
---

One or two sentences that say what the page covers and what it was written from.

## Heading of the first section

The body. Quote exact strings, flags and paths in backticks. Give line numbers for source claims.

## Not verified

- What was not run, with the reason.

## Sources

- `path/to/file` (heading or line numbers)
- the output of `command --help`
````

## Experiment record template

One page per experiment, under `experiments/`. It reads as a short report: an opening paragraph, a
table of fields with the verdict in it, then three `##` sections. The verdict comes first, in the
table, because a reader wants the outcome before the method. A field the source does not record is
written "not recorded"; never infer a build, a date or a command.

````markdown
---
title: "ID Short name"
---

One to three sentences: what was tested, and the outcome in plain words. Include the full title of the
experiment if it says more than the page title.

| Field | Value |
| :-- | :-- |
| **ID** | The id in the source (for example TEST_LOG 6.30), with links to the record before and after it |
| **Date** | The date the source gives, as YYYY-MM-DD |
| **Build** | The tag, or "DEV BUILD" with the commit; if only a tree or commit is recorded say so and infer no version; otherwise "not recorded" |
| **Verdict** | The source's verdict, in its words; if it gives none, say so |
| **Raw results** | Where they are; "not in the repository" if they are in a gitignored directory |

## What was tested

**Question**

What the experiment was meant to find out, in the source's words.

**Setup and environment**

The session, the machine, the load, the sample sizes, the arms.

**Commands run**

The commands, or "Exact command lines not recorded." followed by the prose the source gives.

## Result

The numbers and observations, as the source gives them. Use `###` or bold lead-ins for phases.

## Not verified

What the experiment did not cover.

## Sources

- `path/to/log` (section and lines)
````

## Not verified

What the experiment did not cover.

## Sources

- `path/to/log` (section and lines)
````

## Not verified

- The CI job `docs.yml` was read, not run, for this page. The commands in the build section were taken
  from `docs/CODE_LAYOUT.md` and `crates/xtask/src/main.rs`.
- Whether the sidebar check accepts a page nested in a new folder without a folder entry: this page is
  the first nested under `contributing/`, and the check ran with it.

## Sources

- `crates/xtask/src/docs_check.rs` (module header, `FORBIDDEN`, `ALLOWED`, `check`)
- `crates/xtask/src/docs_html.rs` (module header)
- `docs/CODE_LAYOUT.md` ("Documentation site")
- `docs-site/astro.config.mjs` (sidebar, `markdown.smartypants`)
- `docs-site/src/content/docs/index.md` (the "not verified" definition and the Sources convention)
- `docs/COMMENT_STYLE.md` ("Truth rule") and `CLAUDE.md` ("Working style", "Git and PRs")
