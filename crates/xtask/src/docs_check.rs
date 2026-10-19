//! `docs-check-src`: checks the markdown of the site (`docs-site/src/content/docs`) and the
//! Starlight config before the site is built.
//!
//! It fails when: a page has no front matter or no non-empty `title`; a page repeats its title as
//! a `# ` heading (Starlight prints the title as the heading); a page has fewer than 2 lines of
//! body text; a page is not named in the sidebar of `astro.config.mjs` (the generated CLI
//! reference and the mirrored `project/` pages are covered by an `autogenerate` entry instead);
//! one of the four environment pages is
//! missing; a hand-written page has no `## Sources` list; a link ends in `.md` (Starlight serves
//! no such file; links are `/spawn-at/<slug>/`); the config does not set
//! `markdown.smartypants: false`; or any page contains text that must not be published (AI
//! attribution, `TODO`, `TBD`, `lorem`, an absolute home-directory path, the owner's first name).
//! The one string allowed to contain that name is the extension identifier `spawn-at@harsh.local`
//! (`ALLOWED`).

use std::fs;
use std::path::{Path, PathBuf};

use crate::docs_cli::sidebar_autogenerates;

/// Pages every build must have, relative to the content folder.
pub const ENVIRONMENT_PAGES: [&str; 4] = [
    "environments/gnome-wayland.md",
    "environments/sway.md",
    "environments/x11-xwayland.md",
    "environments/unsupported-desktops.md",
];

/// Text that must not appear in any page: (needle, whole word only, reason). Matching ignores case.
const FORBIDDEN: [(&str, bool, &str); 14] = [
    ("co-authored-by", false, "AI attribution trailer"),
    ("generated with", false, "AI attribution line"),
    ("claude.ai", false, "AI session link"),
    ("anthropic.com", false, "AI vendor link"),
    ("noreply@anthropic", false, "AI attribution address"),
    ("session_0", false, "AI session id"),
    ("todo", true, "unfinished marker"),
    ("tbd", true, "unfinished marker"),
    ("lorem", true, "placeholder text"),
    ("/home/", false, "absolute path under /home"),
    ("/users/", false, "absolute path under /Users"),
    ("\\users\\", false, "absolute Windows user path"),
    ("harsh", true, "the owner's name"),
    ("🤖", false, "AI attribution marker"),
];

/// Splits a page into its front-matter text and the body after it. `None` if the page does not
/// start with a `---` line or the block is never closed by another `---` line.
pub fn split_front_matter(text: &str) -> Option<(&str, &str)> {
    let rest = text.strip_prefix("---\n")?;
    let mut offset = 0;
    for line in rest.split_inclusive('\n') {
        if line.trim_end() == "---" {
            return Some((&rest[..offset], &rest[offset + line.len()..]));
        }
        offset += line.len();
    }
    None
}

/// The value of the top-level `title:` key of a front-matter block, without surrounding quotes.
/// Empty when the key is missing or has no value.
pub fn front_matter_title(front: &str) -> String {
    for line in front.lines() {
        if let Some(value) = line.strip_prefix("title:") {
            return value
                .trim()
                .trim_matches(|c| c == '"' || c == '\'')
                .to_string();
        }
    }
    String::new()
}

/// Non-blank lines that are not headings.
fn body_lines(text: &str) -> usize {
    text.lines()
        .filter(|l| !l.trim().is_empty() && !l.trim_start().starts_with('#'))
        .count()
}

/// Lines of `body` outside code fences, with their 1-based numbers (counted from the body).
fn prose_lines(body: &str) -> Vec<(usize, &str)> {
    let mut in_fence = false;
    let mut out = Vec::new();
    for (i, line) in body.lines().enumerate() {
        if line.trim_start().starts_with("```") || line.trim_start().starts_with("~~~") {
            in_fence = !in_fence;
        } else if !in_fence {
            out.push((i + 1, line));
        }
    }
    out
}

/// True if the body has a line `# ...` (a level-1 heading) outside code fences.
pub fn has_h1(body: &str) -> bool {
    prose_lines(body).iter().any(|(_, l)| l.starts_with("# "))
}

/// Link targets in `body` (outside code fences) that end in `.md`, with or without a `#fragment`.
/// The book this site replaces linked pages that way; Starlight does not serve those files.
pub fn md_links(body: &str) -> Vec<String> {
    let mut found = Vec::new();
    for (_, line) in prose_lines(body) {
        let mut rest = line;
        while let Some(i) = rest.find("](") {
            let after = &rest[i + 2..];
            let Some(end) = after.find(')') else { break };
            let target = &after[..end];
            let path = target.split('#').next().unwrap_or("");
            if path.ends_with(".md") && !target.contains("://") {
                found.push(target.to_string());
            }
            rest = &after[end..];
        }
    }
    found
}

/// True if `hay` holds `needle` (ASCII or not) ignoring case; with `whole_word` the characters on
/// both sides must not be letters or digits.
pub fn contains_text(hay: &str, needle: &str, whole_word: bool) -> bool {
    let hay = hay.to_lowercase();
    let needle = needle.to_lowercase();
    let mut from = 0;
    while let Some(i) = hay[from..].find(&needle) {
        let start = from + i;
        let end = start + needle.len();
        if !whole_word {
            return true;
        }
        let before = hay[..start].chars().next_back();
        let after = hay[end..].chars().next();
        let edge = |c: Option<char>| c.is_none_or(|c| !c.is_alphanumeric());
        if edge(before) && edge(after) {
            return true;
        }
        from = start + needle.len().max(1);
    }
    false
}

/// Strings that contain a forbidden word but are published on purpose. Only the exact text is
/// allowed: `spawn-at@harsh.local` is the identifier of the GNOME Shell extension, so the Files
/// and paths page has to name it. Every other `harsh` still fails.
const ALLOWED: [&str; 1] = ["spawn-at@harsh.local"];

/// `text` with each allowed string replaced by a space. An occurrence counts only when it is
/// not part of a longer word (the characters on both sides are not letters or digits), so
/// `xspawn-at@harsh.local` or `spawn-at@harsh.localhost` are left as they are and still fail.
fn without_allowed(text: &str) -> String {
    let mut out = text.to_string();
    for allowed in ALLOWED {
        let mut result = String::with_capacity(out.len());
        let mut rest = out.as_str();
        let mut before: Option<char> = None;
        while let Some(i) = rest.find(allowed) {
            let end = i + allowed.len();
            let prev = rest[..i].chars().next_back().or(before);
            let next = rest[end..].chars().next();
            let edge = |c: Option<char>| c.is_none_or(|c| !c.is_alphanumeric());
            if edge(prev) && edge(next) {
                result.push_str(&rest[..i]);
                result.push(' ');
            } else {
                result.push_str(&rest[..end]);
            }
            before = rest[..end].chars().next_back();
            rest = &rest[end..];
        }
        result.push_str(rest);
        out = result;
    }
    out
}

/// Reasons the text may not be published; empty when it is clean.
pub fn forbidden_in(text: &str) -> Vec<String> {
    let text = without_allowed(text);
    FORBIDDEN
        .iter()
        .filter(|(needle, word, _)| contains_text(&text, needle, *word))
        .map(|(needle, _, why)| format!("contains `{needle}` ({why})"))
        .collect()
}

/// True if the page has a `## Sources` heading followed by at least one list item before the
/// next heading.
pub fn has_sources_list(text: &str) -> bool {
    let mut in_sources = false;
    for line in text.lines() {
        let t = line.trim_start();
        if t.starts_with('#') {
            if in_sources {
                return false;
            }
            in_sources = t.trim_start_matches('#').trim() == "Sources" && t.starts_with("## ");
        } else if in_sources && (t.starts_with("- ") || t.starts_with("* ")) {
            return true;
        }
    }
    false
}

/// The sidebar name of a page file: `install.md` is `install`, `commands/index.md` is `commands`,
/// and the top-level `index.md` is `index`.
pub fn page_slug(rel: &str) -> String {
    let no_ext = rel.strip_suffix(".md").unwrap_or(rel);
    match no_ext.strip_suffix("/index") {
        Some(dir) => dir.to_string(),
        None => no_ext.to_string(),
    }
}

/// True if `config` names the page `slug` as a string, in `'single'` or `"double"` quotes. Text
/// search, not a JavaScript parse: `'install'` and `slug: 'commands'` both match.
pub fn sidebar_names(config: &str, slug: &str) -> bool {
    config.contains(&format!("'{slug}'")) || config.contains(&format!("\"{slug}\""))
}

/// True if the config turns smart punctuation off. Left on, markdown turns `--id` in prose into
/// an en dash.
pub fn disables_smartypants(config: &str) -> bool {
    config
        .lines()
        .any(|l| !l.trim_start().starts_with("//") && l.contains("smartypants: false"))
}

fn markdown_files(dir: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            markdown_files(&path, out)?;
        } else if path.extension().is_some_and(|e| e == "md") {
            out.push(path);
        }
    }
    Ok(())
}

/// A page generated from the binary (not hand-written, so it has no `Sources` list and the
/// sidebar reaches it through `autogenerate`).
fn is_generated(rel: &str) -> bool {
    rel.starts_with("reference/") && rel != "reference/index.md"
}

/// A page written by `docs-mirror` from a file of `docs/`. Unlike a generated reference page it
/// has a `## Sources` list (the mirror appends one) and every other rule applies; only the
/// sidebar-name rule is replaced by the `autogenerate` entry for the folder `project`.
fn is_mirrored(rel: &str) -> bool {
    rel.starts_with("project/") && rel != "project/index.md"
}

/// All problems found under `docs` (the content folder) with the sidebar config at `config`,
/// one line each. Empty means the site source passes.
pub fn check(docs: &Path, config: &Path) -> Result<Vec<String>, String> {
    let config_text = fs::read_to_string(config)
        .map_err(|e| format!("cannot read {}: {}", config.display(), e))?;
    let mut problems = Vec::new();

    if !disables_smartypants(&config_text) {
        problems.push(format!(
            "{}: `markdown: {{ smartypants: false }}` is not set, so `--id` in prose would be \
             rendered with an en dash",
            config.display()
        ));
    }
    if !sidebar_autogenerates(&config_text, "reference") {
        problems.push(format!(
            "{}: no `autogenerate: {{ directory: 'reference' }}` sidebar entry for the CLI reference",
            config.display()
        ));
    }
    for rel in ENVIRONMENT_PAGES {
        if !docs.join(rel).is_file() {
            problems.push(format!("{rel}: environment page does not exist"));
        }
    }

    let mut files = Vec::new();
    markdown_files(docs, &mut files)
        .map_err(|e| format!("cannot read {}: {}", docs.display(), e))?;
    files.sort();
    for file in files {
        let rel = file
            .strip_prefix(docs)
            .unwrap_or(&file)
            .to_string_lossy()
            .replace('\\', "/");
        let text = fs::read_to_string(&file)
            .map_err(|e| format!("cannot read {}: {}", file.display(), e))?;
        for why in forbidden_in(&text) {
            problems.push(format!("{rel}: {why}"));
        }
        let Some((front, body)) = split_front_matter(&text) else {
            problems.push(format!(
                "{rel}: no front matter (a `---` block with `title:`)"
            ));
            continue;
        };
        if front_matter_title(front).is_empty() {
            problems.push(format!("{rel}: front matter has no `title`"));
        }
        if has_h1(body) {
            problems.push(format!(
                "{rel}: has a `# ` heading; Starlight prints the title as the heading"
            ));
        }
        if body_lines(body) < 2 {
            problems.push(format!(
                "{rel}: empty page (fewer than 2 lines of body text)"
            ));
        }
        for target in md_links(body) {
            problems.push(format!(
                "{rel}: link to `{target}` ends in .md; link to /spawn-at/<slug>/ instead"
            ));
        }
        if !is_generated(&rel) {
            if !has_sources_list(body) {
                problems.push(format!("{rel}: no `## Sources` heading with a list"));
            }
            let covered = is_mirrored(&rel) && sidebar_autogenerates(&config_text, "project");
            if !covered && !sidebar_names(&config_text, &page_slug(&rel)) {
                problems.push(format!(
                    "{rel}: page `{}` is not named in the sidebar of {}",
                    page_slug(&rel),
                    config.display()
                ));
            }
        }
    }
    Ok(problems)
}

/// Entry point of `cargo xtask docs-check-src [DOCS_DIR] [--config FILE]` (defaults
/// `docs-site/src/content/docs` and `docs-site/astro.config.mjs`).
pub fn run(args: &[String]) -> Result<(), String> {
    let mut docs = PathBuf::from("docs-site/src/content/docs");
    let mut config = PathBuf::from("docs-site/astro.config.mjs");
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--config" => {
                config = PathBuf::from(it.next().ok_or("--config needs a value")?);
            }
            other if !other.starts_with("--") => docs = PathBuf::from(other),
            other => return Err(format!("unknown option {other}")),
        }
    }
    let problems = check(&docs, &config)?;
    if problems.is_empty() {
        println!("==> site source OK ({})", docs.display());
        Ok(())
    } else {
        Err(problems.join("\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `todo` as a word is forbidden but `autodocs`/`mastodon` are not; `harsh` as a word is
    /// forbidden but the repository owner's account name `harshp2008` is not.
    #[test]
    fn test_whole_word_matching() {
        assert!(contains_text("a TODO here", "todo", true));
        assert!(!contains_text("mastodon todos", "todo", true));
        assert!(contains_text("by Harsh.", "harsh", true));
        assert!(!contains_text(
            "github.com/harshp2008/spawn-at",
            "harsh",
            true
        ));
    }

    /// Each forbidden class is caught: attribution trailer, session link, TODO, lorem, an
    /// absolute home path.
    #[test]
    fn test_forbidden_in_catches_each_class() {
        for bad in [
            "Co-Authored-By: Someone",
            "Generated with a tool",
            "see https://claude.ai/code/session_01",
            "TODO: write this",
            "Lorem ipsum",
            "/home/someone/project",
            "TBD",
        ] {
            assert!(!forbidden_in(bad).is_empty(), "not caught: {bad}");
        }
        assert!(forbidden_in("spawn-at spawn --anchor center gnome-calculator").is_empty());
    }

    /// The extension identifier `spawn-at@harsh.local` passes, alone, in prose, in code spans, in
    /// a path and twice in a line; the name `harsh` anywhere else still fails, also next to the
    /// identifier, and so do the identifier extended by letters or digits, other casing and a
    /// look-alike. A failure of the first group means the Files and paths page cannot name the
    /// extension; a failure of the second means the allowance leaks.
    #[test]
    fn test_extension_id_is_allowed_and_nothing_else_with_harsh() {
        for ok in [
            "spawn-at@harsh.local",
            "The id is `spawn-at@harsh.local`.",
            "~/.local/share/gnome-shell/extensions/spawn-at@harsh.local/",
            "gnome-extensions enable spawn-at@harsh.local, gnome-extensions disable spawn-at@harsh.local",
            "(spawn-at@harsh.local)",
        ] {
            assert!(forbidden_in(ok).is_empty(), "wrongly rejected: {ok}");
        }
        for bad in [
            "harsh",
            "by Harsh.",
            "spawn-at@harsh.local and harsh",
            "harsh and spawn-at@harsh.local",
            "xspawn-at@harsh.local",
            "spawn-at@harsh.localhost",
            "SPAWN-AT@HARSH.LOCAL",
            "spawn-at@harsh.example",
            "other@harsh.local",
            "/home/someone/spawn-at@harsh.local",
        ] {
            assert!(!forbidden_in(bad).is_empty(), "wrongly accepted: {bad}");
        }
    }

    /// A Sources list needs the `##` heading and at least one item; an empty heading, a deeper
    /// heading, or items under another heading do not count.
    #[test]
    fn test_has_sources_list() {
        assert!(has_sources_list("# T\ntext\n## Sources\n\n- `README.md`\n"));
        assert!(!has_sources_list("# T\n## Sources\n\n## Next\n- item\n"));
        assert!(!has_sources_list("# T\n### Sources\n- item\n"));
        assert!(!has_sources_list("# T\n- item\n"));
    }

    /// Front matter is the block between the first two `---` lines; a page without the opening
    /// line, or with a block that never closes, has none. The body starts after the closing line.
    #[test]
    fn test_split_front_matter() {
        let (front, body) = split_front_matter("---\ntitle: \"A\"\n---\n\nText\n").unwrap();
        assert_eq!(front, "title: \"A\"\n");
        assert_eq!(body, "\nText\n");
        assert!(split_front_matter("# A\n\nText\n").is_none());
        assert!(split_front_matter("---\ntitle: A\n\nText\n").is_none());
    }

    /// Quoted, single-quoted and bare titles read the same; a missing or empty `title` gives "".
    #[test]
    fn test_front_matter_title() {
        assert_eq!(front_matter_title("title: \"Install\"\n"), "Install");
        assert_eq!(front_matter_title("title: 'Install'\n"), "Install");
        assert_eq!(
            front_matter_title("sidebar:\n  label: X\ntitle: Install\n"),
            "Install"
        );
        assert_eq!(front_matter_title("title:\n"), "");
        assert_eq!(front_matter_title("description: x\n"), "");
    }

    /// A `# ` line is a duplicate title, but `## ` is not, and a `# comment` inside a code fence
    /// (a shell comment) is not a heading.
    #[test]
    fn test_has_h1_ignores_fences_and_h2() {
        assert!(has_h1("Text\n# Title\n"));
        assert!(!has_h1("## Section\n```bash\n# comment\n```\n"));
    }

    /// Links ending in `.md` are found with and without a fragment; site links, external links
    /// and `.md` text inside a code fence are not reported.
    #[test]
    fn test_md_links() {
        let body = "[a](findings.md) [b](../x/y.md#frag) [c](/spawn-at/findings/) \
                    [d](https://example.org/readme.md)\n```\n[e](hidden.md)\n```\n";
        assert_eq!(md_links(body), vec!["findings.md", "../x/y.md#frag"]);
    }

    /// File names map to the name the sidebar uses: folder index pages take the folder name.
    #[test]
    fn test_page_slug() {
        assert_eq!(page_slug("install.md"), "install");
        assert_eq!(page_slug("commands/index.md"), "commands");
        assert_eq!(page_slug("environments/sway.md"), "environments/sway");
        assert_eq!(page_slug("index.md"), "index");
    }

    /// A slug counts as named only as a whole quoted string: `'commands'` does not satisfy
    /// `commands/spawn`, and an unquoted mention in a comment does not count.
    #[test]
    fn test_sidebar_names() {
        let config =
            "items: ['install', { slug: \"commands\", label: 'Overview' }, 'commands/spawn']";
        assert!(sidebar_names(config, "install"));
        assert!(sidebar_names(config, "commands"));
        assert!(sidebar_names(config, "commands/spawn"));
        assert!(!sidebar_names(config, "commands/query"));
        assert!(!sidebar_names("// install\n", "install"));
    }

    /// `smartypants: false` passes; leaving it out, `true`, or only a comment that mentions it
    /// fails. This is the setting that stops `--id` in prose from becoming an en dash.
    #[test]
    fn test_disables_smartypants() {
        assert!(disables_smartypants("markdown: { smartypants: false },"));
        assert!(!disables_smartypants("markdown: { smartypants: true },"));
        assert!(!disables_smartypants("export default {};"));
        assert!(!disables_smartypants("  // smartypants: false\n"));
    }

    const GOOD_PAGE: &str =
        "---\ntitle: \"P\"\n---\n\nSome real text.\nMore text.\n\n## Sources\n\n- `README.md`\n";

    fn site(tag: &str, files: &[(&str, &str)]) -> (PathBuf, PathBuf) {
        let dir =
            std::env::temp_dir().join(format!("xtask-docs-check-{}-{}", std::process::id(), tag));
        let _ = fs::remove_dir_all(&dir);
        for (rel, text) in files {
            let path = dir.join("docs").join(rel);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, text).unwrap();
        }
        (dir.join("docs"), dir.join("astro.config.mjs"))
    }

    fn config_for(pages: &[&str]) -> String {
        let names: Vec<String> = pages.iter().map(|p| format!("'{p}'")).collect();
        format!(
            "markdown: {{ smartypants: false }},\nitems: [{}, {{ autogenerate: {{ directory: 'reference' }} }}]",
            names.join(", ")
        )
    }

    fn env_slugs() -> Vec<String> {
        ENVIRONMENT_PAGES.iter().map(|p| page_slug(p)).collect()
    }

    /// A complete small site passes with no problems.
    #[test]
    fn test_check_passes_a_complete_site() {
        let mut files: Vec<(&str, &str)> = vec![("index.md", GOOD_PAGE)];
        for p in ENVIRONMENT_PAGES {
            files.push((p, GOOD_PAGE));
        }
        let (docs, config) = site("complete", &files);
        let mut names: Vec<String> = vec!["index".to_string()];
        names.extend(env_slugs());
        let refs: Vec<&str> = names.iter().map(String::as_str).collect();
        fs::write(&config, config_for(&refs)).unwrap();
        assert_eq!(check(&docs, &config).unwrap(), Vec::<String>::new());
        let _ = fs::remove_dir_all(docs.parent().unwrap());
    }

    /// A missing environment page, a page without front matter, a page with an H1, an empty
    /// page, a `.md` link, a page without Sources, a page missing from the sidebar and a page
    /// with a forbidden word each produce their own problem line.
    #[test]
    fn test_check_reports_each_kind_of_problem() {
        let mut files: Vec<(&str, &str)> = vec![
            ("index.md", "---\ntitle: \"I\"\n---\n"),
            ("nofront.md", "# Plain\n\nText\nMore\n"),
            ("h1.md", "---\ntitle: \"H\"\n---\n\n# H\n\nText\nMore\n"),
            (
                "link.md",
                "---\ntitle: \"L\"\n---\n\nsee [x](findings.md)\nMore\n",
            ),
            ("stray.md", "---\ntitle: \"S\"\n---\n\nTODO later\nMore\n"),
            ("untitled.md", "---\ndescription: d\n---\n\nText\nMore\n"),
        ];
        for p in &ENVIRONMENT_PAGES[..3] {
            files.push((p, GOOD_PAGE));
        }
        let (docs, config) = site("problems", &files);
        fs::write(&config, config_for(&["index", "h1", "link", "stray"])).unwrap();
        let problems = check(&docs, &config).unwrap().join("\n");
        for expected in [
            "unsupported-desktops.md: environment page does not exist",
            "index.md: empty page",
            "index.md: no `## Sources`",
            "nofront.md: no front matter",
            "h1.md: has a `# ` heading",
            "link.md: link to `findings.md` ends in .md",
            "stray.md: contains `todo`",
            "untitled.md: front matter has no `title`",
            "untitled.md: page `untitled` is not named in the sidebar",
        ] {
            assert!(
                problems.contains(expected),
                "missing `{expected}` in:\n{problems}"
            );
        }
        let _ = fs::remove_dir_all(docs.parent().unwrap());
    }

    /// A config without `smartypants: false` or without the reference `autogenerate` entry fails
    /// even when every page is fine.
    #[test]
    fn test_check_reports_config_problems() {
        let mut files: Vec<(&str, &str)> = vec![("index.md", GOOD_PAGE)];
        for p in ENVIRONMENT_PAGES {
            files.push((p, GOOD_PAGE));
        }
        let (docs, config) = site("config", &files);
        fs::write(&config, "items: ['index']").unwrap();
        let problems = check(&docs, &config).unwrap().join("\n");
        assert!(problems.contains("smartypants: false"), "{problems}");
        assert!(problems.contains("autogenerate"), "{problems}");
        let _ = fs::remove_dir_all(docs.parent().unwrap());
    }

    /// Generated reference pages are exempt from the Sources rule and the sidebar-name rule
    /// (the sidebar autogenerates them) but not from the word rules.
    #[test]
    fn test_generated_pages_are_exempt_from_sources_only() {
        assert!(is_generated("reference/spawn-at-spawn.md"));
        assert!(!is_generated("reference/index.md"));
        assert!(!is_generated("environments/sway.md"));
    }

    /// Only `project/<page>.md` counts as mirrored; the hand-written `project/index.md` and the
    /// generated reference pages do not.
    #[test]
    fn test_is_mirrored() {
        assert!(is_mirrored("project/test-plan.md"));
        assert!(!is_mirrored("project/index.md"));
        assert!(!is_mirrored("reference/spawn-at-spawn.md"));
        assert!(!is_mirrored("install.md"));
    }

    /// A mirrored page needs no sidebar entry of its own when the config autogenerates the
    /// `project` folder, and fails the sidebar rule when it does not.
    #[test]
    fn test_mirrored_page_is_covered_by_the_project_autogenerate_entry() {
        let mut files: Vec<(&str, &str)> =
            vec![("index.md", GOOD_PAGE), ("project/test-plan.md", GOOD_PAGE)];
        for p in ENVIRONMENT_PAGES {
            files.push((p, GOOD_PAGE));
        }
        let (docs, config) = site("mirrored-sidebar", &files);
        let mut names: Vec<String> = vec!["index".to_string()];
        names.extend(env_slugs());
        let refs: Vec<&str> = names.iter().map(String::as_str).collect();

        fs::write(&config, config_for(&refs)).unwrap();
        let problems = check(&docs, &config).unwrap().join("\n");
        assert!(
            problems.contains("project/test-plan.md: page `project/test-plan` is not named"),
            "{problems}"
        );

        let with_project = format!(
            "{}\n{{ autogenerate: {{ directory: 'project' }} }}",
            config_for(&refs)
        );
        fs::write(&config, with_project).unwrap();
        assert_eq!(check(&docs, &config).unwrap(), Vec::<String>::new());
        let _ = fs::remove_dir_all(docs.parent().unwrap());
    }

    /// The `autogenerate` exemption covers the sidebar rule only: a mirrored page with a `# `
    /// heading, a `.md` link, no Sources list or forbidden text still fails.
    #[test]
    fn test_mirrored_page_still_obeys_every_other_rule() {
        let bad = "---\ntitle: \"P\"\n---\n\n# P\n\nSee [x](CONFIG.md)\nTODO\n";
        let mut files: Vec<(&str, &str)> = vec![("index.md", GOOD_PAGE), ("project/bad.md", bad)];
        for p in ENVIRONMENT_PAGES {
            files.push((p, GOOD_PAGE));
        }
        let (docs, config) = site("mirrored-rules", &files);
        let mut names: Vec<String> = vec!["index".to_string()];
        names.extend(env_slugs());
        let refs: Vec<&str> = names.iter().map(String::as_str).collect();
        let with_project = format!(
            "{}\n{{ autogenerate: {{ directory: 'project' }} }}",
            config_for(&refs)
        );
        fs::write(&config, with_project).unwrap();
        let problems = check(&docs, &config).unwrap().join("\n");
        for expected in [
            "project/bad.md: has a `# ` heading",
            "project/bad.md: link to `CONFIG.md` ends in .md",
            "project/bad.md: no `## Sources`",
            "project/bad.md: contains `todo`",
        ] {
            assert!(
                problems.contains(expected),
                "missing `{expected}` in:\n{problems}"
            );
        }
        assert!(!problems.contains("not named in the sidebar"), "{problems}");
        let _ = fs::remove_dir_all(docs.parent().unwrap());
    }

    /// A generated page with front matter and body passes without a Sources list or a sidebar
    /// entry of its own.
    #[test]
    fn test_check_accepts_generated_page_without_sources_or_sidebar_entry() {
        let mut files: Vec<(&str, &str)> = vec![
            ("index.md", GOOD_PAGE),
            (
                "reference/spawn-at-spawn.md",
                "---\ntitle: \"spawn-at spawn\"\n---\n\nGenerated text.\nMore.\n",
            ),
        ];
        for p in ENVIRONMENT_PAGES {
            files.push((p, GOOD_PAGE));
        }
        let (docs, config) = site("generated", &files);
        let mut names: Vec<String> = vec!["index".to_string()];
        names.extend(env_slugs());
        let refs: Vec<&str> = names.iter().map(String::as_str).collect();
        fs::write(&config, config_for(&refs)).unwrap();
        assert_eq!(check(&docs, &config).unwrap(), Vec::<String>::new());
        let _ = fs::remove_dir_all(docs.parent().unwrap());
    }
}
