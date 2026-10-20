//! `docs-cli-reference`: writes one Starlight page per `spawn-at` command from the built binary's
//! own `--help` output, and fails if a command that `--help` lists has no page.
//!
//! The pages are generated at build time into `docs-site/src/content/docs/reference/` (git
//! ignores everything there except the hand-written `index.md`), so they cannot drift from the
//! binary. The walk starts at the root command and follows every `Commands:` list it prints, so
//! nested commands (`query layout`, `config set`) get a page too. The built-in `help` entry that
//! clap adds to every list is skipped: it is not a command of its own. Each page starts with a
//! front-matter block (`title`, `description`, `sidebar.label`) and has no `# ` heading, because
//! Starlight prints the title as the page heading. The `description` is the first line of that
//! command's `--help`; the sidebar label is the command without `spawn-at `; the first line of the
//! body points to the hand-written page of the command (`COMMAND_PAGES`, checked against the files
//! that exist) or, for a command without one, to the Commands index. The text from `--help` itself
//! is never changed. The sidebar lists the pages through an `autogenerate` entry for the folder
//! `reference` in `docs-site/astro.config.mjs`, which this module checks for.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// One command of the CLI with its verbatim `--help` text.
pub struct CommandHelp {
    /// Words after the program name: empty for the root, `["query", "layout"]` for a nested one.
    pub path: Vec<String>,
    /// stdout of `<bin> <path...> --help`, unchanged.
    pub help: String,
}

/// Names listed under a `Commands:` heading of clap help text, in the order printed. The `help`
/// entry is left out. A name is the first word of an indented line; the list ends at the first
/// line that is blank or not indented.
pub fn parse_subcommands(help: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut in_list = false;
    for line in help.lines() {
        if !in_list {
            in_list = line.trim_end() == "Commands:";
            continue;
        }
        if line.trim().is_empty() || !line.starts_with(' ') {
            break;
        }
        if let Some(name) = line.split_whitespace().next() {
            if name != "help" {
                names.push(name.to_string());
            }
        }
    }
    names
}

/// File name of the page of a command, relative to the reference directory.
pub fn page_file(path: &[String]) -> String {
    let mut parts = vec!["spawn-at".to_string()];
    parts.extend(path.iter().cloned());
    format!("{}.md", parts.join("-"))
}

/// The command as a user types it.
pub fn command_line(path: &[String]) -> String {
    let mut parts = vec!["spawn-at".to_string()];
    parts.extend(path.iter().cloned());
    parts.join(" ")
}

/// Runs `<bin> <args...>` with colour off and returns stdout, or the reason it did not work.
fn run_capture(bin: &Path, args: &[&str]) -> Result<String, String> {
    let out = Command::new(bin)
        .args(args)
        .env("NO_COLOR", "1")
        .output()
        .map_err(|e| format!("cannot run {}: {}", bin.display(), e))?;
    if !out.status.success() {
        return Err(format!(
            "`{} {}` exited with {:?}: {}",
            bin.display(),
            args.join(" "),
            out.status.code(),
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    String::from_utf8(out.stdout).map_err(|e| format!("non-UTF-8 output: {}", e))
}

/// Help text of the root command and of every command below it.
pub fn discover(bin: &Path) -> Result<Vec<CommandHelp>, String> {
    if !bin.is_file() {
        return Err(format!(
            "binary not found: {} (build it first: cargo build --locked -p spawn-at)",
            bin.display()
        ));
    }
    let mut found = Vec::new();
    let mut queue: Vec<Vec<String>> = vec![Vec::new()];
    while let Some(path) = queue.pop() {
        let mut args: Vec<&str> = path.iter().map(String::as_str).collect();
        args.push("--help");
        let help = run_capture(bin, &args)?;
        for name in parse_subcommands(&help).into_iter().rev() {
            let mut child = path.clone();
            child.push(name);
            queue.push(child);
        }
        found.push(CommandHelp { path, help });
    }
    found.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(found)
}

/// A code fence longer than any run of backticks inside `text`.
fn fence_for(text: &str) -> String {
    let mut longest = 0;
    let mut run = 0;
    for ch in text.chars() {
        if ch == '`' {
            run += 1;
            longest = longest.max(run);
        } else {
            run = 0;
        }
    }
    "`".repeat(longest.max(2) + 1)
}

/// `text` as a double-quoted YAML scalar: backslash and double quote are escaped.
pub fn yaml_quote(text: &str) -> String {
    format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\""))
}

/// The hand-written page of each command that has one, as its path under the content folder
/// (without `.md`): a file of `commands/`, or for `config` the Configuration page. A command that
/// is not listed, or whose file is missing, has none and the reference page points to the
/// Commands index instead. Nested commands (`query layout`, `config set`) use the page of their
/// first word.
const COMMAND_PAGES: [(&str, &str); 13] = [
    ("spawn", "commands/spawn"),
    ("transform", "commands/transform"),
    ("focus", "commands/window-state"),
    ("defocus", "commands/window-state"),
    ("maximize", "commands/window-state"),
    ("minimize", "commands/window-state"),
    ("restore", "commands/window-state"),
    ("close", "commands/window-state"),
    ("query", "commands/query"),
    ("install", "commands/manage"),
    ("uninstall", "commands/manage"),
    ("update", "commands/manage"),
    ("config", "configuration"),
];

/// Where the hand-written text of a command is: a site path such as `/spawn-at/commands/spawn/`
/// and the title of that page.
#[derive(Debug, Clone, PartialEq)]
pub struct HandWritten {
    pub url: String,
    pub title: String,
}

/// The hand-written page of the command at `path`, if `COMMAND_PAGES` lists one and its file
/// exists in `content_dir` (the `docs-site/src/content/docs` folder) with a title. The title
/// is read from the page, so the link text cannot drift from it.
pub fn hand_written_page(path: &[String], content_dir: &Path) -> Option<HandWritten> {
    let first = path.first()?;
    let (_, page) = COMMAND_PAGES.iter().find(|(name, _)| name == first)?;
    let text = fs::read_to_string(content_dir.join(format!("{page}.md"))).ok()?;
    let (front, _) = crate::docs_check::split_front_matter(&text)?;
    let title = crate::docs_check::front_matter_title(front);
    if title.is_empty() {
        return None;
    }
    Some(HandWritten {
        url: format!("/spawn-at/{page}/"),
        title,
    })
}

/// The one line under the title that sends a reader to the hand-written text.
pub fn pointer_line(page: Option<&HandWritten>) -> String {
    match page {
        Some(p) => format!(
            "Hand-written page for this command: [{}]({}).",
            p.title, p.url
        ),
        None => "The hand-written pages for each command are on [Commands](/spawn-at/commands/)."
            .to_string(),
    }
}

/// The first non-empty line of a `--help` text: the short description of the command.
pub fn first_help_line(help: &str) -> Option<&str> {
    help.lines().map(str::trim).find(|l| !l.is_empty())
}

/// The name of the page in the sidebar: the command without the leading `spawn-at ` (`spawn`,
/// `query windows`); the root command keeps `spawn-at`. The title stays the full command line.
pub fn sidebar_label(path: &[String]) -> String {
    if path.is_empty() {
        "spawn-at".to_string()
    } else {
        path.join(" ")
    }
}

/// The markdown of one page: front matter, the line that points to the hand-written page, the
/// command that produced the text, and the text. `description` is the first line of the help
/// text, `sidebar.label` the command without `spawn-at `. There is no `# ` heading; Starlight
/// prints `title` as the heading.
pub fn render_page(
    path: &[String],
    help: &str,
    version: Option<&str>,
    hand_written: Option<&HandWritten>,
) -> String {
    let line = command_line(path);
    let description = first_help_line(help)
        .map(str::to_string)
        .unwrap_or_else(|| format!("Output of {line} --help, unchanged."));
    let mut page = format!(
        "---\ntitle: {}\ndescription: {}\nsidebar:\n  label: {}\n---\n\n\
         {}\n\n\
         This page is generated from the built binary at build time. \
         The text below is the output of `{line} --help`, unchanged.\n\n",
        yaml_quote(&line),
        yaml_quote(&description),
        yaml_quote(&sidebar_label(path)),
        pointer_line(hand_written)
    );
    let body = help.trim_end();
    let fence = fence_for(body);
    page.push_str(&format!("{fence}text\n$ {line} --help\n{body}\n{fence}\n"));
    if let Some(version) = version {
        let version = version.trim_end();
        let fence = fence_for(version);
        page.push_str(&format!(
            "\nThe version line of the binary these pages were generated from \
             (`spawn-at -V`). A build that is not on a release tag prints `DEV BUILD` and its \
             commit.\n\n{fence}text\n$ spawn-at -V\n{version}\n{fence}\n"
        ));
    }
    page
}

/// True if `config` (the text of `astro.config.mjs`) has a sidebar entry
/// `autogenerate: { directory: '<dir>' }` for the folder `dir`, with either quote style. Text
/// search, not a JavaScript parse: it looks at the 80 bytes after each `autogenerate`.
pub fn sidebar_autogenerates(config: &str, dir: &str) -> bool {
    let wanted = [format!("'{dir}'"), format!("\"{dir}\"")];
    config.match_indices("autogenerate").any(|(i, _)| {
        let mut end = (i + 80).min(config.len());
        while !config.is_char_boundary(end) {
            end -= 1;
        }
        let window = &config[i..end];
        let window = window.split('}').next().unwrap_or(window);
        window.contains("directory") && wanted.iter().any(|w| window.contains(w.as_str()))
    })
}

/// Problems with the pages of `commands`: a page file that is missing, and a sidebar that does
/// not list the folder `reference` by `autogenerate` (so the generated pages would be built but
/// not reachable from the menu). An empty list means the reference is complete.
pub fn check_coverage(
    commands: &[Vec<String>],
    reference_dir: &Path,
    sidebar_config: &str,
) -> Vec<String> {
    let mut problems = Vec::new();
    for path in commands {
        let file = page_file(path);
        if !reference_dir.join(&file).is_file() {
            problems.push(format!(
                "`{}` is listed by --help but has no page {}",
                command_line(path),
                reference_dir.join(&file).display()
            ));
        }
    }
    if !sidebar_autogenerates(sidebar_config, "reference") {
        problems.push(
            "the sidebar in astro.config.mjs has no `autogenerate: { directory: 'reference' }` \
             entry, so the generated reference pages would not be in the menu"
                .to_string(),
        );
    }
    problems
}

/// Removes `spawn-at*.md` files of `dir` that are not in `keep`: pages of a command that no
/// longer exists. `index.md` and any other file are left alone.
fn remove_stale_pages(dir: &Path, keep: &[PathBuf]) -> Result<(), String> {
    let entries = fs::read_dir(dir).map_err(|e| format!("cannot read {}: {}", dir.display(), e))?;
    for entry in entries {
        let path = entry
            .map_err(|e| format!("cannot read {}: {}", dir.display(), e))?
            .path();
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if name.starts_with("spawn-at") && name.ends_with(".md") && !keep.contains(&path) {
            fs::remove_file(&path)
                .map_err(|e| format!("cannot remove {}: {}", path.display(), e))?;
        }
    }
    Ok(())
}

/// Generates the pages and then runs the coverage check. Returns the paths written.
pub fn generate(
    bin: &Path,
    reference_dir: &Path,
    sidebar_config: &str,
) -> Result<Vec<PathBuf>, String> {
    let commands = discover(bin)?;
    let version = run_capture(bin, &["-V"])?;
    fs::create_dir_all(reference_dir)
        .map_err(|e| format!("cannot create {}: {}", reference_dir.display(), e))?;
    // `docs-site/src/content/docs`, where the hand-written `commands/` pages and `configuration.md` are
    let content_dir = reference_dir.parent().unwrap_or(reference_dir);
    let mut written = Vec::new();
    for cmd in &commands {
        let version = if cmd.path.is_empty() {
            Some(version.as_str())
        } else {
            None
        };
        let hand_written = hand_written_page(&cmd.path, content_dir);
        let file = reference_dir.join(page_file(&cmd.path));
        fs::write(
            &file,
            render_page(&cmd.path, &cmd.help, version, hand_written.as_ref()),
        )
        .map_err(|e| format!("cannot write {}: {}", file.display(), e))?;
        written.push(file);
    }
    remove_stale_pages(reference_dir, &written)?;
    let paths: Vec<Vec<String>> = commands.iter().map(|c| c.path.clone()).collect();
    let problems = check_coverage(&paths, reference_dir, sidebar_config);
    if problems.is_empty() {
        Ok(written)
    } else {
        Err(problems.join("\n"))
    }
}

/// Entry point of `cargo xtask docs-cli-reference --bin <path> [--out <dir>] [--config <file>]`.
pub fn run(args: &[String]) -> Result<(), String> {
    let mut bin = PathBuf::from("target/debug/spawn-at");
    let mut out = PathBuf::from("docs-site/src/content/docs/reference");
    let mut config = PathBuf::from("docs-site/astro.config.mjs");
    let mut it = args.iter();
    while let Some(flag) = it.next() {
        let value = it.next().ok_or_else(|| format!("{} needs a value", flag))?;
        match flag.as_str() {
            "--bin" => bin = PathBuf::from(value),
            "--out" => out = PathBuf::from(value),
            "--config" => config = PathBuf::from(value),
            other => return Err(format!("unknown option {}", other)),
        }
    }
    let config_text = fs::read_to_string(&config)
        .map_err(|e| format!("cannot read {}: {}", config.display(), e))?;
    let written = generate(&bin, &out, &config_text)?;
    println!(
        "==> wrote {} reference pages to {}",
        written.len(),
        out.display()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const ROOT_HELP: &str = "\
A tool.

Usage: spawn-at [OPTIONS] <COMMAND>

Commands:
  install    Install the extension
  query      Query state
  help       Print this message or the help of the given subcommand(s)

Options:
  -h, --help
";

    /// `help` is skipped, the two real commands stay in printed order, and the `Options:` list
    /// after the blank line is not read as commands.
    #[test]
    fn test_parse_subcommands_skips_help_and_stops_at_blank_line() {
        assert_eq!(parse_subcommands(ROOT_HELP), vec!["install", "query"]);
    }

    /// Help text without a `Commands:` heading (a leaf command) has no subcommands.
    #[test]
    fn test_parse_subcommands_leaf_has_none() {
        assert!(
            parse_subcommands("Usage: spawn-at focus [OPTIONS]\n\nOptions:\n  -h\n").is_empty()
        );
    }

    /// Page names keep the command path, so `query layout` and `query` cannot collide.
    #[test]
    fn test_page_file_and_command_line() {
        let path = vec!["query".to_string(), "layout".to_string()];
        assert_eq!(page_file(&path), "spawn-at-query-layout.md");
        assert_eq!(page_file(&[]), "spawn-at.md");
        assert_eq!(command_line(&path), "spawn-at query layout");
    }

    /// A help text that itself contains a 3-backtick run gets a 4-backtick fence, so the
    /// generated block cannot be closed early.
    #[test]
    fn test_render_page_fence_outgrows_backticks_in_the_text() {
        let page = render_page(&["x".to_string()], "see ```code``` here\n", None, None);
        assert!(page.contains("````text\n$ spawn-at x --help\nsee ```code``` here\n````\n"));
    }

    /// The version block appears only when a version is given (the root page).
    #[test]
    fn test_render_page_version_block_only_for_root() {
        assert!(
            render_page(&[], "h\n", Some("DEV BUILD\n"), None).contains("$ spawn-at -V\nDEV BUILD")
        );
        assert!(!render_page(&["x".to_string()], "h\n", None, None).contains("-V"));
    }

    /// A missing binary is an error that names the file and how to build it, not an empty list.
    #[test]
    fn test_discover_missing_binary_fails_loudly() {
        let err = discover(Path::new("/nonexistent/spawn-at")).err().unwrap();
        assert!(
            err.contains("binary not found") && err.contains("cargo build"),
            "{}",
            err
        );
    }

    fn temp_dir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("xtask-docs-cli-{}-{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Held by every test that writes and then runs the stand-in script. A process forked by one
    /// test while another test still has the script open for writing makes that test's `exec` fail
    /// with "Text file busy"; the lock keeps the write and the runs of one test apart from the
    /// forks of the others.
    #[cfg(unix)]
    static SCRIPT_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[cfg(unix)]
    fn fake_cli(dir: &Path) -> (std::sync::MutexGuard<'static, ()>, PathBuf) {
        use std::os::unix::fs::PermissionsExt;
        let guard = SCRIPT_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let script = dir.join("fake-spawn-at");
        let body = "#!/bin/sh\n\
            case \"$*\" in\n\
            '-V') echo 'spawn-at 9.9.9' ;;\n\
            '--help') printf 'Root\\n\\nCommands:\\n  alpha  A\\n  beta   B\\n  help   h\\n\\nOptions:\\n  -h\\n' ;;\n\
            'beta --help') printf 'Beta\\n\\nCommands:\\n  inner  I\\n\\nOptions:\\n  -h\\n' ;;\n\
            *) printf 'Leaf for %s\\n' \"$*\" ;;\n\
            esac\n";
        fs::write(&script, body).unwrap();
        fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
        (guard, script)
    }

    /// The sidebar entry the tests use: the same shape as in `docs-site/astro.config.mjs`.
    const SIDEBAR: &str = "items: [{ autogenerate: { directory: 'reference' } }],";

    /// Against a stand-in CLI with a nested command: all four pages are written, the nested one
    /// is found by following `beta`'s own list, and only the root page carries the version.
    #[cfg(unix)]
    #[test]
    fn test_generate_walks_nested_commands() {
        let dir = temp_dir("generate");
        let (_guard, bin) = fake_cli(&dir);
        let out = dir.join("reference");
        let written = generate(&bin, &out, SIDEBAR).unwrap();
        assert_eq!(written.len(), 4);
        let root = fs::read_to_string(out.join("spawn-at.md")).unwrap();
        assert!(root.contains("spawn-at 9.9.9"));
        let inner = fs::read_to_string(out.join("spawn-at-beta-inner.md")).unwrap();
        assert!(inner.contains("Leaf for beta inner --help") && !inner.contains("9.9.9"));
        // each page's description is the first line of its own help, not one shared sentence
        assert!(root.contains("description: \"Root\"\n"), "{root}");
        assert!(
            inner.contains("description: \"Leaf for beta inner --help\"\n"),
            "{inner}"
        );
        // the stand-in has no `commands/` folder next to `reference/`, so every page points to the index
        assert!(inner.contains("[Commands](/spawn-at/commands/)"), "{inner}");
        let _ = fs::remove_dir_all(&dir);
    }

    /// If the sidebar config has no `autogenerate` entry for `reference`, the build fails and the
    /// message names the config, because the generated pages would not be in the menu.
    #[cfg(unix)]
    #[test]
    fn test_generate_fails_when_sidebar_does_not_autogenerate_reference() {
        let dir = temp_dir("sidebar");
        let (_guard, bin) = fake_cli(&dir);
        let err = generate(&bin, &dir.join("reference"), "items: ['install']")
            .err()
            .unwrap();
        assert!(
            err.contains("astro.config.mjs") && err.contains("autogenerate"),
            "{}",
            err
        );
        let _ = fs::remove_dir_all(&dir);
    }

    /// A missing page file is reported even when the sidebar autogenerates the folder.
    #[test]
    fn test_check_coverage_reports_missing_page_file() {
        let dir = temp_dir("coverage");
        let problems = check_coverage(&[vec!["gone".to_string()]], &dir, SIDEBAR);
        assert_eq!(problems.len(), 1);
        assert!(problems[0].contains("has no page"), "{:?}", problems);
        let _ = fs::remove_dir_all(&dir);
    }

    /// The sidebar check accepts both quote styles and either spacing, and rejects another
    /// folder, a `directory` that is not next to an `autogenerate`, and an empty config.
    #[test]
    fn test_sidebar_autogenerates() {
        assert!(sidebar_autogenerates(
            "{ autogenerate: { directory: \"reference\" } }",
            "reference"
        ));
        assert!(sidebar_autogenerates(SIDEBAR, "reference"));
        assert!(!sidebar_autogenerates(
            "{ autogenerate: { directory: 'guides' } }",
            "reference"
        ));
        assert!(!sidebar_autogenerates(
            "autogenerate: { directory: 'guides' }, { slug: 'x', directory: 'reference' }",
            "reference"
        ));
        assert!(!sidebar_autogenerates("", "reference"));
    }

    /// A page starts with a `---` block holding `title`, `description` (the first line of the
    /// help text) and `sidebar.label`, then the line that points to the hand-written text, then
    /// the generated paragraph; there is no `# ` heading line (Starlight would show the title
    /// twice).
    #[test]
    fn test_render_page_has_front_matter_and_no_h1() {
        let page = render_page(
            &["query".to_string(), "layout".to_string()],
            "Print the layout\n\nUsage: x\n",
            None,
            None,
        );
        assert!(
            page.starts_with(
                "---\ntitle: \"spawn-at query layout\"\n\
                 description: \"Print the layout\"\n\
                 sidebar:\n  label: \"query layout\"\n---\n\n\
                 The hand-written pages for each command are on [Commands](/spawn-at/commands/).\n\n\
                 This page is generated from the built binary"
            ),
            "{page}"
        );
        assert!(!page.lines().any(|l| l.starts_with("# ")), "{page}");
    }

    /// The description is the first non-empty line of that command's help, so two pages do not
    /// share one string; it is quoted for YAML; an empty help falls back to the old sentence.
    #[test]
    fn test_description_is_the_first_help_line() {
        assert_eq!(
            first_help_line("\n  Move a window \nmore\n"),
            Some("Move a window")
        );
        assert_eq!(first_help_line("\n\n"), None);
        let a = render_page(&["a".to_string()], "Do \"a\"\n\nUsage\n", None, None);
        let b = render_page(&["b".to_string()], "Do b\n\nUsage\n", None, None);
        assert!(a.contains("description: \"Do \\\"a\\\"\"\n"), "{a}");
        assert!(b.contains("description: \"Do b\"\n"), "{b}");
        let empty = render_page(&["c".to_string()], "", None, None);
        assert!(
            empty.contains("description: \"Output of spawn-at c --help, unchanged.\""),
            "{empty}"
        );
    }

    /// The sidebar label drops the leading `spawn-at `; the root keeps its name, because the
    /// rest would be empty; the title is untouched.
    #[test]
    fn test_sidebar_label_drops_the_program_name() {
        assert_eq!(sidebar_label(&[]), "spawn-at");
        assert_eq!(sidebar_label(&["spawn".to_string()]), "spawn");
        assert_eq!(
            sidebar_label(&["query".to_string(), "windows".to_string()]),
            "query windows"
        );
        let page = render_page(
            &["config".to_string(), "set".to_string()],
            "h\n",
            None,
            None,
        );
        assert!(page.contains("title: \"spawn-at config set\""), "{page}");
        assert!(
            page.contains("sidebar:\n  label: \"config set\"\n"),
            "{page}"
        );
    }

    /// The pointer line names a page that exists: the map lists the command, the file is in
    /// `commands/` (for `config`, `configuration.md` at the top of the content folder) and has a
    /// title. A listed command without its file, a command that is not listed, a nested command
    /// (it uses the page of its first word) and a page without a title are each handled, and the
    /// help text under the pointer is unchanged.
    #[test]
    fn test_hand_written_page_is_built_from_the_files_that_exist() {
        let content = temp_dir("handwritten");
        let commands = content.join("commands");
        fs::create_dir_all(&commands).unwrap();
        fs::write(
            commands.join("spawn.md"),
            "---\ntitle: \"spawn\"\n---\n\nText\n",
        )
        .unwrap();
        fs::write(
            commands.join("window-state.md"),
            "---\ntitle: \"Focus and window state\"\n---\n\nText\n",
        )
        .unwrap();
        fs::write(commands.join("query.md"), "no front matter\n").unwrap();
        fs::write(
            content.join("configuration.md"),
            "---\ntitle: \"Configuration\"\n---\n\nText\n",
        )
        .unwrap();
        let s = |w: &[&str]| w.iter().map(|x| x.to_string()).collect::<Vec<_>>();

        assert_eq!(
            hand_written_page(&s(&["spawn"]), &content),
            Some(HandWritten {
                url: "/spawn-at/commands/spawn/".to_string(),
                title: "spawn".to_string()
            })
        );
        let focus = hand_written_page(&s(&["focus"]), &content).unwrap();
        assert_eq!(focus.url, "/spawn-at/commands/window-state/");
        assert_eq!(focus.title, "Focus and window state");
        // a listed command whose file is missing, and one whose file has no title
        assert_eq!(hand_written_page(&s(&["transform"]), &content), None);
        assert_eq!(hand_written_page(&s(&["query", "layout"]), &content), None);
        // `config` and each of its subcommands point to the Configuration page, which is not
        // under `commands/`
        let config = HandWritten {
            url: "/spawn-at/configuration/".to_string(),
            title: "Configuration".to_string(),
        };
        for words in [&["config"][..], &["config", "set"], &["config", "show"]] {
            assert_eq!(hand_written_page(&s(words), &content), Some(config.clone()));
        }
        // the Configuration page is missing: the Commands index is used
        assert_eq!(
            hand_written_page(&s(&["config", "set"]), &content.join("nowhere")),
            None
        );
        // not in the map; the root has no first word
        assert_eq!(hand_written_page(&s(&["help"]), &content), None);
        assert_eq!(hand_written_page(&[], &content), None);

        assert_eq!(
            pointer_line(Some(&config)),
            "Hand-written page for this command: [Configuration](/spawn-at/configuration/)."
        );
        assert_eq!(
            pointer_line(Some(&focus)),
            "Hand-written page for this command: [Focus and window state](/spawn-at/commands/window-state/)."
        );
        assert_eq!(
            pointer_line(None),
            "The hand-written pages for each command are on [Commands](/spawn-at/commands/)."
        );
        let page = render_page(&s(&["focus"]), "Focus it\n\nUsage: y\n", None, Some(&focus));
        assert!(
            page.contains("---\n\nHand-written page for this command: [Focus and window state]"),
            "{page}"
        );
        assert!(
            page.ends_with("```text\n$ spawn-at focus --help\nFocus it\n\nUsage: y\n```\n"),
            "{page}"
        );
        let _ = fs::remove_dir_all(&content);
    }

    /// A double quote or backslash in a title cannot end the YAML string early.
    #[test]
    fn test_yaml_quote_escapes() {
        assert_eq!(yaml_quote("a\"b\\c"), "\"a\\\"b\\\\c\"");
    }

    /// Pages of commands that no longer exist are removed, while `index.md` and a file that does
    /// not start with `spawn-at` stay. This keeps the output the same as a run on an empty folder.
    #[cfg(unix)]
    #[test]
    fn test_generate_removes_stale_command_pages_only() {
        let dir = temp_dir("stale");
        let (_guard, bin) = fake_cli(&dir);
        let out = dir.join("reference");
        fs::create_dir_all(&out).unwrap();
        fs::write(out.join("spawn-at-gone.md"), "old").unwrap();
        fs::write(out.join("index.md"), "hand-written").unwrap();
        fs::write(out.join("notes.md"), "keep").unwrap();
        generate(&bin, &out, SIDEBAR).unwrap();
        assert!(!out.join("spawn-at-gone.md").exists());
        assert!(out.join("index.md").is_file() && out.join("notes.md").is_file());
        assert!(out.join("spawn-at-beta.md").is_file());
        let _ = fs::remove_dir_all(&dir);
    }

    /// Two runs on the same binary write byte-identical files.
    #[cfg(unix)]
    #[test]
    fn test_generate_is_deterministic() {
        let dir = temp_dir("determinism");
        let (_guard, bin) = fake_cli(&dir);
        let (a, b) = (dir.join("a"), dir.join("b"));
        let first = generate(&bin, &a, SIDEBAR).unwrap();
        generate(&bin, &b, SIDEBAR).unwrap();
        for path in first {
            let name = path.file_name().unwrap();
            assert_eq!(
                fs::read(&path).unwrap(),
                fs::read(b.join(name)).unwrap(),
                "{name:?}"
            );
        }
        let _ = fs::remove_dir_all(&dir);
    }
}
