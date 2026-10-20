//! `docs-mirror`: writes selected `docs/*.md` files as pages of the site's "Project documents"
//! section (`docs-site/src/content/docs/project/`) at build time, so `docs/` stays the only copy.
//!
//! The pages are generated and git ignores everything in that folder except the hand-written
//! `index.md`. Which files are mirrored is a fixed list (`MIRRORED`: file, page name, sidebar
//! order); `TEST_LOG.md` and `RELEASE_NOTES_*.md` are excluded on purpose (`EXCLUDED_FILES`,
//! `EXCLUDED_PREFIXES`). A `docs/*.md` file that is in neither list fails the run, so a new file
//! is always a decision. Outside code fences, the text of a file is changed in four ways:
//!
//! - the first `# ` heading becomes the front-matter `title` (double-quoted YAML) and is removed,
//!   because Starlight prints the title as the heading; `sidebar.order` and the short
//!   `sidebar.label` come from the list;
//! - a link to a mirrored sibling (`DESIGN_SWAY.md#x`) becomes `/spawn-at/project/design-sway/#x`;
//! - a link to an excluded file (`TEST_LOG.md#x`) or out of `docs/` (`../CHANGELOG.md`) becomes a
//!   GitHub `blob/main` URL; links to `#anchors` and to other sites are left alone;
//! - a `## Sources` section naming the source file with its GitHub URL is appended.
//!
//! The sidebar reaches the pages through an `autogenerate` entry for the folder `project` in
//! `docs-site/astro.config.mjs`, which this module checks for, as `docs_cli` does for `reference`.

use std::fs;
use std::path::{Path, PathBuf};

use crate::docs_check::has_sources_list;
use crate::docs_cli::{sidebar_autogenerates, yaml_quote};

/// One mirrored file.
pub struct Entry {
    /// File name in `docs/`.
    pub file: &'static str,
    /// Page name: the generated file is `<slug>.md` and the page is `/spawn-at/project/<slug>/`.
    pub slug: &'static str,
    /// Position in the sidebar group (`sidebar.order`); `project/index.md` is 0.
    pub order: u32,
    /// Short name in the sidebar (`sidebar.label`), in sentence case, written from the file's
    /// `# ` heading. The page title and the browser tab still follow the heading.
    pub label: &'static str,
}

/// Every mirrored file. The order is the sidebar order: how the code is laid out, how placement
/// behaves, the settings, the legacy extension, the design notes, then tests and house rules.
pub const MIRRORED: [Entry; 13] = [
    Entry {
        file: "CODE_LAYOUT.md",
        slug: "code-layout",
        order: 1,
        label: "Code layout",
    },
    Entry {
        file: "behavior-notes.md",
        slug: "behavior-notes",
        order: 2,
        label: "Behavior notes",
    },
    Entry {
        file: "CONFIG.md",
        slug: "config",
        order: 3,
        label: "Settings",
    },
    Entry {
        file: "legacy-gnome.md",
        slug: "legacy-gnome",
        order: 4,
        label: "Legacy GNOME (42–44)",
    },
    Entry {
        file: "DESIGN_PLATFORMS.md",
        slug: "design-platforms",
        order: 5,
        label: "Design: more platforms",
    },
    Entry {
        file: "DESIGN_SWAY.md",
        slug: "design-sway",
        order: 6,
        label: "Design: Sway backend",
    },
    Entry {
        file: "DESIGN_READY_WAIT.md",
        slug: "design-ready-wait",
        order: 7,
        label: "Design: ready-wait",
    },
    Entry {
        file: "DESIGN_RUNNER_SETUP.md",
        slug: "design-runner-setup",
        order: 8,
        label: "Design: runner setup",
    },
    Entry {
        file: "TEST_PLAN.md",
        slug: "test-plan",
        order: 9,
        label: "Test plan",
    },
    Entry {
        file: "TEST_COVERAGE_AUDIT.md",
        slug: "test-coverage-audit",
        order: 10,
        label: "Test coverage audit",
    },
    Entry {
        file: "REFACTOR_PROOFS.md",
        slug: "refactor-proofs",
        order: 11,
        label: "Refactor proofs",
    },
    Entry {
        file: "COMMENT_STYLE.md",
        slug: "comment-style",
        order: 12,
        label: "Comment style",
    },
    Entry {
        file: "KNOWLEDGE.md",
        slug: "knowledge",
        order: 13,
        label: "Knowledge base",
    },
];

/// Files of `docs/` that are not mirrored: the experiment log (a 1900-line page that names the
/// author's machine in run directories and journal lines) stays on GitHub.
pub const EXCLUDED_FILES: [&str; 1] = ["TEST_LOG.md"];

/// File-name prefixes that are not mirrored: the release notes are linked from the changelog page.
pub const EXCLUDED_PREFIXES: [&str; 1] = ["RELEASE_NOTES_"];

/// Where the pages are served.
const SITE_BASE: &str = "/spawn-at/project/";
/// Where files of the repository can be read.
const GITHUB_BLOB: &str = "https://github.com/harshp2008/spawn-at/blob/main/";

fn is_excluded(file: &str) -> bool {
    EXCLUDED_FILES.contains(&file) || EXCLUDED_PREFIXES.iter().any(|p| file.starts_with(p))
}

fn entry_for(file: &str) -> Option<&'static Entry> {
    MIRRORED.iter().find(|e| e.file == file)
}

/// Problems with the list against the `.md` file names present in `docs/`: a file that is neither
/// mirrored nor excluded, a mirrored file that does not exist, a slug or an order used twice, a
/// slug with a character other than `a-z`, `0-9` and `-`.
pub fn manifest_problems(present: &[String]) -> Vec<String> {
    let mut problems = Vec::new();
    for file in present {
        if entry_for(file).is_none() && !is_excluded(file) {
            problems.push(format!(
                "docs/{file}: neither mirrored nor excluded; add it to MIRRORED or \
                 EXCLUDED_FILES in crates/xtask/src/docs_mirror.rs"
            ));
        }
    }
    for entry in &MIRRORED {
        if !present.iter().any(|f| f == entry.file) {
            problems.push(format!(
                "docs/{}: listed in MIRRORED but the file does not exist",
                entry.file
            ));
        }
        if !entry
            .slug
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        {
            problems.push(format!(
                "slug `{}` has a character other than a-z, 0-9 and -",
                entry.slug
            ));
        }
        if MIRRORED.iter().filter(|e| e.slug == entry.slug).count() > 1 {
            problems.push(format!("slug `{}` is used twice", entry.slug));
        }
        if MIRRORED.iter().filter(|e| e.order == entry.order).count() > 1 {
            problems.push(format!("sidebar order {} is used twice", entry.order));
        }
    }
    problems
}

/// New target for a link found in a mirrored file, or `None` when the link stays as written
/// (an `#anchor`, another site, a site-absolute path). `Err` for a `.md` file of `docs/` that is
/// not in either list.
fn rewrite_target(target: &str) -> Result<Option<String>, String> {
    if target.is_empty()
        || target.starts_with('#')
        || target.starts_with('/')
        || target.starts_with("mailto:")
        || target.contains("://")
    {
        return Ok(None);
    }
    let (path, frag) = match target.find('#') {
        Some(i) => (&target[..i], &target[i..]),
        None => (target, ""),
    };
    if let Some(rest) = path.strip_prefix("../") {
        if rest.starts_with("../") {
            return Err(format!("`{target}` goes up more than one folder"));
        }
        return Ok(Some(format!("{GITHUB_BLOB}{rest}{frag}")));
    }
    if path.contains('/') {
        return Ok(Some(format!("{GITHUB_BLOB}docs/{path}{frag}")));
    }
    if let Some(entry) = entry_for(path) {
        return Ok(Some(format!("{SITE_BASE}{}/{frag}", entry.slug)));
    }
    if is_excluded(path) || !path.ends_with(".md") {
        return Ok(Some(format!("{GITHUB_BLOB}docs/{path}{frag}")));
    }
    Err(format!(
        "`{target}` links to a docs/ file that is neither mirrored nor excluded"
    ))
}

/// `line` with every `](target)` link target rewritten by `rewrite_target`. Inline code is not
/// skipped, because `docs_check::md_links` does not skip it either.
fn rewrite_links(line: &str) -> Result<String, String> {
    let mut out = String::with_capacity(line.len());
    let mut rest = line;
    while let Some(i) = rest.find("](") {
        let after = &rest[i + 2..];
        let Some(end) = after.find(')') else { break };
        let target = &after[..end];
        out.push_str(&rest[..i + 2]);
        match rewrite_target(target)? {
            Some(new) => out.push_str(&new),
            None => out.push_str(target),
        }
        rest = &after[end..];
    }
    out.push_str(rest);
    Ok(out)
}

fn is_fence(line: &str) -> bool {
    let t = line.trim_start();
    t.starts_with("```") || t.starts_with("~~~")
}

/// The `## Sources` section of a mirrored page: its source file and the GitHub URL, which ends in
/// `.md` and so is written with `://` (the site check allows that for other sites).
fn sources_section(file: &str) -> String {
    format!(
        "\n## Sources\n\n- [`docs/{file}`]({GITHUB_BLOB}docs/{file}) in the repository. This page \
         is generated from that file when the site is built; edit that file, not this page.\n"
    )
}

/// The page for `docs/<entry.file>` whose text is `text`.
pub fn render(entry: &Entry, text: &str) -> Result<String, String> {
    let mut title = None;
    let mut body = String::new();
    let mut in_fence = false;
    for line in text.lines() {
        if is_fence(line) {
            in_fence = !in_fence;
            body.push_str(line);
            body.push('\n');
            continue;
        }
        if in_fence {
            body.push_str(line);
            body.push('\n');
            continue;
        }
        if title.is_none() {
            if let Some(t) = line.strip_prefix("# ") {
                title = Some(t.trim().to_string());
                continue;
            }
        }
        if line.trim_end() == "## Sources" {
            return Err(format!(
                "docs/{}: already has a `## Sources` heading; the mirror appends one",
                entry.file
            ));
        }
        body.push_str(&rewrite_links(line).map_err(|e| format!("docs/{}: {e}", entry.file))?);
        body.push('\n');
    }
    let title =
        title.ok_or_else(|| format!("docs/{}: no `# ` heading to use as title", entry.file))?;
    let mut page = format!(
        "---\ntitle: {}\nsidebar:\n  order: {}\n  label: {}\n---\n\n",
        yaml_quote(&title),
        entry.order,
        yaml_quote(entry.label)
    );
    page.push_str(body.trim_start_matches('\n'));
    page.push_str(&sources_section(entry.file));
    debug_assert!(has_sources_list(&page));
    Ok(page)
}

/// `.md` file names directly inside `dir`, sorted.
fn md_names(dir: &Path) -> Result<Vec<String>, String> {
    let entries = fs::read_dir(dir).map_err(|e| format!("cannot read {}: {}", dir.display(), e))?;
    let mut names = Vec::new();
    for entry in entries {
        let path = entry
            .map_err(|e| format!("cannot read {}: {}", dir.display(), e))?
            .path();
        if path.is_file() && path.extension().is_some_and(|e| e == "md") {
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                names.push(name.to_string());
            }
        }
    }
    names.sort();
    Ok(names)
}

/// Removes `*.md` files of `dir` that are not in `keep`: pages of a file that is no longer
/// mirrored. `index.md` (hand-written) and files of any other type are left alone.
fn remove_stale_pages(dir: &Path, keep: &[PathBuf]) -> Result<(), String> {
    for name in md_names(dir)? {
        let path = dir.join(&name);
        if name != "index.md" && !keep.contains(&path) {
            fs::remove_file(&path)
                .map_err(|e| format!("cannot remove {}: {}", path.display(), e))?;
        }
    }
    Ok(())
}

/// Writes one page per `MIRRORED` entry into `out_dir` and removes stale ones. Fails, writing
/// nothing, if the list does not match `docs_dir`, if the sidebar in `sidebar_config` has no
/// `autogenerate` entry for `project`, or if a file cannot be turned into a page.
pub fn generate(
    docs_dir: &Path,
    out_dir: &Path,
    sidebar_config: &str,
) -> Result<Vec<PathBuf>, String> {
    let mut problems = manifest_problems(&md_names(docs_dir)?);
    if !sidebar_autogenerates(sidebar_config, "project") {
        problems.push(
            "the sidebar in astro.config.mjs has no `autogenerate: { directory: 'project' }` \
             entry, so the mirrored pages would not be in the menu"
                .to_string(),
        );
    }
    let mut pages = Vec::new();
    if problems.is_empty() {
        for entry in &MIRRORED {
            let path = docs_dir.join(entry.file);
            let text = fs::read_to_string(&path)
                .map_err(|e| format!("cannot read {}: {}", path.display(), e))?;
            match render(entry, &text) {
                Ok(page) => pages.push((out_dir.join(format!("{}.md", entry.slug)), page)),
                Err(e) => problems.push(e),
            }
        }
    }
    if !problems.is_empty() {
        return Err(problems.join("\n"));
    }
    fs::create_dir_all(out_dir)
        .map_err(|e| format!("cannot create {}: {}", out_dir.display(), e))?;
    let mut written = Vec::new();
    for (path, page) in pages {
        fs::write(&path, page).map_err(|e| format!("cannot write {}: {}", path.display(), e))?;
        written.push(path);
    }
    remove_stale_pages(out_dir, &written)?;
    Ok(written)
}

/// Entry point of `cargo xtask docs-mirror [--docs DIR] [--out DIR] [--config FILE]` (defaults
/// `docs`, `docs-site/src/content/docs/project` and `docs-site/astro.config.mjs`).
pub fn run(args: &[String]) -> Result<(), String> {
    let mut docs = PathBuf::from("docs");
    let mut out = PathBuf::from("docs-site/src/content/docs/project");
    let mut config = PathBuf::from("docs-site/astro.config.mjs");
    let mut it = args.iter();
    while let Some(flag) = it.next() {
        let value = it.next().ok_or_else(|| format!("{flag} needs a value"))?;
        match flag.as_str() {
            "--docs" => docs = PathBuf::from(value),
            "--out" => out = PathBuf::from(value),
            "--config" => config = PathBuf::from(value),
            other => return Err(format!("unknown option {other}")),
        }
    }
    let config_text = fs::read_to_string(&config)
        .map_err(|e| format!("cannot read {}: {}", config.display(), e))?;
    let written = generate(&docs, &out, &config_text)?;
    println!(
        "==> wrote {} project pages to {}",
        written.len(),
        out.display()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::docs_check::{forbidden_in, has_h1, md_links, split_front_matter};

    const CONFIG: &str = "items: [{ autogenerate: { directory: 'project' } }]";

    fn temp(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("xtask-docs-mirror-{}-{}", std::process::id(), tag));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn entry(file: &str) -> &'static Entry {
        entry_for(file).unwrap()
    }

    /// Names of every file the list covers, as `docs/` would hold them, plus the excluded ones.
    fn all_names() -> Vec<String> {
        let mut names: Vec<String> = MIRRORED.iter().map(|e| e.file.to_string()).collect();
        names.push("TEST_LOG.md".to_string());
        names.push("RELEASE_NOTES_0.2.1.md".to_string());
        names
    }

    /// A title with `&`, backticks, a double quote and a backslash stays one YAML scalar: the
    /// quote and the backslash are escaped, `&` and the backticks are kept. Unquoted, `&` would
    /// start a YAML anchor and the page would lose its title.
    #[test]
    fn test_title_is_yaml_quoted() {
        let page = render(
            entry("CONFIG.md"),
            "# Tests & `config` \"a\" \\ b\n\nText\n",
        )
        .unwrap();
        assert!(
            page.starts_with(
                "---\ntitle: \"Tests & `config` \\\"a\\\" \\\\ b\"\nsidebar:\n  order: 3\n  label: \"Settings\"\n---\n\nText\n"
            ),
            "{page}"
        );
        assert!(!page.contains("\n# "), "the H1 is removed: {page}");
    }

    /// The sidebar label is its own front-matter key under `sidebar:`; the page title keeps
    /// following the heading, so a long heading gets a short label and the page is still
    /// called by its heading.
    #[test]
    fn test_sidebar_label_is_separate_from_the_title() {
        let page = render(
            entry("legacy-gnome.md"),
            "# Legacy GNOME (42–44) Differences & Compatibility Analysis\n\nText\n",
        )
        .unwrap();
        let (front, _) = split_front_matter(&page).unwrap();
        assert!(
            front.contains("title: \"Legacy GNOME (42–44) Differences & Compatibility Analysis\""),
            "{front}"
        );
        assert!(
            front.contains("sidebar:\n  order: 4\n  label: \"Legacy GNOME (42–44)\""),
            "{front}"
        );
        assert_eq!(
            crate::docs_check::front_matter_title(front),
            "Legacy GNOME (42–44) Differences & Compatibility Analysis"
        );
    }

    /// Every label is filled in, starts with a capital letter, has no space at either end, is at
    /// most 24 characters (a sidebar line at the narrowest width of the desktop layout) and is
    /// used once, so two pages cannot look the same in the menu.
    #[test]
    fn test_labels_are_short_and_distinct() {
        let mut seen: Vec<&str> = Vec::new();
        for e in &MIRRORED {
            let label = e.label;
            assert!(!label.is_empty(), "{}: empty label", e.file);
            assert_eq!(label, label.trim(), "{}: spaces around the label", e.file);
            assert!(
                label.chars().next().is_some_and(char::is_uppercase),
                "{}: `{label}` does not start with a capital",
                e.file
            );
            assert!(
                label.chars().count() <= 24,
                "{}: `{label}` is longer than 24 characters",
                e.file
            );
            assert!(!seen.contains(&label), "label `{label}` is used twice");
            seen.push(label);
        }
    }

    /// A page without a `# ` heading outside a fence cannot get a title; a `# ` line inside a
    /// fence (a shell comment) is not one.
    #[test]
    fn test_title_comes_from_a_heading_outside_fences() {
        assert!(render(entry("CONFIG.md"), "text\n```\n# comment\n```\n").is_err());
        let page = render(entry("CONFIG.md"), "# T\n```\n# comment\n```\n").unwrap();
        assert!(page.contains("```\n# comment\n```"), "{page}");
    }

    /// Sibling links go to `/spawn-at/project/<slug>/` with the fragment kept; the same text in
    /// a fence is untouched, as is an in-page anchor and a link to another site.
    #[test]
    fn test_links_are_rewritten_outside_fences_only() {
        let text = "# T\n\nSee [sway](DESIGN_SWAY.md#4-x) and [self](#top) and \
                    [web](https://example.com/a.md).\n\n```text\n[sway](DESIGN_SWAY.md)\n```\n";
        let page = render(entry("CONFIG.md"), text).unwrap();
        assert!(
            page.contains("[sway](/spawn-at/project/design-sway/#4-x) and [self](#top) and [web](https://example.com/a.md)"),
            "{page}"
        );
        assert!(
            page.contains("```text\n[sway](DESIGN_SWAY.md)\n```"),
            "{page}"
        );
        let (_, body) = split_front_matter(&page).unwrap();
        assert_eq!(md_links(body), Vec::<String>::new());
    }

    /// A link to an excluded docs file (the experiment log, release notes) or to a file outside
    /// `docs/` becomes a GitHub URL with its fragment, because the site has no page for it.
    #[test]
    fn test_links_to_excluded_and_parent_files_go_to_github() {
        let text = "# T\n\n[log](TEST_LOG.md#610-x) [notes](RELEASE_NOTES_0.2.0.md) \
                    [cl](../CHANGELOG.md) [v](../verify/README.md) [s](../scripts/sway-headless.sh)\n";
        let page = render(entry("CONFIG.md"), text).unwrap();
        for expected in [
            "(https://github.com/harshp2008/spawn-at/blob/main/docs/TEST_LOG.md#610-x)",
            "(https://github.com/harshp2008/spawn-at/blob/main/docs/RELEASE_NOTES_0.2.0.md)",
            "(https://github.com/harshp2008/spawn-at/blob/main/CHANGELOG.md)",
            "(https://github.com/harshp2008/spawn-at/blob/main/verify/README.md)",
            "(https://github.com/harshp2008/spawn-at/blob/main/scripts/sway-headless.sh)",
        ] {
            assert!(page.contains(expected), "missing {expected} in:\n{page}");
        }
    }

    /// A link to a docs file that the lists do not know is an error, not a dead link.
    #[test]
    fn test_link_to_an_unlisted_docs_file_is_an_error() {
        let err = render(entry("CONFIG.md"), "# T\n\n[x](NEW_DOC.md)\n").unwrap_err();
        assert!(err.contains("NEW_DOC.md"), "{err}");
    }

    /// The appended Sources section names the file and its GitHub URL, passes the site rule, and
    /// a file that already has its own `## Sources` is refused rather than given two.
    #[test]
    fn test_sources_section_is_appended_once() {
        let page = render(entry("KNOWLEDGE.md"), "# T\n\nText\n").unwrap();
        assert!(
            page.ends_with(
                "- [`docs/KNOWLEDGE.md`](https://github.com/harshp2008/spawn-at/blob/main/docs/KNOWLEDGE.md) in the repository. This page is generated from that file when the site is built; edit that file, not this page.\n"
            ),
            "{page}"
        );
        assert!(has_sources_list(&page));
        assert!(render(entry("KNOWLEDGE.md"), "# T\n\n## Sources\n\n- x\n").is_err());
    }

    /// The complete set of names passes; an unlisted file, a missing mirrored file and a
    /// duplicate are each reported.
    #[test]
    fn test_manifest_completeness() {
        assert_eq!(manifest_problems(&all_names()), Vec::<String>::new());

        let mut with_new = all_names();
        with_new.push("NEW_DOC.md".to_string());
        let problems = manifest_problems(&with_new).join("\n");
        assert!(
            problems.contains("docs/NEW_DOC.md: neither mirrored nor excluded"),
            "{problems}"
        );

        let without_one: Vec<String> = all_names()
            .into_iter()
            .filter(|n| n != "TEST_PLAN.md")
            .collect();
        let problems = manifest_problems(&without_one).join("\n");
        assert!(
            problems.contains("docs/TEST_PLAN.md: listed in MIRRORED but the file does not exist"),
            "{problems}"
        );
    }

    /// The list itself has no repeated slug or order and only `a-z0-9-` in slugs (checked by
    /// `manifest_problems` over the full set of names).
    #[test]
    fn test_manifest_has_unique_slugs_and_orders() {
        let mut slugs: Vec<&str> = MIRRORED.iter().map(|e| e.slug).collect();
        slugs.sort_unstable();
        slugs.dedup();
        assert_eq!(slugs.len(), MIRRORED.len());
        let mut orders: Vec<u32> = MIRRORED.iter().map(|e| e.order).collect();
        orders.sort_unstable();
        assert_eq!(orders, (1..=MIRRORED.len() as u32).collect::<Vec<_>>());
    }

    /// `generate` writes one page per entry, then a later run with a stale page in the folder
    /// removes it, keeps the hand-written `index.md` and leaves a non-markdown file alone.
    #[test]
    fn test_generate_writes_pages_and_removes_stale_ones() {
        let base = temp("generate");
        let docs = base.join("docs");
        let out = base.join("project");
        fs::create_dir_all(&docs).unwrap();
        fs::create_dir_all(&out).unwrap();
        for name in all_names() {
            fs::write(docs.join(&name), "# Title\n\nText\n").unwrap();
        }
        fs::write(out.join("index.md"), "hand-written").unwrap();
        fs::write(out.join("old-page.md"), "stale").unwrap();
        fs::write(out.join("notes.txt"), "other").unwrap();

        let written = generate(&docs, &out, CONFIG).unwrap();
        assert_eq!(written.len(), 13);
        assert!(out.join("test-plan.md").is_file());
        assert!(!out.join("old-page.md").exists(), "stale page removed");
        assert_eq!(
            fs::read_to_string(out.join("index.md")).unwrap(),
            "hand-written"
        );
        assert!(out.join("notes.txt").is_file(), "non-markdown file kept");
        assert!(
            !out.join("test-log.md").exists(),
            "excluded file not mirrored"
        );
        let _ = fs::remove_dir_all(&base);
    }

    /// `generate` writes nothing when `docs/` holds a file the lists do not know, or when the
    /// sidebar has no `autogenerate` entry for `project`.
    #[test]
    fn test_generate_fails_on_unlisted_file_or_missing_sidebar_entry() {
        let base = temp("fails");
        let docs = base.join("docs");
        let out = base.join("project");
        fs::create_dir_all(&docs).unwrap();
        for name in all_names() {
            fs::write(docs.join(&name), "# Title\n\nText\n").unwrap();
        }
        fs::write(docs.join("STRAY.md"), "# Stray\n").unwrap();
        let err = generate(&docs, &out, CONFIG).unwrap_err();
        assert!(
            err.contains("docs/STRAY.md: neither mirrored nor excluded"),
            "{err}"
        );
        assert!(!out.exists(), "nothing written");

        fs::remove_file(docs.join("STRAY.md")).unwrap();
        let err = generate(&docs, &out, "items: ['index']").unwrap_err();
        assert!(err.contains("autogenerate"), "{err}");
        assert!(!out.exists(), "nothing written");
        let _ = fs::remove_dir_all(&base);
    }

    /// `remove_stale_pages` keeps the listed pages and `index.md`, and removes any other `.md`.
    #[test]
    fn test_remove_stale_pages() {
        let dir = temp("stale");
        for name in ["index.md", "keep.md", "gone.md", "other.txt"] {
            fs::write(dir.join(name), "x").unwrap();
        }
        remove_stale_pages(&dir, &[dir.join("keep.md")]).unwrap();
        assert!(dir.join("index.md").is_file());
        assert!(dir.join("keep.md").is_file());
        assert!(!dir.join("gone.md").exists());
        assert!(dir.join("other.txt").is_file());
        let _ = fs::remove_dir_all(&dir);
    }

    /// The real `docs/` folder matches the lists, and every real file renders into a page with
    /// no `# ` heading, no `.md` link, a Sources list and none of the forbidden text. This is the
    /// check that fails in `cargo test` (not only in `docs-site`) when someone adds a `docs/`
    /// file, or writes a path or name into a mirrored file that the site must not publish.
    #[test]
    fn test_real_docs_render_into_clean_pages() {
        let docs = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs");
        assert_eq!(
            manifest_problems(&md_names(&docs).unwrap()),
            Vec::<String>::new()
        );
        for e in &MIRRORED {
            let text = fs::read_to_string(docs.join(e.file)).unwrap();
            let page = render(e, &text).unwrap_or_else(|err| panic!("{err}"));
            let (front, body) = split_front_matter(&page).unwrap();
            let title = crate::docs_check::front_matter_title(front);
            assert!(!title.is_empty(), "{}", e.file);
            // a label is a short form of the heading, never longer than the title it stands for
            assert!(
                e.label.chars().count() <= title.chars().count(),
                "{}: label `{}` is longer than the title `{title}`",
                e.file,
                e.label
            );
            assert!(!has_h1(body), "{}: has a # heading", e.file);
            assert_eq!(md_links(body), Vec::<String>::new(), "{}", e.file);
            assert!(has_sources_list(body), "{}", e.file);
            assert_eq!(forbidden_in(&page), Vec::<String>::new(), "{}", e.file);
        }
    }
}
