//! `docs-cli-reference`: writes one book page per `spawn-at` command from the built binary's own
//! `--help` output, and fails if a command that `--help` lists has no page.
//!
//! The pages are generated at build time and are not committed, so they cannot drift from the
//! binary. The walk starts at the root command and follows every `Commands:` list it prints, so
//! nested commands (`query layout`, `config set`) get a page too. The built-in `help` entry that
//! clap adds to every list is skipped: it is not a command of its own.

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

/// The markdown of one page: a heading, the command that produced the text, and the text.
pub fn render_page(path: &[String], help: &str, version: Option<&str>) -> String {
    let line = command_line(path);
    let mut page = format!(
        "# `{line}`\n\nThis page is generated from the built binary at build time. \
         The text below is the output of `{line} --help`, unchanged.\n\n"
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

/// Problems with the pages of `commands`: a page file that is missing, and a page that
/// `SUMMARY.md` does not link. An empty list means the reference is complete.
pub fn check_coverage(
    commands: &[Vec<String>],
    reference_dir: &Path,
    summary: &str,
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
        if !summary.contains(&format!("(reference/{file})")) {
            problems.push(format!(
                "`{}` is listed by --help but SUMMARY.md does not link reference/{}",
                command_line(path),
                file
            ));
        }
    }
    problems
}

/// Generates the pages and then runs the coverage check. Returns the paths written.
pub fn generate(bin: &Path, reference_dir: &Path, summary: &str) -> Result<Vec<PathBuf>, String> {
    let commands = discover(bin)?;
    let version = run_capture(bin, &["-V"])?;
    fs::create_dir_all(reference_dir)
        .map_err(|e| format!("cannot create {}: {}", reference_dir.display(), e))?;
    let mut written = Vec::new();
    for cmd in &commands {
        let version = if cmd.path.is_empty() {
            Some(version.as_str())
        } else {
            None
        };
        let file = reference_dir.join(page_file(&cmd.path));
        fs::write(&file, render_page(&cmd.path, &cmd.help, version))
            .map_err(|e| format!("cannot write {}: {}", file.display(), e))?;
        written.push(file);
    }
    let paths: Vec<Vec<String>> = commands.iter().map(|c| c.path.clone()).collect();
    let problems = check_coverage(&paths, reference_dir, summary);
    if problems.is_empty() {
        Ok(written)
    } else {
        Err(problems.join("\n"))
    }
}

/// Entry point of `cargo xtask docs-cli-reference --bin <path> [--out <dir>] [--summary <file>]`.
pub fn run(args: &[String]) -> Result<(), String> {
    let mut bin = PathBuf::from("target/debug/spawn-at");
    let mut out = PathBuf::from("docs-site/src/reference");
    let mut summary = PathBuf::from("docs-site/src/SUMMARY.md");
    let mut it = args.iter();
    while let Some(flag) = it.next() {
        let value = it.next().ok_or_else(|| format!("{} needs a value", flag))?;
        match flag.as_str() {
            "--bin" => bin = PathBuf::from(value),
            "--out" => out = PathBuf::from(value),
            "--summary" => summary = PathBuf::from(value),
            other => return Err(format!("unknown option {}", other)),
        }
    }
    let summary_text = fs::read_to_string(&summary)
        .map_err(|e| format!("cannot read {}: {}", summary.display(), e))?;
    let written = generate(&bin, &out, &summary_text)?;
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
        let page = render_page(&["x".to_string()], "see ```code``` here\n", None);
        assert!(page.contains("````text\n$ spawn-at x --help\nsee ```code``` here\n````\n"));
    }

    /// The version block appears only when a version is given (the root page).
    #[test]
    fn test_render_page_version_block_only_for_root() {
        assert!(render_page(&[], "h\n", Some("DEV BUILD\n")).contains("$ spawn-at -V\nDEV BUILD"));
        assert!(!render_page(&["x".to_string()], "h\n", None).contains("-V"));
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

    #[cfg(unix)]
    fn fake_cli(dir: &Path) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;
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
        script
    }

    /// Against a stand-in CLI with a nested command: all four pages are written, the nested one
    /// is found by following `beta`'s own list, and only the root page carries the version.
    #[cfg(unix)]
    #[test]
    fn test_generate_walks_nested_commands() {
        let dir = temp_dir("generate");
        let bin = fake_cli(&dir);
        let out = dir.join("reference");
        let summary = "(reference/spawn-at.md) (reference/spawn-at-alpha.md) \
                       (reference/spawn-at-beta.md) (reference/spawn-at-beta-inner.md)";
        let written = generate(&bin, &out, summary).unwrap();
        assert_eq!(written.len(), 4);
        let root = fs::read_to_string(out.join("spawn-at.md")).unwrap();
        assert!(root.contains("spawn-at 9.9.9"));
        let inner = fs::read_to_string(out.join("spawn-at-beta-inner.md")).unwrap();
        assert!(inner.contains("Leaf for beta inner --help") && !inner.contains("9.9.9"));
        let _ = fs::remove_dir_all(&dir);
    }

    /// A command that `--help` lists but SUMMARY.md does not link fails the build, naming it.
    #[cfg(unix)]
    #[test]
    fn test_generate_fails_when_summary_misses_a_command() {
        let dir = temp_dir("summary");
        let bin = fake_cli(&dir);
        let summary =
            "(reference/spawn-at.md) (reference/spawn-at-alpha.md) (reference/spawn-at-beta.md)";
        let err = generate(&bin, &dir.join("reference"), summary)
            .err()
            .unwrap();
        assert!(
            err.contains("spawn-at beta inner") && err.contains("SUMMARY.md"),
            "{}",
            err
        );
        let _ = fs::remove_dir_all(&dir);
    }

    /// A missing page file is reported even when SUMMARY.md links it.
    #[test]
    fn test_check_coverage_reports_missing_page_file() {
        let dir = temp_dir("coverage");
        let problems = check_coverage(
            &[vec!["gone".to_string()]],
            &dir,
            "(reference/spawn-at-gone.md)",
        );
        assert_eq!(problems.len(), 1);
        assert!(problems[0].contains("has no page"), "{:?}", problems);
        let _ = fs::remove_dir_all(&dir);
    }
}
