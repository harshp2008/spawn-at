//! `docs-check-src`: checks the markdown of the book before it is built.
//!
//! It fails when: a page linked from `SUMMARY.md` is missing or has no body; one of the four
//! environment pages is missing; a hand-written page has no `## Sources` list; or any page
//! contains text that must not be published (AI attribution, `TODO`, `TBD`, `lorem`, an
//! absolute home-directory path, the owner's first name).

use std::fs;
use std::path::{Path, PathBuf};

/// Pages every build must have, relative to the book's `src/`.
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

/// Link targets (`.md` files) of a `SUMMARY.md`, in order.
pub fn summary_links(summary: &str) -> Vec<String> {
    let mut links = Vec::new();
    for line in summary.lines() {
        let mut rest = line;
        while let Some(i) = rest.find("](") {
            let after = &rest[i + 2..];
            if let Some(end) = after.find(')') {
                let target = &after[..end];
                if target.ends_with(".md") && !target.contains("://") {
                    links.push(target.to_string());
                }
                rest = &after[end..];
            } else {
                break;
            }
        }
    }
    links
}

/// Non-blank lines that are not headings.
fn body_lines(text: &str) -> usize {
    text.lines()
        .filter(|l| !l.trim().is_empty() && !l.trim_start().starts_with('#'))
        .count()
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

/// Reasons the text may not be published; empty when it is clean.
pub fn forbidden_in(text: &str) -> Vec<String> {
    FORBIDDEN
        .iter()
        .filter(|(needle, word, _)| contains_text(text, needle, *word))
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

/// A page generated from the binary (not hand-written, so it has no `Sources` list).
fn is_generated(rel: &str) -> bool {
    rel.starts_with("reference/") && rel != "reference/index.md"
}

/// All problems found under `src`, one line each. Empty means the book source passes.
pub fn check(src: &Path) -> Result<Vec<String>, String> {
    let summary_path = src.join("SUMMARY.md");
    let summary = fs::read_to_string(&summary_path)
        .map_err(|e| format!("cannot read {}: {}", summary_path.display(), e))?;
    let mut problems = Vec::new();

    for rel in summary_links(&summary) {
        match fs::read_to_string(src.join(&rel)) {
            Err(_) => problems.push(format!("{rel}: linked from SUMMARY.md but missing")),
            Ok(text) if body_lines(&text) < 2 => problems.push(format!(
                "{rel}: empty page (fewer than 2 lines of body text)"
            )),
            Ok(_) => {}
        }
    }
    for rel in ENVIRONMENT_PAGES {
        if !summary_links(&summary).iter().any(|l| l == rel) {
            problems.push(format!(
                "{rel}: environment page is not linked from SUMMARY.md"
            ));
        }
        if !src.join(rel).is_file() {
            problems.push(format!("{rel}: environment page does not exist"));
        }
    }

    let mut files = Vec::new();
    markdown_files(src, &mut files).map_err(|e| format!("cannot read {}: {}", src.display(), e))?;
    files.sort();
    for file in files {
        let rel = file
            .strip_prefix(src)
            .unwrap_or(&file)
            .to_string_lossy()
            .replace('\\', "/");
        let text = fs::read_to_string(&file)
            .map_err(|e| format!("cannot read {}: {}", file.display(), e))?;
        for why in forbidden_in(&text) {
            problems.push(format!("{rel}: {why}"));
        }
        if rel != "SUMMARY.md" && !is_generated(&rel) && !has_sources_list(&text) {
            problems.push(format!("{rel}: no `## Sources` heading with a list"));
        }
    }
    Ok(problems)
}

/// Entry point of `cargo xtask docs-check-src [dir]` (default `docs-site/src`).
pub fn run(args: &[String]) -> Result<(), String> {
    let src = PathBuf::from(args.first().map(String::as_str).unwrap_or("docs-site/src"));
    let problems = check(&src)?;
    if problems.is_empty() {
        println!("==> book source OK ({})", src.display());
        Ok(())
    } else {
        Err(problems.join("\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Only `.md` targets that are not URLs are page links; a nested list item and a bare
    /// heading are handled, and two links on one line are both found.
    #[test]
    fn test_summary_links() {
        let s = "# Summary\n- [A](a.md)\n  - [B](dir/b.md) [C](c.md)\n- [Ext](https://x.org/y.md)\n---\n";
        assert_eq!(summary_links(s), vec!["a.md", "dir/b.md", "c.md"]);
    }

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

    /// A Sources list needs the `##` heading and at least one item; an empty heading, a deeper
    /// heading, or items under another heading do not count.
    #[test]
    fn test_has_sources_list() {
        assert!(has_sources_list("# T\ntext\n## Sources\n\n- `README.md`\n"));
        assert!(!has_sources_list("# T\n## Sources\n\n## Next\n- item\n"));
        assert!(!has_sources_list("# T\n### Sources\n- item\n"));
        assert!(!has_sources_list("# T\n- item\n"));
    }

    fn book(tag: &str, files: &[(&str, &str)]) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("xtask-docs-check-{}-{}", std::process::id(), tag));
        let _ = fs::remove_dir_all(&dir);
        for (rel, text) in files {
            let path = dir.join(rel);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, text).unwrap();
        }
        dir
    }

    const PAGE: &str = "# P\n\nSome real text.\nMore text.\n\n## Sources\n\n- `README.md`\n";

    fn full_summary() -> String {
        let mut s = String::from("# Summary\n- [Intro](intro.md)\n");
        for p in ENVIRONMENT_PAGES {
            s.push_str(&format!("- [E]({p})\n"));
        }
        s
    }

    /// A complete small book passes with no problems.
    #[test]
    fn test_check_passes_a_complete_book() {
        let summary = full_summary();
        let mut files: Vec<(&str, &str)> = vec![("SUMMARY.md", &summary), ("intro.md", PAGE)];
        for p in ENVIRONMENT_PAGES {
            files.push((p, PAGE));
        }
        let dir = book("complete", &files);
        assert_eq!(check(&dir).unwrap(), Vec::<String>::new());
        let _ = fs::remove_dir_all(&dir);
    }

    /// A missing environment page, an empty page, a page without Sources and a page with a
    /// forbidden word each produce their own problem line.
    #[test]
    fn test_check_reports_each_kind_of_problem() {
        let summary = full_summary();
        let mut files: Vec<(&str, &str)> = vec![
            ("SUMMARY.md", &summary),
            ("intro.md", "# Intro\n"),
            ("stray.md", "# S\n\nTODO later\n"),
        ];
        for p in &ENVIRONMENT_PAGES[..3] {
            files.push((p, PAGE));
        }
        let dir = book("problems", &files);
        let problems = check(&dir).unwrap().join("\n");
        assert!(
            problems.contains("unsupported-desktops.md: environment page does not exist"),
            "{problems}"
        );
        assert!(problems.contains("intro.md: empty page"), "{problems}");
        assert!(problems.contains("intro.md: no `## Sources`"), "{problems}");
        assert!(problems.contains("stray.md: contains `todo`"), "{problems}");
        let _ = fs::remove_dir_all(&dir);
    }

    /// Generated reference pages are exempt from the Sources rule but not from the word rules.
    #[test]
    fn test_generated_pages_are_exempt_from_sources_only() {
        assert!(is_generated("reference/spawn-at-spawn.md"));
        assert!(!is_generated("reference/index.md"));
        assert!(!is_generated("environments/sway.md"));
    }
}
