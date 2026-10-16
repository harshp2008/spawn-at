//! `docs-check-html`: checks the assembled site (book plus `api/`) without any network access.
//!
//! It fails when: a relative link, script, stylesheet or image points at a file that is not in
//! the site; a `#fragment` names an id that its page does not have; a script, stylesheet, image,
//! iframe or other resource would be fetched from another host; a file names a known
//! third-party font, CDN or analytics host; or a page has an element with the `playground` class.
//! Links to other sites in `<a href>` are counted and listed, not fetched: they are navigation,
//! not requests the page makes by itself.
//!
//! The `playground` rule exists because mdBook's own `book.js` contains the code that sends a
//! code block to `play.rust-lang.org`. That code only runs when the user presses the Run button
//! of an element with the `playground` class, which `[output.html.playground] runnable = false`
//! stops mdBook from producing. So the host name is not searched for (it is always in the
//! script); the condition that could make the request is.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Component, Path, PathBuf};

/// Hosts that must not appear anywhere in the site's own files.
const THIRD_PARTY: [&str; 8] = [
    "googleapis",
    "gstatic",
    "googletagmanager",
    "google-analytics",
    "cdnjs",
    "jsdelivr",
    "unpkg.com",
    "plausible.io",
];

/// One start tag with its attributes (names lower-cased, values with the common entities decoded).
#[derive(Debug, PartialEq)]
pub struct Tag {
    /// Lower-case tag name.
    pub name: String,
    /// Attributes in source order.
    pub attrs: Vec<(String, String)>,
}

impl Tag {
    /// Value of the attribute `name`, if present.
    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    }
}

fn decode_entities(s: &str) -> String {
    s.replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&#x27;", "'")
        .replace("&#x2F;", "/")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            if let Some(v) = s
                .get(i + 1..i + 3)
                .and_then(|h| u8::from_str_radix(h, 16).ok())
            {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Start tags of an HTML document. Comments, end tags and the bodies of `<script>` and
/// `<style>` are skipped.
pub fn scan_tags(html: &str) -> Vec<Tag> {
    let b = html.as_bytes();
    let lower = html.to_ascii_lowercase();
    let mut tags = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if b[i] != b'<' {
            i += 1;
            continue;
        }
        if html[i..].starts_with("<!--") {
            i = html[i..].find("-->").map_or(b.len(), |e| i + e + 3);
            continue;
        }
        if !b.get(i + 1).is_some_and(|c| c.is_ascii_alphabetic()) {
            i += 1;
            continue;
        }
        let name_start = i + 1;
        let mut j = name_start;
        while j < b.len() && !b[j].is_ascii_whitespace() && b[j] != b'>' && b[j] != b'/' {
            j += 1;
        }
        let name = lower[name_start..j].to_string();
        let mut attrs = Vec::new();
        loop {
            while j < b.len() && (b[j].is_ascii_whitespace() || b[j] == b'/') {
                j += 1;
            }
            if j >= b.len() || b[j] == b'>' {
                break;
            }
            let k_start = j;
            while j < b.len() && !b[j].is_ascii_whitespace() && !matches!(b[j], b'=' | b'>' | b'/')
            {
                j += 1;
            }
            let key = lower[k_start..j].to_string();
            while j < b.len() && b[j].is_ascii_whitespace() {
                j += 1;
            }
            let mut value = String::new();
            if j < b.len() && b[j] == b'=' {
                j += 1;
                while j < b.len() && b[j].is_ascii_whitespace() {
                    j += 1;
                }
                if j < b.len() && (b[j] == b'"' || b[j] == b'\'') {
                    let q = b[j];
                    let v_start = j + 1;
                    j = v_start;
                    while j < b.len() && b[j] != q {
                        j += 1;
                    }
                    value = decode_entities(&html[v_start..j.min(b.len())]);
                    j += 1;
                } else {
                    let v_start = j;
                    while j < b.len() && !b[j].is_ascii_whitespace() && b[j] != b'>' {
                        j += 1;
                    }
                    value = decode_entities(&html[v_start..j]);
                }
            }
            if !key.is_empty() {
                attrs.push((key, value));
            }
        }
        i = (j + 1).min(b.len());
        if name == "script" || name == "style" {
            let close = format!("</{name}");
            i = lower[i..].find(&close).map_or(b.len(), |e| i + e);
        }
        tags.push(Tag { name, attrs });
    }
    tags
}

/// Where a reference points.
#[derive(Debug, PartialEq)]
pub enum Target {
    /// Another site, or a scheme that is not a file (`mailto:`).
    External(String),
    /// A file inside the site and an optional fragment.
    Local(PathBuf, Option<String>),
    /// Nothing to check (empty, `javascript:`, `data:`).
    Skip,
    /// A site-absolute path that is not under the site prefix.
    OutsideSite(String),
}

fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in path.components() {
        match c {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// Classifies `href` found in the file `from` (relative to the site root). `prefix` is the path
/// the site is served under (`/spawn-at/`), used for site-absolute links.
pub fn classify(href: &str, from: &Path, prefix: &str) -> Target {
    let href = href.trim();
    if href.is_empty() || href.starts_with("javascript:") || href.starts_with("data:") {
        return Target::Skip;
    }
    if href.starts_with("//") {
        return Target::External(href.to_string());
    }
    let scheme_end = href.find(['/', '?', '#', ':']);
    if scheme_end.is_some_and(|i| href.as_bytes()[i] == b':') {
        return Target::External(href.to_string());
    }
    let (path_part, fragment) = match href.split_once('#') {
        Some((p, f)) => (p, Some(percent_decode(f))),
        None => (href, None),
    };
    let path_part = path_part.split('?').next().unwrap_or("");
    let path_part = percent_decode(path_part);
    let resolved = if path_part.is_empty() {
        from.to_path_buf()
    } else if let Some(abs) = path_part.strip_prefix('/') {
        match abs.strip_prefix(prefix.trim_matches('/')) {
            Some(rest)
                if prefix.trim_matches('/').is_empty()
                    || rest.is_empty()
                    || rest.starts_with('/') =>
            {
                normalize(Path::new(rest.trim_start_matches('/')))
            }
            _ => return Target::OutsideSite(href.to_string()),
        }
    } else {
        normalize(&from.parent().unwrap_or(Path::new("")).join(&path_part))
    };
    Target::Local(resolved, fragment)
}

/// Ids and anchor names defined in a page.
pub fn collect_ids(html: &str) -> HashSet<String> {
    let mut ids = HashSet::new();
    for tag in scan_tags(html) {
        if let Some(id) = tag.attr("id") {
            ids.insert(id.to_string());
        }
        if tag.name == "a" {
            if let Some(name) = tag.attr("name") {
                ids.insert(name.to_string());
            }
        }
    }
    ids
}

fn files_under(dir: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            files_under(&path, out)?;
        } else {
            out.push(path);
        }
    }
    Ok(())
}

/// Options of a site check.
pub struct Options {
    /// Path the site is served under, e.g. `/spawn-at/`.
    pub prefix: String,
    /// Directories (relative to the root, with a trailing `/`) written by another tool, such as
    /// rustdoc's `api/`. The `<a>` links of their pages are not verified: rustdoc checks its own
    /// intra-doc links, and with `--no-deps` it leaves links into the documentation of
    /// dependencies (copied from their trait docs) that cannot resolve here. Their `src/` listings
    /// are not searched for host names, because they show our own source code. Scripts,
    /// stylesheets, images and the like are still checked.
    pub generated: Vec<String>,
}

/// Result of a site check.
pub struct Report {
    /// Problems, one line each.
    pub problems: Vec<String>,
    /// Number of HTML files read.
    pub html_files: usize,
    /// `<a href>` links to other sites (not fetched).
    pub external_links: usize,
}

fn local_exists(root: &Path, rel: &Path) -> Option<PathBuf> {
    let full = root.join(rel);
    if full.is_file() {
        Some(rel.to_path_buf())
    } else if full.is_dir() && full.join("index.html").is_file() {
        Some(rel.join("index.html"))
    } else {
        None
    }
}

/// Checks every file under `root`.
pub fn check_site(root: &Path, opts: &Options) -> Result<Report, String> {
    let mut files = Vec::new();
    files_under(root, &mut files).map_err(|e| format!("cannot read {}: {}", root.display(), e))?;
    files.sort();
    let mut problems = Vec::new();
    let mut ids_cache: HashMap<PathBuf, HashSet<String>> = HashMap::new();
    let (mut html_files, mut external_links) = (0, 0);

    for file in &files {
        let rel = file.strip_prefix(root).unwrap_or(file).to_path_buf();
        let rel_s = rel.to_string_lossy().replace('\\', "/");
        let ext = rel.extension().and_then(|e| e.to_str()).unwrap_or("");
        if !matches!(ext, "html" | "css" | "js" | "svg") {
            continue;
        }
        let Ok(text) = fs::read_to_string(file) else {
            continue;
        };
        let generated = opts.generated.iter().any(|p| rel_s.starts_with(p.as_str()));
        let skip_hosts = rel_s.contains("searchindex")
            || rel_s.contains("search-index")
            || rel_s.contains("sidebar-items")
            || (generated && rel_s.contains("/src/"));
        if !skip_hosts {
            for host in THIRD_PARTY {
                if text.contains(host) {
                    problems.push(format!("{rel_s}: names third-party host `{host}`"));
                }
            }
        }
        if ext == "css"
            && (text.contains("url(http")
                || text.contains("url(\"http")
                || text.contains("url('http")
                || text.contains("@import"))
        {
            problems.push(format!(
                "{rel_s}: stylesheet loads something from another host or imports"
            ));
        }
        if ext != "html" {
            continue;
        }
        html_files += 1;

        for tag in scan_tags(&text) {
            if tag
                .attr("class")
                .is_some_and(|c| c.split_whitespace().any(|t| t == "playground"))
            {
                problems.push(format!(
                    "{rel_s}: <{}> has the `playground` class (its Run button sends code to play.rust-lang.org)",
                    tag.name
                ));
            }
            let (attr, is_resource) = match tag.name.as_str() {
                "a" | "area" => ("href", false),
                "link" => ("href", true),
                "script" | "img" | "iframe" | "source" | "embed" | "audio" | "video" | "track" => {
                    ("src", true)
                }
                "object" => ("data", true),
                _ => continue,
            };
            if generated && !is_resource {
                continue;
            }
            let Some(href) = tag.attr(attr) else { continue };
            match classify(href, &rel, &opts.prefix) {
                Target::Skip => {}
                Target::External(url) => {
                    let canonical = tag.name == "link"
                        && tag.attr("rel").is_some_and(|r| r.contains("canonical"));
                    if is_resource && !canonical {
                        problems.push(format!(
                            "{rel_s}: <{}> loads {url} from another host",
                            tag.name
                        ));
                    } else {
                        external_links += 1;
                    }
                }
                Target::OutsideSite(url) => {
                    problems.push(format!(
                        "{rel_s}: <{}> {url} is a site-absolute path outside {}",
                        tag.name, opts.prefix
                    ));
                }
                Target::Local(target, fragment) => {
                    let Some(found) = local_exists(root, &target) else {
                        problems.push(format!(
                            "{rel_s}: <{}> {href} -> {} does not exist",
                            tag.name,
                            target.display()
                        ));
                        continue;
                    };
                    let Some(fragment) = fragment else { continue };
                    if fragment.is_empty() || found.extension().is_none_or(|e| e != "html") {
                        continue;
                    }
                    let ids = ids_cache.entry(found.clone()).or_insert_with(|| {
                        collect_ids(&fs::read_to_string(root.join(&found)).unwrap_or_default())
                    });
                    if !ids.contains(&fragment) {
                        problems.push(format!(
                            "{rel_s}: <{}> {href} -> no id `{fragment}` in {}",
                            tag.name,
                            found.display()
                        ));
                    }
                }
            }
        }
    }
    Ok(Report {
        problems,
        html_files,
        external_links,
    })
}

/// Entry point of `cargo xtask docs-check-html <site-dir> [--site-prefix P] [--generated-dir DIR]...`.
pub fn run(args: &[String]) -> Result<(), String> {
    let mut root = None;
    let mut opts = Options {
        prefix: "/spawn-at/".to_string(),
        generated: Vec::new(),
    };
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--site-prefix" => {
                opts.prefix = it.next().ok_or("--site-prefix needs a value")?.clone()
            }
            "--generated-dir" => opts
                .generated
                .push(it.next().ok_or("--generated-dir needs a value")?.clone()),
            other if root.is_none() && !other.starts_with("--") => {
                root = Some(PathBuf::from(other))
            }
            other => return Err(format!("unknown argument {other}")),
        }
    }
    let root = root.ok_or(
        "usage: cargo xtask docs-check-html <site-dir> [--site-prefix P] [--generated-dir DIR]",
    )?;
    let report = check_site(&root, &opts)?;
    println!(
        "==> {} HTML files read under {}; {} links to other sites (not fetched)",
        report.html_files,
        root.display(),
        report.external_links
    );
    if report.problems.is_empty() {
        println!("==> site OK: no broken internal link, no missing id, no third-party request");
        Ok(())
    } else {
        Err(report.problems.join("\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Attributes are read with quotes of both kinds and unquoted; comments and script bodies
    /// are not scanned for tags.
    #[test]
    fn test_scan_tags_attributes_comments_and_scripts() {
        let html = "<!-- <a href=\"gone\"> --><a href='x.html#y' class=c>t</a>\
                    <script>var s = '<a href=\"no\">';</script><img src=\"a&amp;b.png\">";
        let tags = scan_tags(html);
        let names: Vec<_> = tags.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(names, vec!["a", "script", "img"]);
        assert_eq!(tags[0].attr("href"), Some("x.html#y"));
        assert_eq!(tags[2].attr("src"), Some("a&b.png"));
    }

    /// A `>` inside a quoted value does not end the tag.
    #[test]
    fn test_scan_tags_gt_inside_quotes() {
        let tags = scan_tags("<a title=\"a>b\" href=\"c.html\">");
        assert_eq!(tags[0].attr("href"), Some("c.html"));
    }

    /// Relative links resolve against the page's directory, `..` is folded, a directory link
    /// is accepted later as `index.html`, and a same-page `#id` points at the page itself.
    #[test]
    fn test_classify_relative_and_fragment() {
        let from = Path::new("environments/sway.html");
        assert_eq!(
            classify("../install.html#x", from, "/spawn-at/"),
            Target::Local(PathBuf::from("install.html"), Some("x".into()))
        );
        assert_eq!(
            classify("#top", from, "/spawn-at/"),
            Target::Local(from.to_path_buf(), Some("top".into()))
        );
        assert_eq!(
            classify("a%20b.html", from, "/spawn-at/"),
            Target::Local(PathBuf::from("environments/a b.html"), None)
        );
    }

    /// Other hosts, protocol-relative URLs and `mailto:` are external; a path starting with the
    /// site prefix maps to the root; any other absolute path is outside the site.
    #[test]
    fn test_classify_external_and_absolute() {
        let from = Path::new("a/b.html");
        assert!(matches!(
            classify("https://x.org/", from, "/spawn-at/"),
            Target::External(_)
        ));
        assert!(matches!(
            classify("//cdn.example/x.js", from, "/spawn-at/"),
            Target::External(_)
        ));
        assert!(matches!(
            classify("mailto:a@b.c", from, "/spawn-at/"),
            Target::External(_)
        ));
        assert_eq!(
            classify("/spawn-at/api/x.html", from, "/spawn-at/"),
            Target::Local(PathBuf::from("api/x.html"), None)
        );
        assert!(matches!(
            classify("/other/x.html", from, "/spawn-at/"),
            Target::OutsideSite(_)
        ));
        assert_eq!(
            classify("javascript:void(0)", from, "/spawn-at/"),
            Target::Skip
        );
    }

    fn site(files: &[(&str, &str)], tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("xtask-docs-html-{}-{}", std::process::id(), tag));
        let _ = fs::remove_dir_all(&dir);
        for (rel, text) in files {
            let path = dir.join(rel);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, text).unwrap();
        }
        dir
    }

    fn opts() -> Options {
        Options {
            prefix: "/spawn-at/".into(),
            generated: vec![],
        }
    }

    /// A good site passes; a broken page link, a missing id and a missing stylesheet are each
    /// reported with the page that holds them.
    #[test]
    fn test_check_site_good_and_broken() {
        let good = site(&[
            ("index.html", "<link rel=stylesheet href=\"c.css\"><a href=\"p.html#s\">p</a><a href=\"https://github.com/x\">g</a>"),
            ("p.html", "<h2 id=\"s\">S</h2>"),
            ("c.css", "body{}"),
        ], "good");
        let r = check_site(&good, &opts()).unwrap();
        assert!(r.problems.is_empty(), "{:?}", r.problems);
        assert_eq!((r.html_files, r.external_links), (2, 1));
        let bad = site(&[
            ("index.html", "<link rel=stylesheet href=\"missing.css\"><a href=\"p.html#nope\">p</a><a href=\"gone.html\">g</a>"),
            ("p.html", "<h2 id=\"s\">S</h2>"),
        ], "bad");
        let problems = check_site(&bad, &opts()).unwrap().problems.join("\n");
        assert!(
            problems.contains("missing.css")
                && problems.contains("no id `nope`")
                && problems.contains("gone.html"),
            "{problems}"
        );
    }

    /// Third-party requests fail: a script from a CDN, a font host in a stylesheet, an image
    /// from another host. An `<a>` to another site does not.
    #[test]
    fn test_check_site_flags_third_party_requests() {
        let dir = site(&[
            ("index.html", "<script src=\"https://cdn.example/x.js\"></script><img src=\"http://img.example/a.png\"><a href=\"https://ok.example\">ok</a>"),
            ("c.css", "@import url(https://fonts.googleapis.com/css);"),
        ], "third");
        let problems = check_site(&dir, &opts()).unwrap().problems.join("\n");
        assert!(
            problems.contains("<script> loads https://cdn.example/x.js"),
            "{problems}"
        );
        assert!(
            problems.contains("<img> loads http://img.example/a.png"),
            "{problems}"
        );
        assert!(problems.contains("googleapis"), "{problems}");
        assert!(!problems.contains("ok.example"), "{problems}");
    }

    /// An element with the `playground` class fails the check; a class that merely contains the
    /// word (`playground-note`) does not.
    #[test]
    fn test_playground_class_is_flagged() {
        let dir = site(
            &[(
                "index.html",
                "<pre class=\"playground\"><code>x</code></pre><p class=\"playground-note\">ok</p>",
            )],
            "playground",
        );
        let problems = check_site(&dir, &opts()).unwrap().problems;
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert!(problems[0].contains("`playground` class"), "{problems:?}");
    }

    /// Under a generated directory a broken `<a>` link is not reported (rustdoc leaves some into
    /// dependency docs), a missing stylesheet and a script from another host still are, and a
    /// `src/` listing that mentions a CDN host name is not searched.
    #[test]
    fn test_generated_dir_skips_anchors_and_source_listings_only() {
        let dir = site(
            &[
                (
                    "api/a.html",
                    "<a href=\"zzz.html\">z</a><a href=\"b.html#42\">b</a>\
                     <link rel=stylesheet href=\"missing.css\">\
                     <script src=\"https://cdn.example/x.js\"></script>",
                ),
                ("api/b.html", "x"),
                (
                    "api/src/k/lib.rs.html",
                    "<pre>const H: &str = \"unpkg.com\";</pre>",
                ),
                ("book.html", "<a href=\"zzz.html\">z</a>"),
            ],
            "generated",
        );
        let o = Options {
            prefix: "/spawn-at/".into(),
            generated: vec!["api/".into()],
        };
        let problems = check_site(&dir, &o).unwrap().problems.join("\n");
        assert!(!problems.contains("api/a.html: <a>"), "{problems}");
        assert!(
            problems.contains("api/a.html: <link>") && problems.contains("missing.css"),
            "{problems}"
        );
        assert!(
            problems.contains("api/a.html: <script> loads https://cdn.example/x.js"),
            "{problems}"
        );
        assert!(!problems.contains("lib.rs.html"), "{problems}");
        assert!(problems.contains("book.html: <a> zzz.html"), "{problems}");
    }
}
